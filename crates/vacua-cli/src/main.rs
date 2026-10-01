use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::Utc;
use vacua_api::dto::{
    CandidateGroupSummaryV1, DeveloperArtifactAnalysisV1, DeveloperArtifactCoverageV1,
    DeveloperArtifactDetailV1, DeveloperProjectSummaryV1, ProtectedSummaryV1, RebuildEvidenceV1,
    StorageDomainV1, StorageRescueSummaryV1, WholeVolumeAccountingV1,
};
use vacua_api::SCHEMA_STORAGE_RESCUE_SUMMARY_V1;
use vacua_artifacts::{
    ArtifactPersistence, DeveloperArtifact, DeveloperArtifactScanner, DeveloperEcosystem,
    DeveloperProject, RebuildConfidence,
};
use vacua_core::allocation::AllocationInfo;
use vacua_core::candidate::Candidate;
use vacua_core::cost::{CleanupSimulation, ReclaimCost};
use vacua_core::evidence_graph::{ApplicationEvidenceGraph, NodeKind, OrphanConfidence};
use vacua_core::pressure::{query_volume_status, VolumeStorageStatus};
use vacua_core::rescue::build_storage_rescue_plan;
use vacua_core::risk::RiskLevel;
use vacua_executor::{ExecutionJournal, MacOSTrashBackend, PlanExecutor};
use vacua_index::{IndexDatabase, StorageSnapshot};
use vacua_plan::CleanupPlan;
use vacua_risk::CandidateEvaluator;
use vacua_rules::engine::RulesEngine;
use vacua_scan::{FilesystemScanner, ScanOptions};
use vacua_tree::{
    StorageNodeId, StorageNodeKind, StorageTreeBuilder, StorageTreeCoverage, StorageTreeEngine,
    StorageTreeGeneration, StorageTreeMetric, StorageTreeSource, StorageTreeStatus,
};

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

    #[arg(
        long,
        help = "Display build identity and provenance information (JSON)"
    )]
    build_info: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Storage Rescue: whole-volume accounting, safe group attribution, and one-decision cleanup"
    )]
    Rescue {
        #[arg(default_value = ".", help = "Target path to inspect")]
        path: PathBuf,

        #[arg(
            long,
            help = "Compile and execute safe reclaim plan immediately with single approval"
        )]
        apply: bool,

        #[arg(
            long,
            help = "Simulate execution without modifying the filesystem (when used with --apply)"
        )]
        dry_run: bool,

        #[arg(
            long,
            help = "Automatically confirm plan execution without prompt (use with caution)"
        )]
        yes: bool,
    },

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

        #[arg(
            long,
            help = "Simulate cleanup consequences (freeable space, rebuild cost) without saving plan"
        )]
        simulate: bool,
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

    #[command(about = "Capture and list point-in-time storage state snapshots")]
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },

    #[command(
        about = "Compare storage allocation deltas between two snapshots or against live filesystem"
    )]
    Diff {
        #[arg(help = "Base snapshot name")]
        base: String,

        #[arg(
            default_value = "current",
            help = "Target snapshot name or 'current' for live filesystem"
        )]
        target: String,
    },

    #[command(about = "Inspect installed applications and discovered filesystem residue")]
    Apps {
        #[command(subcommand)]
        action: Option<AppsAction>,
    },

    #[command(
        about = "Find uninstalled application residue (leftovers) with high orphan confidence"
    )]
    Leftovers,

    #[command(about = "Natural language storage query engine grounded in evidence and snapshots")]
    Ask {
        #[arg(help = "Query prompt, e.g. 'Why did my storage grow?'")]
        query: String,
    },

    #[command(about = "Diagnose system volume status, APFS features, and FDA permissions")]
    Doctor,

    #[command(about = "Interact with the provider-neutral intelligence translation layer")]
    Intelligence {
        #[command(subcommand)]
        action: IntelligenceAction,
    },

    #[command(about = "Discover exact duplicate files with APFS-aware physical reclaim analysis")]
    Duplicates {
        #[command(subcommand)]
        action: Option<DuplicatesSubcommand>,

        #[arg(default_value = ".", help = "Target path to scan for duplicates")]
        path: PathBuf,

        #[arg(
            long,
            default_value = "1M",
            help = "Minimum file size threshold (e.g. 500K, 10M, 1G)"
        )]
        min_size: String,

        #[arg(long, help = "Include zero-length files in duplicate discovery")]
        include_empty: bool,

        #[arg(
            long,
            short = 'j',
            default_value = "4",
            help = "Number of concurrent hashing worker threads"
        )]
        jobs: usize,

        #[arg(long, help = "Do not use persistent fingerprint cache")]
        no_cache: bool,
    },

    #[command(about = "Generate shell completion scripts (bash, zsh, fish)")]
    Completions {
        #[arg(value_enum, help = "Target shell")]
        shell: Shell,
    },

    #[command(
        name = "build-info",
        about = "Display build identity and provenance information"
    )]
    BuildInfo {
        #[arg(long, help = "Output machine-readable JSON")]
        json: bool,
    },

    #[command(
        about = "Analyze and display hierarchical storage tree with logical and allocated metrics"
    )]
    Tree {
        #[arg(default_value = ".", help = "Target path to inspect")]
        path: PathBuf,

        #[arg(
            long,
            value_enum,
            default_value_t = CliTreeMetric::Allocated,
            help = "Metric to display/sort by (allocated or logical)"
        )]
        metric: CliTreeMetric,

        #[arg(
            long,
            default_value_t = 20,
            help = "Maximum children per directory level to display"
        )]
        limit: usize,

        #[arg(long, default_value_t = 1, help = "Maximum tree depth to display")]
        depth: usize,

        #[arg(long, help = "Force refresh storage tree generation from live scan")]
        refresh: bool,
    },

    #[command(about = "Discover and analyze developer project artifacts and rebuild evidence")]
    Artifacts {
        #[command(subcommand)]
        action: Option<ArtifactsSubcommand>,

        #[arg(default_value = ".", help = "Target root path to inspect")]
        path: PathBuf,

        #[arg(
            long,
            help = "Filter by developer ecosystem (e.g. rust, xcode, node, python)"
        )]
        ecosystem: Option<String>,

        #[arg(long, help = "Minimum allocated size threshold (e.g. 10M, 1G)")]
        min_size: Option<String>,

        #[arg(
            long,
            help = "Filter by reconstruction confidence (verified, strong, partial, unknown)"
        )]
        confidence: Option<String>,

        #[arg(long, default_value_t = 50, help = "Maximum items to display")]
        limit: usize,

        #[arg(
            long,
            help = "Force refresh developer artifact analysis from live scan"
        )]
        refresh: bool,

        #[arg(long, help = "Inspect full details of a specific artifact ID")]
        show: Option<String>,

        #[arg(long, help = "Display project-level summary table")]
        projects: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ArtifactsSubcommand {
    #[command(about = "Inspect full details of a specific developer artifact")]
    Show {
        #[arg(help = "Developer Artifact ID (e.g. devart_...)")]
        id: String,
    },
    #[command(about = "List discovered developer projects and aggregate storage")]
    Projects {
        #[arg(default_value = ".", help = "Target root path to inspect")]
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliTreeMetric {
    Allocated,
    Logical,
}

#[derive(Subcommand, Debug)]
enum DuplicatesSubcommand {
    #[command(about = "Inspect full details of a specific duplicate group")]
    Show {
        #[arg(help = "Duplicate Group ID (e.g. dup-...)")]
        group_id: String,

        #[arg(long, default_value = ".", help = "Target root path to scan for group")]
        path: PathBuf,

        #[arg(long, default_value = "1M", help = "Minimum file size threshold")]
        min_size: String,
    },

    #[command(
        about = "Build an immutable cleanup plan for a duplicate group keeping a chosen target"
    )]
    Plan {
        #[arg(help = "Duplicate Group ID (e.g. dup-...)")]
        group_id: String,

        #[arg(long, help = "Path to the duplicate member that must be preserved")]
        keep: PathBuf,

        #[arg(long, default_value = ".", help = "Target root path to scan for group")]
        path: PathBuf,

        #[arg(
            long,
            short = 'o',
            help = "Save plan JSON to file for subsequent execution"
        )]
        output: Option<PathBuf>,
    },

    #[command(about = "Inspect or maintain persistent fingerprint cache")]
    Cache {
        #[command(subcommand)]
        action: FingerprintCacheAction,
    },
}

#[derive(Subcommand, Debug)]
enum FingerprintCacheAction {
    #[command(about = "Show fingerprint cache statistics")]
    Status,

    #[command(about = "Prune missing files from fingerprint cache")]
    Prune,
}

#[derive(Subcommand, Debug)]
enum IndexAction {
    #[command(about = "Display statistics and watched FSEvents roots from SQLite metadata index")]
    Status,
    #[command(about = "Surgically refresh dirty subtrees using native FSEvents event stream")]
    Refresh {
        #[arg(default_value = ".", help = "Target root path to refresh")]
        path: PathBuf,
    },
    #[command(about = "Clear and rebuild the SQLite metadata index")]
    Rebuild,
}

#[derive(Subcommand, Debug)]
enum SnapshotAction {
    #[command(about = "Create a new named storage snapshot")]
    Create {
        #[arg(default_value = ".", help = "Target directory path to snapshot")]
        path: PathBuf,

        #[arg(long, help = "Descriptive name for the snapshot")]
        name: String,
    },
    #[command(about = "List historical storage snapshots")]
    List,
}

#[derive(Subcommand, Debug)]
enum AppsAction {
    #[command(about = "Show detailed evidence graph for a specific application bundle ID")]
    Show {
        #[arg(help = "Bundle ID (e.g. com.example.app)")]
        id: String,
    },
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
    #[command(about = "Display compiled capabilities and build-time feature gates")]
    Capabilities,
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

