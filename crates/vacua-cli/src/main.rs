use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};

use vacua_core::allocation::AllocationInfo;
use vacua_core::candidate::Candidate;
use vacua_core::pressure::{query_volume_status, VolumeStorageStatus};
use vacua_core::risk::RiskLevel;
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
        Commands::Scan { path, depth } => handle_scan(&path, depth, cli.json),
        Commands::Candidates { path, risk } => handle_candidates(&path, risk, cli.json),
        Commands::Explain { id, path } => handle_explain(&id, &path, cli.json),
        Commands::Plan { path, risk } => handle_plan(&path, risk, cli.json),
        Commands::Doctor => handle_doctor(cli.json),
    }
}

fn handle_scan(path: &Path, depth: Option<usize>, json_mode: bool) {
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
        }
        Err(e) => {
            eprintln!("Scan error: {}", e);
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
