use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vacua_api::*;
use vacua_content::DuplicateScanOptions;
use vacua_core::candidate::Candidate;
use vacua_core::evidence_graph::{ApplicationEvidenceGraph, NodeKind, OrphanConfidence};
use vacua_core::pressure::query_volume_status;
use vacua_core::risk::RiskLevel;
use vacua_index::{ExecutionJournal, IndexDatabase};
use vacua_plan::CleanupPlan;
use vacua_risk::CandidateEvaluator;
use vacua_rules::engine::RulesEngine;
use vacua_scan::{FilesystemScanner, ScanOptions};
use vacua_tree::{
    StorageNodeId, StorageTreeBuilder, StorageTreeCoverage, StorageTreeEngine,
    StorageTreeGeneration, StorageTreeMetric, StorageTreeSource, StorageTreeStatus, TreeError,
};

use crate::policy::{
    AllowedRoot, McpPolicy, PathDisclosureMode, MAX_PROPOSAL_CANDIDATES, MAX_SIMULATION_CANDIDATES,
    MAX_STRING_PARAM_LEN,
};

type CachedCandidates = Arc<Mutex<HashMap<String, (Vec<Candidate>, i64)>>>;

/// Domain service coordinating storage intelligence queries.
pub struct VacuaDomainService {
    policy: McpPolicy,
    index_path: Option<PathBuf>,
    journal_path: Option<PathBuf>,
    cached_candidates: CachedCandidates,
    member_id_map: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl VacuaDomainService {
    pub fn new(
        policy: McpPolicy,
        index_path: Option<PathBuf>,
        journal_path: Option<PathBuf>,
    ) -> Self {
        Self {
            policy,
            index_path,
            journal_path,
            cached_candidates: Arc::new(Mutex::new(HashMap::new())),
            member_id_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn policy(&self) -> &McpPolicy {
        &self.policy
    }

    fn open_index(&self) -> Option<IndexDatabase> {
        let path = self.index_path.clone().unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".vacua").join("index.db")
        });

        if path.exists() {
            IndexDatabase::open_read_only(&path).ok()
        } else {
            None
        }
    }

