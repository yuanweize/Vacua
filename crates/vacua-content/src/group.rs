use crate::identity::PhysicalRelation;
use serde::{Deserialize, Serialize};
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
    pub physical_relation: PhysicalRelation,
    pub is_suggested_keep: bool,
    pub suggest_keep_reason: Option<String>,
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

        // Determine per-member physical relation relative to other members
        for i in 0..members.len() {
            let m_inode = (members[i].device_id, members[i].inode);
            let m_clone = members[i].clone_id;
            let m_clone_refcnt = members[i].clone_refcnt;

            let is_hardlink = members
                .iter()
                .enumerate()
                .any(|(j, other)| i != j && (other.device_id, other.inode) == m_inode);

            let is_clone = if !is_hardlink && m_clone_refcnt > 1 {
                if let Some(cid) = m_clone {
                    cid > 0
                        && members
                            .iter()
                            .enumerate()
                            .any(|(j, other)| i != j && other.clone_id == Some(cid))
                } else {
                    false
                }
            } else {
                false
            };

            members[i].physical_relation = if is_hardlink {
                PhysicalRelation::HardlinkSameInode
            } else if is_clone {
                PhysicalRelation::APFSCloneFamily
            } else {
                PhysicalRelation::Independent
            };
        }

        // Determine overall group physical sharing state
        let any_hardlink = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::HardlinkSameInode);
        let any_clone = members
            .iter()
            .any(|m| m.physical_relation == PhysicalRelation::APFSCloneFamily);
        let all_hardlink = members
            .iter()
            .all(|m| m.physical_relation == PhysicalRelation::HardlinkSameInode);
        let all_clone = members
            .iter()
            .all(|m| m.physical_relation == PhysicalRelation::APFSCloneFamily);

        let physical_sharing_state = if all_hardlink {
            PhysicalRelation::HardlinkSameInode
        } else if all_clone {
            PhysicalRelation::APFSCloneFamily
        } else if any_hardlink || any_clone {
            PhysicalRelation::UnknownShared
        } else {
            PhysicalRelation::Independent
        };

        // Granular physical reclaim calculation across all non-kept members
        let mut confirmed_reclaimable_bytes = 0u64;
        let mut estimated_reclaimable_bytes = 0u64;
        let mut upper_bound_reclaimable_bytes = 0u64;

        for (i, m) in members.iter().enumerate() {
            if i == keep_idx {
                continue;
            }

            match m.physical_relation {
                PhysicalRelation::HardlinkSameInode => {
                    // Hardlinks sharing inode with kept member reclaim 0
                    // in confirmed, estimated, and upper bound.
                }
                PhysicalRelation::APFSCloneFamily => {
                    // APFS clone extents: confirmed 0, estimated is exclusive bytes,
                    // upper bound is allocated bytes.
                    upper_bound_reclaimable_bytes =
                        upper_bound_reclaimable_bytes.saturating_add(m.allocation.allocated_bytes);
                    estimated_reclaimable_bytes =
                        estimated_reclaimable_bytes.saturating_add(m.allocation.exclusive_bytes);
                }
                PhysicalRelation::Independent | PhysicalRelation::UnknownShared => {
                    upper_bound_reclaimable_bytes =
                        upper_bound_reclaimable_bytes.saturating_add(m.allocation.allocated_bytes);
                    confirmed_reclaimable_bytes =
                        confirmed_reclaimable_bytes.saturating_add(m.allocation.allocated_bytes);
                    estimated_reclaimable_bytes =
                        estimated_reclaimable_bytes.saturating_add(m.allocation.allocated_bytes);
                }
            }
        }

        Self {
            group_id,
            logical_size,
            full_digest,
            members,
            logical_duplicate_bytes,
            confirmed_reclaimable_bytes,
            estimated_reclaimable_bytes,
            upper_bound_reclaimable_bytes,
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
                // Prefer shorter path, then earlier modification time
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
            nlink: 1,
            risk: RiskLevel::Review,
            category: "test".to_string(),
            mtime_sec: 100,
            physical_relation: PhysicalRelation::Independent,
            is_suggested_keep: false,
            suggest_keep_reason: None,
        }
    }

    #[test]
    fn test_hardlink_members_have_zero_reclaimable_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 100, 1024 * 1024, None);
        let m2 = dummy_member("/tmp/b", 1, 100, 1024 * 1024, None);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(
            group.physical_sharing_state,
            PhysicalRelation::HardlinkSameInode
        );
        assert_eq!(group.confirmed_reclaimable_bytes, 0);
        assert_eq!(group.estimated_reclaimable_bytes, 0);
        assert_eq!(group.upper_bound_reclaimable_bytes, 0);
        assert_eq!(group.logical_duplicate_bytes, 1024 * 1024);
    }

    #[test]
    fn test_apfs_clone_members_have_conservative_reclaimable_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 101, 1024 * 1024, Some(555));
        let m2 = dummy_member("/tmp/b", 1, 102, 1024 * 1024, Some(555));

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(
            group.physical_sharing_state,
            PhysicalRelation::APFSCloneFamily
        );
        assert_eq!(group.confirmed_reclaimable_bytes, 0);
        assert_eq!(group.upper_bound_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.logical_duplicate_bytes, 1024 * 1024);
    }

    #[test]
    fn test_independent_copies_reclaim_full_allocated_bytes() {
        let m1 = dummy_member("/tmp/a", 1, 101, 1024 * 1024, None);
        let m2 = dummy_member("/tmp/b", 1, 102, 1024 * 1024, None);

        let group = DuplicateGroup::new(1024 * 1024, "abcd1234".to_string(), vec![m1, m2]);
        assert_eq!(group.physical_sharing_state, PhysicalRelation::Independent);
        assert_eq!(group.confirmed_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.estimated_reclaimable_bytes, 1024 * 1024);
        assert_eq!(group.upper_bound_reclaimable_bytes, 1024 * 1024);
    }
}