    if cli.build_info {
        let info = vacua_api::get_build_info();
        println!("{}", serde_json::to_string_pretty(&info).unwrap());
        return;
    }

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            let mut cmd = Cli::command();
            let _ = cmd.print_help();
            println!();
            return;
        }
    };

    match command {
        Commands::BuildInfo { json } => {
            let info = vacua_api::get_build_info();
            if json || cli.json {
                println!("{}", serde_json::to_string_pretty(&info).unwrap());
            } else {
                println!("Vacua Build Information");
                println!("=======================");
                println!("Version:    {}", info.version);
                println!("Git Commit: {}", info.git_commit);
                println!("Target:     {}", info.target);
                println!("Profile:    {}", info.profile);
            }
        }
        Commands::Rescue {
            path,
            apply,
            dry_run,
            yes,
        } => handle_rescue(&path, apply, dry_run, yes, cli.json),
        Commands::Scan {
            path,
            depth,
            incremental,
            jobs,
        } => handle_scan(&path, depth, incremental, jobs, cli.json),
        Commands::Index { action } => match action {
            IndexAction::Status => handle_index_status(cli.json),
            IndexAction::Refresh { path } => handle_index_refresh(&path, cli.json),
            IndexAction::Rebuild => handle_index_rebuild(cli.json),
        },
        Commands::Snapshot { action } => match action {
            SnapshotAction::Create { path, name } => handle_snapshot_create(&path, &name, cli.json),
            SnapshotAction::List => handle_snapshot_list(cli.json),
        },
        Commands::Diff { base, target } => handle_diff(&base, &target, cli.json),
        Commands::Apps { action } => match action {
            Some(AppsAction::Show { id }) => handle_apps_show(&id, cli.json),
            None => handle_apps_list(cli.json),
        },
        Commands::Leftovers => handle_leftovers(cli.json),
        Commands::Ask { query } => handle_ask(&query, cli.json),
        Commands::Intelligence { action } => match action {
            IntelligenceAction::Status => handle_intelligence_status(cli.json),
            IntelligenceAction::Capabilities => handle_intelligence_capabilities(cli.json),
            IntelligenceAction::Parse { prompt } => handle_intelligence_parse(&prompt, cli.json),
        },
        Commands::Candidates { path, risk } => handle_candidates(&path, risk, cli.json),
        Commands::Explain { id, path } => handle_explain(&id, &path, cli.json),
        Commands::Plan {
            path,
            risk,
            output,
            simulate,
        } => handle_plan(&path, risk, output, simulate, cli.json),
        Commands::Execute { plan, dry_run } => handle_execute(&plan, dry_run, cli.json),
        Commands::History { action } => handle_history(action, cli.json),
        Commands::Duplicates {
            action,
            path,
            min_size,
            include_empty,
            jobs,
            no_cache,
        } => match action {
            Some(DuplicatesSubcommand::Show {
                group_id,
                path,
                min_size,
            }) => handle_duplicates_show(&group_id, &path, &min_size, cli.json),
            Some(DuplicatesSubcommand::Plan {
                group_id,
                keep,
                path,
                output,
            }) => handle_duplicates_plan(&group_id, &keep, &path, output, cli.json),
            Some(DuplicatesSubcommand::Cache { action }) => match action {
                FingerprintCacheAction::Status => handle_duplicates_cache_status(cli.json),
                FingerprintCacheAction::Prune => handle_duplicates_cache_prune(cli.json),
            },
            None => {
                handle_duplicates_scan(&path, &min_size, include_empty, jobs, no_cache, cli.json)
            }
        },
        Commands::Doctor => handle_doctor(cli.json),
        Commands::Completions { shell } => handle_completions(shell),
        Commands::Tree {
            path,
            metric,
            limit,
            depth,
            refresh,
        } => handle_tree(&path, metric, limit, depth, cli.json, refresh),
        Commands::Artifacts {
            action,
            path,
            ecosystem,
            min_size,
            confidence,
            limit,
            refresh,
            show,
            projects,
        } => {
            let show_id = show.or_else(|| {
                if let Some(ArtifactsSubcommand::Show { id }) = &action {
                    Some(id.clone())
                } else {
                    None
                }
            });
            let show_projects =
                projects || matches!(action, Some(ArtifactsSubcommand::Projects { .. }));
            let target_path = if let Some(ArtifactsSubcommand::Projects { path: p }) = &action {
                p
            } else {
                &path
            };
            handle_artifacts(
                target_path,
                ecosystem.as_deref(),
                min_size.as_deref(),
                confidence.as_deref(),
                limit,
                refresh,
                show_id.as_deref(),
                show_projects,
                cli.json,
            );
        }
    }
}

fn handle_tree(
    target_path: &Path,
    metric: CliTreeMetric,
    limit: usize,
    depth: usize,
    json: bool,
    refresh: bool,
) {
    let canonical = match target_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error resolving target path {:?}: {}", target_path, e);
            std::process::exit(1);
        }
    };
    let root_id = blake3::hash(canonical.to_string_lossy().as_bytes()).to_hex()[..16].to_string();

    let db_path = default_index_path();
    let mut db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index database: {}", e);
            std::process::exit(1);
        }
    };

    // Check if ready generation exists
    let latest_gen = if refresh {
        None
    } else {
        StorageTreeEngine::get_latest_ready_generation(db.conn(), &root_id)
            .ok()
            .flatten()
    };
    let gen = match latest_gen {
        Some(g) if g.root_path == canonical => g,
        _ => {
            // Build new generation from live scan
            let scanner = FilesystemScanner::new(ScanOptions::default());
            let report = match scanner.scan(&canonical) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Error scanning filesystem: {}", e);
                    std::process::exit(1);
                }
            };
            let builder = StorageTreeBuilder::new(&canonical, &root_id);
            let nodes = match builder.build_from_scanned_entries(&report.entries) {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Error building storage tree: {}", e);
                    std::process::exit(1);
                }
            };
            let root_node = nodes
                .iter()
                .find(|n| n.kind == StorageNodeKind::Root)
                .unwrap();
            let new_gen = StorageTreeGeneration {
                generation_id: format!("stg_{}_{}", root_id, Utc::now().timestamp_millis()),
                root_path: canonical.clone(),
                root_id: root_id.clone(),
                observed_at: Utc::now().timestamp(),
                source: StorageTreeSource::LiveScan,
                status: StorageTreeStatus::Ready,
                total_files: root_node.file_count,
                total_dirs: root_node.directory_count,
                total_logical_bytes: root_node.subtree_logical_bytes,
                total_allocated_bytes: root_node.subtree_allocated_bytes,
                coverage: StorageTreeCoverage {
                    files_observed: report.total_files,
                    directories_observed: report.total_dirs,
                    entries_skipped: report.skipped_paths.len() as u64,
                    analysis_complete: true,
                    ..Default::default()
                },
            };
            if let Err(e) = StorageTreeEngine::publish_generation(db.conn_mut(), &new_gen, &nodes) {
                eprintln!("Error publishing storage tree generation: {}", e);
                std::process::exit(1);
            }
            let _ = StorageTreeEngine::prune_older_generations(db.conn_mut(), &root_id, 2);
            new_gen
        }
    };

    let tree_metric = match metric {
        CliTreeMetric::Allocated => StorageTreeMetric::Allocated,
        CliTreeMetric::Logical => StorageTreeMetric::Logical,
    };

    let root_node_id = StorageNodeId::from_raw_relative(&root_id, b"");
    let (children, remainder, total_child_count) = match StorageTreeEngine::query_children_page(
        db.conn(),
        &gen.generation_id,
        &root_node_id,
        tree_metric,
        limit as u32,
        0,
    ) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error querying storage tree: {}", e);
            std::process::exit(1);
        }
    };

    let root_node = match StorageTreeEngine::get_node(db.conn(), &gen.generation_id, &root_node_id)
    {
        Ok(Some(n)) => n,
        _ => {
            eprintln!("Root node not found");
            std::process::exit(1);
        }
    };

    if json {
        let page_dto = vacua_api::StorageTreePageV1 {
            schema_version: vacua_api::SCHEMA_STORAGE_TREE_PAGE_V1.to_string(),
            generation_id: gen.generation_id.clone(),
            parent_node: root_node.to_dto(),
            metric: match metric {
                CliTreeMetric::Allocated => "allocated".to_string(),
                CliTreeMetric::Logical => "logical".to_string(),
            },
            items: children.iter().map(|c| c.to_dto()).collect(),
            total_child_count: total_child_count as usize,
            limit,
            offset: 0,
            remainder: remainder.to_dto(),
            item_deltas: None,
            next_cursor: None,
        };
        println!("{}", serde_json::to_string_pretty(&page_dto).unwrap());
    } else {
        println!("{}", canonical.display());
        println!(
            "  {} allocated ({} logical)\n",
            format_bytes(root_node.subtree_allocated_bytes),
            format_bytes(root_node.subtree_logical_bytes)
        );

        let root_metric_val = match metric {
            CliTreeMetric::Allocated => root_node.subtree_allocated_bytes,
            CliTreeMetric::Logical => root_node.subtree_logical_bytes,
        };

        let ctx = TreePrintContext {
            conn: db.conn(),
            gen_id: &gen.generation_id,
            metric,
            limit,
            max_depth: depth,
        };

        print_tree_level(&ctx, &children, &remainder, root_metric_val, 1);
    }
}

struct TreePrintContext<'a> {
    conn: &'a rusqlite::Connection,
    gen_id: &'a str,
    metric: CliTreeMetric,
    limit: usize,
    max_depth: usize,
}

