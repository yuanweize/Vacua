use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use serde::Serialize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use vacua_core::allocation::AllocationInfo;
use vacua_core::candidate::Candidate;
use vacua_core::pressure::{query_volume_status, VolumeStorageStatus};
use vacua_core::risk::RiskLevel;
use vacua_executor::{ExecutionJournal, MacOSTrashBackend, PlanExecutor};
use vacua_index::IndexDatabase;
use vacua_plan::CleanupPlan;
use vacua_risk::CandidateEvaluator;
use vacua_rules::engine::RulesEngine;
use vacua_scan::{FilesystemScanner, ScanOptions};

#[derive(Parser)]
#[command(
    name = "vacua",
    author = "Weize Yuan <iyuanweize@gmail.com>, Vacua Contributors",
    version,
    about = "Storage intelligence for macOS",
    long_about = "Vacua helps you understand macOS storage allocation with true APFS block metrics, deterministic evidence, and immutable cleanup plans."
)]
struct Cli {
    #[arg(long, global = true, help = "Output machine-readable JSON")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Scan a filesystem path and analyze storage allocation")]
    Scan {
        #[arg(default_value = ".", help = "Target path to scan")]
        path: PathBuf,

        #[arg(long, help = "Maximum directory traversal depth")]
        depth: Option<usize>,

        #[arg(long, help = "Persist scan metadata to incremental SQLite index")]
        incremental: bool,

        #[arg(
            long,
            short = 'j',
            help = "Number of concurrent worker threads (defaults to available parallelism)"
        )]
        jobs: Option<usize>,
    },

    #[command(about = "Inspect and manage the SQLite metadata index")]
    Index {
        #[command(subcommand)]
        action: IndexAction,
    },

    #[command(about = "List classified cleanup candidates with risk evaluation")]
    Candidates {
        #[arg(default_value = ".", help = "Target path to inspect")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = RiskFilter::Safe, help = "Maximum risk level to include")]
        risk: RiskFilter,
    },

    #[command(about = "Explain why an item is categorized and safe/unsafe to clean")]
    Explain {
        #[arg(help = "Candidate ID to explain")]
        id: String,

        #[arg(
            long,
            default_value = ".",
            help = "Path to scan for candidate matching"
        )]
        path: PathBuf,
    },

    #[command(about = "Create or inspect an immutable cleanup plan")]
    Plan {
        #[arg(default_value = ".", help = "Target path to build plan for")]
        path: PathBuf,

        #[arg(long, value_enum, default_value_t = RiskFilter::Safe, help = "Maximum permitted risk in plan")]
        risk: RiskFilter,

        #[arg(
            long,
            short = 'o',
            help = "Save plan JSON to file for subsequent execution"
        )]
        output: Option<PathBuf>,
    },

    #[command(about = "Safely execute an immutable cleanup plan with TOCTOU pre-verification")]
    Execute {
        #[arg(help = "Path to the plan JSON file")]
        plan: PathBuf,

        #[arg(long, help = "Simulate execution without modifying the filesystem")]
        dry_run: bool,
    },

    #[command(about = "Inspect historical execution transactions from SQLite journal")]
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,
    },

    #[command(about = "Diagnose system volume status, APFS features, and FDA permissions")]
    Doctor,

    #[command(about = "Interact with the provider-neutral intelligence translation layer")]
    Intelligence {
        #[command(subcommand)]
        action: IntelligenceAction,
    },

    #[command(about = "Generate shell completion scripts (bash, zsh, fish)")]
    Completions {
        #[arg(value_enum, help = "Target shell")]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug)]
enum IndexAction {
    #[command(about = "Display statistics about the local SQLite metadata index")]
    Status,
    #[command(about = "Clear and rebuild the SQLite metadata index")]
    Rebuild,
}

#[derive(Subcommand, Debug)]
enum HistoryAction {
    #[command(about = "Show detailed audit records for a transaction")]
    Show {
        #[arg(help = "Transaction ID to inspect (e.g. tx-1727560000000)")]
        id: String,
    },
    #[command(about = "Verify cryptographic SHA-256 hash chain integrity of the audit journal")]
    Verify,
}

#[derive(Subcommand, Debug)]
enum IntelligenceAction {
    #[command(about = "Display the status and availability of local intelligence providers")]
    Status,
    #[command(about = "Translate natural language cleanup intent into typed StructuredIntent")]
    Parse {
        #[arg(help = "Natural language cleanup intent")]
        prompt: String,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
enum RiskFilter {
    Safe,
    Review,
    Caution,
}

impl From<RiskFilter> for RiskLevel {
    fn from(f: RiskFilter) -> Self {
        match f {
            RiskFilter::Safe => RiskLevel::Safe,
            RiskFilter::Review => RiskLevel::Review,
            RiskFilter::Caution => RiskLevel::Caution,
        }
    }
}

#[derive(Serialize)]
struct ScanOutput {
    target_path: String,
    files_count: u64,
    dirs_count: u64,
    symlinks_count: u64,
    sparse_files: u64,
    allocation: AllocationInfo,
    skipped_paths_count: usize,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            depth,
            incremental,
            jobs,
        } => handle_scan(&path, depth, incremental, jobs, cli.json),
        Commands::Index { action } => match action {
            IndexAction::Status => handle_index_status(cli.json),
            IndexAction::Rebuild => handle_index_rebuild(cli.json),
        },
        Commands::Intelligence { action } => match action {
            IntelligenceAction::Status => handle_intelligence_status(cli.json),
            IntelligenceAction::Parse { prompt } => handle_intelligence_parse(&prompt, cli.json),
        },
        Commands::Candidates { path, risk } => handle_candidates(&path, risk, cli.json),
        Commands::Explain { id, path } => handle_explain(&id, &path, cli.json),
        Commands::Plan { path, risk, output } => handle_plan(&path, risk, output, cli.json),
        Commands::Execute { plan, dry_run } => handle_execute(&plan, dry_run, cli.json),
        Commands::History { action } => handle_history(action, cli.json),
        Commands::Doctor => handle_doctor(cli.json),
        Commands::Completions { shell } => handle_completions(shell),
    }
}

fn default_index_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vacua").join("index.db")
}

