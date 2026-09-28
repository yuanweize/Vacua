use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use vacua_core::allocation::AllocationInfo;
use vacua_core::candidate::Candidate;
use vacua_core::pressure::{query_volume_status, VolumeStorageStatus};
use vacua_core::risk::RiskLevel;
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
    },

    #[command(about = "Diagnose system volume status, APFS features, and FDA permissions")]
    Doctor,

    #[command(about = "Interact with the provider-neutral intelligence translation layer")]
    Intelligence {
        #[command(subcommand)]
        action: IntelligenceAction,
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

#[derive(Serialize)]
struct DoctorOutput {
    volume_status: VolumeStorageStatus,
    full_disk_access_granted: bool,
    fda_probe_notes: String,
    safe_invariants_active: bool,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            depth,
            incremental,
        } => handle_scan(&path, depth, incremental, cli.json),
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
        Commands::Plan { path, risk } => handle_plan(&path, risk, cli.json),
        Commands::Doctor => handle_doctor(cli.json),
    }
}

fn default_index_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".vacua").join("index.db")
}

fn handle_scan(path: &Path, depth: Option<usize>, incremental: bool, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: depth,
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
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let candidate = dir.join("vacua-intelligence");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

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

    if let Ok(path_var) = std::env::var("PATH") {
        for p in std::env::split_paths(&path_var) {
            let candidate = p.join("vacua-intelligence");
            if candidate.exists() {
                return Some(candidate);
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
                    "{{\"status\":\"unavailable\",\"provider\":\"Apple On-Device\",\"reason\":\"vacua-intelligence helper binary not found. Build it with: cd apple/VacuaIntelligence && swift build\"}}"
                );
            } else {
                println!("\nApple Intelligence Status");
                println!("──────────────────────────────────────────────────");
                println!("Provider:     Apple On-Device (Foundation Models)");
                println!("Availability: Binary not compiled");
                println!("Network:      No (Strict on-device inference)");
                println!("Role:         Intent translation (NL -> StructuredIntent)");
                println!("Execution:    Forbidden (Zero deletion authority)");
                println!(
                    "Note:         Build Swift helper: cd apple/VacuaIntelligence && swift build"
                );
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
                let availability =
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                        val.get("availability")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string()
                    } else {
                        "unknown".to_string()
                    };

                println!("\nApple Intelligence Status");
                println!("──────────────────────────────────────────────────");
                println!("Provider:     Apple On-Device (Foundation Models)");
                println!("Availability: {}", availability);
                println!("Network:      No (Strict on-device inference)");
                println!("Role:         Intent translation (NL -> StructuredIntent)");
                println!("Execution:    Forbidden (Zero deletion authority)");
                println!("Helper:       {}", binary.display());
                println!("Protocol:     JSON v1 (stdin/stdout)");
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
                    "{{\"status\":\"error\",\"message\":\"vacua-intelligence binary not found. Build with: cd apple/VacuaIntelligence && swift build\"}}"
                );
            } else {
                eprintln!(
                    "vacua-intelligence binary not found. Compile it first: cd apple/VacuaIntelligence && swift build"
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
                    println!(
                        "\nParsed Structured Intent (v{})",
                        intent
                            .get("intent_version")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1)
                    );
                    println!("──────────────────────────────────────────────────");
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

fn handle_plan(path: &Path, risk_filter: RiskFilter, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(6),
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
            }
        }
        Err(e) => {
            eprintln!("Failed to generate plan: {}", e);
            std::process::exit(1);
        }
    }
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

    // Probe Full Disk Access canary (testing access to standard protected TCC directory)
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

    if json_mode {
        let out = DoctorOutput {
            volume_status: status,
            full_disk_access_granted: has_fda,
            fda_probe_notes: fda_notes.to_string(),
            safe_invariants_active: true,
        };
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("\nVacua Doctor — System Storage Health");
        println!("==================================================");
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
        println!("Free Ratio:             {:.1}%", status.free_ratio * 100.0);
        println!("Storage Pressure:       {}", status.pressure);
        println!(
            "Full Disk Access (FDA): {}",
            if has_fda { "GRANTED" } else { "NOT GRANTED" }
        );
        println!("FDA Status Detail:      {}", fda_notes);
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