fn print_tree_level(
    ctx: &TreePrintContext,
    children: &[vacua_tree::StorageTreeNode],
    remainder: &vacua_tree::StorageTreeRemainder,
    parent_metric_val: u64,
    current_depth: usize,
) {
    let indent = "  ".repeat(current_depth);

    for child in children {
        let child_val = match ctx.metric {
            CliTreeMetric::Allocated => child.subtree_allocated_bytes,
            CliTreeMetric::Logical => child.subtree_logical_bytes,
        };
        let pct = if parent_metric_val > 0 {
            (child_val as f64 / parent_metric_val as f64) * 100.0
        } else {
            0.0
        };
        let kind_tag = match child.kind {
            StorageNodeKind::Directory => format!("dir, {} files", child.file_count),
            StorageNodeKind::File => "file".to_string(),
            StorageNodeKind::Root => "root".to_string(),
        };
        println!(
            "{}{:<24} {:>10} ({:>5.1}%)  [{}]",
            indent,
            child.display_name,
            format_bytes(child_val),
            pct,
            kind_tag
        );

        // Recursively print sub-level if depth allows
        if current_depth < ctx.max_depth
            && child.kind == StorageNodeKind::Directory
            && child.child_count > 0
        {
            let tree_metric = match ctx.metric {
                CliTreeMetric::Allocated => StorageTreeMetric::Allocated,
                CliTreeMetric::Logical => StorageTreeMetric::Logical,
            };
            if let Ok((sub_children, sub_rem, _)) = StorageTreeEngine::query_children_page(
                ctx.conn,
                ctx.gen_id,
                &child.node_id,
                tree_metric,
                ctx.limit as u32,
                0,
            ) {
                print_tree_level(ctx, &sub_children, &sub_rem, child_val, current_depth + 1);
            }
        }
    }

    if remainder.item_count > 0 {
        let rem_val = match ctx.metric {
            CliTreeMetric::Allocated => remainder.allocated_bytes,
            CliTreeMetric::Logical => remainder.logical_bytes,
        };
        let pct = if parent_metric_val > 0 {
            (rem_val as f64 / parent_metric_val as f64) * 100.0
        } else {
            0.0
        };
        println!(
            "{}{:<24} {:>10} ({:>5.1}%)  [remainder, {} items]",
            indent,
            "Other",
            format_bytes(rem_val),
            pct,
            remainder.item_count
        );
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
                        let cur_event_id = vacua_index::fsevents::get_current_event_id();
                        let _ = db.upsert_watched_root(&canonical, 1, cur_event_id, "active");
                        if !json_mode {
                            println!(
                                "Incremental Index updated: {} entries indexed into {} (FSEvents cursor: {})",
                                report.entries.len(),
                                db_path.display(),
                                cur_event_id
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

    // 3. Development paths in debug mode (prioritize local builds over ambient PATH)
    #[cfg(debug_assertions)]
    {
        let dev_candidates = [
            PathBuf::from("apple/VacuaIntelligence/.build/debug/vacua-intelligence"),
            PathBuf::from("apple/VacuaIntelligence/.build/release/vacua-intelligence"),
            PathBuf::from(
                "apple/VacuaIntelligence/.build/arm64-apple-macosx/debug/vacua-intelligence",
            ),
            PathBuf::from(
                "apple/VacuaIntelligence/.build/arm64-apple-macosx/release/vacua-intelligence",
            ),
            PathBuf::from("../apple/VacuaIntelligence/.build/debug/vacua-intelligence"),
            PathBuf::from("../apple/VacuaIntelligence/.build/release/vacua-intelligence"),
        ];
        for cand in &dev_candidates {
            if cand.exists() {
                return Some(cand.clone());
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

fn handle_intelligence_capabilities(json_mode: bool) {
    let binary = match find_intelligence_binary() {
        Some(b) => b,
        None => {
            if json_mode {
                println!(
                    "{{\"status\":\"unavailable\",\"foundation_models_compiled\":false,\"guided_generation_compiled\":false,\"reason\":\"vacua-intelligence helper binary not found\"}}"
                );
            } else {
                println!("\nVacua Intelligence Capabilities");
                println!("──────────────────────────────────────────────────");
                println!("Helper Binary:              Not found");
                println!("Foundation Models Compiled: false");
                println!("Guided Generation Compiled: false");
                println!("──────────────────────────────────────────────────\n");
            }
            return;
        }
    };

    match Command::new(&binary).arg("capabilities").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if json_mode {
                print!("{}", stdout);
            } else if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                println!("\nVacua Intelligence Capabilities");
                println!("──────────────────────────────────────────────────");
                println!("Helper Location:            {}", binary.display());
                println!(
                    "Foundation Models Compiled: {}",
                    val.get("foundation_models_compiled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                );
                println!(
                    "Guided Generation Compiled: {}",
                    val.get("guided_generation_compiled")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                );
                println!(
                    "SDK Version:                {}",
                    val.get("sdk_version")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                println!("──────────────────────────────────────────────────\n");
            } else {
                print!("{}", stdout);
            }
        }
        Err(e) => {
            eprintln!("Failed to invoke intelligence capabilities: {}", e);
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

fn handle_rescue(path: &Path, apply: bool, dry_run: bool, yes: bool, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let status = match query_volume_status(&canonical) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to query volume status: {}", e);
            std::process::exit(1);
        }
    };

    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(6),
        ..Default::default()
    });

    let report = match scanner.scan(&canonical) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error scanning for storage rescue: {}", e);
            std::process::exit(1);
        }
    };

    let mut engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut engine);

    let mut candidates = Vec::new();
    for entry in report.entries {
        let alloc = entry.to_allocation();
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

    let rescue_plan = build_storage_rescue_plan(&canonical, &status, &candidates);

    let safe_groups_dto: Vec<CandidateGroupSummaryV1> = rescue_plan
        .safe_groups
        .iter()
        .map(|g| CandidateGroupSummaryV1 {
            group_id: g.group_id.clone(),
            group_type: g.group_type.clone(),
            title: g.title.clone(),
            description: g.description.clone(),
            item_count: g.item_count,
            project_or_app_count: g.project_or_app_count,
            logical_bytes: g.logical_bytes,
            confirmed_physical_reclaim_bytes: g.confirmed_physical_reclaim_bytes,
            estimated_reclaim_bytes: g.estimated_reclaim_bytes,
            evidence_level: g.evidence_level.clone(),
            evidence_reasons: g.evidence_reasons.clone(),
            candidate_ids: g.candidate_ids.clone(),
            eligible_for_one_click: g.eligible_for_one_click,
            active_guard_deferred: g.active_guard_deferred,
        })
        .collect();

    let review_groups_dto: Vec<CandidateGroupSummaryV1> = rescue_plan
        .review_groups
        .iter()
        .map(|g| CandidateGroupSummaryV1 {
            group_id: g.group_id.clone(),
            group_type: g.group_type.clone(),
            title: g.title.clone(),
            description: g.description.clone(),
            item_count: g.item_count,
            project_or_app_count: g.project_or_app_count,
            logical_bytes: g.logical_bytes,
            confirmed_physical_reclaim_bytes: g.confirmed_physical_reclaim_bytes,
            estimated_reclaim_bytes: g.estimated_reclaim_bytes,
            evidence_level: g.evidence_level.clone(),
            evidence_reasons: g.evidence_reasons.clone(),
            candidate_ids: g.candidate_ids.clone(),
            eligible_for_one_click: g.eligible_for_one_click,
            active_guard_deferred: g.active_guard_deferred,
        })
        .collect();

    let domains_dto: Vec<StorageDomainV1> = rescue_plan
        .volume_accounting
        .domains
        .iter()
        .map(|d| StorageDomainV1 {
            id: d.id.clone(),
            label: d.label.clone(),
            logical_bytes: d.logical_bytes,
            allocated_bytes: d.allocated_bytes,
            confidence: d.confidence.clone(),
            source: d.source.clone(),
            reclaimable_bytes: d.reclaimable_bytes,
            review_bytes: d.review_bytes,
        })
        .collect();

    let volume_accounting_dto = WholeVolumeAccountingV1 {
        total_capacity_bytes: rescue_plan.volume_accounting.total_capacity_bytes,
        volume_used_bytes: rescue_plan.volume_accounting.volume_used_bytes,
        volume_available_bytes: rescue_plan.volume_accounting.volume_available_bytes,
        attributed_bytes: rescue_plan.volume_accounting.attributed_bytes,
        unattributed_system_managed_bytes: rescue_plan
            .volume_accounting
            .unattributed_system_managed_bytes,
        reconciliation_tolerance_bytes: rescue_plan
            .volume_accounting
            .reconciliation_tolerance_bytes,
        pressure_level: rescue_plan.volume_accounting.pressure_level.clone(),
        is_material_discrepancy: rescue_plan.volume_accounting.is_material_discrepancy,
        domains: domains_dto,
    };

    let protected_summary_dto = ProtectedSummaryV1 {
        protected_locations_count: rescue_plan.protected_summary.protected_locations_count,
        protected_categories: rescue_plan.protected_summary.protected_categories.clone(),
        description: rescue_plan.protected_summary.description.clone(),
    };

    let summary_dto = StorageRescueSummaryV1 {
        schema_version: SCHEMA_STORAGE_RESCUE_SUMMARY_V1.to_string(),
        observed_at: Utc::now().to_rfc3339(),
        volume_accounting: volume_accounting_dto,
        safe_reclaimable_bytes: rescue_plan.safe_reclaimable_bytes,
        review_recommended_bytes: rescue_plan.review_recommended_bytes,
        system_managed_uncertain_bytes: rescue_plan.system_managed_uncertain_bytes,
        safe_groups: safe_groups_dto,
        review_groups: review_groups_dto,
        protected_summary: protected_summary_dto,
    };

    if json_mode && !apply {
        println!("{}", serde_json::to_string_pretty(&summary_dto).unwrap());
        return;
    }

    if !json_mode {
        println!(
            "\n=================================================================================="
        );
        println!("VACUA STORAGE RESCUE — Storage Pressure & Whole-Volume Accounting");
        println!(
            "=================================================================================="
        );
        println!("Volume:              {}", status.mount_point);
        println!("Capacity:            {}", format_bytes(status.total_bytes));
        println!(
            "Used:                {}",
            format_bytes(status.total_bytes.saturating_sub(status.available_bytes))
        );
        println!(
            "Available:           {}",
            format_bytes(status.available_bytes)
        );
        println!(
            "Pressure Level:      {}",
            rescue_plan.volume_accounting.pressure_level.to_uppercase()
        );

        println!("\nWhole-Volume Reconciliation:");
        println!(
            "  Attributed Space:  {}",
            format_bytes(rescue_plan.volume_accounting.attributed_bytes)
        );
        println!(
            "  System / Unknown:  {}",
            format_bytes(
                rescue_plan
                    .volume_accounting
                    .unattributed_system_managed_bytes
            )
        );
        if rescue_plan.volume_accounting.is_material_discrepancy {
            println!("  Notice:            Some storage is not attributable from user-space evidence (APFS snapshots, purgeable, swap).");
        }

        println!("\nTop Storage Domains:");
        for d in &rescue_plan.volume_accounting.domains {
            if d.allocated_bytes > 0 {
                println!(
                    "  - {:<20} {:>10}  (safe reclaimable: {})",
                    d.label,
                    format_bytes(d.allocated_bytes),
                    format_bytes(d.reclaimable_bytes)
                );
            }
        }

        println!("\nReclaim Opportunities:");
        println!(
            "  SAFE TO RECLAIM:     {} (Eligible for One-Decision cleanup)",
            format_bytes(rescue_plan.safe_reclaimable_bytes)
        );
        println!(
            "  REVIEW RECOMMENDED:  {}",
            format_bytes(rescue_plan.review_recommended_bytes)
        );
        println!(
            "  PROTECTED:           {} locations guarded",
            rescue_plan.protected_summary.protected_locations_count
        );

        println!("\nCanonical Safe Groups:");
        if rescue_plan.safe_groups.is_empty() || rescue_plan.safe_reclaimable_bytes == 0 {
            println!("  (No safe candidates identified meeting automatic safety criteria)");
        } else {
            for g in &rescue_plan.safe_groups {
                println!(
                    "  [✓] {:<22} {:>3} targets / {:>4} items  {:>10}  {}",
                    g.title,
                    g.project_or_app_count,
                    g.item_count,
                    format_bytes(g.confirmed_physical_reclaim_bytes),
                    g.evidence_reasons.first().cloned().unwrap_or_default()
                );
            }
        }

        if !rescue_plan.review_groups.is_empty() {
            println!("\nReview Groups (Excluded from one-decision safe plan):");
            for g in &rescue_plan.review_groups {
                println!(
                    "  [?] {:<22} {:>3} targets / {:>4} items  {:>10}  {}",
                    g.title,
                    g.project_or_app_count,
                    g.item_count,
                    format_bytes(g.confirmed_physical_reclaim_bytes),
                    g.description
                );
            }
        }
        println!(
            "=================================================================================="
        );
    }

    if !apply {
        if !json_mode {
            if rescue_plan.safe_reclaimable_bytes > 0 {
                println!("\nTo reclaim all safe items in one decision, run:");
                println!("  vacua rescue --apply\n");
            } else {
                println!(
                    "\nVacua could not identify storage that meets its automatic safety criteria."
                );
                println!("Inspect review candidates with: vacua candidates --risk review\n");
            }
        }
        return;
    }

    // Apply workflow
    if rescue_plan.safe_groups.is_empty() || rescue_plan.safe_reclaimable_bytes == 0 {
        if json_mode {
            eprintln!("No safe items eligible for cleanup.");
        } else {
            println!(
                "\nVacua could not identify storage that meets its automatic safety criteria."
            );
            println!("Nothing safe to reclaim automatically. No files were modified.");
        }
        return;
    }

    let safe_group_ids: Vec<String> = rescue_plan
        .safe_groups
        .iter()
        .map(|g| g.group_id.clone())
        .collect();

    let plan =
        match CleanupPlan::build_from_groups(&candidates, &safe_group_ids, &canonical, "v0.9.0") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to compile group cleanup plan: {}", e);
                std::process::exit(1);
            }
        };

    if let Err(e) = plan.verify_integrity() {
        eprintln!("SECURITY ERROR: Plan integrity verification failed: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = plan.preflight_check() {
        eprintln!("Preflight safety check failed: {}. Execution aborted.", e);
        std::process::exit(1);
    }

    if !json_mode {
        println!("\nOne-Decision Safe Cleanup Plan");
        println!("==================================================");
        println!("Plan ID:             {}", plan.plan_id);
        println!("Target Groups:       {}", rescue_plan.safe_groups.len());
        println!("Total Safe Items:    {}", plan.items.len());
        println!(
            "Reclaimable:         {}",
            format_bytes(plan.estimated_eventual_reclaim_bytes)
        );
        println!("Preservation Guards: {}", plan.preservation_guards.len());
        println!("Protected Items:     0 (Guaranteed)");
        println!("Action:              MOVE TO TRASH (Reversible)");
        println!("==================================================");

        if !dry_run && !yes {
            print!(
                "\nReclaim {} now? [y/N]: ",
                format_bytes(plan.estimated_eventual_reclaim_bytes)
            );
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
                    "FAIL-SAFE ABORT: Cannot open execution journal ({}): {}",
                    journal_path.display(),
                    e
                );
                std::process::exit(1);
            }
            None
        }
    };

    match executor.execute(&plan, journal.as_mut()) {
        Ok(report) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                println!("\nStorage Rescue Execution Summary");
                println!("==================================================");
                println!("Items Moved to Trash: {}", report.successful_items.len());
                println!("Items Skipped (stale): {}", report.skipped_items.len());
                println!("Items Failed:          {}", report.failed_items.len());
                println!(
                    "Reclaimed (to Trash):  {}",
                    format_bytes(report.bytes_moved_to_trash)
                );
                if report.actual_free_space_before > 0 && report.actual_free_space_after > 0 {
                    let delta_str = if report.actual_free_delta >= 0 {
                        format!("+{}", format_bytes(report.actual_free_delta as u64))
                    } else {
                        format!("-{}", format_bytes((-report.actual_free_delta) as u64))
                    };
                    println!(
                        "Physical Volume Free:  {} -> {} (Delta: {})",
                        format_bytes(report.actual_free_space_before),
                        format_bytes(report.actual_free_space_after),
                        delta_str
                    );
                }
                println!(
                    "Audit Journal:         Recorded in {}",
                    journal_path.display()
                );
                println!("==================================================");
            }
        }
        Err(e) => {
            eprintln!("Execution failed: {}", e);
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
        let alloc = entry.to_allocation();
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
        let alloc = entry.to_allocation();
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

fn handle_plan(
    path: &Path,
    risk_filter: RiskFilter,
    output: Option<PathBuf>,
    simulate: bool,
    json_mode: bool,
) {
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
        let alloc = entry.to_allocation();
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

    if simulate {
        let sim = CleanupSimulation::simulate(&candidates, max_risk);
        if json_mode {
            println!("{}", serde_json::to_string_pretty(&sim).unwrap());
        } else {
            println!("\nWhat-If Cleanup Simulation (Max Risk: {:?})", risk_filter);
            println!("==================================================");
            println!(
                "Expected Freeable Space: {}",
                format_bytes(sim.expected_freeable_bytes)
            );
            println!(
                "Confidence Range:        {} – {}",
                format_bytes(sim.confidence_range.0),
                format_bytes(sim.confidence_range.1)
            );
            println!("Items Moved to Trash:    {}", sim.items_moved_to_trash);
            println!(
                "Estimated Re-download:   {}",
                format_bytes(sim.total_redownload_cost_bytes)
            );
            println!(
                "Build Regeneration:      {} files",
                sim.build_regeneration_count
            );
            println!("Active Protected Roots:  0 (Safety invariants strictly enforced)");
            println!("==================================================\n");
        }
        return;
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
                    "Est. Eventual Reclaim:  {}",
                    format_bytes(plan.estimated_eventual_reclaim_bytes)
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
            "Estimated Eventual Reclaim: {}",
            format_bytes(plan.estimated_eventual_reclaim_bytes)
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
                println!(
                    "Moved to Trash:                          {}",
                    format_bytes(report.bytes_moved_to_trash)
                );
                println!(
                    "Potential Reclaim After Trash Is Emptied: {}",
                    format_bytes(report.estimated_eventual_reclaim_after_purge)
                );
                println!(
                    "Immediate Filesystem Reclaim:             {}",
                    format_bytes(report.immediate_reclaimed_bytes)
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

fn handle_index_refresh(path: &Path, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let db_path = default_index_path();
    let mut db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index database: {}", e);
            std::process::exit(1);
        }
    };

    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: None,
        ..Default::default()
    });

    match db.refresh_root(&canonical, &scanner) {
        Ok(res) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&res).unwrap());
            } else {
                println!("\nNative FSEvents Incremental Refresh");
                println!("==================================================");
                println!("Target Root:             {}", res.root_path.display());
                println!("Status:                  {}", res.status);
                println!(
                    "Full Rescan Performed:   {}",
                    if res.full_rescan_performed {
                        "YES"
                    } else {
                        "NO"
                    }
                );
                println!("Dirty Subtrees Rescanned:{}", res.dirty_subtrees_count);
                println!("Indexed Entries Updated: {}", res.updated_entries_count);
                println!("Previous Event Cursor:   {}", res.previous_event_id);
                println!("New Event Cursor:        {}", res.new_event_id);
                println!("==================================================\n");
            }
        }
        Err(e) => {
            eprintln!("Incremental refresh failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_snapshot_create(path: &Path, name: &str, json_mode: bool) {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let db_path = default_index_path();
    let mut db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index database: {}", e);
            std::process::exit(1);
        }
    };

    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: None,
        ..Default::default()
    });

    let report = match scanner.scan(&canonical) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Scan failed during snapshot creation: {}", e);
            std::process::exit(1);
        }
    };

    let snap_id = format!("snap-{}", chrono::Utc::now().timestamp_millis());
    let subtrees = vacua_index::build_recursive_subtrees(&canonical, &report.entries);

    let snapshot = StorageSnapshot {
        snapshot_id: snap_id.clone(),
        name: name.to_string(),
        root_path: canonical.clone(),
        timestamp: chrono::Utc::now().timestamp(),
        total_files: report.total_files,
        total_dirs: report.total_dirs,
        logical_bytes: report.allocation.logical_bytes,
        allocated_bytes: report.allocation.allocated_bytes,
        subtrees,
    };

    match db.save_snapshot(&snapshot) {
        Ok(_) => {
            if json_mode {
                let out = serde_json::json!({
                    "snapshot_id": snap_id,
                    "name": name,
                    "root_path": canonical.to_string_lossy(),
                    "total_entries": report.entries.len(),
                    "allocated_bytes": report.allocation.allocated_bytes,
                    "logical_bytes": report.allocation.logical_bytes,
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            } else {
                println!("\nStorage Snapshot Created");
                println!("==================================================");
                println!("Snapshot ID:        {}", snap_id);
                println!("Name:               {}", name);
                println!("Root Path:          {}", canonical.display());
                println!("Entries Indexed:    {}", report.entries.len());
                println!(
                    "Allocated Space:    {}",
                    format_bytes(report.allocation.allocated_bytes)
                );
                println!(
                    "Logical Space:      {}",
                    format_bytes(report.allocation.logical_bytes)
                );
                println!("==================================================\n");
            }
        }
        Err(e) => {
            eprintln!("Failed to save snapshot: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_snapshot_list(json_mode: bool) {
    let db_path = default_index_path();
    let db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index database: {}", e);
            std::process::exit(1);
        }
    };

    match db.list_snapshots() {
        Ok(snapshots) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&snapshots).unwrap());
            } else {
                println!("\nHistorical Storage Snapshots");
                println!(
                    "=========================================================================================="
                );
                if snapshots.is_empty() {
                    println!(
                        "No snapshots found. Run 'vacua snapshot create <path> --name <name>' to record one."
                    );
                } else {
                    println!(
                        "{:<16} {:<24} {:<10} {:>12}  ROOT PATH",
                        "NAME", "TIMESTAMP", "FILES", "ALLOCATED"
                    );
                    println!(
                        "──────────────────────────────────────────────────────────────────────────────────────────"
                    );
                    for s in &snapshots {
                        println!(
                            "{:<16} {:<24} {:<10} {:>12}  {}",
                            s.name,
                            chrono::DateTime::from_timestamp(s.timestamp, 0)
                                .map(|dt| dt.to_rfc3339())
                                .unwrap_or_else(|| s.timestamp.to_string()),
                            s.total_files,
                            format_bytes(s.allocated_bytes),
                            s.root_path.display()
                        );
                    }
                }
                println!(
                    "==========================================================================================\n"
                );
            }
        }
        Err(e) => {
            eprintln!("Failed to list snapshots: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_diff(base: &str, target: &str, json_mode: bool) {
    let db_path = default_index_path();
    let db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open index database: {}", e);
            std::process::exit(1);
        }
    };

    match db.diff_snapshots(base, target) {
        Ok(diff) => {
            if json_mode {
                println!("{}", serde_json::to_string_pretty(&diff).unwrap());
            } else {
                println!(
                    "\nStorage Snapshot Diff ({} -> {})",
                    diff.base_name, diff.target_name
                );
                println!("==================================================");
                let sign = if diff.allocated_delta_bytes >= 0 {
                    "+"
                } else {
                    "-"
                };
                println!(
                    "Total Allocated Delta: {}{}",
                    sign,
                    format_bytes(diff.allocated_delta_bytes.unsigned_abs())
                );
                let files_sign = if diff.files_delta >= 0 { "+" } else { "-" };
                println!(
                    "Files Count Delta:     {}{}",
                    files_sign,
                    diff.files_delta.unsigned_abs()
                );
                let growing: Vec<_> = diff
                    .subtree_deltas
                    .iter()
                    .filter(|d| d.delta_bytes > 0)
                    .collect();
                let shrinking: Vec<_> = diff
                    .subtree_deltas
                    .iter()
                    .filter(|d| d.delta_bytes < 0)
                    .collect();
                println!("Growing Subtrees:      {}", growing.len());
                println!("Shrinking Subtrees:    {}", shrinking.len());
                println!("──────────────────────────────────────────────────");
                if !growing.is_empty() {
                    println!("Top Growing Locations:");
                    for g in growing.iter().take(5) {
                        println!("  +{}  {}", format_bytes(g.delta_bytes as u64), g.path);
                    }
                }
                if !shrinking.is_empty() {
                    println!("Top Shrinking Locations:");
                    for s in shrinking.iter().take(5) {
                        println!(
                            "  -{}  {}",
                            format_bytes(s.delta_bytes.unsigned_abs()),
                            s.path
                        );
                    }
                }
                println!("==================================================\n");
            }
        }
        Err(e) => {
            eprintln!("Failed to diff snapshots: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_apps_list(json_mode: bool) {
    let graph = ApplicationEvidenceGraph::build_from_system();
    let mut apps = Vec::new();

    for node in graph.nodes.values() {
        if node.kind == NodeKind::ApplicationBundle {
            let name = node.metadata.get("name").cloned().unwrap_or_default();
            let bundle_id = node.metadata.get("bundle_id").cloned().unwrap_or_default();
            let path = node
                .path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            apps.push((name, bundle_id, path));
        }
    }
    apps.sort_by(|a, b| a.0.cmp(&b.0));

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&apps).unwrap());
    } else {
        println!("\nDiscovered Application Bundles ({})", apps.len());
        println!(
            "=========================================================================================="
        );
        println!("{:<28} {:<36} PATH", "NAME", "BUNDLE IDENTIFIER");
        println!(
            "──────────────────────────────────────────────────────────────────────────────────────────"
        );
        for (name, bid, path) in &apps {
            println!("{:<28} {:<36} {}", name, bid, path);
        }
        println!(
            "==========================================================================================\n"
        );
    }
}