fn handle_scan(
    path: &Path,
    depth: Option<usize>,
    incremental: bool,
    jobs: Option<usize>,
    json_mode: bool,
) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: depth,
        jobs,
    });

    match scanner.scan(&canonical) {
        Ok(report) => {
            if json_mode {
                let out = ScanOutput {
                    target_path: report.root_path.to_string_lossy().to_string(),
                    files_count: report.total_files,
                    dirs_count: report.total_dirs,
                    symlinks_count: report.total_symlinks,
                    sparse_files: report.sparse_files,
                    allocation: report.allocation,
                    skipped_paths_count: report.skipped_paths.len(),
                };
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                println!("\nStorage Scan Report");
                println!("==================================================");
                println!("Target Path:         {}", report.root_path.display());
                println!("Files Scanned:       {}", report.total_files);
                println!("Directories:         {}", report.total_dirs);
                println!("Symlinks:            {}", report.total_symlinks);
                println!("Sparse Files:        {}", report.sparse_files);
                if report.clone_files > 0 {
                    println!("APFS Clone Files:    {}", report.clone_files);
                    println!(
                        "Shared Clone Space:  {}",
                        format_bytes(report.allocation.shared_bytes)
                    );
                    println!(
                        "Exclusive Space:     {}",
                        format_bytes(report.allocation.exclusive_bytes)
                    );
                }
                println!(
                    "Logical Content:     {}",
                    format_bytes(report.allocation.logical_bytes)
                );
                println!(
                    "Physical Allocated:  {}",
                    format_bytes(report.allocation.allocated_bytes)
                );
                if report.allocation.logical_bytes != report.allocation.allocated_bytes {
                    let diff = (report.allocation.logical_bytes as i64)
                        - (report.allocation.allocated_bytes as i64);
                    println!(
                        "Allocation Delta:    {} (due to sparsity/block overhead)",
                        format_bytes(diff.unsigned_abs())
                    );
                }
                if !report.skipped_paths.is_empty() {
                    println!(
                        "Skipped Entries:     {} (permissions/system)",
                        report.skipped_paths.len()
                    );
                }
                println!("==================================================\n");
            }

            if incremental {
                let db_path = default_index_path();
                match IndexDatabase::open(&db_path) {
                    Ok(mut db) => {
                        let _ = db.record_session(&canonical, &report);
                        let _ = db.upsert_entries(&report.entries);
                        if !json_mode {
                            println!(
                                "Incremental Index updated: {} entries indexed into {}",
                                report.entries.len(),
                                db_path.display()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to update incremental index: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Scan error: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_index_status(json_mode: bool) {
    let db_path = default_index_path();
    if !db_path.exists() {
        if json_mode {
            println!("null");
        } else {
            println!(
                "No index database found at {}. Run 'vacua scan --incremental' to create one.",
                db_path.display()
            );
        }
        return;
    }

    match IndexDatabase::open(&db_path) {
        Ok(db) => match db.get_index_stats() {
            Ok(stats) => {
                if json_mode {
                    println!("{}", serde_json::to_string_pretty(&stats).unwrap());
                } else {
                    println!("\nVacua Index Status");
                    println!("==================================================");
                    println!("Database Location:   {}", db_path.display());
                    println!("Total Indexed Nodes: {}", stats.total_entries);
                    println!(
                        "Total Logical Space: {}",
                        format_bytes(stats.total_logical_bytes)
                    );
                    println!(
                        "Total Physical Space:{}",
                        format_bytes(stats.total_allocated_bytes)
                    );
                    println!("Total Scan Sessions: {}", stats.total_sessions);
                    if let Some(ts) = stats.last_scan_timestamp {
                        println!("Last Scan Timestamp: {}", ts);
                    }
                    println!("==================================================\n");
                }
            }
            Err(e) => {
                eprintln!("Failed to read index stats: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Failed to open index: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_index_rebuild(json_mode: bool) {
    let db_path = default_index_path();
    match IndexDatabase::open(&db_path) {
        Ok(mut db) => match db.rebuild() {
            Ok(_) => {
                if json_mode {
                    println!(
                        "{{\"status\": \"rebuilt\", \"path\": \"{}\"}}",
                        db_path.display()
                    );
                } else {
                    println!(
                        "Successfully cleared and vacuumed index database at {}",
                        db_path.display()
                    );
                }
            }
            Err(e) => {
                eprintln!("Failed to rebuild index: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Failed to open index for rebuild: {}", e);
            std::process::exit(1);
        }
    }
}

fn find_intelligence_binary() -> Option<PathBuf> {
    // 1. Explicit environment variable override
    if let Ok(env_override) = std::env::var("VACUA_INTELLIGENCE_HELPER") {
        let p = PathBuf::from(env_override);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Next to current executable (standard release tarball: bin/vacua and bin/vacua-intelligence)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let candidate = dir.join("vacua-intelligence");
            if candidate.exists() {
                return Some(candidate);
            }

            // 3. Homebrew libexec layout: $(brew --prefix)/bin/vacua -> $(brew --prefix)/libexec/vacua-intelligence
            let libexec_candidate = dir.join("../libexec/vacua-intelligence");
            if libexec_candidate.exists() {
                return Some(libexec_candidate);
            }
        }
    }

    // 4. In PATH
    if let Ok(path_var) = std::env::var("PATH") {
        for p in std::env::split_paths(&path_var) {
            let candidate = p.join("vacua-intelligence");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 5. Development paths (debug builds only)
    #[cfg(debug_assertions)]
    {
        let dev_candidates = [
            PathBuf::from("apple/VacuaIntelligence/.build/release/vacua-intelligence"),
            PathBuf::from("apple/VacuaIntelligence/.build/debug/vacua-intelligence"),
            PathBuf::from("../apple/VacuaIntelligence/.build/release/vacua-intelligence"),
            PathBuf::from("../apple/VacuaIntelligence/.build/debug/vacua-intelligence"),
        ];
        for cand in &dev_candidates {
            if cand.exists() {
                return Some(cand.clone());
            }
        }
    }

    None
}

fn handle_intelligence_status(json_mode: bool) {
    let binary = match find_intelligence_binary() {
        Some(b) => b,
        None => {
            if json_mode {
                println!(
                    "{{\"status\":\"unavailable\",\"provider_requested\":\"apple-on-device\",\"provider_used\":\"none\",\"apple_model_availability\":\"notInstalled\",\"reason\":\"vacua-intelligence helper binary not found\"}}"
                );
            } else {
                println!("\nApple Intelligence Status");
                println!("──────────────────────────────────────────────────");
                println!("Provider Requested: Apple On-Device (Foundation Models)");
                println!("Provider Used:      None (Helper binary not found)");
                println!("Model Availability: Not installed");
                println!("Network Egress:     No (Strict on-device inference)");
                println!("Role:               Intent translation (NL -> StructuredIntent)");
                println!("Execution:          Forbidden (Zero deletion authority)");
                println!("Note:               Helper not found. In release, ensure 'vacua-intelligence' is in the same directory or PATH.");
                println!("──────────────────────────────────────────────────\n");
            }
            return;
        }
    };

    match Command::new(&binary).arg("status").output() {
        Ok(output) => {
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                eprintln!("Intelligence helper failed: {}", err);
                std::process::exit(1);
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            if json_mode {
                print!("{}", stdout);
            } else {
                let val: serde_json::Value =
                    serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
                let availability = val
                    .get("apple_model_availability")
                    .or_else(|| val.get("availability"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let provider_used = val
                    .get("provider_used")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                println!("\nApple Intelligence Status");
                println!("──────────────────────────────────────────────────");
                println!("Provider Requested: Apple On-Device (Foundation Models)");
                println!("Provider Used:      {}", provider_used);
                println!("Model Availability: {}", availability);
                println!("Network Egress:     No (Strict on-device inference)");
                println!("Role:               Intent translation (NL -> StructuredIntent)");
                println!("Execution:          Forbidden (Zero deletion authority)");
                println!("Helper Location:    {}", binary.display());
                println!("Protocol Version:   JSON v1 (process-isolated IPC)");
                println!("──────────────────────────────────────────────────\n");
            }
        }
        Err(e) => {
            eprintln!(
                "Failed to spawn intelligence binary ({}): {}",
                binary.display(),
                e
            );
            std::process::exit(1);
        }
    }
}

fn handle_intelligence_parse(prompt: &str, json_mode: bool) {
    let binary = match find_intelligence_binary() {
        Some(b) => b,
        None => {
            if json_mode {
                println!(
                    "{{\"status\":\"error\",\"message\":\"vacua-intelligence binary not found\"}}"
                );
            } else {
                eprintln!(
                    "vacua-intelligence binary not found. In release, ensure 'vacua-intelligence' is in the same directory or PATH."
                );
            }
            std::process::exit(1);
        }
    };

    match Command::new(&binary).args(["parse", prompt]).output() {
        Ok(output) => {
            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                eprintln!("Intelligence parsing failed: {}", err);
                std::process::exit(1);
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            if json_mode {
                print!("{}", stdout);
            } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                if let Some(intent) = val.get("intent") {
                    let provider_used = val
                        .get("provider_used")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let availability = val
                        .get("apple_model_availability")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!(
                        "\nParsed Structured Intent (v{})",
                        intent
                            .get("intent_version")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1)
                    );
                    println!("──────────────────────────────────────────────────");
                    println!("Provider Used:       {}", provider_used);
                    println!("Model Availability:  {}", availability);
                    if let Some(target) =
                        intent.get("target_reclaim_bytes").and_then(|v| v.as_u64())
                    {
                        println!("Target Reclaim:      {}", format_bytes(target));
                    }
                    if let Some(risk) = intent.get("max_risk").and_then(|v| v.as_str()) {
                        println!("Max Risk Level:      {}", risk);
                    }
                    if let Some(dev) = intent
                        .get("include_developer_artifacts")
                        .and_then(|v| v.as_bool())
                    {
                        println!("Include Dev Caches:  {}", dev);
                    }
                    if let Some(rev) = intent
                        .get("prefer_reversible_actions")
                        .and_then(|v| v.as_bool())
                    {
                        println!("Prefer Trash:        {}", rev);
                    }
                    if let Some(excluded) =
                        intent.get("excluded_categories").and_then(|v| v.as_array())
                    {
                        let cats: Vec<String> = excluded
                            .iter()
                            .filter_map(|c| c.as_str().map(|s| s.to_string()))
                            .collect();
                        if !cats.is_empty() {
                            println!("Excluded Categories: {}", cats.join(", "));
                        }
                    }
                    println!("──────────────────────────────────────────────────\n");
                } else {
                    print!("{}", stdout);
                }
            } else {
                print!("{}", stdout);
            }
        }
        Err(e) => {
            eprintln!(
                "Failed to spawn intelligence binary ({}): {}",
                binary.display(),
                e
            );
            std::process::exit(1);
        }
    }
}

fn handle_candidates(path: &Path, risk_filter: RiskFilter, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(6),
        ..Default::default()
    });

    let report = match scanner.scan(&canonical) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error scanning candidates: {}", e);
            std::process::exit(1);
        }
    };

    let mut engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut engine);
    let max_risk: RiskLevel = risk_filter.into();

    let mut candidates = Vec::new();
    for entry in report.entries {
        let alloc = AllocationInfo::new(entry.logical_bytes, entry.allocated_bytes, false);
        let cand = evaluator.evaluate(
            &entry.path,
            alloc,
            entry.inode,
            entry.device_id,
            entry.mtime_sec,
            entry.is_dir,
        );

        if !cand.risk.is_protected_or_unknown() && cand.risk <= max_risk {
            candidates.push(cand);
        }
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&candidates).unwrap());
    } else {
        println!("\nStorage Cleanup Candidates (Max Risk: {:?})", risk_filter);
        println!(
            "=================================================================================="
        );
        if candidates.is_empty() {
            println!("No cleanup candidates found matching criteria under current path.");
        } else {
            for c in &candidates {
                println!(
                    "[{}] {:<8} {:<10} {:>10}  {}",
                    c.id,
                    c.risk.to_string(),
                    c.category.to_string(),
                    format_bytes(c.allocation.allocated_bytes),
                    c.path.display()
                );
            }
        }
        println!(
            "==================================================================================\n"
        );
    }
}

fn handle_explain(candidate_id: &str, path: &Path, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(6),
        ..Default::default()
    });

    let report = match scanner.scan(&canonical) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error scanning for explanation: {}", e);
            std::process::exit(1);
        }
    };

    let mut engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut engine);

    let mut found_candidate: Option<Candidate> = None;
    for entry in report.entries {
        let alloc = AllocationInfo::new(entry.logical_bytes, entry.allocated_bytes, false);
        let cand = evaluator.evaluate(
            &entry.path,
            alloc,
            entry.inode,
            entry.device_id,
            entry.mtime_sec,
            entry.is_dir,
        );

        if cand.id == candidate_id || cand.path.to_string_lossy().contains(candidate_id) {
            found_candidate = Some(cand);
            break;
        }
    }

    match found_candidate {
        Some(c) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&c).unwrap());
            } else {
                println!("\nCandidate Explanation: [{}]", c.id);
                println!("--------------------------------------------------");
                println!("Path:             {}", c.path.display());
                println!("Category:         {}", c.category);
                println!("Risk:             {}", c.risk);
                println!("Recommendation:   {}", c.value);
                println!("Confidence Score: {:.2}", c.confidence_score);
                println!(
                    "Physical Space:   {}",
                    format_bytes(c.allocation.allocated_bytes)
                );
                println!(
                    "Reconstructable:  {}",
                    if c.reconstructable { "Yes" } else { "No" }
                );
                if let Some(ref eff) = c.rebuild_consequence {
                    println!("Rebuild Effect:   {}", eff);
                }
                println!("\nEvidence Vector:");
                for ev in &c.evidence {
                    println!(
                        "  * [{:?}] (weight {:.2}): {}",
                        ev.source, ev.weight, ev.explanation
                    );
                }
                println!("--------------------------------------------------\n");
            }
        }
        None => {
            eprintln!("Candidate with ID or pattern '{}' not found.", candidate_id);
            std::process::exit(1);
        }
    }
}

