use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebuildCost {
    Instant,
    Low,
    Medium,
    High,
    Unknown,
}

impl std::fmt::Display for RebuildCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Instant => write!(f, "Instant (Ephemeral)"),
            Self::Low => write!(f, "Low (<1 min)"),
            Self::Medium => write!(f, "Medium (1-5 min)"),
            Self::High => write!(f, "High (>10 min compile)"),
            Self::Unknown => write!(f, "Unknown Friction"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostTier {
    HighValueLowCost,
    HighValueHighRebuildCost,
    LowValueHighCost,
}

impl std::fmt::Display for CostTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HighValueLowCost => write!(f, "High Value / Low Rebuild Cost"),
            Self::HighValueHighRebuildCost => write!(f, "High Value / High Rebuild Cost"),
            Self::LowValueHighCost => write!(f, "Low Value / High Replacement Cost"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveProjectStatus {
    Active,
    Dormant,
    Unknown,
}

impl std::fmt::Display for ActiveProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active (<7d)"),
            Self::Dormant => write!(f, "Dormant (>7d)"),
            Self::Unknown => write!(f, "Unlinked"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclaimCostInputs {
    pub category: String,
    pub allocated_bytes: u64,
    pub reclaim_confidence: f32,
    pub active_project: ActiveProjectStatus,
    pub running_process: bool,
    pub is_reversible: bool,
    pub clone_uncertainty: bool,
    pub mtime_sec: i64,
}

/// Deterministic model assessing the trade-off between physical space reclaimed
/// and the time/network resources required to regenerate deleted items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclaimCost {
    pub expected_freeable_bytes: u64,
    pub reclaim_confidence: f32,
    pub rebuild_cost: RebuildCost,
    pub network_redownload_bytes: u64,
    pub network_estimate_confidence: f32,
    pub active_project: ActiveProjectStatus,
    pub reversibility: bool,
    pub tier: CostTier,
    pub advice: String,
}

impl ReclaimCost {
    pub fn evaluate(inputs: &ReclaimCostInputs) -> Self {
        let (rebuild_cost, redownload_bytes, net_confidence, tier, advice) = match inputs.category.as_str() {
            "BUILD_ARTIFACT" | "BuildArtifact" => {
                match inputs.active_project {
                    ActiveProjectStatus::Active => (
                        RebuildCost::High,
                        0,
                        1.0,
                        CostTier::HighValueHighRebuildCost,
                        "Active project artifact: Reclaiming space will trigger full compilation on next build.".to_string(),
                    ),
                    ActiveProjectStatus::Dormant => (
                        RebuildCost::Low,
                        0,
                        1.0,
                        CostTier::HighValueLowCost,
                        "Dormant project artifact (>7d inactive): Safe to clean; easily regenerated if reopened.".to_string(),
                    ),
                    ActiveProjectStatus::Unknown => (
                        RebuildCost::Medium,
                        0,
                        1.0,
                        CostTier::HighValueLowCost,
                        "Build artifact with unlinked project: Regenerable upon request.".to_string(),
                    ),
                }
            }
            "PACKAGE_MANAGER_CACHE" | "PackageManagerCache" => (
                RebuildCost::Medium,
                inputs.allocated_bytes,
                0.85,
                CostTier::HighValueLowCost,
                "Package manager cache: Reclaiming frees physical blocks; upper bound network redownload required on demand.".to_string(),
            ),
            "CONTAINER_DATA" | "ContainerData" => {
                if inputs.running_process || inputs.active_project == ActiveProjectStatus::Active {
                    (
                        RebuildCost::High,
                        inputs.allocated_bytes,
                        0.75,
                        CostTier::LowValueHighCost,
                        "Active container data: May disrupt running developer workloads.".to_string(),
                    )
                } else {
                    (
                        RebuildCost::Medium,
                        inputs.allocated_bytes,
                        0.75,
                        CostTier::HighValueLowCost,
                        "Stopped container layers: High reclaim potential with standard image re-pull cost.".to_string(),
                    )
                }
            }
            _ => (
                RebuildCost::Instant,
                0,
                1.0,
                CostTier::HighValueLowCost,
                "Standard ephemeral cache: Freeable immediately with minimal rebuild friction.".to_string(),
            ),
        };

        Self {
            expected_freeable_bytes: (inputs.allocated_bytes as f32 * inputs.reclaim_confidence)
                as u64,
            reclaim_confidence: inputs.reclaim_confidence,
            rebuild_cost,
            network_redownload_bytes: redownload_bytes,
            network_estimate_confidence: net_confidence,
            active_project: inputs.active_project,
            reversibility: inputs.is_reversible,
            tier,
            advice,
        }
    }

    pub fn from_candidate(c: &crate::candidate::Candidate) -> Self {
        let active = detect_active_project(&c.path);
        let inputs = ReclaimCostInputs {
            category: c.category.as_str().to_string(),
            allocated_bytes: c.allocation.allocated_bytes,
            reclaim_confidence: c.allocation.reclaim_confidence,
            active_project: active,
            running_process: false,
            is_reversible: c.risk.is_reversible(),
            clone_uncertainty: c.allocation.extent_uncertainty,
            mtime_sec: c.mtime_sec,
        };
        Self::evaluate(&inputs)
    }
}

/// Detects whether an artifact belongs to an active project by checking recent Git index activity.
pub fn detect_active_project(path: &Path) -> ActiveProjectStatus {
    let mut curr = Some(path);
    while let Some(p) = curr {
        let git_dir = p.join(".git");
        if git_dir.exists() {
            let index_path = git_dir.join("index");
            if let Ok(meta) = index_path.metadata() {
                if let Ok(mtime) = meta.modified() {
                    if let Ok(elapsed) = mtime.elapsed() {
                        if elapsed.as_secs() < 7 * 86400 {
                            return ActiveProjectStatus::Active;
                        } else {
                            return ActiveProjectStatus::Dormant;
                        }
                    }
                }
            }
            return ActiveProjectStatus::Dormant;
        }
        curr = p.parent();
    }
    ActiveProjectStatus::Unknown
}

/// Simulation summary for what-if plan cleanup evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupSimulation {
    pub plan_id: String,
    pub total_items: usize,
    pub expected_freeable_bytes: u64,
    pub confidence_range: (u64, u64),
    pub items_moved_to_trash: usize,
    pub total_redownload_cost_bytes: u64,
    pub build_regeneration_count: usize,
    pub protected_dependencies_count: usize,
    pub active_processes_detected: usize,
}

impl CleanupSimulation {
    pub fn simulate(
        candidates: &[crate::candidate::Candidate],
        max_risk: crate::risk::RiskLevel,
    ) -> Self {
        let mut total_items = 0;
        let mut expected_freeable_bytes = 0u64;
        let mut min_freeable = 0u64;
        let mut max_freeable = 0u64;
        let mut items_moved_to_trash = 0;
        let mut total_redownload_cost_bytes = 0u64;
        let mut build_regeneration_count = 0;

        for c in candidates {
            if !c.risk.is_protected_or_unknown() && c.risk <= max_risk {
                total_items += 1;
                let cost = ReclaimCost::from_candidate(c);
                expected_freeable_bytes =
                    expected_freeable_bytes.saturating_add(cost.expected_freeable_bytes);
                min_freeable = min_freeable.saturating_add(c.allocation.confirmed_freeable_bytes());
                max_freeable =
                    max_freeable.saturating_add(c.allocation.upper_bound_freeable_bytes());
                items_moved_to_trash += 1;
                total_redownload_cost_bytes =
                    total_redownload_cost_bytes.saturating_add(cost.network_redownload_bytes);
                if matches!(cost.rebuild_cost, RebuildCost::Medium | RebuildCost::High) {
                    build_regeneration_count += 1;
                }
            }
        }

        Self {
            plan_id: format!("sim-{}", chrono::Utc::now().timestamp_millis()),
            total_items,
            expected_freeable_bytes,
            confidence_range: (min_freeable, max_freeable),
            items_moved_to_trash,
            total_redownload_cost_bytes,
            build_regeneration_count,
            protected_dependencies_count: 0,
            active_processes_detected: 0,
        }
    }
}