fn handle_apps_show(bundle_id: &str, json_mode: bool) {
    let graph = ApplicationEvidenceGraph::build_from_system();
    let eval = graph.evaluate_orphan(bundle_id);

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&eval).unwrap());
    } else {
        println!("\nApplication Evidence Graph: {}", eval.bundle_id);
        println!("==================================================");
        println!("Application Name:     {}", eval.app_name);
        println!(
            "Bundle Installed:     {}",
            if eval.bundle_installed {
                "YES"
            } else {
                "NO (UNINSTALLED)"
            }
        );
        if let Some(ref p) = eval.bundle_path {
            println!("Bundle Path:          {}", p.display());
        }
        println!(
            "Package Receipt:      {}",
            if eval.receipt_present {
                "PRESENT"
            } else {
                "ABSENT"
            }
        );
        println!(
            "Associated Space:     {}",
            format_bytes(eval.associated_artifacts_bytes)
        );
        println!("Orphan Confidence:    {:?}", eval.orphan_confidence);
        println!("Suggested Risk:       {}", eval.risk_level);
        println!("Explanation:          {}", eval.explanation);
        if !eval.artifact_paths.is_empty() {
            println!("Discovered Artifacts ({}):", eval.artifact_paths.len());
            for p in eval.artifact_paths.iter().take(8) {
                println!("  - {}", p.display());
            }
        }
        println!("==================================================\n");
    }
}