    fn open_index_writable(&self) -> Option<IndexDatabase> {
        let path = self.index_path.clone().unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".vacua").join("index.db")
        });

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        IndexDatabase::open(&path).ok()
    }

    fn open_journal(&self) -> Option<ExecutionJournal> {
        let path = self.journal_path.clone().unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".vacua").join("journal.db")
        });

        if path.exists() {
            ExecutionJournal::open_read_only(&path).ok()
        } else {
            None
        }
    }

    /// Retrieve or evaluate candidates for an authoritative root.
    pub fn get_or_evaluate_candidates_for_root(
        &self,
        root: &AllowedRoot,
    ) -> Result<Vec<Candidate>, VacuaErrorResponse> {
        let now = Utc::now().timestamp();
        {
            let lock = self.cached_candidates.lock().unwrap();
            if let Some((ref cands, ts)) = lock.get(&root.root_id) {
                if now - ts < 15 {
                    return Ok(cands.clone());
                }
            }
        }

        let scanner = FilesystemScanner::new(ScanOptions {
            cross_mounts: false,
            max_depth: Some(5),
            ..Default::default()
        });

        let mut candidates = Vec::new();
        if let Ok(report) = scanner.scan(&root.canonical_path) {
            let mut engine = RulesEngine::new();
            let mut evaluator = CandidateEvaluator::new(&mut engine);

            for entry in report.entries {
                let alloc = entry.to_allocation();
                let mut cand = evaluator.evaluate(
                    &entry.path,
                    alloc,
                    entry.inode,
                    entry.device_id,
                    entry.mtime_sec,
                    entry.is_dir,
                );

                // Assign opaque deterministic candidate ID (no absolute paths leaked)
                let rel = entry
                    .path
                    .strip_prefix(&root.canonical_path)
                    .unwrap_or(&entry.path);
                cand.id = self.policy.generate_candidate_id(&root.root_id, rel);

                candidates.push(cand);
            }
        }

        // Sort deterministically: allocated desc, id asc
        candidates.sort_by(|a, b| {
            b.allocation
                .allocated_bytes
                .cmp(&a.allocation.allocated_bytes)
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut lock = self.cached_candidates.lock().unwrap();
        lock.insert(root.root_id.clone(), (candidates.clone(), now));
        Ok(candidates)
    }

    /// Storage summary tool implementation.
    pub fn storage_summary(
        &self,
        root_id: Option<&str>,
        deprecated_path: Option<&str>,
    ) -> Result<StorageSummaryV1, VacuaErrorResponse> {
        let root = if let Some(p) = deprecated_path {
            let target_path = PathBuf::from(p);
            if !self.policy.is_path_allowed(&target_path) {
                return Err(VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaPolicyDenied,
                    format!("Path '{}' is outside configured allowed roots", p),
                ));
            }
            // Find which root contains this target path
            let canonical = target_path.canonicalize().map_err(|e| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Cannot canonicalize path '{}': {}", p, e),
                )
            })?;
            self.policy
                .allowed_roots
                .iter()
                .find(|r| canonical.starts_with(&r.canonical_path))
                .ok_or_else(|| {
                    VacuaErrorResponse::new(
                        VacuaErrorCode::VacuaPolicyDenied,
                        "Path does not match any configured allowed root",
                    )
                })?
        } else {
            self.policy.get_root(root_id)?
        };

        let status = query_volume_status(&root.canonical_path).map_err(|e| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInternal,
                format!(
                    "Failed to query volume storage status for root '{}': {}",
                    root.root_id, e
                ),
            )
        })?;

        let candidates = self.get_or_evaluate_candidates_for_root(root)?;
        let non_protected: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| !c.risk.is_protected_or_unknown())
            .collect();

        let mut confirmed_reclaim = 0u64;
        let mut estimated_reclaim = 0u64;
        let mut upper_reclaim = 0u64;
        let mut reviewable_reclaim = 0u64;

        for c in &non_protected {
            let (lower, est, upper) = Self::compute_reclaim_bounds(c);
            confirmed_reclaim = confirmed_reclaim.saturating_add(lower);
            estimated_reclaim = estimated_reclaim.saturating_add(est);
            upper_reclaim = upper_reclaim.saturating_add(upper);

            if c.risk == RiskLevel::Review || c.risk == RiskLevel::Caution {
                reviewable_reclaim = reviewable_reclaim.saturating_add(est);
            }
        }

        let (index_freshness, is_stale) = if let Some(index) = self.open_index() {
            if let Ok(stats) = index.get_index_stats() {
                if let Some(ts) = stats.last_scan_timestamp {
                    let diff = Utc::now().timestamp() - ts;
                    (
                        Some(format!("{} seconds ago", diff.max(0))),
                        diff > 86400 * 7,
                    )
                } else {
                    (None, false)
                }
            } else {
                (None, false)
            }
        } else {
            (None, false)
        };

        Ok(StorageSummaryV1 {
            schema_version: SCHEMA_STORAGE_SUMMARY_V1.to_string(),
            observed_at: Utc::now().to_rfc3339(),
            target_path: self.policy.format_path(&root.canonical_path),
            mount_point: self.policy.format_path(Path::new(&status.mount_point)),
            filesystem_type: status.filesystem_type,
            total_space_bytes: status.total_bytes,
            free_space_bytes: status.free_bytes,
            available_space_bytes: status.available_bytes,
            purgeable_space_bytes: None,
            pressure_level: format!("{:?}", status.pressure),
            candidate_count: non_protected.len(),
            candidate_reclaim_bytes: estimated_reclaim,
            candidate_reviewable_bytes: reviewable_reclaim,
            candidate_confirmed_reclaim_bytes: confirmed_reclaim,
            candidate_estimated_reclaim_bytes: estimated_reclaim,
            candidate_reclaim_upper_bound: upper_reclaim,
            index_freshness,
            is_stale,
        })
    }

    /// List snapshots tool implementation obeying root policy.
    pub fn list_snapshots(
        &self,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<SnapshotListResponseV1, VacuaErrorResponse> {
        let index = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, "Index database not found")
        })?;

        let all_summaries = index
            .list_snapshots()
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        // STRICT ROOT BOUNDARY: Only return snapshots whose root_path falls within an AllowedRoot
        let summaries: Vec<_> = all_summaries
            .into_iter()
            .filter(|s| self.policy.is_path_allowed(&s.root_path))
            .collect();

        let total_count = summaries.len();
        let limit = self.policy.clamp_limit(limit);

        let query_fp = "snapshots:all";
        let offset = if let Some(c) = cursor {
            McpPolicy::decode_cursor_v2(c, "snapshot", "all", query_fp)?
        } else {
            0
        }
        .min(total_count);

        let end = (offset + limit).min(total_count);
        let paged_items = summaries[offset..end]
            .iter()
            .map(|s| SnapshotSummaryV1 {
                snapshot_id: s.snapshot_id.clone(),
                name: self.policy.sanitize_string(&s.name),
                root_path: self.policy.format_path(&s.root_path),
                created_at: chrono::DateTime::from_timestamp(s.timestamp, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default(),
                total_files: s.total_files,
                allocated_bytes: s.allocated_bytes,
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor_v2(
                "snapshot", "all", query_fp, end, 1,
            ))
        } else {
            None
        };

        Ok(SnapshotListResponseV1 {
            schema_version: SCHEMA_SNAPSHOT_LIST_V1.to_string(),
            items: paged_items,
            total_count,
            limit,
            next_cursor,
        })
    }

    /// Get snapshot details for vacua://snapshot/{snapshot_id}.
    pub fn get_snapshot(&self, snapshot_id: &str) -> Result<SnapshotDetailV1, VacuaErrorResponse> {
        let index = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, "Index database not found")
        })?;

        let snapshot = index
            .get_snapshot(snapshot_id)
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
            .ok_or_else(|| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Snapshot '{}' not found", snapshot_id),
                )
            })?;

        if !self.policy.is_path_allowed(&snapshot.root_path) {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaPolicyDenied,
                format!(
                    "Snapshot '{}' root is outside configured allowed roots",
                    snapshot_id
                ),
            ));
        }

        Ok(SnapshotDetailV1 {
            schema_version: SCHEMA_SNAPSHOT_DETAIL_V1.to_string(),
            snapshot_id: snapshot.snapshot_id,
            name: self.policy.sanitize_string(&snapshot.name),
            root_path: self.policy.format_path(&snapshot.root_path),
            created_at: chrono::DateTime::from_timestamp(snapshot.timestamp, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            total_files: snapshot.total_files,
            total_dirs: snapshot.total_dirs,
            logical_bytes: snapshot.logical_bytes,
            allocated_bytes: snapshot.allocated_bytes,
            subtree_count: snapshot.subtrees.len(),
        })
    }

    /// Diff snapshots tool implementation with strict root authorization.
    pub fn diff_snapshots(
        &self,
        base: &str,
        target: Option<&str>,
    ) -> Result<SnapshotDiffV1, VacuaErrorResponse> {
        let index = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, "Index database not found")
        })?;

        let base_snap = index
            .get_snapshot(base)
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
            .ok_or_else(|| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Base snapshot '{}' not found", base),
                )
            })?;

        if !self.policy.is_path_allowed(&base_snap.root_path) {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaPolicyDenied,
                "Base snapshot root path is outside configured allowed roots",
            ));
        }

        let target_name = target.unwrap_or("current");
        if target_name != "current" {
            let target_snap = index
                .get_snapshot(target_name)
                .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
                .ok_or_else(|| {
                    VacuaErrorResponse::new(
                        VacuaErrorCode::VacuaNotFound,
                        format!("Target snapshot '{}' not found", target_name),
                    )
                })?;

            if !self.policy.is_path_allowed(&target_snap.root_path) {
                return Err(VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaPolicyDenied,
                    "Target snapshot root path is outside configured allowed roots",
                ));
            }
        }

        let diff = index
            .diff_snapshots(base, target_name)
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, e.to_string()))?;

        let sorted_deltas = diff.subtree_deltas;

        let mut growing: Vec<_> = sorted_deltas
            .iter()
            .filter(|d| d.allocated_delta_bytes > 0)
            .cloned()
            .collect();
        growing.sort_by_key(|b| std::cmp::Reverse(b.allocated_delta_bytes));

        let mut shrinking: Vec<_> = sorted_deltas
            .iter()
            .filter(|d| d.allocated_delta_bytes < 0)
            .cloned()
            .collect();
        shrinking.sort_by_key(|a| a.allocated_delta_bytes);

        let top_growing = growing
            .into_iter()
            .take(10)
            .map(|d| SubtreeDeltaV1 {
                display_path: self.policy.format_path(Path::new(&d.path)),
                delta_bytes: d.allocated_delta_bytes,
                files_delta: d.file_count_delta,
            })
            .collect();

        let top_shrinking = shrinking
            .into_iter()
            .take(10)
            .map(|d| SubtreeDeltaV1 {
                display_path: self.policy.format_path(Path::new(&d.path)),
                delta_bytes: d.allocated_delta_bytes,
                files_delta: d.file_count_delta,
            })
            .collect();

        Ok(SnapshotDiffV1 {
            schema_version: SCHEMA_SNAPSHOT_DIFF_V1.to_string(),
            base_snapshot: self.policy.sanitize_string(base),
            target_snapshot: self.policy.sanitize_string(target_name),
            allocated_delta_bytes: diff.allocated_delta_bytes,
            logical_delta_bytes: diff.logical_delta_bytes,
            files_delta: diff.files_delta,
            top_growing,
            top_shrinking,
        })
    }

    /// List candidates tool implementation scoped to root.
    pub fn list_candidates(
        &self,
        root_id: Option<&str>,
        max_risk: Option<McpRiskFilter>,
        category: Option<&str>,
        min_reclaim: Option<u64>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<CandidateListResponseV1, VacuaErrorResponse> {
        let root = self.policy.get_root(root_id)?;
        let all_cands = self.get_or_evaluate_candidates_for_root(root)?;

        let target_risk = match max_risk.unwrap_or(McpRiskFilter::Safe) {
            McpRiskFilter::Safe => RiskLevel::Safe,
            McpRiskFilter::Review => RiskLevel::Review,
            McpRiskFilter::Caution => RiskLevel::Caution,
            McpRiskFilter::Protected => RiskLevel::Protected,
            McpRiskFilter::Unknown => RiskLevel::Unknown,
        };

        if let Some(cat) = category {
            if cat.len() > MAX_STRING_PARAM_LEN {
                return Err(VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaLimitExceeded,
                    "Category string exceeds maximum length limit",
                ));
            }
        }

        let filtered: Vec<&Candidate> = all_cands
            .iter()
            .filter(|c| c.risk <= target_risk)
            .filter(|c| !c.risk.is_protected_or_unknown())
            .filter(|c| {
                if let Some(cat) = category {
                    c.category.to_string().eq_ignore_ascii_case(cat)
                } else {
                    true
                }
            })
            .filter(|c| {
                if let Some(min) = min_reclaim {
                    c.allocation.allocated_bytes >= min
                } else {
                    true
                }
            })
            .collect();

        let total_count = filtered.len();
        let limit = self.policy.clamp_limit(limit);

        let query_fp = format!(
            "risk={:?}:cat={:?}:min={:?}",
            target_risk, category, min_reclaim
        );
        let offset = if let Some(c) = cursor {
            McpPolicy::decode_cursor_v2(c, "candidate", &root.root_id, &query_fp)?
        } else {
            0
        }
        .min(total_count);

        let end = (offset + limit).min(total_count);
        let items = filtered[offset..end]
            .iter()
            .map(|c| {
                let (lower, est, upper) = Self::compute_reclaim_bounds(c);
                CandidateSummaryV1 {
                    candidate_id: c.id.clone(),
                    display_path: self.policy.format_path(&c.path),
                    category: c.category.to_string(),
                    risk: c.risk.to_string(),
                    allocated_bytes: c.allocation.allocated_bytes,
                    reclaim_estimate_bytes: est,
                    reconstructable: c.reconstructable,
                    confirmed_reclaim_lower_bound: lower,
                    reclaim_upper_bound: upper,
                }
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor_v2(
                "candidate",
                &root.root_id,
                &query_fp,
                end,
                1,
            ))
        } else {
            None
        };

        Ok(CandidateListResponseV1 {
            schema_version: SCHEMA_CANDIDATE_LIST_V1.to_string(),
            items,
            total_count,
            limit,
            next_cursor,
        })
    }

    /// Compute conservative lower bound, estimate, and upper bound for reclaim.
    fn compute_reclaim_bounds(cand: &Candidate) -> (u64, u64, u64) {
        let alloc = cand.allocation.allocated_bytes;
        if cand.allocation.is_clone
            || cand.allocation.extent_uncertainty
            || cand.allocation.clone_refcnt > 1
        {
            if let Some(priv_bytes) = cand.allocation.kernel_private_bytes {
                (priv_bytes, priv_bytes, alloc)
            } else {
                // Unknown private bytes or uncertain clone: lower bound 0, upper bound alloc
                (0, cand.allocation.exclusive_bytes.min(alloc), alloc)
            }
        } else {
            let exclusive = cand.allocation.exclusive_bytes;
            (
                exclusive,
                cand.allocation.potentially_reclaimable_bytes.min(alloc),
                alloc,
            )
        }
    }

    /// Explain candidate tool implementation.
    pub fn explain_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<CandidateDetailV1, VacuaErrorResponse> {
        if candidate_id.len() > MAX_STRING_PARAM_LEN {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                "Candidate ID exceeds maximum length limit",
            ));
        }

        // Search in all configured allowed roots
        let mut matched = None;
        for root in &self.policy.allowed_roots {
            let cands = self.get_or_evaluate_candidates_for_root(root)?;
            if let Some(found) = cands.into_iter().find(|c| c.id == candidate_id) {
                matched = Some(found);
                break;
            }
        }

        let cand = matched.ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaNotFound,
                format!("Candidate with ID '{}' not found", candidate_id),
            )
        })?;

        let signals: Vec<String> = cand
            .evidence
            .iter()
            .map(|e| {
                format!(
                    "{:?}: {} ({:.2})",
                    e.source,
                    self.policy.sanitize_string(&e.signal),
                    e.weight
                )
            })
            .collect();

        let (lower, est, upper) = Self::compute_reclaim_bounds(&cand);

        Ok(CandidateDetailV1 {
            schema_version: SCHEMA_CANDIDATE_DETAIL_V1.to_string(),
            candidate_id: cand.id.clone(),
            display_path: self.policy.format_path(&cand.path),
            category: cand.category.to_string(),
            risk: cand.risk.to_string(),
            allocated_bytes: cand.allocation.allocated_bytes,
            logical_bytes: cand.allocation.logical_bytes,
            confirmed_reclaim_lower_bound: lower,
            estimated_reclaim_bytes: est,
            reclaim_upper_bound: upper,
            reconstructable: cand.reconstructable,
            rebuild_consequence: cand
                .rebuild_consequence
                .as_ref()
                .map(|s| self.policy.sanitize_string(s)),
            cost_model_status: "EXPERIMENTAL".to_string(),
            evidence_count: cand.evidence.len(),
            evidence_signals: signals,
            observed_mtime: Some(cand.mtime_sec),
        })
    }

    /// List applications tool implementation with explicit system metadata scope check.
    pub fn list_applications(
        &self,
        filter: Option<ApplicationFilter>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<ApplicationListResponseV1, VacuaErrorResponse> {
        let graph = ApplicationEvidenceGraph::build_from_system();
        let mut apps = Vec::new();

        for node in graph.nodes.values() {
            if node.kind == NodeKind::ApplicationBundle {
                let name = node.metadata.get("name").cloned().unwrap_or_default();
                let bundle_id = node.metadata.get("bundle_id").cloned().unwrap_or_default();
                let path_opt = node.path.as_ref();

                // If system app metadata is NOT allowed, strictly filter to paths inside allowed roots
                if !self.policy.allow_system_app_metadata {
                    if let Some(p) = path_opt {
                        if !self.policy.is_path_allowed(p) {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                let path_str = path_opt.map(|p| self.policy.format_path(p));
                apps.push((name, bundle_id, path_str));
            }
        }

        apps.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let active_filter = filter.unwrap_or_default();
        let filtered: Vec<(String, String, Option<String>)> = apps
            .into_iter()
            .filter(|(_name, bundle_id, _path)| match active_filter {
                ApplicationFilter::OrphansOnly => {
                    let eval = graph.evaluate_orphan(bundle_id);
                    eval.orphan_confidence > OrphanConfidence::NotOrphan
                }
                ApplicationFilter::InstalledOnly => {
                    let eval = graph.evaluate_orphan(bundle_id);
                    eval.bundle_installed
                }
                ApplicationFilter::All => true,
            })
            .collect();

        let total_count = filtered.len();
        let limit = self.policy.clamp_limit(limit);

        let query_fp = format!("app_filter={:?}", active_filter);
        let offset = if let Some(c) = cursor {
            McpPolicy::decode_cursor_v2(c, "application", "system", &query_fp)?
        } else {
            0
        }
        .min(total_count);

        let end = (offset + limit).min(total_count);
        let items = filtered[offset..end]
            .iter()
            .map(|(name, bid, path)| {
                let eval = graph.evaluate_orphan(bid);
                ApplicationSummaryV1 {
                    application_id: bid.clone(),
                    app_name: self.policy.sanitize_string(name),
                    display_path: path.clone(),
                    installed: eval.bundle_installed,
                    orphan_confidence: format!("{:?}", eval.orphan_confidence),
                }
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor_v2(
                "application",
                "system",
                &query_fp,
                end,
                1,
            ))
        } else {
            None
        };

        Ok(ApplicationListResponseV1 {
            schema_version: SCHEMA_APPLICATION_LIST_V1.to_string(),
            items,
            total_count,
            limit,
            next_cursor,
        })
    }

    /// Get application detail tool implementation.
    pub fn get_application(
        &self,
        application_id: &str,
    ) -> Result<ApplicationDetailV1, VacuaErrorResponse> {
        if application_id.len() > MAX_STRING_PARAM_LEN {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                "Application ID exceeds maximum length limit",
            ));
        }

        let graph = ApplicationEvidenceGraph::build_from_system();
        let eval = graph.evaluate_orphan(application_id);

        if !eval.bundle_installed && eval.artifact_paths.is_empty() {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaNotFound,
                format!(
                    "Application '{}' not found in system evidence",
                    application_id
                ),
            ));
        }

        // Scope filter: if system app metadata is disallowed, only include artifacts within allowed roots
        let filtered_paths: Vec<&PathBuf> = eval
            .artifact_paths
            .iter()
            .filter(|p| {
                if self.policy.allow_system_app_metadata {
                    true
                } else {
                    self.policy.is_path_allowed(p)
                }
            })
            .collect();

        let artifacts: Vec<ApplicationArtifactV1> = filtered_paths
            .into_iter()
            .map(|p| {
                // TRUE ALLOCATION: st_blocks * 512, NOT metadata.len()
                let alloc_bytes = {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        std::fs::symlink_metadata(p)
                            .map(|m| m.blocks() * 512)
                            .unwrap_or(0)
                    }
                    #[cfg(not(unix))]
                    {
                        std::fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0)
                    }
                };

                let artifact_id = self.policy.generate_artifact_id(p);
                ApplicationArtifactV1 {
                    artifact_id,
                    display_path: self.policy.format_path(p),
                    kind: "FilesystemArtifact".to_string(),
                    allocated_bytes: alloc_bytes,
                    risk: eval.risk_level.to_string(),
                }
            })
            .collect();

        Ok(ApplicationDetailV1 {
            schema_version: SCHEMA_APPLICATION_DETAIL_V1.to_string(),
            application_id: eval.bundle_id.clone(),
            app_name: self.policy.sanitize_string(&eval.app_name),
            display_path: eval
                .bundle_path
                .as_ref()
                .map(|p| self.policy.format_path(p)),
            installed: eval.bundle_installed,
            orphan_confidence: format!("{:?}", eval.orphan_confidence),
            artifact_count: artifacts.len(),
            estimated_reclaim_bytes: eval.associated_artifacts_bytes,
            artifacts,
        })
    }

    /// List duplicates tool implementation scoped to root.
    pub async fn list_duplicates(
        &self,
        root_id: Option<&str>,
        min_size_bytes: Option<u64>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<DuplicateListResponseV1, VacuaErrorResponse> {
        let _permit = self.policy.acquire_expensive_permit().await?;
        let root = self.policy.get_root(root_id)?.clone();

        let db = self.open_index();
        let opts = DuplicateScanOptions {
            min_size: min_size_bytes.unwrap_or(1024 * 1024),
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: db.is_some(),
        };

        let canonical_root = root.canonical_path.clone();
        let (mut groups, _stats) = tokio::time::timeout(
            self.policy.timeout,
            tokio::task::spawn_blocking(move || {
                vacua_content::DuplicateEngine::scan_path(&canonical_root, &opts, db.as_ref())
            }),
        )
        .await
        .map_err(|_| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaBusy, "Duplicate scan timed out")
        })?
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        groups.sort_by(|a, b| {
            b.confirmed_reclaimable_bytes
                .cmp(&a.confirmed_reclaimable_bytes)
                .then_with(|| a.group_id.cmp(&b.group_id))
        });

        let total_count = groups.len();
        let limit = self.policy.clamp_limit(limit);

        let query_fp = format!("dup_min={:?}", min_size_bytes);
        let offset = if let Some(c) = cursor {
            McpPolicy::decode_cursor_v2(c, "duplicate", &root.root_id, &query_fp)?
        } else {
            0
        }
        .min(total_count);

        let end = (offset + limit).min(total_count);
        let items = groups[offset..end]
            .iter()
            .map(|g| {
                let known_members = g
                    .members
                    .iter()
                    .filter(|m| m.allocation.kernel_private_bytes.is_some())
                    .count();
                let unknown_members = g.members.len() - known_members;

                DuplicateGroupSummaryV1 {
                    group_id: self
                        .policy
                        .generate_duplicate_group_id(&root.root_id, &g.group_id),
                    member_count: g.members.len(),
                    file_size: g.logical_size,
                    logical_duplicate_bytes: g.logical_duplicate_bytes,
                    kernel_private_bytes: g
                        .members
                        .iter()
                        .map(|m| m.allocation.kernel_private_bytes.unwrap_or(0))
                        .sum(),
                    confirmed_reclaim_lower_bound: g.confirmed_reclaimable_bytes,
                    estimated_reclaim: g.estimated_reclaimable_bytes,
                    content_identity_verified: true,
                    algorithm: "BLAKE3".to_string(),
                    kernel_private_bytes_known_members: known_members,
                    kernel_private_bytes_unknown_members: unknown_members,
                }
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor_v2(
                "duplicate",
                &root.root_id,
                &query_fp,
                end,
                1,
            ))
        } else {
            None
        };

        Ok(DuplicateListResponseV1 {
            schema_version: SCHEMA_DUPLICATE_LIST_V1.to_string(),
            items,
            total_count,
            limit,
            next_cursor,
        })
    }

    /// Get duplicate group tool implementation.
    pub async fn get_duplicate_group(
        &self,
        group_id: &str,
    ) -> Result<DuplicateGroupDetailV1, VacuaErrorResponse> {
        if group_id.len() > MAX_STRING_PARAM_LEN {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                "Group ID exceeds maximum length limit",
            ));
        }

        let _permit = self.policy.acquire_expensive_permit().await?;

        // Scan across allowed roots
        let db = self.open_index();
        let opts = DuplicateScanOptions {
            min_size: 1,
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: db.is_some(),
        };

        for root in &self.policy.allowed_roots {
            let canonical_root = root.canonical_path.clone();
            let db_clone = self.open_index();
            let opts_clone = opts.clone();

            let (groups, _stats) = tokio::time::timeout(
                self.policy.timeout,
                tokio::task::spawn_blocking(move || {
                    vacua_content::DuplicateEngine::scan_path(
                        &canonical_root,
                        &opts_clone,
                        db_clone.as_ref(),
                    )
                }),
            )
            .await
            .map_err(|_| {
                VacuaErrorResponse::new(VacuaErrorCode::VacuaBusy, "Duplicate group scan timed out")
            })?
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

            for group in groups {
                let computed_id = self
                    .policy
                    .generate_duplicate_group_id(&root.root_id, &group.group_id);
                if computed_id == group_id || group.group_id == group_id {
                    let known_members = group
                        .members
                        .iter()
                        .filter(|m| m.allocation.kernel_private_bytes.is_some())
                        .count();
                    let unknown_members = group.members.len() - known_members;

                    let mut member_map_lock = self.member_id_map.lock().unwrap();
                    let members: Vec<DuplicateMemberV1> = group
                        .members
                        .iter()
                        .map(|m| {
                            let member_id = self.policy.generate_duplicate_member_id(&m.path);
                            member_map_lock.insert(member_id.clone(), m.path.clone());

                            DuplicateMemberV1 {
                                member_id,
                                display_path: self.policy.format_path(&m.path),
                                physical_relation: format!("{:?}", m.physical_relation),
                                allocated_bytes: m.allocation.allocated_bytes,
                                kernel_private_bytes: m
                                    .allocation
                                    .kernel_private_bytes
                                    .unwrap_or(0),
                                kernel_private_bytes_known: m
                                    .allocation
                                    .kernel_private_bytes
                                    .is_some(),
                                is_cloud_placeholder: vacua_content::is_cloud_placeholder(&m.path),
                            }
                        })
                        .collect();

                    return Ok(DuplicateGroupDetailV1 {
                        schema_version: SCHEMA_DUPLICATE_GROUP_V1.to_string(),
                        group_id: computed_id,
                        file_size: group.logical_size,
                        logical_duplicate_bytes: group.logical_duplicate_bytes,
                        kernel_private_bytes: group
                            .members
                            .iter()
                            .map(|m| m.allocation.kernel_private_bytes.unwrap_or(0))
                            .sum(),
                        confirmed_reclaim_lower_bound: group.confirmed_reclaimable_bytes,
                        estimated_reclaim: group.estimated_reclaimable_bytes,
                        reclaim_upper_bound: group.upper_bound_reclaimable_bytes,
                        content_identity_verified: true,
                        algorithm: "BLAKE3".to_string(),
                        kernel_private_bytes_known_members: known_members,
                        kernel_private_bytes_unknown_members: unknown_members,
                        members,
                    });
                }
            }
        }

        Err(VacuaErrorResponse::new(
            VacuaErrorCode::VacuaNotFound,
            format!("Duplicate group '{}' not found", group_id),
        ))
    }

    /// Simulate cleanup tool implementation.
    pub fn simulate_cleanup(
        &self,
        candidate_ids: &[String],
    ) -> Result<CleanupSimulationV1, VacuaErrorResponse> {
        if candidate_ids.len() > MAX_SIMULATION_CANDIDATES {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                format!(
                    "Number of candidate IDs ({}) exceeds maximum simulation budget of {}",
                    candidate_ids.len(),
                    MAX_SIMULATION_CANDIDATES
                ),
            ));
        }

        let mut all_cands = Vec::new();
        for root in &self.policy.allowed_roots {
            all_cands.extend(self.get_or_evaluate_candidates_for_root(root)?);
        }

        let cand_map: HashMap<&str, &Candidate> =
            all_cands.iter().map(|c| (c.id.as_str(), c)).collect();

        let mut matched = Vec::new();
        let mut blocked = Vec::new();

        for id in candidate_ids {
            if let Some(cand) = cand_map.get(id.as_str()) {
                if cand.risk.is_protected_or_unknown() {
                    blocked.push(format!("{}: Protected or Unknown risk", id));
                } else {
                    matched.push(*cand);
                }
            } else {
                blocked.push(format!("{}: Candidate ID not found", id));
            }
        }

        let mut confirmed_lower = 0u64;
        let mut eventual_reclaim = 0u64;
        let mut reclaim_upper = 0u64;

        for c in &matched {
            let (lower, est, upper) = Self::compute_reclaim_bounds(c);
            confirmed_lower = confirmed_lower.saturating_add(lower);
            eventual_reclaim = eventual_reclaim.saturating_add(est);
            reclaim_upper = reclaim_upper.saturating_add(upper);
        }

        let highest_risk = matched
            .iter()
            .map(|c| c.risk)
            .max()
            .map(|r| r.to_string())
            .unwrap_or_else(|| "Safe".to_string());

        let rebuild_consequences: Vec<String> = matched
            .iter()
            .filter_map(|c| c.rebuild_consequence.as_ref())
            .map(|s| self.policy.sanitize_string(s))
            .collect();

        Ok(CleanupSimulationV1 {
            schema_version: SCHEMA_CLEANUP_SIMULATION_V1.to_string(),
            candidate_count: matched.len(),
            immediate_reclaim_bytes: 0, // Moving to Trash never frees disk space immediately
            eventual_reclaim_estimate_bytes: eventual_reclaim,
            confirmed_lower_bound_bytes: confirmed_lower,
            reclaim_upper_bound_bytes: reclaim_upper,
            highest_risk,
            rebuild_consequences,
            blocked_items: blocked,
            cost_model_status: "EXPERIMENTAL".to_string(),
        })
    }

    /// Propose CleanupPlan tool implementation.
    /// Strictly proposal-only. Serialized plan is disabled unless allow_plan_export && Full mode.
    pub fn propose_cleanup_plan(
        &self,
        candidate_ids: &[String],
    ) -> Result<CleanupPlanProposalV1, VacuaErrorResponse> {
        if candidate_ids.len() > MAX_PROPOSAL_CANDIDATES {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaLimitExceeded,
                format!(
                    "Number of candidate IDs ({}) exceeds maximum proposal budget of {}",
                    candidate_ids.len(),
                    MAX_PROPOSAL_CANDIDATES
                ),
            ));
        }

        let mut all_cands = Vec::new();
        for root in &self.policy.allowed_roots {
            all_cands.extend(self.get_or_evaluate_candidates_for_root(root)?);
        }

        let cand_map: HashMap<&str, &Candidate> =
            all_cands.iter().map(|c| (c.id.as_str(), c)).collect();

        let mut selected = Vec::new();
        for id in candidate_ids {
            if let Some(cand) = cand_map.get(id.as_str()) {
                if cand.risk.is_protected_or_unknown() {
                    return Err(VacuaErrorResponse::new(
                        VacuaErrorCode::VacuaProtected,
                        format!(
                            "Candidate '{}' is Protected or Unknown risk and cannot enter a plan",
                            id
                        ),
                    ));
                }
                selected.push((*cand).clone());
            } else {
                return Err(VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Candidate '{}' not found", id),
                ));
            }
        }

        if selected.is_empty() {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInvalidArgument,
                "Cannot propose an empty cleanup plan",
            ));
        }

        let max_risk = selected
            .iter()
            .map(|c| c.risk)
            .max()
            .unwrap_or(RiskLevel::Safe);
        let plan =
            CleanupPlan::build_with_guards(&selected, max_risk, env!("CARGO_PKG_VERSION"), vec![])
                .map_err(|e| {
                    VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string())
                })?;

        let items = plan
            .items
            .iter()
            .map(|item| PlanProposalItemV1 {
                candidate_id: item.candidate_id.clone(),
                display_path: self.policy.format_path(&item.path),
                action: "Trash".to_string(),
                risk: item.risk.to_string(),
                estimated_bytes: item.allocated_bytes,
            })
            .collect();

        // PRIVACY ENFORCEMENT: serialized_plan contains raw absolute PathBufs.
        // It MUST remain None unless explicitly opted-in via --allow-plan-export AND path_disclosure == Full!
        let serialized_plan = if self.policy.allow_plan_export
            && self.policy.path_disclosure == PathDisclosureMode::Full
        {
            serde_json::to_string(&plan).ok()
        } else {
            None
        };

        Ok(CleanupPlanProposalV1 {
            schema_version: SCHEMA_PLAN_PROPOSAL_V1.to_string(),
            plan_schema_version: plan.plan_schema_version,
            plan_id: plan.plan_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            created_at: plan.created_at.to_rfc3339(),
            item_count: plan.items.len(),
            highest_risk: max_risk.to_string(),
            estimated_eventual_reclaim_bytes: plan.estimated_eventual_reclaim_bytes,
            immediate_reclaim_bytes: 0,
            preservation_guard_count: plan.preservation_guards.len(),
            items,
            proposal_status: "PROPOSAL_ONLY_NOT_EXECUTABLE_VIA_MCP".to_string(),
            serialized_plan,
        })
    }

    /// History summary tool implementation using read-only journal.
    pub fn history_summary(&self, limit: Option<usize>) -> HistorySummaryV1 {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        if let Some(journal) = self.open_journal() {
            if let Ok(txs) = journal.list_transactions(limit) {
                let total_tx = txs.len();
                let total_moved: u64 = txs.iter().map(|t| t.total_reclaimed_bytes).sum();
                let recent = txs
                    .into_iter()
                    .map(|t| HistoryTransactionV1 {
                        transaction_id: t.transaction_id,
                        plan_hash: t.plan_hash,
                        timestamp: t.timestamp,
                        total_items: t.total_items,
                        successful_count: t.successful_count,
                        reclaimed_bytes: t.total_reclaimed_bytes,
                        bytes_moved_to_trash: t.total_reclaimed_bytes,
                    })
                    .collect();

                return HistorySummaryV1 {
                    schema_version: SCHEMA_HISTORY_SUMMARY_V1.to_string(),
                    total_transactions: total_tx,
                    total_reclaimed_bytes: total_moved,
                    total_bytes_moved_to_trash: total_moved,
                    recent_transactions: recent,
                };
            }
        }

        HistorySummaryV1 {
            schema_version: SCHEMA_HISTORY_SUMMARY_V1.to_string(),
            total_transactions: 0,
            total_reclaimed_bytes: 0,
            total_bytes_moved_to_trash: 0,
            recent_transactions: vec![],
        }
    }

    /// Verify history tool implementation using read-only journal.
    pub fn verify_history(&self) -> Result<HistoryVerificationV1, VacuaErrorResponse> {
        let journal = self.open_journal().ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaNotFound,
                "Execution journal database not found",
            )
        })?;

        let report = journal
            .verify_chain()
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        Ok(HistoryVerificationV1 {
            schema_version: SCHEMA_HISTORY_VERIFICATION_V1.to_string(),
            total_records: report.total_records,
            is_valid: report.is_valid,
            first_hash: report.first_hash,
            latest_hash: report.latest_hash,
            broken_record_id: report.broken_record_id,
            error_detail: report.error_detail,
        })
    }

    /// Capabilities tool implementation.
    pub fn get_capabilities(&self) -> ServerCapabilitiesV1 {
        let allowed_roots: Vec<String> = self
            .policy
            .allowed_roots
            .iter()
            .map(|r| r.display_name.clone())
            .collect();

        ServerCapabilitiesV1 {
            schema_version: SCHEMA_SERVER_CAPABILITIES_V1.to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
            mcp_protocol_generation: "2026-07-28".to_string(),
            platform: "macOS".to_string(),
            transport: "stdio".to_string(),
            path_disclosure_mode: self.policy.path_disclosure.as_str().to_string(),
            allowed_roots,
            mutation_authority: false,
            executor_linked: false,
            read_only_tier: true,
            analyze_only_tier: true,
            propose_only_tier: true,
            index_available: self
                .index_path
                .as_ref()
                .map(|p| p.exists())
                .unwrap_or(false),
            snapshot_engine_available: true,
            duplicate_engine_available: true,
            app_evidence_graph_available: true,
            apple_foundation_models_status: "Implemented / Not Queried by MCP (Zero Subprocess)"
                .to_string(),
            plan_export_enabled: self.policy.allow_plan_export,
            system_app_metadata_enabled: self.policy.allow_system_app_metadata,
            foundation_models_integration: "IMPLEMENTED".to_string(),
            foundation_models_runtime_status: "NOT_QUERIED_BY_MCP".to_string(),
        }
    }

    /// Analyze or build storage tree for authoritative root.
    pub async fn analyze_storage_map(
        &self,
        root_id: Option<&str>,
        force_refresh: Option<bool>,
    ) -> Result<StorageTreeAnalysisV1, VacuaErrorResponse> {
        let _permit = self.policy.acquire_expensive_permit().await?;
        let root = self.policy.get_root(root_id)?.clone();

        let mut db = self.open_index_writable().ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaInternal,
                "Failed to open index database for writing",
            )
        })?;

        let canonical_root = root.canonical_path.clone();
        let root_id_str = root.root_id.clone();
        let force = force_refresh.unwrap_or(false);

        let analysis_dto = tokio::time::timeout(
            self.policy.timeout,
            tokio::task::spawn_blocking(move || -> Result<StorageTreeAnalysisV1, VacuaErrorResponse> {
                let existing_entries = if !force {
                    db.get_entries_under_root(&canonical_root).unwrap_or_default()
                } else {
                    vec![]
                };

                let builder = StorageTreeBuilder::new(&canonical_root, &root_id_str);
                let (nodes, coverage, source) = if !existing_entries.is_empty() {
                    let cov = StorageTreeCoverage {
                        files_observed: existing_entries.iter().filter(|e| e.file_type != "directory").count() as u64,
                        directories_observed: existing_entries.iter().filter(|e| e.file_type == "directory").count() as u64,
                        entries_skipped: 0,
                        permission_errors: 0,
                        mount_boundary_skips: 0,
                        special_files_skipped: 0,
                        cloud_placeholders_observed: 0,
                        analysis_complete: true,
                    };
                    let n = builder.build_from_indexed_entries(&existing_entries)
                        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;
                    (n, cov, StorageTreeSource::Indexed)
                } else {
                    let scanner = FilesystemScanner::new(ScanOptions {
                        cross_mounts: false,
                        max_depth: None,
                        jobs: Some(2),
                    });
                    let report = scanner
                        .scan(&canonical_root)
                        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;
                    let cov = StorageTreeCoverage {
                        files_observed: report.total_files,
                        directories_observed: report.total_dirs,
                        entries_skipped: report.skipped_paths.len() as u64,
                        permission_errors: report.skipped_paths.len() as u64,
                        mount_boundary_skips: 0,
                        special_files_skipped: 0,
                        cloud_placeholders_observed: 0,
                        analysis_complete: true,
                    };
                    let _session_id = db
                        .record_session(&canonical_root, &report)
                        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;
                    let n = builder.build_from_scanned_entries(&report.entries)
                        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;
                    (n, cov, StorageTreeSource::LiveScan)
                };

                let root_node = nodes.first().ok_or_else(|| {
                    VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, "Empty storage tree generated")
                })?;

                let gen = StorageTreeGeneration {
                    generation_id: format!("stg_{}_{}", root_id_str, Utc::now().timestamp_millis()),
                    root_path: canonical_root.clone(),
                    root_id: root_id_str.clone(),
                    observed_at: Utc::now().timestamp(),
                    source,
                    status: StorageTreeStatus::Ready,
                    total_files: root_node.file_count,
                    total_dirs: root_node.directory_count,
                    total_logical_bytes: root_node.subtree_logical_bytes,
                    total_allocated_bytes: root_node.subtree_allocated_bytes,
                    coverage: coverage.clone(),
                };

                StorageTreeEngine::publish_generation(db.conn_mut(), &gen, &nodes)
                    .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

                let _ = StorageTreeEngine::prune_older_generations(db.conn_mut(), &root_id_str, 2);

                let mut root_dto = root_node.to_dto();
                root_dto.display_path = canonical_root.to_string_lossy().to_string();

                Ok(StorageTreeAnalysisV1 {
                    schema_version: SCHEMA_STORAGE_TREE_ANALYSIS_V1.to_string(),
                    generation_id: gen.generation_id,
                    root_path: canonical_root.to_string_lossy().to_string(),
                    root_id: gen.root_id,
                    observed_at: Utc::now().to_rfc3339(),
                    source: format!("{:?}", gen.source),
                    root_node: root_dto,
                    total_files: gen.total_files,
                    total_dirs: gen.total_dirs,
                    total_logical_bytes: gen.total_logical_bytes,
                    total_allocated_bytes: gen.total_allocated_bytes,
                    physical_sharing_uncertainty: true,
                    allocation_semantics: "Filesystem allocation attributed to this namespace tree. APFS clone sharing may cause physical overlap; not guaranteed unique physical storage.".to_string(),
                    coverage: gen.coverage.to_dto(),
                })
            }),
        )
        .await
        .map_err(|_| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaBusy, "Storage tree analysis timed out")
        })?
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))??;

        Ok(analysis_dto)
    }

    /// Retrieve bounded storage tree child nodes with remainder accounting.
    #[allow(clippy::too_many_arguments)]
    pub fn get_storage_map(
        &self,
        root_id: &str,
        generation_id: &str,
        node_id: Option<&str>,
        metric: Option<&str>,
        limit: Option<usize>,
        offset: Option<usize>,
        compare_snapshot_id: Option<&str>,
    ) -> Result<StorageTreePageV1, VacuaErrorResponse> {
        let root = self.policy.get_root(Some(root_id))?;
        let db = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaNotFound,
                "Metadata index database not found",
            )
        })?;

        let gen =
            StorageTreeEngine::get_generation(db.conn(), generation_id).map_err(|e| match e {
                TreeError::GenerationNotFound(_) => VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaStaleState,
                    format!("Tree generation {} not found or pruned", generation_id),
                ),
                other => VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, other.to_string()),
            })?;

        if gen.root_id != root.root_id {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaPolicyDenied,
                format!(
                    "Generation {} does not belong to root {}",
                    generation_id, root.root_id
                ),
            ));
        }

        let parent_node_id = if let Some(nid) = node_id {
            StorageNodeId::new(nid)
        } else {
            StorageNodeId::from_relative_path(&root.root_id, Path::new(""))
        };

        let metric_enum = match metric {
            Some("logical") => StorageTreeMetric::Logical,
            _ => StorageTreeMetric::Allocated,
        };

        let limit_val = limit.unwrap_or(60).min(200) as u32;
        let offset_val = offset.unwrap_or(0) as u32;

        let (children, remainder, total_child_count) = StorageTreeEngine::query_children_page(
            db.conn(),
            generation_id,
            &parent_node_id,
            metric_enum,
            limit_val,
            offset_val,
        )
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        let parent_node = StorageTreeEngine::get_node(db.conn(), generation_id, &parent_node_id)
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
            .ok_or_else(|| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Parent node {} not found", parent_node_id),
                )
            })?;

        let mut parent_dto = parent_node.to_dto();
        let rel_parent = String::from_utf8_lossy(&parent_node.raw_relative_path);
        let parent_full = root.canonical_path.join(rel_parent.as_ref());
        parent_dto.display_path = self.policy.format_path(&parent_full);

        let item_deltas = if let Some(snap_id) = compare_snapshot_id {
            if let Ok(Some(snapshot)) = db.get_snapshot(snap_id) {
                if let Ok(deltas) =
                    StorageTreeEngine::compare_nodes_with_snapshot(&gen, &snapshot, &children)
                {
                    Some(deltas.into_iter().map(|d| d.to_dto()).collect())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let items: Vec<StorageTreeNodeV1> = children
            .into_iter()
            .map(|c| {
                let rel = String::from_utf8_lossy(&c.raw_relative_path);
                let full = root.canonical_path.join(rel.as_ref());
                let mut dto = c.to_dto();
                dto.display_path = self.policy.format_path(&full);
                dto
            })
            .collect();

        let next_offset = if (offset_val as usize) + items.len() < (total_child_count as usize) {
            Some((offset_val as usize) + items.len())
        } else {
            None
        };

        let next_cursor = next_offset.map(|off| {
            format!(
                "{}:{}:{}:{}",
                generation_id,
                parent_node_id.as_str(),
                metric.unwrap_or("allocated"),
                off
            )
        });

        Ok(StorageTreePageV1 {
            schema_version: SCHEMA_STORAGE_TREE_PAGE_V1.to_string(),
            generation_id: generation_id.to_string(),
            parent_node: parent_dto,
            metric: metric.unwrap_or("allocated").to_string(),
            items,
            total_child_count: total_child_count as usize,
            limit: limit_val as usize,
            offset: offset_val as usize,
            remainder: remainder.to_dto(),
            next_cursor,
            item_deltas,
        })
    }

    /// Retrieve detailed metrics for a single node, optionally with snapshot delta comparison.
    pub fn get_storage_node(
        &self,
        root_id: &str,
        generation_id: &str,
        node_id: &str,
        compare_snapshot_id: Option<&str>,
    ) -> Result<StorageTreeNodeDetailV1, VacuaErrorResponse> {
        let root = self.policy.get_root(Some(root_id))?;
        let db = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(
                VacuaErrorCode::VacuaNotFound,
                "Metadata index database not found",
            )
        })?;

        let gen =
            StorageTreeEngine::get_generation(db.conn(), generation_id).map_err(|e| match e {
                TreeError::GenerationNotFound(_) => VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaStaleState,
                    format!("Tree generation {} not found or pruned", generation_id),
                ),
                other => VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, other.to_string()),
            })?;

        if gen.root_id != root.root_id {
            return Err(VacuaErrorResponse::new(
                VacuaErrorCode::VacuaPolicyDenied,
                format!(
                    "Generation {} does not belong to root {}",
                    generation_id, root.root_id
                ),
            ));
        }

        let target_node_id = StorageNodeId::new(node_id);
        let node = StorageTreeEngine::get_node(db.conn(), generation_id, &target_node_id)
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
            .ok_or_else(|| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Node {} not found in generation {}", node_id, generation_id),
                )
            })?;

        let rel = String::from_utf8_lossy(&node.raw_relative_path);
        let full_path = root.canonical_path.join(rel.as_ref());
        let display_path = self.policy.format_path(&full_path);

        let percentage_of_parent = if let Some(ref pid) = node.parent_id {
            if let Ok(Some(parent)) = StorageTreeEngine::get_node(db.conn(), generation_id, pid) {
                if parent.subtree_allocated_bytes > 0 {
                    Some(
                        (node.subtree_allocated_bytes as f64
                            / parent.subtree_allocated_bytes as f64)
                            * 100.0,
                    )
                } else {
                    Some(0.0)
                }
            } else {
                Some(100.0)
            }
        } else {
            Some(100.0)
        };

        let root_node = StorageTreeEngine::get_node(
            db.conn(),
            generation_id,
            &StorageNodeId::from_relative_path(&root.root_id, Path::new("")),
        )
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        let percentage_of_root = if let Some(r) = root_node {
            if r.subtree_allocated_bytes > 0 {
                (node.subtree_allocated_bytes as f64 / r.subtree_allocated_bytes as f64) * 100.0
            } else {
                0.0
            }
        } else {
            100.0
        };

        let hardlink_info = if node.is_hardlink_alias {
            Some(
                "Hardlink alias — allocated storage is attributed to another representative path."
                    .to_string(),
            )
        } else if node.hardlink_alias_count > 0 {
            Some(format!(
                "Primary hardlink representative with {} alias(es).",
                node.hardlink_alias_count
            ))
        } else {
            None
        };

        let delta = if let Some(snap_id) = compare_snapshot_id {
            let snapshot = db
                .get_snapshot(snap_id)
                .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
                .ok_or_else(|| {
                    VacuaErrorResponse::new(
                        VacuaErrorCode::VacuaNotFound,
                        format!("Snapshot {} not found", snap_id),
                    )
                })?;

            let d =
                StorageTreeEngine::compare_with_snapshot(&gen, &snapshot, &node).map_err(|e| {
                    match e {
                        TreeError::SnapshotRootMismatch { .. } => VacuaErrorResponse::new(
                            VacuaErrorCode::VacuaPolicyDenied,
                            e.to_string(),
                        ),
                        other => VacuaErrorResponse::new(
                            VacuaErrorCode::VacuaInternal,
                            other.to_string(),
                        ),
                    }
                })?;
            Some(d.to_dto())
        } else {
            None
        };

        let mut node_dto = node.to_dto();
        node_dto.display_path = display_path;

        Ok(StorageTreeNodeDetailV1 {
            schema_version: SCHEMA_STORAGE_TREE_NODE_DETAIL_V1.to_string(),
            node: node_dto,
            percentage_of_parent,
            percentage_of_root,
            hardlink_info,
            allocation_semantics: "Filesystem allocation attributed to this namespace tree. APFS clone sharing may cause physical overlap; not guaranteed unique physical storage.".to_string(),
            delta,
        })
    }
}
