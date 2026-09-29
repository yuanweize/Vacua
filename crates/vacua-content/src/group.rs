use crate::identity::PhysicalRelation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use vacua_core::{AllocationInfo, RiskLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateMember {
    pub path: PathBuf,
    pub device_id: u64,
    pub inode: u64,
    pub allocation: AllocationInfo,
    pub clone_id: Option<u64>,
    pub clone_refcnt: u32,
    pub nlink: u64,
    pub risk: RiskLevel,
    pub category: String,
    pub mtime_sec: i64,
    pub mtime_nsec: i64,
    pub ctime_sec: i64,
    pub ctime_nsec: i64,
    pub physical_relation: PhysicalRelation,
    pub is_suggested_keep: bool,
    pub suggest_keep_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicatePlanEstimate {
    pub confirmed_reclaimable_bytes: u64,
    pub estimated_reclaimable_bytes: u64,
    pub upper_bound_reclaimable_bytes: u64,
}

impl DuplicatePlanEstimate {
    /// Dynamically calculates confirmed, estimated, and upper bound physical reclaim
    /// given a chosen keep path and a list of paths selected for removal.
    pub fn for_keep(
        group: &DuplicateGroup,
        keep_path: &Path,
        selected_remove_paths: &[PathBuf],
    ) -> Self {
        Self::calculate_reclaim(&group.members, keep_path, selected_remove_paths)
    }

    pub fn calculate_reclaim(
        members: &[DuplicateMember],
        keep_path: &Path,
        selected_remove_paths: &[PathBuf],
    ) -> Self {
        let mut confirmed = 0u64;
        let mut estimated = 0u64;
        let mut upper = 0u64;

        let keep_member = members.iter().find(|m| m.path == keep_path);

        // Count how many times each (dev, inode) is selected for removal
        let mut removed_inodes_count: HashMap<(u64, u64), u64> = HashMap::new();
        for path in selected_remove_paths {
            if let Some(m) = members.iter().find(|m| &m.path == path) {
                *removed_inodes_count
                    .entry((m.device_id, m.inode))
                    .or_default() += 1;
            }
        }

        for path in selected_remove_paths {
            if path == keep_path {
                continue;
            }
            if let Some(m) = members.iter().find(|m| &m.path == path) {
                let dev_ino = (m.device_id, m.inode);
                let same_inode_as_keep =
                    keep_member.is_some_and(|km| (km.device_id, km.inode) == dev_ino);

                // If this member shares the inode with the kept file, unlinking it reclaims 0 bytes!
                if same_inode_as_keep {
                    continue;
                }

                // If hardlink: if not all links are removed (or if external hardlinks exist),
                // deleting this link does not free inode blocks.
                let removed_count = *removed_inodes_count.get(&dev_ino).unwrap_or(&0);
                if m.nlink > removed_count {
                    // Links still exist elsewhere, unlink reclaim is 0.
                    continue;
                }

                match m.physical_relation {
                    PhysicalRelation::HardlinkSharedExternal => {
                        // External hardlinks exist; cannot reclaim blocks.
                    }
                    PhysicalRelation::APFSCloneSharedExternal => {
                        // Shared with clones outside the group.
                        if let Some(priv_bytes) = m.allocation.kernel_private_bytes {
                            confirmed = confirmed.saturating_add(priv_bytes);
                            estimated = estimated.saturating_add(priv_bytes);
                        } else {
                            estimated = estimated.saturating_add(m.allocation.exclusive_bytes);
                        }
                        upper = upper.saturating_add(m.allocation.allocated_bytes);
                    }
                    PhysicalRelation::APFSCloneInGroup => {
                        if let Some(priv_bytes) = m.allocation.kernel_private_bytes {
                            confirmed = confirmed.saturating_add(priv_bytes);
                            estimated = estimated.saturating_add(priv_bytes);
                        } else {
                            estimated = estimated.saturating_add(m.allocation.exclusive_bytes);
                        }
                        upper = upper.saturating_add(m.allocation.allocated_bytes);
                    }
                    PhysicalRelation::Independent
                    | PhysicalRelation::HardlinkAliasInGroup
                    | PhysicalRelation::UnknownShared => {
                        confirmed = confirmed.saturating_add(m.allocation.allocated_bytes);
                        estimated = estimated.saturating_add(m.allocation.allocated_bytes);
                        upper = upper.saturating_add(m.allocation.allocated_bytes);
                    }
                }
            }
        }

        Self {
            confirmed_reclaimable_bytes: confirmed,
            estimated_reclaimable_bytes: estimated,
            upper_bound_reclaimable_bytes: upper,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub logical_size: u64,
    pub full_digest: String,
    pub members: Vec<DuplicateMember>,

    pub logical_duplicate_bytes: u64,

    pub confirmed_reclaimable_bytes: u64,
    pub estimated_reclaimable_bytes: u64,
    pub upper_bound_reclaimable_bytes: u64,

    pub physical_sharing_state: PhysicalRelation,
}

impl DuplicateGroup {
    /// Constructs a DuplicateGroup with deterministic ID and scientifically correct
    /// physical reclaim calculations.
    pub fn new(logical_size: u64, full_digest: String, mut members: Vec<DuplicateMember>) -> Self {
        // Deterministic sort of members: by path
        members.sort_by(|a, b| a.path.cmp(&b.path));

        // Generate stable group ID: dup-<first 16 hex chars of blake3(full_digest + size)>
        let mut hasher = blake3::Hasher::new();
        hasher.update(full_digest.as_bytes());
        hasher.update(&logical_size.to_le_bytes());
        let id_hash = hasher.finalize().to_hex();
        let group_id = format!("dup-{}", &id_hash[..16]);

        let member_count = members.len();
        let logical_duplicate_bytes = if member_count > 1 {
            (member_count as u64 - 1).saturating_mul(logical_size)
        } else {
            0
        };

        // Determine deterministic suggest-keep member first
        let keep_idx = Self::choose_suggested_keep_index(&members);
        for (i, m) in members.iter_mut().enumerate() {
            if i == keep_idx {
                m.is_suggested_keep = true;
                m.suggest_keep_reason = Some(
                    "Deterministic selection: highest protection / oldest modification / canonical location"
                        .to_string(),
                );
            } else {
                m.is_suggested_keep = false;
                m.suggest_keep_reason = None;
            }
        }

        // Determine per-member physical relation relative to other members and filesystem facts
        for i in 0..members.len() {
            let m_inode = (members[i].device_id, members[i].inode);
            let m_nlink = members[i].nlink;
            let m_clone = members[i].clone_id;
            let m_clone_refcnt = members[i].clone_refcnt;

            let same_inode_visible_count = members
                .iter()
                .filter(|o| (o.device_id, o.inode) == m_inode)
                .count();

            let same_clone_visible_count = if let Some(cid) = m_clone {
                if cid > 0 {
                    members.iter().filter(|o| o.clone_id == Some(cid)).count()
                } else {
                    0
                }
            } else {
                0
            };

            members[i].physical_relation = if m_nlink as usize > same_inode_visible_count {
                PhysicalRelation::HardlinkSharedExternal
            } else if same_inode_visible_count > 1 {
                PhysicalRelation::HardlinkAliasInGroup
            } else if m_clone_refcnt > 1 {
                if m_clone_refcnt as usize > same_clone_visible_count
                    || same_clone_visible_count <= 1
                {
                    PhysicalRelation::APFSCloneSharedExternal
                } else {
                    PhysicalRelation::APFSCloneInGroup
                }
            } else {
                PhysicalRelation::Independent
            };
        }

        // Determine overall group physical sharing state
        let any_hardlink_ext = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::HardlinkSharedExternal);
        let any_hardlink_grp = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::HardlinkAliasInGroup);
        let any_clone_ext = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::APFSCloneSharedExternal);
        let any_clone_grp = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::APFSCloneInGroup);

        let all_hardlink = members.iter().all(|m| {
            m.physical_relation == PhysicalRelation::HardlinkAliasInGroup
                || m.physical_relation == PhysicalRelation::HardlinkSharedExternal
        });
        let all_clone = members.iter().all(|m| {
            m.physical_relation == PhysicalRelation::APFSCloneInGroup
                || m.physical_relation == PhysicalRelation::APFSCloneSharedExternal
        });

        let physical_sharing_state = if any_hardlink_ext {
            PhysicalRelation::HardlinkSharedExternal
        } else if all_hardlink {
            PhysicalRelation::HardlinkAliasInGroup
        } else if any_clone_ext {
            PhysicalRelation::APFSCloneSharedExternal
        } else if all_clone {
            PhysicalRelation::APFSCloneInGroup
        } else if any_hardlink_grp || any_clone_grp {
            PhysicalRelation::UnknownShared
        } else {
            PhysicalRelation::Independent
        };

        // Granular physical reclaim calculation across all non-kept members based on suggested keep
        let remove_paths: Vec<PathBuf> = members
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != keep_idx)
            .map(|(_, m)| m.path.clone())
            .collect();

        let initial_estimate = DuplicatePlanEstimate::calculate_reclaim(
            &members,
            &members[keep_idx].path,
            &remove_paths,
        );

        Self {
            group_id,
            logical_size,
            full_digest,
            members,
            logical_duplicate_bytes,
            confirmed_reclaimable_bytes: initial_estimate.confirmed_reclaimable_bytes,
            estimated_reclaimable_bytes: initial_estimate.estimated_reclaimable_bytes,
            upper_bound_reclaimable_bytes: initial_estimate.upper_bound_reclaimable_bytes,
            physical_sharing_state,
        }
    }

    /// Deterministically picks the best candidate member to keep:
    /// 1. Higher risk level (e.g. Protected / Review preferred over Safe / Unknown)
    /// 2. Shorter path length / canonical appearance
    /// 3. Earlier modification time
    fn choose_suggested_keep_index(members: &[DuplicateMember]) -> usize {
        if members.is_empty() {
            return 0;
        }

        let mut best_idx = 0;
        for i in 1..members.len() {
            let curr = &members[i];
            let best = &members[best_idx];

            let curr_risk_rank = Self::risk_priority(&curr.risk);
            let best_risk_rank = Self::risk_priority(&best.risk);

            if curr_risk_rank > best_risk_rank {
                best_idx = i;
            } else if curr_risk_rank == best_risk_rank {
                let curr_len = curr.path.as_os_str().len();
                let best_len = best.path.as_os_str().len();
                if curr_len < best_len || (curr_len == best_len && curr.mtime_sec < best.mtime_sec)
                {
                    best_idx = i;
                }
            }
        }

        best_idx
    }

    fn risk_priority(risk: &RiskLevel) -> u8 {
        match risk {
            RiskLevel::Protected => 5,
            RiskLevel::Review => 4,
            RiskLevel::Caution => 3,
            RiskLevel::Safe => 2,
            RiskLevel::Unknown => 1,
        }
    }

    /// Verifies if a given path is a valid member of this duplicate group.
    pub fn find_member(&self, path: &Path) -> Option<&DuplicateMember> {
        self.members.iter().find(|m| m.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_member(
        path: &str,
        dev: u64,
        ino: u64,
        alloc: u64,
        clone_id: Option<u64>,
        nlink: u64,
    ) -> DuplicateMember {
        DuplicateMember {
            path: PathBuf::from(path),
            device_id: dev,
            inode: ino,
            allocation: AllocationInfo::new_with_clone(
                alloc,
                alloc,
                clone_id,
                if clone_id.is_some() { 2 } else { 1 },
                clone_id.is_some(),
                false,
            ),
            clone_id,
            clone_refcnt: if clone_id.is_some() { 2 } else { 1 },
            nlink,
            risk: RiskLevel::Review,
            category: "test".to_string(),
            mtime_sec: 100,
            mtime_nsec: 0,
            ctime_sec: 100,
            ctime_nsec: 0,
            physical_relation: PhysicalRelation::Independent,
            is_suggested_keep: false,
            suggest_keep_reason: None,
        }
    }

    #[test]
    fn test_hardlink_members_have_zero_reclaimable_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 100, 1024 * 1024, None, 2);
        let m2 = dummy_member("/tmp/b", 1, 100, 1024 * 1024, None, 2);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(
            group.physical_sharing_state,
            PhysicalRelation::HardlinkAliasInGroup
        );
        assert_eq!(group.confirmed_reclaimable_bytes, 0);
        assert_eq!(group.estimated_reclaimable_bytes, 0);
        assert_eq!(group.upper_bound_reclaimable_bytes, 0);
        assert_eq!(group.logical_duplicate_bytes, 1024 * 1024);
    }

    #[test]
    fn test_external_hardlink_detected() {
        // nlink is 3, but only 1 visible in group
        let m1 = dummy_member("/tmp/a", 1, 100, 1024 * 1024, None, 3);
        let m2 = dummy_member("/tmp/b", 1, 101, 1024 * 1024, None, 1);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(
            group.members[0].physical_relation,
            PhysicalRelation::HardlinkSharedExternal
        );
    }

    #[test]
    fn test_apfs_clone_members_have_conservative_reclaimable_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 101, 1024 * 1024, Some(555), 1);
        let m2 = dummy_member("/tmp/b", 1, 102, 1024 * 1024, Some(555), 1);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(
            group.physical_sharing_state,
            PhysicalRelation::APFSCloneInGroup
        );
        assert_eq!(group.confirmed_reclaimable_bytes, 0);
        assert_eq!(group.upper_bound_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.logical_duplicate_bytes, 1024 * 1024);
    }

    #[test]
    fn test_independent_copies_reclaim_full_allocated_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 101, 1024 * 1024, None, 1);
        let m2 = dummy_member("/tmp/b", 1, 102, 1024 * 1024, None, 1);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(group.physical_sharing_state, PhysicalRelation::Independent);
        assert_eq!(group.confirmed_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.estimated_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.upper_bound_reclaimable_bytes, 1024 * 1024);
    }
}