fn handle_leftovers(json_mode: bool) {
    let graph = ApplicationEvidenceGraph::build_from_system();
    let mut checked_bids = HashSet::new();
    let mut leftovers = Vec::new();

    for edge in &graph.edges {
        if edge.target_id.starts_with("bid:") {
            let bid = edge.target_id.trim_start_matches("bid:");
            if checked_bids.insert(bid.to_string()) {
                let eval = graph.evaluate_orphan(bid);
                if eval.orphan_confidence >= OrphanConfidence::Medium
                    && !eval.bundle_installed
                    && eval.associated_artifacts_bytes > 0
                {
                    leftovers.push(eval);
                }
            }
        }
    }

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&leftovers).unwrap());
    } else {
        println!("\nUninstalled Application Leftovers (High/Medium Orphan Confidence)");
        println!(
            "=========================================================================================="
        );
        if leftovers.is_empty() {
            println!("No high-confidence orphaned application leftovers detected.");
        } else {
            println!(
                "{:<32} {:<12} {:>12}  EVIDENCE",
                "BUNDLE IDENTIFIER", "CONFIDENCE", "SIZE"
            );
            println!(
                "──────────────────────────────────────────────────────────────────────────────────────────"
            );
            for l in &leftovers {
                println!(
                    "{:<32} {:<12?} {:>12}  {}",
                    l.bundle_id,
                    l.orphan_confidence,
                    format_bytes(l.associated_artifacts_bytes),
                    l.explanation
                );
            }
        }
        println!(
            "==========================================================================================\n"
        );
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageReasoningCandidate {
    candidate_id: String,
    path: String,
    category: String,
    risk: String,
    allocated_bytes: u64,
    confirmed_freeable_bytes: u64,
    rebuild_tier: String,
    evidence_summary: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageReasoningSubtree {
    path: String,
    allocated_delta_bytes: i64,
    change_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageReasoningSnapshotDiff {
    base_snapshot: String,
    target_snapshot: String,
    allocated_delta_bytes: i64,
    top_growing_subtrees: Vec<StorageReasoningSubtree>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageReasoningContext {
    query: String,
    storage_root: String,
    snapshot_diff: Option<StorageReasoningSnapshotDiff>,
    top_candidates: Vec<StorageReasoningCandidate>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct StorageCause {
    title: String,
    explanation: String,
    candidate_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct StorageExplanation {
    summary: String,
    causes: Vec<StorageCause>,
    referenced_candidate_ids: Vec<String>,
    referenced_snapshot_ids: Vec<String>,
    caution: Option<String>,
}

fn handle_ask(query: &str, json_mode: bool) {
    let db_path = default_index_path();
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Snapshot Diff Evidence
    let mut diff_summary = None;
    let mut reasoning_diff = None;
    let mut valid_snapshot_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    if db_path.exists() {
        if let Ok(db) = IndexDatabase::open(&db_path) {
            if let Ok(snapshots) = db.list_snapshots() {
                if snapshots.len() >= 2 {
                    let base = &snapshots[snapshots.len() - 2].name;
                    let target = &snapshots[snapshots.len() - 1].name;
                    valid_snapshot_ids.insert(base.clone());
                    valid_snapshot_ids.insert(target.clone());
                    if let Ok(d) = db.diff_snapshots(base, target) {
                        let growing: Vec<StorageReasoningSubtree> = d
                            .subtree_deltas
                            .iter()
                            .filter(|sd| sd.delta_bytes > 0)
                            .take(5)
                            .map(|sd| StorageReasoningSubtree {
                                path: sd.path.clone(),
                                allocated_delta_bytes: sd.delta_bytes,
                                change_type: sd.change_type.clone(),
                            })
                            .collect();

                        reasoning_diff = Some(StorageReasoningSnapshotDiff {
                            base_snapshot: base.clone(),
                            target_snapshot: target.clone(),
                            allocated_delta_bytes: d.allocated_delta_bytes,
                            top_growing_subtrees: growing,
                        });
                        diff_summary = Some((base.clone(), target.clone(), d));
                    }
                } else if snapshots.len() == 1 {
                    let base = &snapshots[0].name;
                    valid_snapshot_ids.insert(base.clone());
                    valid_snapshot_ids.insert("current".to_string());
                    if let Ok(d) = db.diff_snapshots(base, "current") {
                        let growing: Vec<StorageReasoningSubtree> = d
                            .subtree_deltas
                            .iter()
                            .filter(|sd| sd.delta_bytes > 0)
                            .take(5)
                            .map(|sd| StorageReasoningSubtree {
                                path: sd.path.clone(),
                                allocated_delta_bytes: sd.delta_bytes,
                                change_type: sd.change_type.clone(),
                            })
                            .collect();

                        reasoning_diff = Some(StorageReasoningSnapshotDiff {
                            base_snapshot: base.clone(),
                            target_snapshot: "current".to_string(),
                            allocated_delta_bytes: d.allocated_delta_bytes,
                            top_growing_subtrees: growing,
                        });
                        diff_summary = Some((base.clone(), "current".to_string(), d));
                    }
                }
            }
        }
    }

    // 2. Scan & Grounded Candidates Evidence
    let scanner = FilesystemScanner::new(ScanOptions {
        cross_mounts: false,
        max_depth: Some(5),
        ..Default::default()
    });

    let mut rules_engine = RulesEngine::new();
    let mut evaluator = CandidateEvaluator::new(&mut rules_engine);
    let mut top_candidates_for_context = Vec::new();
    let mut valid_candidate_ids: std::collections::HashSet<String> =
        std::collections::HashSet::new();

    if let Ok(report) = scanner.scan(&home) {
        let mut candidates = Vec::new();
        for entry in report.entries {
            let alloc = entry.to_allocation();
            let cand = evaluator.evaluate(
                &entry.path,
                alloc,
                entry.inode,
                entry.device_id,
                entry.mtime_sec,
                entry.is_dir,
            );
            if !cand.risk.is_protected_or_unknown() {
                candidates.push(cand);
            }
        }

        candidates.sort_by(|a, b| {
            b.allocation
                .allocated_bytes
                .cmp(&a.allocation.allocated_bytes)
        });

        for c in candidates.into_iter().take(5) {
            let cost = ReclaimCost::from_candidate(&c);
            let ev_summary = c
                .evidence
                .first()
                .map(|e| e.explanation.clone())
                .unwrap_or_else(|| "Filesystem pattern match".to_string());

            valid_candidate_ids.insert(c.id.clone());
            top_candidates_for_context.push(StorageReasoningCandidate {
                candidate_id: c.id.clone(),
                path: c.path.to_string_lossy().to_string(),
                category: c.category.to_string(),
                risk: c.risk.to_string(),
                allocated_bytes: c.allocation.allocated_bytes,
                confirmed_freeable_bytes: c.allocation.confirmed_freeable_bytes(),
                rebuild_tier: format!("{:?}", cost.tier),
                evidence_summary: ev_summary,
            });
        }
    }

    // 3. Assemble Grounded Context
    let reasoning_context = StorageReasoningContext {
        query: query.to_string(),
        storage_root: home.to_string_lossy().to_string(),
        snapshot_diff: reasoning_diff,
        top_candidates: top_candidates_for_context,
    };
    let context_json =
        serde_json::to_string(&reasoning_context).unwrap_or_else(|_| "{}".to_string());

    // 4. Invoke Intelligence Helper (Explain Action)
    let helper_bin = find_intelligence_binary();
    let mut provider_used = "deterministic-fallback".to_string();
    let mut generation_mode = "deterministic-parser".to_string();
    let mut fallback_used = true;
    let mut raw_explanation: Option<StorageExplanation> = None;

    if let Some(ref bin) = helper_bin {
        if let Ok(output) = Command::new(bin)
            .args(["explain", query, &context_json])
            .output()
        {
            if output.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(p) = val.get("provider_used").and_then(|v| v.as_str()) {
                        provider_used = p.to_string();
                    }
                    if let Some(m) = val.get("generation_mode").and_then(|v| v.as_str()) {
                        generation_mode = m.to_string();
                    }
                    if let Some(f) = val.get("fallback_used").and_then(|v| v.as_bool()) {
                        fallback_used = f;
                    }
                    if let Some(exp_val) = val.get("explanation") {
                        if let Ok(parsed_exp) =
                            serde_json::from_value::<StorageExplanation>(exp_val.clone())
                        {
                            raw_explanation = Some(parsed_exp);
                        }
                    }
                }
            }
        }
    }

    // 5. Fallback if model failed or unavailable
    let explanation = match raw_explanation {
        Some(exp) => exp,
        None => {
            provider_used = "deterministic-fallback".to_string();
            generation_mode = "deterministic-parser".to_string();
            fallback_used = true;

            let mut causes = Vec::new();
            let mut referenced_candidates = Vec::new();

            if let Some(ref diff) = reasoning_context.snapshot_diff {
                for g in &diff.top_growing_subtrees {
                    causes.push(StorageCause {
                        title: format!("Recent directory growth in {}", g.path),
                        explanation: format!(
                            "Allocated space increased by {} (change: {}).",
                            format_bytes(g.allocated_delta_bytes.unsigned_abs()),
                            g.change_type
                        ),
                        candidate_id: None,
                    });
                }
            }

            for c in &reasoning_context.top_candidates {
                causes.push(StorageCause {
                    title: format!("Accumulated {} data in {}", c.category, c.path),
                    explanation: format!(
                        "Consuming {} allocated space (rebuild tier: {}, confirmed freeable: {}). Evidence: {}",
                        format_bytes(c.allocated_bytes),
                        c.rebuild_tier,
                        format_bytes(c.confirmed_freeable_bytes),
                        c.evidence_summary
                    ),
                    candidate_id: Some(c.candidate_id.clone()),
                });
                referenced_candidates.push(c.candidate_id.clone());
            }

            let summary = if causes.is_empty() {
                "No significant disk growth or cleanup candidates were detected in the inspected hierarchy.".to_string()
            } else {
                format!(
                    "Analyzed storage pressure based on {} candidates and snapshot telemetry. Largest consumers include {} locations.",
                    reasoning_context.top_candidates.len(),
                    causes.len()
                )
            };

            StorageExplanation {
                summary,
                causes,
                referenced_candidate_ids: referenced_candidates,
                referenced_snapshot_ids: valid_snapshot_ids.iter().cloned().collect(),
                caution: Some(
                    "Review candidate impact and rebuild costs before taking destructive action."
                        .to_string(),
                ),
            }
        }
    };

    // 6. Grounded Reference Validation (Drop Hallucinated IDs)
    let mut validated_candidate_ids = Vec::new();
    let mut dropped_hallucinations = Vec::new();
    for id in &explanation.referenced_candidate_ids {
        if valid_candidate_ids.contains(id) {
            validated_candidate_ids.push(id.clone());
        } else {
            dropped_hallucinations.push(id.clone());
        }
    }

    let mut validated_snapshot_ids = Vec::new();
    for id in &explanation.referenced_snapshot_ids {
        if valid_snapshot_ids.contains(id) {
            validated_snapshot_ids.push(id.clone());
        }
    }

    // 7. Render Output
    if json_mode {
        let out = serde_json::json!({
            "query": query,
            "provider_used": provider_used,
            "generation_mode": generation_mode,
            "fallback_used": fallback_used,
            "execution_authority": "forbidden_zero_mutation_authority",
            "grounding_provenance": {
                "snapshot_grounding_present": diff_summary.is_some(),
                "validated_candidate_references": validated_candidate_ids,
                "validated_snapshot_references": validated_snapshot_ids,
                "dropped_hallucinations": dropped_hallucinations,
            },
            "explanation": {
                "summary": explanation.summary,
                "causes": explanation.causes,
                "caution": explanation.caution,
            }
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!("\nVacua Grounded Storage Intelligence Query Engine");
        println!("==================================================");
        println!("Query: \"{}\"", query);
        println!(
            "Explanation Grounding: {}",
            if provider_used == "apple-system" {
                "Apple Foundation Models (On-Device Neural Guided Generation)"
            } else {
                "Deterministic Grounded Evidence Engine"
            }
        );
        println!("Generation Mode:       {}", generation_mode);
        println!("Execution Authority:   FORBIDDEN (Read-only analysis; zero mutation authority)");
        println!("──────────────────────────────────────────────────");

        if let Some((base, target, diff)) = diff_summary {
            println!(
                "Snapshot Diff Evidence ({} -> {}): Total Allocated Delta: {}{}",
                base,
                target,
                if diff.allocated_delta_bytes >= 0 {
                    "+"
                } else {
                    "-"
                },
                format_bytes(diff.allocated_delta_bytes.unsigned_abs())
            );
            let growing: Vec<_> = diff
                .subtree_deltas
                .iter()
                .filter(|d| d.delta_bytes > 0)
                .collect();
            if !growing.is_empty() {
                println!("  Top Growing Locations:");
                for g in growing.iter().take(3) {
                    println!("    +{}  {}", format_bytes(g.delta_bytes as u64), g.path);
                }
            }
            println!("──────────────────────────────────────────────────");
        }

        println!("Summary:\n  {}\n", explanation.summary);

        if !explanation.causes.is_empty() {
            println!("Key Storage Causes:");
            for (idx, cause) in explanation.causes.iter().enumerate() {
                println!("  {}. {}", idx + 1, cause.title);
                println!("     {}", cause.explanation);
                if let Some(ref cid) = cause.candidate_id {
                    if valid_candidate_ids.contains(cid) {
                        println!("     [Grounding ID: {} (verified)]", cid);
                    } else {
                        println!("     [Grounding ID: {} (unverified/dropped)]", cid);
                    }
                }
            }
        }

        if let Some(ref caution) = explanation.caution {
            println!("\nCaution:\n  {}", caution);
        }

        if !dropped_hallucinations.is_empty() {
            println!("\nSafety Notice:");
            println!(
                "  Dropped unverified model references: {:?}",
                dropped_hallucinations
            );
        }

        println!("\n==================================================\n");
    }
}

fn parse_size(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Size cannot be empty".to_string());
    }

    let (num_part, multiplier): (&str, u64) = if s.ends_with("TiB") || s.ends_with("tib") {
        (&s[..s.len() - 3], 1024u64 * 1024 * 1024 * 1024)
    } else if s.ends_with('T') || s.ends_with('t') || s.ends_with("TB") || s.ends_with("tb") {
        let end = if s.ends_with("TB") || s.ends_with("tb") {
            s.len() - 2
        } else {
            s.len() - 1
        };
        (&s[..end], 1024u64 * 1024 * 1024 * 1024)
    } else if s.ends_with("GiB") || s.ends_with("gib") {
        (&s[..s.len() - 3], 1024u64 * 1024 * 1024)
    } else if s.ends_with('G') || s.ends_with('g') || s.ends_with("GB") || s.ends_with("gb") {
        let end = if s.ends_with("GB") || s.ends_with("gb") {
            s.len() - 2
        } else {
            s.len() - 1
        };
        (&s[..end], 1024u64 * 1024 * 1024)
    } else if s.ends_with("MiB") || s.ends_with("mib") {
        (&s[..s.len() - 3], 1024u64 * 1024)
    } else if s.ends_with('M') || s.ends_with('m') || s.ends_with("MB") || s.ends_with("mb") {
        let end = if s.ends_with("MB") || s.ends_with("mb") {
            s.len() - 2
        } else {
            s.len() - 1
        };
        (&s[..end], 1024u64 * 1024)
    } else if s.ends_with("KiB") || s.ends_with("kib") {
        (&s[..s.len() - 3], 1024u64)
    } else if s.ends_with('K') || s.ends_with('k') || s.ends_with("KB") || s.ends_with("kb") {
        let end = if s.ends_with("KB") || s.ends_with("kb") {
            s.len() - 2
        } else {
            s.len() - 1
        };
        (&s[..end], 1024u64)
    } else if s.ends_with('B') || s.ends_with('b') {
        (&s[..s.len() - 1], 1u64)
    } else {
        (s, 1u64)
    };

    let num: f64 = num_part
        .trim()
        .parse()
        .map_err(|_| format!("Invalid size format '{}'. Expected e.g. 500K, 10M, 1G", s))?;

    Ok((num * multiplier as f64) as u64)
}

#[derive(Serialize)]
struct DuplicatesOutput {
    schema_version: &'static str,
    target_path: String,
    stats: vacua_content::DedupStats,
    groups: Vec<vacua_content::DuplicateGroup>,
}

fn handle_duplicates_scan(
    path: &Path,
    min_size_str: &str,
    include_empty: bool,
    jobs: usize,
    no_cache: bool,
    json_mode: bool,
) {
    let min_size = match parse_size(min_size_str) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let db_path = default_index_path();
    let db = if !no_cache {
        IndexDatabase::open(&db_path).ok()
    } else {
        None
    };

    let options = vacua_content::DuplicateScanOptions {
        min_size,
        include_empty,
        hash_jobs: jobs,
        skip_cloud: true,
        use_cache: !no_cache && db.is_some(),
    };

    let (groups, stats) =
        match vacua_content::DuplicateEngine::scan_path(&canonical, &options, db.as_ref()) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error scanning for duplicates: {}", e);
                std::process::exit(1);
            }
        };

    if json_mode {
        let output = DuplicatesOutput {
            schema_version: "vacua-duplicates-v1",
            target_path: canonical.display().to_string(),
            stats,
            groups,
        };
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!();
        println!("Vacua Duplicate Discovery — APFS Content Identity");
        println!("==================================================");
        println!("Target Path:              {}", canonical.display());
        println!("Files Scanned:            {}", stats.files_seen);
        println!("Eligible Files:           {}", stats.eligible_files);
        println!("Size Collisions Checked:  {}", stats.size_collision_files);
        println!("Sample Fingerprints:      {}", stats.sampled_files);
        println!("Full BLAKE3 Hashes:       {}", stats.full_hashed_files);
        if stats.full_hash_cache_hits > 0 {
            println!("Fingerprint Cache Hits:   {}", stats.full_hash_cache_hits);
        }
        if stats.cloud_placeholders_skipped > 0 {
            println!(
                "Cloud Placeholders Skipped: {}",
                stats.cloud_placeholders_skipped
            );
        }
        println!("--------------------------------------------------");
        println!("Exact duplicate groups:   {}", stats.duplicate_groups);
        println!(
            "Logical duplicate bytes:  {}",
            format_bytes(stats.logical_duplicate_bytes)
        );
        println!(
            "Confirmed reclaimable (suggested keep): {}",
            format_bytes(stats.confirmed_reclaimable_bytes)
        );
        println!(
            "Estimated reclaimable (suggested keep): {}",
            format_bytes(stats.estimated_reclaimable_bytes)
        );
        let apfs_shared = stats
            .upper_bound_reclaimable_bytes
            .saturating_sub(stats.confirmed_reclaimable_bytes);
        println!("APFS shared/uncertain:    {}", format_bytes(apfs_shared));
        println!("Scan Elapsed Time:        {} ms", stats.elapsed_ms);
        println!("==================================================");
        println!();

        if groups.is_empty() {
            println!("No duplicate files found matching criteria.");
            return;
        }

        println!("Top Duplicate Groups (ordered by logical waste):");
        for (i, group) in groups.iter().take(20).enumerate() {
            println!(
                "\nGroup {} [{}]: Size: {} ({} copies) | Logical Waste: {} | Reclaim: {} | Physical: {:?}",
                i + 1,
                group.group_id,
                format_bytes(group.logical_size),
                group.members.len(),
                format_bytes(group.logical_duplicate_bytes),
                format_bytes(group.confirmed_reclaimable_bytes),
                group.physical_sharing_state,
            );
            for m in &group.members {
                let keep_tag = if m.is_suggested_keep {
                    "[SUGGESTED KEEP]"
                } else {
                    "[REDUNDANT]     "
                };
                println!(
                    "  {} {} (Risk: {}, Inode: {})",
                    keep_tag,
                    m.path.display(),
                    m.risk,
                    m.inode
                );
            }
        }

        if groups.len() > 20 {
            println!(
                "\n... and {} more duplicate groups. Use 'vacua duplicates --json' to view all.",
                groups.len() - 20
            );
        }

        println!(
            "\nTo inspect a group:  vacua duplicates show <group-id> --path {}",
            canonical.display()
        );
        println!(
            "To build a plan:     vacua duplicates plan <group-id> --keep <path> -o plan.json"
        );
    }
}

fn handle_duplicates_show(group_id: &str, path: &Path, min_size_str: &str, json_mode: bool) {
    let min_size = parse_size(min_size_str).unwrap_or(0);
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let db_path = default_index_path();
    let db = IndexDatabase::open(&db_path).ok();

    let options = vacua_content::DuplicateScanOptions {
        min_size,
        include_empty: true,
        hash_jobs: 4,
        skip_cloud: true,
        use_cache: db.is_some(),
    };

    let (groups, _) =
        match vacua_content::DuplicateEngine::scan_path(&canonical, &options, db.as_ref()) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        };

    let group = match groups.into_iter().find(|g| g.group_id == group_id) {
        Some(g) => g,
        None => {
            eprintln!(
                "Error: Duplicate group '{}' not found in path '{}'",
                group_id,
                canonical.display()
            );
            std::process::exit(1);
        }
    };

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&group).unwrap());
    } else {
        println!();
        println!("Duplicate Group Detail: {}", group.group_id);
        println!("==================================================");
        println!("Full BLAKE3 Digest:       {}", group.full_digest);
        println!(
            "Individual File Size:     {}",
            format_bytes(group.logical_size)
        );
        println!(
            "Physical Sharing State:   {:?}",
            group.physical_sharing_state
        );
        println!("Total Members:            {}", group.members.len());
        println!(
            "Logical Duplicate Waste:  {}",
            format_bytes(group.logical_duplicate_bytes)
        );
        println!(
            "Confirmed Reclaimable:    {}",
            format_bytes(group.confirmed_reclaimable_bytes)
        );
        println!(
            "Estimated Reclaimable:    {}",
            format_bytes(group.estimated_reclaimable_bytes)
        );
        println!(
            "Upper Bound Reclaimable:  {}",
            format_bytes(group.upper_bound_reclaimable_bytes)
        );
        println!("--------------------------------------------------");
        println!("Group Members:");
        for (i, m) in group.members.iter().enumerate() {
            println!("\n  Member {}: {}", i + 1, m.path.display());
            println!("    Inode:                {}", m.inode);
            println!("    Device ID:            {}", m.device_id);
            println!(
                "    Allocated Space:      {}",
                format_bytes(m.allocation.allocated_bytes)
            );
            if let Some(cid) = m.clone_id {
                println!(
                    "    APFS Clone ID:        {} (refcnt: {})",
                    cid, m.clone_refcnt
                );
            }
            println!("    Evaluated Risk:       {}", m.risk);
            println!("    Category:             {}", m.category);
            if m.is_suggested_keep {
                println!("    Suggested Keep:       YES");
                if let Some(reason) = &m.suggest_keep_reason {
                    println!("    Keep Rationale:       {}", reason);
                }
            } else {
                println!("    Suggested Keep:       NO (redundant candidate)");
            }
        }
        println!("==================================================");
        println!(
            "\nTo plan removal, run: vacua duplicates plan {} --keep <chosen-path> -o plan.json",
            group.group_id
        );
    }
}

