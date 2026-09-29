use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use vacua_api::*;
use vacua_content::DuplicateScanOptions;
use vacua_core::candidate::Candidate;
use vacua_core::evidence_graph::{ApplicationEvidenceGraph, NodeKind, OrphanConfidence};
use vacua_core::pressure::{query_volume_status, StoragePressure, VolumeStorageStatus};
use vacua_core::risk::RiskLevel;
use vacua_index::{ExecutionJournal, IndexDatabase};
use vacua_plan::CleanupPlan;
use vacua_risk::CandidateEvaluator;
use vacua_rules::engine::RulesEngine;
use vacua_scan::{FilesystemScanner, ScanOptions};

use crate::policy::McpPolicy;

type CachedCandidates = Arc<Mutex<Option<(Vec<Candidate>, i64)>>>;

/// Domain service coordinating storage intelligence queries.
pub struct VacuaDomainService {
    policy: McpPolicy,
    index_path: Option<PathBuf>,
    journal_path: Option<PathBuf>,
    cached_candidates: CachedCandidates,
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
            cached_candidates: Arc::new(Mutex::new(None)),
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
            IndexDatabase::open(&path).ok()
        } else {
            None
        }
    }

    fn open_journal(&self) -> Option<ExecutionJournal> {
        let path = self.journal_path.clone().unwrap_or_else(|| {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            home.join(".vacua").join("journal.db")
        });

        if path.exists() {
            ExecutionJournal::open(&path).ok()
        } else {
            None
        }
    }

    /// Retrieve or evaluate candidates under allowed roots.
    pub fn get_or_evaluate_candidates(&self) -> Vec<Candidate> {
        let now = Utc::now().timestamp();
        {
            let lock = self.cached_candidates.lock().unwrap();
            if let Some((ref cands, ts)) = *lock {
                if now - ts < 15 {
                    return cands.clone();
                }
            }
        }

        let primary_root = self.policy.primary_root();
        let scanner = FilesystemScanner::new(ScanOptions {
            cross_mounts: false,
            max_depth: Some(5),
            ..Default::default()
        });

        let mut candidates = Vec::new();
        if let Ok(report) = scanner.scan(&primary_root) {
            let mut engine = RulesEngine::new();
            let mut evaluator = CandidateEvaluator::new(&mut engine);

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
        }

        // Sort deterministically: reclaim desc, candidate_id asc
        candidates.sort_by(|a, b| {
            b.allocation
                .allocated_bytes
                .cmp(&a.allocation.allocated_bytes)
                .then_with(|| a.id.cmp(&b.id))
        });

        let mut lock = self.cached_candidates.lock().unwrap();
        *lock = Some((candidates.clone(), now));
        candidates
    }

    /// Storage summary tool implementation.
    pub fn storage_summary(&self, req_path: Option<&str>) -> StorageSummaryV1 {
        let target_path = if let Some(p) = req_path {
            PathBuf::from(p)
        } else {
            self.policy.primary_root()
        };

        let status = query_volume_status(&target_path).unwrap_or_else(|_| VolumeStorageStatus {
            mount_point: target_path.to_string_lossy().to_string(),
            filesystem_type: "apfs".to_string(),
            total_bytes: 0,
            free_bytes: 0,
            available_bytes: 0,
            free_ratio: 0.0,
            pressure: StoragePressure::Normal,
        });

        let candidates = self.get_or_evaluate_candidates();
        let candidate_reclaim: u64 = candidates
            .iter()
            .map(|c| c.allocation.allocated_bytes)
            .sum();

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

        StorageSummaryV1 {
            schema_version: SCHEMA_STORAGE_SUMMARY_V1.to_string(),
            observed_at: Utc::now().to_rfc3339(),
            target_path: self.policy.format_path(&target_path),
            mount_point: self.policy.format_path(Path::new(&status.mount_point)),
            filesystem_type: status.filesystem_type,
            total_space_bytes: status.total_bytes,
            free_space_bytes: status.free_bytes,
            available_space_bytes: status.available_bytes,
            purgeable_space_bytes: None,
            pressure_level: format!("{:?}", status.pressure),
            candidate_count: candidates.len(),
            candidate_reclaim_bytes: candidate_reclaim,
            candidate_reviewable_bytes: candidate_reclaim,
            index_freshness,
            is_stale,
        }
    }

    /// List snapshots tool implementation.
    pub fn list_snapshots(
        &self,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<SnapshotListResponseV1, VacuaErrorResponse> {
        let index = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, "Index database not found")
        })?;

        let summaries = index
            .list_snapshots()
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        let total_count = summaries.len();
        let limit = self.policy.clamp_limit(limit);
        let offset = cursor
            .and_then(McpPolicy::decode_cursor)
            .unwrap_or(0)
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
            Some(McpPolicy::encode_cursor(end))
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

    /// Diff snapshots tool implementation.
    pub fn diff_snapshots(
        &self,
        base: &str,
        target: Option<&str>,
    ) -> Result<SnapshotDiffV1, VacuaErrorResponse> {
        let index = self.open_index().ok_or_else(|| {
            VacuaErrorResponse::new(VacuaErrorCode::VacuaNotFound, "Index database not found")
        })?;

        let target_name = target.unwrap_or("current");
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

    /// List candidates tool implementation.
    pub fn list_candidates(
        &self,
        max_risk: Option<&str>,
        category: Option<&str>,
        min_reclaim: Option<u64>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> CandidateListResponseV1 {
        let all_cands = self.get_or_evaluate_candidates();
        let target_risk = max_risk
            .and_then(|r| match r.to_lowercase().as_str() {
                "safe" => Some(RiskLevel::Safe),
                "review" => Some(RiskLevel::Review),
                "caution" => Some(RiskLevel::Caution),
                "protected" => Some(RiskLevel::Protected),
                "unknown" => Some(RiskLevel::Unknown),
                _ => None,
            })
            .unwrap_or(RiskLevel::Safe);

        let filtered: Vec<&Candidate> = all_cands
            .iter()
            .filter(|c| c.risk <= target_risk)
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
        let offset = cursor
            .and_then(McpPolicy::decode_cursor)
            .unwrap_or(0)
            .min(total_count);

        let end = (offset + limit).min(total_count);
        let items = filtered[offset..end]
            .iter()
            .map(|c| CandidateSummaryV1 {
                candidate_id: c.id.clone(),
                display_path: self.policy.format_path(&c.path),
                category: c.category.to_string(),
                risk: c.risk.to_string(),
                allocated_bytes: c.allocation.allocated_bytes,
                reclaim_estimate_bytes: c.allocation.allocated_bytes,
                reconstructable: c.reconstructable,
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor(end))
        } else {
            None
        };

        CandidateListResponseV1 {
            schema_version: SCHEMA_CANDIDATE_LIST_V1.to_string(),
            items,
            total_count,
            limit,
            next_cursor,
        }
    }

    /// Explain candidate tool implementation.
    pub fn explain_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<CandidateDetailV1, VacuaErrorResponse> {
        let all_cands = self.get_or_evaluate_candidates();
        let cand = all_cands
            .iter()
            .find(|c| c.id == candidate_id)
            .ok_or_else(|| {
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

        Ok(CandidateDetailV1 {
            schema_version: SCHEMA_CANDIDATE_DETAIL_V1.to_string(),
            candidate_id: cand.id.clone(),
            display_path: self.policy.format_path(&cand.path),
            category: cand.category.to_string(),
            risk: cand.risk.to_string(),
            allocated_bytes: cand.allocation.allocated_bytes,
            logical_bytes: cand.allocation.logical_bytes,
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

    /// List applications tool implementation.
    pub fn list_applications(
        &self,
        filter: Option<&str>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> ApplicationListResponseV1 {
        let graph = ApplicationEvidenceGraph::build_from_system();
        let mut apps = Vec::new();

        for node in graph.nodes.values() {
            if node.kind == NodeKind::ApplicationBundle {
                let name = node.metadata.get("name").cloned().unwrap_or_default();
                let bundle_id = node.metadata.get("bundle_id").cloned().unwrap_or_default();
                let path_str = node.path.as_ref().map(|p| self.policy.format_path(p));
                apps.push((name, bundle_id, path_str));
            }
        }

        // Sort deterministically by name, then bundle_id
        apps.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        let filtered: Vec<(String, String, Option<String>)> = apps
            .into_iter()
            .filter(|(_name, bundle_id, _path)| {
                if let Some(f) = filter {
                    if f.eq_ignore_ascii_case("orphans-only") {
                        let eval = graph.evaluate_orphan(bundle_id);
                        eval.orphan_confidence > OrphanConfidence::NotOrphan
                    } else if f.eq_ignore_ascii_case("installed-only") {
                        let eval = graph.evaluate_orphan(bundle_id);
                        eval.bundle_installed
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .collect();

        let total_count = filtered.len();
        let limit = self.policy.clamp_limit(limit);
        let offset = cursor
            .and_then(McpPolicy::decode_cursor)
            .unwrap_or(0)
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
            Some(McpPolicy::encode_cursor(end))
        } else {
            None
        };

        ApplicationListResponseV1 {
            schema_version: SCHEMA_APPLICATION_LIST_V1.to_string(),
            items,
            total_count,
            limit,
            next_cursor,
        }
    }

    /// Get application detail tool implementation.
    pub fn get_application(
        &self,
        application_id: &str,
    ) -> Result<ApplicationDetailV1, VacuaErrorResponse> {
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

        let artifacts: Vec<ApplicationArtifactV1> = eval
            .artifact_paths
            .iter()
            .map(|p| {
                let bytes = std::fs::symlink_metadata(p).map(|m| m.len()).unwrap_or(0);
                ApplicationArtifactV1 {
                    artifact_id: self.policy.sanitize_string(&p.to_string_lossy()),
                    display_path: self.policy.format_path(p),
                    kind: "FilesystemArtifact".to_string(),
                    allocated_bytes: bytes,
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
            artifact_count: eval.artifact_paths.len(),
            estimated_reclaim_bytes: eval.associated_artifacts_bytes,
            artifacts,
        })
    }

    /// List duplicates tool implementation.
    pub async fn list_duplicates(
        &self,
        min_size_bytes: Option<u64>,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> Result<DuplicateListResponseV1, VacuaErrorResponse> {
        let _permit = self.policy.acquire_expensive_permit().await;
        let root = self.policy.primary_root();

        let db = self.open_index();
        let opts = DuplicateScanOptions {
            min_size: min_size_bytes.unwrap_or(1024 * 1024),
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: db.is_some(),
        };

        let (mut groups, _stats) = tokio::task::spawn_blocking(move || {
            vacua_content::DuplicateEngine::scan_path(&root, &opts, db.as_ref())
        })
        .await
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        // Deterministic sort: confirmed reclaim lower bound desc, group_id asc
        groups.sort_by(|a, b| {
            b.confirmed_reclaimable_bytes
                .cmp(&a.confirmed_reclaimable_bytes)
                .then_with(|| a.group_id.cmp(&b.group_id))
        });

        let total_count = groups.len();
        let limit = self.policy.clamp_limit(limit);
        let offset = cursor
            .and_then(McpPolicy::decode_cursor)
            .unwrap_or(0)
            .min(total_count);

        let end = (offset + limit).min(total_count);
        let items = groups[offset..end]
            .iter()
            .map(|g| DuplicateGroupSummaryV1 {
                group_id: g.group_id.clone(),
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
            })
            .collect();

        let next_cursor = if end < total_count {
            Some(McpPolicy::encode_cursor(end))
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
        let _permit = self.policy.acquire_expensive_permit().await;
        let root = self.policy.primary_root();

        let db = self.open_index();
        let opts = DuplicateScanOptions {
            min_size: 1,
            include_empty: false,
            hash_jobs: 2,
            skip_cloud: true,
            use_cache: db.is_some(),
        };

        let target_group_id = group_id.to_string();
        let (groups, _stats) = tokio::task::spawn_blocking(move || {
            vacua_content::DuplicateEngine::scan_path(&root, &opts, db.as_ref())
        })
        .await
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?
        .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

        let group = groups
            .into_iter()
            .find(|g| g.group_id == target_group_id)
            .ok_or_else(|| {
                VacuaErrorResponse::new(
                    VacuaErrorCode::VacuaNotFound,
                    format!("Duplicate group '{}' not found", group_id),
                )
            })?;

        let members = group
            .members
            .iter()
            .map(|m| DuplicateMemberV1 {
                member_id: self.policy.sanitize_string(&m.path.to_string_lossy()),
                display_path: self.policy.format_path(&m.path),
                physical_relation: format!("{:?}", m.physical_relation),
                allocated_bytes: m.allocation.allocated_bytes,
                kernel_private_bytes: m
                    .allocation
                    .kernel_private_bytes
                    .unwrap_or(m.allocation.allocated_bytes),
                is_cloud_placeholder: vacua_content::is_cloud_placeholder(&m.path),
            })
            .collect();

        Ok(DuplicateGroupDetailV1 {
            schema_version: SCHEMA_DUPLICATE_GROUP_V1.to_string(),
            group_id: group.group_id.clone(),
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
            members,
        })
    }

    /// Simulate cleanup tool implementation.
    pub fn simulate_cleanup(
        &self,
        candidate_ids: &[String],
    ) -> Result<CleanupSimulationV1, VacuaErrorResponse> {
        let all_cands = self.get_or_evaluate_candidates();
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

        let eventual_reclaim: u64 = matched.iter().map(|c| c.allocation.allocated_bytes).sum();

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
            immediate_reclaim_bytes: 0, // Trash movement does NOT equal reclaim
            eventual_reclaim_estimate_bytes: eventual_reclaim,
            highest_risk,
            rebuild_consequences,
            blocked_items: blocked,
            cost_model_status: "EXPERIMENTAL".to_string(),
        })
    }

    /// Propose CleanupPlan tool implementation.
    /// Crucial safety rule: Never executes, never trashes, never persists to disk.
    pub fn propose_cleanup_plan(
        &self,
        candidate_ids: &[String],
    ) -> Result<CleanupPlanProposalV1, VacuaErrorResponse> {
        let all_cands = self.get_or_evaluate_candidates();
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
        let plan = CleanupPlan::build_with_guards(&selected, max_risk, "0.5.0", vec![])
            .map_err(|e| VacuaErrorResponse::new(VacuaErrorCode::VacuaInternal, e.to_string()))?;

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
            serialized_plan: serde_json::to_string(&plan).ok(),
        })
    }

    /// History summary tool implementation.
    pub fn history_summary(&self, limit: Option<usize>) -> HistorySummaryV1 {
        let limit = limit.unwrap_or(20).clamp(1, 100);
        if let Some(journal) = self.open_journal() {
            if let Ok(txs) = journal.list_transactions(limit) {
                let total_tx = txs.len();
                let total_reclaimed = txs.iter().map(|t| t.total_reclaimed_bytes).sum();
                let recent = txs
                    .into_iter()
                    .map(|t| HistoryTransactionV1 {
                        transaction_id: t.transaction_id,
                        plan_hash: t.plan_hash,
                        timestamp: t.timestamp,
                        total_items: t.total_items,
                        successful_count: t.successful_count,
                        reclaimed_bytes: t.total_reclaimed_bytes,
                    })
                    .collect();

                return HistorySummaryV1 {
                    schema_version: SCHEMA_HISTORY_SUMMARY_V1.to_string(),
                    total_transactions: total_tx,
                    total_reclaimed_bytes: total_reclaimed,
                    recent_transactions: recent,
                };
            }
        }

        HistorySummaryV1 {
            schema_version: SCHEMA_HISTORY_SUMMARY_V1.to_string(),
            total_transactions: 0,
            total_reclaimed_bytes: 0,
            recent_transactions: vec![],
        }
    }

    /// Verify history tool implementation.
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
            .map(|r| self.policy.format_path(r))
            .collect();

        ServerCapabilitiesV1 {
            schema_version: SCHEMA_SERVER_CAPABILITIES_V1.to_string(),
            server_version: "0.5.0".to_string(),
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
            apple_foundation_models_status: "Available / Host Ineligible".to_string(),
        }
    }
}
