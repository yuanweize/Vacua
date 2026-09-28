use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebuildCost {
    Instant,
    Low,
    Medium,
    High,
}

impl std::fmt::Display for RebuildCost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Instant => write!(f, "Instant (Ephemeral)"),
            Self::Low => write!(f, "Low (<1 min)"),
            Self::Medium => write!(f, "Medium (1-5 min)"),
            Self::High => write!(f, "High (>10 min compile)"),
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

/// Deterministic model assessing the trade-off between physical space reclaimed
/// and the time/network resources required to regenerate deleted items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReclaimCost {
    pub expected_freeable_bytes: u64,
    pub reclaim_confidence: f32,
    pub rebuild_cost: RebuildCost,
    pub network_redownload_bytes: u64,
    pub active_project: bool,
    pub reversibility: bool,
    pub tier: CostTier,
    pub advice: String,
}

impl ReclaimCost {
    pub fn evaluate(
        category_name: &str,
        allocated_bytes: u64,
        reclaim_confidence: f32,
        is_active: bool,
        is_reversible: bool,
    ) -> Self {
        let (rebuild_cost, redownload_bytes, tier, advice) = match category_name {
            "BUILD_ARTIFACT" | "BuildArtifact" => {
                if is_active {
                    (
                        RebuildCost::High,
                        0,
                        CostTier::HighValueHighRebuildCost,
                        "Active project artifact: Reclaiming space will trigger full rebuild on next compilation."
                            .to_string(),
                    )
                } else {
                    (
                        RebuildCost::Low,
                        0,
                        CostTier::HighValueLowCost,
                        "Dormant build artifact: Safe to clean; easily regenerated if project is reopened."
                            .to_string(),
                    )
                }
            }
            "PACKAGE_MANAGER_CACHE" | "PackageManagerCache" => (
                RebuildCost::Medium,
                allocated_bytes,
                CostTier::HighValueLowCost,
                "Package manager cache: Reclaiming frees physical space; missing dependencies redownloaded on demand."
                    .to_string(),
            ),
            "CONTAINER_DATA" | "ContainerData" => {
                if is_active {
                    (
                        RebuildCost::High,
                        allocated_bytes,
                        CostTier::LowValueHighCost,
                        "Active container data: May disrupt running developer services.".to_string(),
                    )
                } else {
                    (
                        RebuildCost::Medium,
                        allocated_bytes,
                        CostTier::HighValueLowCost,
                        "Stopped container layers: High reclaim potential with standard re-pull cost.".to_string(),
                    )
                }
            }
            _ => (
                RebuildCost::Instant,
                0,
                CostTier::HighValueLowCost,
                "Standard ephemeral cache: Freeable immediately.".to_string(),
            ),
        };

        Self {
            expected_freeable_bytes: (allocated_bytes as f32 * reclaim_confidence) as u64,
            reclaim_confidence,
            rebuild_cost,
            network_redownload_bytes: redownload_bytes,
            active_project: is_active,
            reversibility: is_reversible,
            tier,
            advice,
        }
    }

    pub fn from_candidate(c: &crate::candidate::Candidate) -> Self {
        Self::evaluate(
            c.category.as_str(),
            c.allocation.allocated_bytes,
            c.allocation.reclaim_confidence,
            false,
            c.risk.is_reversible(),
        )
    }
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
                min_freeable =
                    min_freeable.saturating_add((c.allocation.allocated_bytes as f32 * 0.8) as u64);
                max_freeable = max_freeable.saturating_add(c.allocation.allocated_bytes);
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