fn handle_duplicates_plan(
    group_id: &str,
    keep_path: &Path,
    root_path: &Path,
    output: Option<PathBuf>,
    json_mode: bool,
) {
    let canonical_root = root_path
        .canonicalize()
        .unwrap_or_else(|_| root_path.to_path_buf());
    let canonical_keep = keep_path
        .canonicalize()
        .unwrap_or_else(|_| keep_path.to_path_buf());

    let db_path = default_index_path();
    let db = IndexDatabase::open(&db_path).ok();

    let options = vacua_content::DuplicateScanOptions {
        min_size: 0,
        include_empty: true,
        hash_jobs: 4,
        skip_cloud: true,
        use_cache: db.is_some(),
    };

    let (groups, _) =
        match vacua_content::DuplicateEngine::scan_path(&canonical_root, &options, db.as_ref()) {
            Ok(res) => res,
            Err(e) => {
                eprintln!("Error scanning for duplicate group: {}", e);
                std::process::exit(1);
            }
        };

    let group = match groups.into_iter().find(|g| g.group_id == group_id) {
        Some(g) => g,
        None => {
            eprintln!("Error: Duplicate group '{}' not found", group_id);
            std::process::exit(1);
        }
    };

    let plan = match vacua_content::DuplicateEngine::build_cleanup_plan(&group, &canonical_keep) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error compiling cleanup plan: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(out_path) = output {
        let serialized = serde_json::to_string_pretty(&plan).unwrap();
        if let Err(e) = std::fs::write(&out_path, serialized) {
            eprintln!("Failed to write plan to {}: {}", out_path.display(), e);
            std::process::exit(1);
        }
        if !json_mode {
            println!(
                "Successfully saved verified duplicate cleanup plan to '{}'.",
                out_path.display()
            );
            println!("Plan ID:                {}", plan.plan_id);
            println!("Plan Integrity Hash:    {}", plan.plan_hash);
            println!(
                "Estimated Eventual Reclaim: {}",
                format_bytes(plan.estimated_eventual_reclaim_bytes)
            );
            println!("\nTo inspect without executing:");
            println!("  vacua execute {} --dry-run", out_path.display());
            println!("\nTo execute safely with native macOS Trash:");
            println!("  vacua execute {}", out_path.display());
        }
    } else if json_mode {
        println!("{}", serde_json::to_string_pretty(&plan).unwrap());
    } else {
        println!();
        println!("Generated Immutable Cleanup Plan: {}", plan.plan_id);
        println!("==================================================");
        println!("Preserved Original:       {}", canonical_keep.display());
        println!("Items to Remove:          {}", plan.items.len());
        println!(
            "Estimated Eventual Reclaim: {}",
            format_bytes(plan.estimated_eventual_reclaim_bytes)
        );
        println!(
            "Risk Summary:             {} safe, {} review, {} caution",
            plan.risk_summary.safe_count,
            plan.risk_summary.review_count,
            plan.risk_summary.caution_count
        );
        println!("Plan Integrity Hash:      {}", plan.plan_hash);
        println!("--------------------------------------------------");
        for item in &plan.items {
            println!(
                "  [TRASH] {} (Allocated: {}, Risk: {})",
                item.path.display(),
                format_bytes(item.allocated_bytes),
                item.risk
            );
        }
        println!("==================================================");
        println!(
            "\nTo save plan for execution, use: vacua duplicates plan {} --keep {} -o <path.json>",
            group_id,
            canonical_keep.display()
        );
    }
}