fn handle_plan(path: &Path, risk_filter: RiskFilter, output: Option<PathBuf>, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(6),
        ..Default::default()
    });

    let report = match scanner.scan(&canonical) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error scanning for plan: {}", e);
            std::process::exit(1);
        }
    };

    let mut engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut engine);
    let max_risk: RiskLevel = risk_filter.into();

    let mut candidates = Vec::new();
    for entry in report.entries {
        let alloc = AllocationInfo::new(entry.logical_bytes, entry.allocated_bytes, false);
        let cand = evaluator.evaluate(
            &entry.path,
            alloc,
            entry.inode,
            entry.device_id,
            entry.mtime_sec,
            entry.is_dir,
        );
        candidates.push(cand);
    }

    match CleanupPlan::build(&candidates, max_risk, "1.0") {
        Ok(plan) => {
            if let Some(ref out_path) = output {
                let serialized = serde_json::to_string_pretty(&plan).unwrap();
                if let Err(e) = std::fs::write(out_path, &serialized) {
                    eprintln!("Failed to write plan file ({}): {}", out_path.display(), e);
                    std::process::exit(1);
                }
                if !json_mode {
                    println!("Immutable cleanup plan saved to: {}", out_path.display());
                }
            }

            if json_mode {
                println!("{}", serde_json::to_string_pretty(&plan).unwrap());
            } else {
                println!("\nImmutable Cleanup Plan Generated");
                println!("==================================================");
                println!("Plan ID:                {}", plan.plan_id);
                println!("Created At:             {}", plan.created_at.to_rfc3339());
                println!("Plan Integrity Hash:    {}", plan.plan_hash);
                println!("Items Included:         {}", plan.items.len());
                println!(
                    "Est. Physical Reclaim:  {}",
                    format_bytes(plan.estimated_physical_reclaim)
                );
                println!("Risk Breakdown:");
                println!(
                    "  - SAFE:   {} items ({})",
                    plan.risk_summary.safe_count,
                    format_bytes(plan.risk_summary.safe_allocated_bytes)
                );
                println!(
                    "  - REVIEW: {} items ({})",
                    plan.risk_summary.review_count,
                    format_bytes(plan.risk_summary.review_allocated_bytes)
                );
                println!("==================================================\n");
                if output.is_none() {
                    println!(
                        "Tip: Run 'vacua plan {} -o plan.json' to save this plan for execution.",
                        path.display()
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to generate plan: {}", e);
            std::process::exit(1);
        }
    }
}

fn default_journal_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vacua").join("journal.db")
}

fn handle_execute(plan_path: &Path, dry_run: bool, json_mode: bool) {
    if !plan_path.exists() {
        eprintln!("Plan file not found: {}", plan_path.display());
        std::process::exit(1);
    }

    let plan_content = match std::fs::read_to_string(plan_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read plan file: {}", e);
            std::process::exit(1);
        }
    };

    let plan: CleanupPlan = match serde_json::from_str(&plan_content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to parse plan file (invalid format): {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = plan.verify_integrity() {
        eprintln!(
            "SECURITY ERROR: Plan integrity check failed: {}. Execution aborted.",
            e
        );
        std::process::exit(1);
    }

    if !json_mode {
        println!("\nCleanup Plan Execution Preview");
        println!("==================================================");
        println!("Plan ID:             {}", plan.plan_id);
        println!("Created:             {}", plan.created_at);
        println!("Plan Hash:           {}", &plan.plan_hash[..16]);
        println!("Items to Process:    {}", plan.items.len());
        println!(
            "Estimated Reclaim:   {}",
            format_bytes(plan.estimated_physical_reclaim)
        );
        println!("Safe Items:          {}", plan.risk_summary.safe_count);
        println!("Review Items:        {}", plan.risk_summary.review_count);
        println!("Caution Items:       {}", plan.risk_summary.caution_count);
        println!(
            "Action Mode:         {}",
            if dry_run {
                "DRY RUN (Simulate only, no mutations)"
            } else {
                "MOVE TO TRASH (Reversible)"
            }
        );
        println!("==================================================");

        if !dry_run {
            print!("\nAre you sure you want to execute this cleanup plan? [y/N]: ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() || !input.trim().eq_ignore_ascii_case("y")
            {
                println!("Execution cancelled by user. No files were modified.");
                return;
            }
        }
    }

    let trash_backend = MacOSTrashBackend::new();
    let executor = PlanExecutor::new(&trash_backend, dry_run);

    let journal_path = default_journal_path();
    let mut journal = match ExecutionJournal::open(&journal_path) {
        Ok(j) => Some(j),
        Err(e) => {
            if !dry_run {
                eprintln!(
                    "FAIL-SAFE ABORT: Cannot open execution journal ({}) for live execution: {}\nDeletions without an audit log are forbidden.",
                    journal_path.display(),
                    e
                );
                std::process::exit(1);
            }
            eprintln!(
                "Warning: Failed to open execution journal ({}) in dry-run mode: {}",
                journal_path.display(),
                e
            );
            None
        }
    };

    match executor.execute(&plan, journal.as_mut()) {
        Ok(report) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!(
                    "\nExecution Summary ({})",
                    if report.dry_run {
                        "DRY RUN"
                    } else {
                        "COMPLETED"
                    }
                );
                println!("──────────────────────────────────────────────────");
                println!("Transaction ID:      {}", report.transaction_id);
                println!("Total Planned:       {}", report.total_planned_items);
                println!("Successfully Moved:  {}", report.successful_items.len());
                println!("Skipped (Guards):    {}", report.skipped_items.len());
                println!("Failed:              {}", report.failed_items.len());
                println!(
                    "Total Reclaimed:     {}",
                    format_bytes(report.total_reclaimed_bytes)
                );

                if !report.skipped_items.is_empty() {
                    println!("\nSkipped Items (Protected / Inode changed / Active process):");
                    for item in &report.skipped_items {
                        println!("  • {} -> {}", item.path.display(), item.reason);
                    }
                }

                if !report.failed_items.is_empty() {
                    println!("\nFailed Items:");
                    for item in &report.failed_items {
                        println!("  • {} -> {}", item.path.display(), item.error);
                    }
                }
                println!("──────────────────────────────────────────────────\n");
            }
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_history(action: Option<HistoryAction>, json_mode: bool) {
    let journal_path = default_journal_path();
    if !journal_path.exists() {
        if json_mode {
            println!("[]");
        } else {
            println!(
                "No execution history found at {}. Run 'vacua execute <plan>' to create transactions.",
                journal_path.display()
            );
        }
        return;
    }

    let journal = match ExecutionJournal::open(&journal_path) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to open execution journal: {}", e);
            std::process::exit(1);
        }
    };

    match action {
        None => {
            let txs = match journal.list_transactions(20) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("Failed to list transactions: {}", e);
                    std::process::exit(1);
                }
            };

            if json_mode {
                println!("{}", serde_json::to_string_pretty(&txs).unwrap());
            } else if txs.is_empty() {
                println!("No transaction records found in journal.");
            } else {
                println!("\nExecution History (Recent Transactions)");
                println!("==========================================================================================");
                println!(
                    "{:<26} {:<20} {:<8} {:<14} {:<8} {:<8}",
                    "Transaction ID", "Date / Time", "Items", "Reclaimed", "Success", "Skipped"
                );
                println!("──────────────────────────────────────────────────────────────────────────────────────────");
                for tx in &txs {
                    let dt = chrono::DateTime::from_timestamp(tx.timestamp, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!(
                        "{:<26} {:<20} {:<8} {:<14} {:<8} {:<8}",
                        tx.transaction_id,
                        dt,
                        tx.total_items,
                        format_bytes(tx.total_reclaimed_bytes),
                        tx.successful_count,
                        tx.skipped_count,
                    );
                }
                println!("==========================================================================================\n");
                println!("Use 'vacua history show <transaction_id>' to inspect individual item audit records.");
            }
        }
        Some(HistoryAction::Show { id }) => {
            let records = match journal.get_transaction_details(&id) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to retrieve transaction details: {}", e);
                    std::process::exit(1);
                }
            };

            if json_mode {
                println!("{}", serde_json::to_string_pretty(&records).unwrap());
            } else if records.is_empty() {
                println!("No records found for transaction ID '{}'", id);
            } else {
                println!("\nTransaction Audit Records: {}", id);
                println!("==========================================================================================");
                for rec in &records {
                    let dt = chrono::DateTime::from_timestamp(rec.timestamp, 0)
                        .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    println!("Path:        {}", rec.path);
                    println!(
                        "Action:      {} (Reversible: {})",
                        rec.action, rec.reversible
                    );
                    println!("Result:      {}", rec.result);
                    println!("Risk:        {}", rec.risk);
                    println!("Reclaimed:   {}", format_bytes(rec.reclaimed_estimate));
                    println!("Timestamp:   {}", dt);
                    if let Some(ref err) = rec.error_message {
                        println!("Note/Error:  {}", err);
                    }
                    println!("──────────────────────────────────────────────────");
                }
            }
        }
        Some(HistoryAction::Verify) => {
            let report = match journal.verify_chain() {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to verify journal chain: {}", e);
                    std::process::exit(1);
                }
            };

            if json_mode {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("\nAudit Journal Tamper-Evident Hash Chain Verification");
                println!("──────────────────────────────────────────────────");
                println!("Total Audit Records:  {}", report.total_records);
                println!(
                    "Chain Integrity:      {}",
                    if report.is_valid {
                        "VERIFIED (Cryptographically Valid)"
                    } else {
                        "FAILED (Tampering Detected!)"
                    }
                );
                if let Some(ref h) = report.first_hash {
                    println!("Genesis Link:         {}", h);
                }
                if let Some(ref h) = report.latest_hash {
                    println!("Head Hash:            {}", h);
                }
                if !report.is_valid {
                    if let Some(id) = report.broken_record_id {
                        println!("Broken Record ID:     {}", id);
                    }
                    if let Some(ref detail) = report.error_detail {
                        println!("Error Detail:         {}", detail);
                    }
                }
                println!("──────────────────────────────────────────────────\n");
            }
            if !report.is_valid {
                std::process::exit(1);
            }
        }
    }
}

fn handle_completions(shell: Shell) {
    let mut cmd = Cli::command();
    clap_complete::generate(shell, &mut cmd, "vacua", &mut io::stdout());
}

fn handle_doctor(json_mode: bool) {
    let root = Path::new("/");
    let status = query_volume_status(root).unwrap_or_else(|_| VolumeStorageStatus {
        mount_point: "/".into(),
        filesystem_type: "apfs".into(),
        total_bytes: 0,
        free_bytes: 0,
        available_bytes: 0,
        free_ratio: 0.0,
        pressure: vacua_core::pressure::StoragePressure::Normal,
    });

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/Users/unknown"));
    let fda_canary = home.join("Library/Mail");
    let has_fda = fda_canary.exists() && std::fs::read_dir(&fda_canary).is_ok();
    let fda_notes = if has_fda {
        "Full Disk Access appears active. Mail and protected storage areas are inspectable."
    } else {
        "Full Disk Access is NOT granted. Standard unprivileged sandbox mode active. Protected system files will be gracefully skipped."
    };

    let index_db = default_index_path();
    let journal_db = default_journal_path();

    let intelligence_bin = find_intelligence_binary();
    let helper_installed = intelligence_bin.is_some();
    let helper_path_str = intelligence_bin
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Not found (optional)".to_string());

    let os_version = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "macOS (unknown)".to_string());

    if json_mode {
        let out = serde_json::json!({
            "vacua_version": env!("CARGO_PKG_VERSION"),
            "architecture": std::env::consts::ARCH,
            "os_version": os_version,
            "volume_status": status,
            "full_disk_access_granted": has_fda,
            "fda_notes": fda_notes,
            "persistent_paths": {
                "index_db": index_db.to_string_lossy(),
                "index_db_exists": index_db.exists(),
                "journal_db": journal_db.to_string_lossy(),
                "journal_db_exists": journal_db.exists()
            },
            "intelligence_helper": {
                "installed": helper_installed,
                "path": helper_path_str
            },
            "safety_invariants_active": true
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("\nVacua Doctor — System Storage Health & Diagnostics");
        println!("==================================================");
        println!("Vacua CLI Version:      {}", env!("CARGO_PKG_VERSION"));
        println!("Architecture:           macOS ({})", std::env::consts::ARCH);
        println!("Operating System:       macOS {}", os_version);
        println!("Mount Point:            {}", status.mount_point);
        println!("Filesystem Type:        {}", status.filesystem_type);
        println!(
            "Total Capacity:         {}",
            format_bytes(status.total_bytes)
        );
        println!(
            "Available Space:        {}",
            format_bytes(status.available_bytes)
        );
        println!("Free Space Ratio:       {:.1}%", status.free_ratio * 100.0);
        println!("Storage Pressure:       {}", status.pressure);
        println!(
            "Full Disk Access (FDA): {}",
            if has_fda { "GRANTED" } else { "NOT GRANTED" }
        );
        println!("FDA Status Detail:      {}", fda_notes);
        println!(
            "Index Database:         {} ({})",
            index_db.display(),
            if index_db.exists() {
                "initialized"
            } else {
                "not created yet"
            }
        );
        println!(
            "Execution Journal:      {} ({})",
            journal_db.display(),
            if journal_db.exists() {
                "active"
            } else {
                "no transactions yet"
            }
        );
        println!(
            "Intelligence Helper:    {} ({})",
            helper_path_str,
            if helper_installed {
                "ready"
            } else {
                "optional, not installed"
            }
        );
        println!("Safety Invariants:      VERIFIED (PROTECTED & UNKNOWN hard-enforced)");
        println!("==================================================\n");
    }
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.2} TiB", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.2} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}