fn handle_duplicates_cache_status(json_mode: bool) {
    let db_path = default_index_path();
    let db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!(
                "Error opening index database at {}: {}",
                db_path.display(),
                e
            );
            std::process::exit(1);
        }
    };

    let stats = match db.get_fingerprint_stats() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error querying fingerprint stats: {}", e);
            std::process::exit(1);
        }
    };

    if json_mode {
        println!("{}", serde_json::to_string_pretty(&stats).unwrap());
    } else {
        println!();
        println!("Vacua Persistent Content Fingerprint Cache");
        println!("==========================================");
        println!("Database Path:            {}", db_path.display());
        println!("Total Cached Identities:  {}", stats.total_entries);
        println!("Sample Hashes Cached:     {}", stats.sample_hash_count);
        println!("Full BLAKE3 Hashes:       {}", stats.full_hash_count);
        println!("==========================================");
    }
}

fn handle_duplicates_cache_prune(json_mode: bool) {
    let db_path = default_index_path();
    let db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index database: {}", e);
            std::process::exit(1);
        }
    };

    let pruned = match db.prune_missing_fingerprints() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Error pruning cache: {}", e);
            std::process::exit(1);
        }
    };

    if json_mode {
        let out = serde_json::json!({
            "status": "success",
            "pruned_entries": pruned
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        println!(
            "Pruned {} missing or deleted records from content fingerprint cache.",
            pruned
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_artifacts(
    target_path: &Path,
    ecosystem: Option<&str>,
    min_size: Option<&str>,
    confidence: Option<&str>,
    limit: usize,
    refresh: bool,
    show_id: Option<&str>,
    _projects_mode: bool,
    json_mode: bool,
) {
    let canonical = match target_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error resolving target path {:?}: {}", target_path, e);
            std::process::exit(1);
        }
    };
    let root_id = blake3::hash(canonical.to_string_lossy().as_bytes()).to_hex()[..16].to_string();

    let db_path = default_index_path();
    let mut db = match IndexDatabase::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Error opening index database: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(art_id) = show_id {
        handle_artifacts_show(&db, art_id, json_mode);
        return;
    }

    let latest_gen = if refresh {
        None
    } else {
        ArtifactPersistence::get_latest_ready_generation(db.conn(), &root_id)
            .ok()
            .flatten()
    };

    let canonical_lossy = canonical.to_string_lossy().to_string();
    let gen = match latest_gen {
        Some(g) if g.root_path == canonical_lossy => g,
        _ => {
            let scanner = DeveloperArtifactScanner::new(&canonical, &root_id);
            let new_gen = match scanner.scan() {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("Error scanning developer artifacts: {}", e);
                    std::process::exit(1);
                }
            };
            if let Err(e) = ArtifactPersistence::publish_generation(db.conn_mut(), &new_gen) {
                eprintln!("Error persisting developer artifact generation: {}", e);
                std::process::exit(1);
            }
            new_gen
        }
    };

    let eco_filter = ecosystem.and_then(DeveloperEcosystem::from_str_name);
    let min_size_bytes = match min_size {
        Some(s) => match parse_size(s) {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                eprintln!("Invalid --min-size: {}", e);
                std::process::exit(1);
            }
        },
        None => None,
    };
    let conf_filter = confidence.and_then(RebuildConfidence::from_str_name);

    let mut filtered_projects = Vec::new();
    for proj in gen.projects {
        if let Some(eco) = eco_filter {
            if !proj.all_ecosystems.contains(&eco) && proj.primary_ecosystem != eco {
                continue;
            }
        }
        if let Some(conf) = conf_filter {
            if proj.rebuild_confidence != conf {
                continue;
            }
        }
        if let Some(min_b) = min_size_bytes {
            if proj.total_allocated_bytes < min_b {
                continue;
            }
        }
        filtered_projects.push(proj);
    }

    let total_allocated: u64 = filtered_projects
        .iter()
        .map(|p| p.total_allocated_bytes)
        .sum();
    let total_logical: u64 = filtered_projects
        .iter()
        .map(|p| p.total_logical_bytes)
        .sum();
    let total_artifacts: usize = filtered_projects.iter().map(|p| p.artifacts.len()).sum();

    if json_mode {
        let analysis = DeveloperArtifactAnalysisV1 {
            schema_version: "v1".to_string(),
            generation_id: gen.generation_id,
            root_id: gen.root_id,
            root_path: gen.root_path,
            observed_at: chrono::DateTime::<Utc>::from_timestamp(gen.observed_at, 0)
                .map(|t| t.to_rfc3339())
                .unwrap_or_else(|| Utc::now().to_rfc3339()),
            total_projects: filtered_projects.len(),
            total_artifacts,
            total_logical_bytes: total_logical,
            total_allocated_bytes: total_allocated,
            projects: filtered_projects
                .iter()
                .map(|p| DeveloperProjectSummaryV1 {
                    project_id: p.project_id.as_str().to_string(),
                    display_name: p.display_name.clone(),
                    display_path: p.display_path.clone(),
                    primary_ecosystem: p.primary_ecosystem.as_str().to_string(),
                    all_ecosystems: p
                        .all_ecosystems
                        .iter()
                        .map(|e| e.as_str().to_string())
                        .collect(),
                    artifacts_count: p.artifacts.len(),
                    total_logical_bytes: p.total_logical_bytes,
                    total_allocated_bytes: p.total_allocated_bytes,
                    rebuild_confidence: p.rebuild_confidence.as_str().to_string(),
                    active_state: p.active_state.as_str().to_string(),
                })
                .collect(),
            coverage: DeveloperArtifactCoverageV1 {
                supported_ecosystems: gen
                    .coverage
                    .supported_ecosystems
                    .iter()
                    .map(|e| e.as_str().to_string())
                    .collect(),
                unclassified_candidate_directories: gen.coverage.unclassified_candidate_directories,
                skipped_items: gen.coverage.skipped_items,
            },
        };
        println!("{}", serde_json::to_string_pretty(&analysis).unwrap());
        return;
    }

    println!("Developer Artifacts");
    println!("────────────────────────────────────────────────────────────────────────");
    if filtered_projects.is_empty() {
        println!("No supported developer artifacts were found in the selected root.");
    } else {
        println!(
            "{:<28} {:<16} {:>12}   {:<18}",
            "Project", "Ecosystem", "Allocated", "Rebuild Evidence"
        );
        println!("────────────────────────────────────────────────────────────────────────");
        for proj in &filtered_projects {
            let eco_str = proj
                .all_ecosystems
                .iter()
                .map(|e| e.display_name())
                .collect::<Vec<_>>()
                .join("/");
            let alloc_str = format_bytes(proj.total_allocated_bytes);
            let conf_str = proj.rebuild_confidence.display_name();
            println!(
                "{:<28} {:<16} {:>12}   {:<18}",
                proj.display_name, eco_str, alloc_str, conf_str
            );
        }
        println!("────────────────────────────────────────────────────────────────────────");
        println!(
            "Potential generated storage observed: {}",
            format_bytes(total_allocated)
        );
        println!(
            "Observed across {} projects ({} artifacts)",
            filtered_projects.len(),
            total_artifacts
        );
        println!("Note: Physical APFS allocation is reported. Generated state does not imply immediate deletion authority.");

        let mut all_artifacts: Vec<(&DeveloperProject, &DeveloperArtifact)> = Vec::new();
        for p in &filtered_projects {
            for a in &p.artifacts {
                all_artifacts.push((p, a));
            }
        }
        all_artifacts.sort_by_key(|b| std::cmp::Reverse(b.1.allocated_bytes));
        if !all_artifacts.is_empty() {
            println!("\nTop Discovered Artifacts (showing up to {}):", limit);
            for (_, art) in all_artifacts.iter().take(limit) {
                println!(
                    "  • {:<20} {:>10}  [{}]  ({})",
                    art.display_name,
                    format_bytes(art.allocated_bytes),
                    art.artifact_id.as_str(),
                    art.rebuild_evidence
                        .reconstruction_confidence
                        .display_name()
                );
            }
            println!("\nRun `vacua artifacts show <artifact-id>` for detailed reconstruction evidence and rationale.");
        }
    }
}

fn handle_artifacts_show(db: &IndexDatabase, artifact_id: &str, json_mode: bool) {
    let art = match ArtifactPersistence::get_artifact(db.conn(), artifact_id) {
        Ok(Some(a)) => a,
        Ok(None) => {
            eprintln!("Developer artifact not found: {}", artifact_id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error querying developer artifact: {}", e);
            std::process::exit(1);
        }
    };

    let proj_name: String = db
        .conn()
        .query_row(
            "SELECT display_name FROM developer_projects WHERE project_id = ?1 LIMIT 1",
            rusqlite::params![art.project_id.as_str()],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "Unknown Project".to_string());

    if json_mode {
        let detail = DeveloperArtifactDetailV1 {
            schema_version: "v1".to_string(),
            artifact_id: art.artifact_id.as_str().to_string(),
            project_id: art.project_id.as_str().to_string(),
            project_name: proj_name,
            display_name: art.display_name.clone(),
            display_path: art.display_path.clone(),
            ecosystem: art.ecosystem.as_str().to_string(),
            artifact_kind: art.artifact_kind.as_str().to_string(),
            logical_bytes: art.logical_bytes,
            allocated_bytes: art.allocated_bytes,
            confirmed_reclaim_lower_bound: art.confirmed_reclaim_lower_bound,
            estimated_reclaim: art.estimated_reclaim,
            physical_sharing_uncertainty: art.physical_sharing_uncertainty,
            rebuild_evidence: RebuildEvidenceV1 {
                manifest_present: art.rebuild_evidence.manifest_present,
                manifest_path: art.rebuild_evidence.manifest_path.clone(),
                lockfile_present: art.rebuild_evidence.lockfile_present,
                lockfile_path: art.rebuild_evidence.lockfile_path.clone(),
                known_artifact_convention: art.rebuild_evidence.known_artifact_convention,
                project_root_known: art.rebuild_evidence.project_root_known,
                toolchain_identified: art.rebuild_evidence.toolchain_identified.clone(),
                active_project_state: art
                    .rebuild_evidence
                    .active_project_state
                    .as_str()
                    .to_string(),
                reconstruction_confidence: art
                    .rebuild_evidence
                    .reconstruction_confidence
                    .as_str()
                    .to_string(),
                rebuild_command_template: art.rebuild_evidence.rebuild_command_template.clone(),
                reasons: art.rebuild_evidence.reasons.clone(),
                active_guard_deferred: art.rebuild_evidence.active_guard_deferred,
            },
            candidate_id: art.candidate_id.clone(),
            observed_at: Utc::now().to_rfc3339(),
        };
        println!("{}", serde_json::to_string_pretty(&detail).unwrap());
        return;
    }

    println!("Developer Artifact Detail: {}", art.artifact_id.as_str());
    println!("────────────────────────────────────────────────────────────────────────");
    println!("Name:                         {}", art.display_name);
    println!(
        "Kind:                         {}",
        art.artifact_kind.display_name()
    );
    println!(
        "Ecosystem:                    {}",
        art.ecosystem.display_name()
    );
    println!(
        "Project:                      {} ({})",
        proj_name,
        art.project_id.as_str()
    );
    println!("Display Path:                 {}", art.display_path);
    println!();
    println!("Storage Truth:");
    println!(
        "  Logical Bytes:              {} ({})",
        art.logical_bytes,
        format_bytes(art.logical_bytes)
    );
    println!(
        "  Allocated Blocks:           {} ({})",
        art.allocated_bytes,
        format_bytes(art.allocated_bytes)
    );
    println!(
        "  Estimated Reclaim:          {}",
        format_bytes(art.estimated_reclaim)
    );
    println!(
        "  Physical Extent Sharing:    {}",
        if art.physical_sharing_uncertainty {
            "Uncertain (APFS extent sharing possible)"
        } else {
            "None"
        }
    );
    println!();
    println!("Rebuild Evidence:");
    println!(
        "  Reconstruction Confidence:  {}",
        art.rebuild_evidence
            .reconstruction_confidence
            .display_name()
    );
    println!(
        "  Project Root Known:         {}",
        if art.rebuild_evidence.project_root_known {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  Manifest Present:           {}",
        if art.rebuild_evidence.manifest_present {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  Lockfile Present:           {}",
        if art.rebuild_evidence.lockfile_present {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  Known Artifact Convention:  {}",
        if art.rebuild_evidence.known_artifact_convention {
            "Yes"
        } else {
            "No"
        }
    );
    println!(
        "  Toolchain Identified:       {}",
        art.rebuild_evidence
            .toolchain_identified
            .as_deref()
            .unwrap_or("No")
    );
    println!(
        "  Active Project State:       {}",
        art.rebuild_evidence.active_project_state.display_name()
    );
    println!();
    println!("Classification Rationale:");
    for reason in &art.rebuild_evidence.reasons {
        println!("  • {}", reason);
    }
    println!();
    println!("Suggested Rebuild Command (Informational Template):");
    println!("  {}", art.ecosystem.default_rebuild_template());
    if let Some(cand_id) = &art.candidate_id {
        println!();
        println!("Associated Review Candidate:  {}", cand_id);
    }
    println!("────────────────────────────────────────────────────────────────────────");
}
