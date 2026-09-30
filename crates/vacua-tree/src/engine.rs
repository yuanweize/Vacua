use crate::error::{Result, TreeError};
use crate::id::StorageNodeId;
use crate::model::*;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use vacua_index::StorageSnapshot;

pub struct StorageTreeEngine;

impl StorageTreeEngine {
    /// Atomically publishes a newly built tree generation into the database.
    pub fn publish_generation(
        conn: &mut Connection,
        generation: &StorageTreeGeneration,
        nodes: &[StorageTreeNode],
    ) -> Result<()> {
        let tx = conn.transaction()?;

        let coverage_json = serde_json::to_string(&generation.coverage)?;
        let source_str = match generation.source {
            StorageTreeSource::Indexed => "Indexed",
            StorageTreeSource::LiveScan => "LiveScan",
            StorageTreeSource::IncrementalRefresh => "IncrementalRefresh",
        };

        // 1. Insert generation record with status Building
        tx.execute(
            r#"
            INSERT INTO storage_tree_generations (
                generation_id, root_path, root_id, observed_at,
                status, source, total_files, total_dirs,
                total_logical_bytes, total_allocated_bytes, coverage_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                generation.generation_id,
                generation.root_path.to_string_lossy().to_string(),
                generation.root_id,
                generation.observed_at,
                "Building",
                source_str,
                generation.total_files,
                generation.total_dirs,
                generation.total_logical_bytes,
                generation.total_allocated_bytes,
                coverage_json,
            ],
        )?;

        // 2. Insert all tree nodes
        {
            let mut stmt = tx.prepare_cached(
                r#"
                INSERT INTO storage_tree_nodes (
                    generation_id, node_id, parent_node_id, raw_relative_path,
                    display_name, display_path, kind, depth,
                    direct_logical_bytes, direct_allocated_bytes,
                    subtree_logical_bytes, subtree_allocated_bytes,
                    file_count, directory_count, hardlink_alias_count,
                    is_hardlink_alias, child_count, mtime_sec
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
                "#,
            )?;

            for node in nodes {
                let kind_str = match node.kind {
                    StorageNodeKind::Root => "root",
                    StorageNodeKind::Directory => "directory",
                    StorageNodeKind::File => "file",
                };
                let parent_id_str = node.parent_id.as_ref().map(|p| p.as_str());

                stmt.execute(params![
                    generation.generation_id,
                    node.node_id.as_str(),
                    parent_id_str,
                    node.raw_relative_path,
                    node.display_name,
                    node.display_path,
                    kind_str,
                    node.depth,
                    node.direct_logical_bytes,
                    node.direct_allocated_bytes,
                    node.subtree_logical_bytes,
                    node.subtree_allocated_bytes,
                    node.file_count,
                    node.directory_count,
                    node.hardlink_alias_count,
                    if node.is_hardlink_alias { 1 } else { 0 },
                    node.child_count,
                    node.mtime_sec,
                ])?;
            }
        }

        // 3. Update status to Ready and commit atomically
        tx.execute(
            "UPDATE storage_tree_generations SET status = 'Ready' WHERE generation_id = ?1",
            params![generation.generation_id],
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Prunes older tree generations for `root_id`, keeping only the latest `keep_count` generations.
    pub fn prune_older_generations(
        conn: &mut Connection,
        root_id: &str,
        keep_count: usize,
    ) -> Result<usize> {
        let mut stmt = conn.prepare(
            r#"
            SELECT generation_id FROM storage_tree_generations
            WHERE root_id = ?1 AND status = 'Ready'
            ORDER BY observed_at DESC, rowid DESC
            LIMIT -1 OFFSET ?2
            "#,
        )?;

        let old_gen_ids: Vec<String> = stmt
            .query_map(params![root_id, keep_count as i64], |row| row.get(0))?
            .filter_map(std::result::Result::ok)
            .collect();
        drop(stmt);

        if old_gen_ids.is_empty() {
            return Ok(0);
        }

        let tx = conn.transaction()?;
        for gen_id in &old_gen_ids {
            tx.execute(
                "DELETE FROM storage_tree_nodes WHERE generation_id = ?1",
                params![gen_id],
            )?;
            tx.execute(
                "DELETE FROM storage_tree_generations WHERE generation_id = ?1",
                params![gen_id],
            )?;
        }
        tx.commit()?;

        Ok(old_gen_ids.len())
    }

    /// Retrieves the latest ready generation for a given root.
    pub fn get_latest_ready_generation(
        conn: &Connection,
        root_id: &str,
    ) -> Result<Option<StorageTreeGeneration>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT generation_id, root_path, root_id, observed_at, source,
                   total_files, total_dirs, total_logical_bytes, total_allocated_bytes, coverage_json
            FROM storage_tree_generations
            WHERE root_id = ?1 AND status = 'Ready'
            ORDER BY observed_at DESC, rowid DESC
            LIMIT 1
            "#,
        )?;

        let mut rows = stmt.query(params![root_id])?;
        if let Some(row) = rows.next()? {
            let gen = parse_generation_row(row)?;
            Ok(Some(gen))
        } else {
            Ok(None)
        }
    }

    /// Retrieves a specific generation by ID.
    pub fn get_generation(conn: &Connection, generation_id: &str) -> Result<StorageTreeGeneration> {
        let mut stmt = conn.prepare(
            r#"
            SELECT generation_id, root_path, root_id, observed_at, source,
                   total_files, total_dirs, total_logical_bytes, total_allocated_bytes, coverage_json
            FROM storage_tree_generations
            WHERE generation_id = ?1 AND status = 'Ready'
            "#,
        )?;

        let mut rows = stmt.query(params![generation_id])?;
        if let Some(row) = rows.next()? {
            let gen = parse_generation_row(row)?;
            Ok(gen)
        } else {
            Err(TreeError::GenerationNotFound(generation_id.to_string()))
        }
    }

    /// Queries bounded children of a given parent node with deterministic remainder accounting.
    pub fn query_children_page(
        conn: &Connection,
        generation_id: &str,
        parent_node_id: &StorageNodeId,
        metric: StorageTreeMetric,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<StorageTreeNode>, StorageTreeRemainder, u64)> {
        // First verify generation exists and is Ready
        Self::get_generation(conn, generation_id)?;

        // Query total count and metric totals for all direct children
        let (total_child_count, total_logical, total_allocated): (u64, u64, u64) = conn.query_row(
            r#"
            SELECT count(*),
                   coalesce(sum(subtree_logical_bytes), 0),
                   coalesce(sum(subtree_allocated_bytes), 0)
            FROM storage_tree_nodes
            WHERE generation_id = ?1 AND parent_node_id = ?2
            "#,
            params![generation_id, parent_node_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Order clause depending on metric
        let query_sql = match metric {
            StorageTreeMetric::Allocated => {
                r#"
                SELECT node_id, parent_node_id, raw_relative_path, display_name, display_path,
                       kind, depth, direct_logical_bytes, direct_allocated_bytes,
                       subtree_logical_bytes, subtree_allocated_bytes, file_count, directory_count,
                       hardlink_alias_count, is_hardlink_alias, child_count, mtime_sec
                FROM storage_tree_nodes
                WHERE generation_id = ?1 AND parent_node_id = ?2
                ORDER BY subtree_allocated_bytes DESC, display_name ASC, node_id ASC
                LIMIT ?3 OFFSET ?4
            "#
            }
            StorageTreeMetric::Logical => {
                r#"
                SELECT node_id, parent_node_id, raw_relative_path, display_name, display_path,
                       kind, depth, direct_logical_bytes, direct_allocated_bytes,
                       subtree_logical_bytes, subtree_allocated_bytes, file_count, directory_count,
                       hardlink_alias_count, is_hardlink_alias, child_count, mtime_sec
                FROM storage_tree_nodes
                WHERE generation_id = ?1 AND parent_node_id = ?2
                ORDER BY subtree_logical_bytes DESC, display_name ASC, node_id ASC
                LIMIT ?3 OFFSET ?4
            "#
            }
        };

        let mut stmt = conn.prepare(query_sql)?;
        let children: Vec<StorageTreeNode> = stmt
            .query_map(
                params![generation_id, parent_node_id.as_str(), limit, offset],
                parse_node_row,
            )?
            .filter_map(std::result::Result::ok)
            .collect();

        let returned_logical: u64 = children.iter().map(|c| c.subtree_logical_bytes).sum();
        let returned_allocated: u64 = children.iter().map(|c| c.subtree_allocated_bytes).sum();
        let returned_count = children.len() as u64;

        let remainder = StorageTreeRemainder {
            item_count: total_child_count.saturating_sub(returned_count),
            logical_bytes: total_logical.saturating_sub(returned_logical),
            allocated_bytes: total_allocated.saturating_sub(returned_allocated),
        };

        Ok((children, remainder, total_child_count))
    }

    /// Retrieves a single node by ID.
    pub fn get_node(
        conn: &Connection,
        generation_id: &str,
        node_id: &StorageNodeId,
    ) -> Result<Option<StorageTreeNode>> {
        let mut stmt = conn.prepare(
            r#"
            SELECT node_id, parent_node_id, raw_relative_path, display_name, display_path,
                   kind, depth, direct_logical_bytes, direct_allocated_bytes,
                   subtree_logical_bytes, subtree_allocated_bytes, file_count, directory_count,
                   hardlink_alias_count, is_hardlink_alias, child_count, mtime_sec
            FROM storage_tree_nodes
            WHERE generation_id = ?1 AND node_id = ?2
            "#,
        )?;

        let mut rows = stmt.query(params![generation_id, node_id.as_str()])?;
        if let Some(row) = rows.next()? {
            let node = parse_node_row(row)?;
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Computes delta between current tree node and a baseline StorageSnapshot.
    pub fn compare_with_snapshot(
        generation: &StorageTreeGeneration,
        snapshot: &StorageSnapshot,
        node: &StorageTreeNode,
    ) -> Result<StorageTreeDelta> {
        let gen_root_str = generation.root_path.to_string_lossy();
        let snap_root_str = snapshot.root_path.to_string_lossy();

        if gen_root_str != snap_root_str {
            return Err(TreeError::SnapshotRootMismatch {
                tree_root: gen_root_str.to_string(),
                snapshot_root: snap_root_str.to_string(),
            });
        }

        // Lookup node in snapshot subtrees
        let rel_path_str = String::from_utf8_lossy(&node.raw_relative_path);
        let maybe_baseline = snapshot
            .subtrees
            .get(rel_path_str.as_ref())
            .or_else(|| snapshot.subtrees.get(&node.display_path));

        let delta = if let Some(baseline) = maybe_baseline {
            let alloc_delta = node.subtree_allocated_bytes as i64 - baseline.allocated_bytes as i64;
            let log_delta = node.subtree_logical_bytes as i64 - baseline.logical_bytes as i64;
            let file_delta = node.file_count as i64 - baseline.file_count as i64;

            let change_kind = if alloc_delta > 0 || log_delta > 0 {
                TreeChangeKind::Grown
            } else if alloc_delta < 0 || log_delta < 0 {
                TreeChangeKind::Shrunk
            } else {
                TreeChangeKind::Unchanged
            };

            StorageTreeDelta {
                node_id: node.node_id.clone(),
                allocated_delta_bytes: alloc_delta,
                logical_delta_bytes: log_delta,
                file_count_delta: file_delta,
                change_kind,
            }
        } else {
            // New node not present in baseline snapshot
            StorageTreeDelta {
                node_id: node.node_id.clone(),
                allocated_delta_bytes: node.subtree_allocated_bytes as i64,
                logical_delta_bytes: node.subtree_logical_bytes as i64,
                file_count_delta: node.file_count as i64,
                change_kind: TreeChangeKind::New,
            }
        };

        Ok(delta)
    }
}

fn parse_generation_row(row: &rusqlite::Row) -> rusqlite::Result<StorageTreeGeneration> {
    let generation_id: String = row.get(0)?;
    let root_path_str: String = row.get(1)?;
    let root_id: String = row.get(2)?;
    let observed_at: i64 = row.get(3)?;
    let source_str: String = row.get(4)?;
    let total_files: u64 = row.get(5)?;
    let total_dirs: u64 = row.get(6)?;
    let total_logical_bytes: u64 = row.get(7)?;
    let total_allocated_bytes: u64 = row.get(8)?;
    let coverage_json: String = row.get(9)?;

    let source = match source_str.as_str() {
        "LiveScan" => StorageTreeSource::LiveScan,
        "IncrementalRefresh" => StorageTreeSource::IncrementalRefresh,
        _ => StorageTreeSource::Indexed,
    };

    let coverage: StorageTreeCoverage = serde_json::from_str(&coverage_json).unwrap_or_default();

    Ok(StorageTreeGeneration {
        generation_id,
        root_path: PathBuf::from(root_path_str),
        root_id,
        observed_at,
        source,
        status: StorageTreeStatus::Ready,
        total_files,
        total_dirs,
        total_logical_bytes,
        total_allocated_bytes,
        coverage,
    })
}

fn parse_node_row(row: &rusqlite::Row) -> rusqlite::Result<StorageTreeNode> {
    let node_id_str: String = row.get(0)?;
    let parent_id_str: Option<String> = row.get(1)?;
    let raw_relative_path: Vec<u8> = row.get(2)?;
    let display_name: String = row.get(3)?;
    let display_path: String = row.get(4)?;
    let kind_str: String = row.get(5)?;
    let depth: u32 = row.get(6)?;
    let direct_logical_bytes: u64 = row.get(7)?;
    let direct_allocated_bytes: u64 = row.get(8)?;
    let subtree_logical_bytes: u64 = row.get(9)?;
    let subtree_allocated_bytes: u64 = row.get(10)?;
    let file_count: u64 = row.get(11)?;
    let directory_count: u64 = row.get(12)?;
    let hardlink_alias_count: u64 = row.get(13)?;
    let is_hardlink_alias_int: i64 = row.get(14)?;
    let child_count: u64 = row.get(15)?;
    let mtime_sec: i64 = row.get(16)?;

    let kind = match kind_str.as_str() {
        "root" => StorageNodeKind::Root,
        "file" => StorageNodeKind::File,
        _ => StorageNodeKind::Directory,
    };

    Ok(StorageTreeNode {
        node_id: StorageNodeId(node_id_str),
        parent_id: parent_id_str.map(StorageNodeId),
        raw_relative_path,
        display_name,
        display_path,
        kind,
        depth,
        direct_logical_bytes,
        direct_allocated_bytes,
        subtree_logical_bytes,
        subtree_allocated_bytes,
        file_count,
        directory_count,
        hardlink_alias_count,
        is_hardlink_alias: is_hardlink_alias_int != 0,
        child_count,
        mtime_sec,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::StorageTreeBuilder;
    use std::path::Path;
    use vacua_index::run_migrations;

    #[test]
    fn test_publish_query_and_pruning_flow() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();

        let root = Path::new("/tmp/test_workspace");
        let root_id = "root_test";
        let builder = StorageTreeBuilder::new(root, root_id);

        let entries = vec![
            crate::builder::RawTreeEntry {
                relative_path: PathBuf::from("docs/report.pdf"),
                is_dir: false,
                logical_bytes: 1000,
                allocated_bytes: 4096,
                device_id: 1,
                inode: 10,
                nlink: 1,
                mtime_sec: 100,
            },
            crate::builder::RawTreeEntry {
                relative_path: PathBuf::from("docs/notes.txt"),
                is_dir: false,
                logical_bytes: 200,
                allocated_bytes: 4096,
                device_id: 1,
                inode: 11,
                nlink: 1,
                mtime_sec: 100,
            },
            crate::builder::RawTreeEntry {
                relative_path: PathBuf::from("src/main.rs"),
                is_dir: false,
                logical_bytes: 500,
                allocated_bytes: 4096,
                device_id: 1,
                inode: 12,
                nlink: 1,
                mtime_sec: 100,
            },
        ];

        let nodes = builder.build_from_raw_entries(entries).unwrap();
        let root_node = nodes
            .iter()
            .find(|n| n.kind == StorageNodeKind::Root)
            .unwrap();

        let gen = StorageTreeGeneration {
            generation_id: "stg_test_1".to_string(),
            root_path: root.to_path_buf(),
            root_id: root_id.to_string(),
            observed_at: 1000,
            source: StorageTreeSource::LiveScan,
            status: StorageTreeStatus::Ready,
            total_files: root_node.file_count,
            total_dirs: root_node.directory_count,
            total_logical_bytes: root_node.subtree_logical_bytes,
            total_allocated_bytes: root_node.subtree_allocated_bytes,
            coverage: StorageTreeCoverage {
                files_observed: 3,
                directories_observed: 2,
                analysis_complete: true,
                ..Default::default()
            },
        };

        StorageTreeEngine::publish_generation(&mut conn, &gen, &nodes).unwrap();

        // Query root children (should be "docs" and "src")
        let root_id_obj = StorageNodeId::from_raw_relative(root_id, b"");
        let (children, remainder, total) = StorageTreeEngine::query_children_page(
            &conn,
            "stg_test_1",
            &root_id_obj,
            StorageTreeMetric::Allocated,
            1, // limit 1 to test remainder!
            0,
        )
        .unwrap();

        assert_eq!(total, 2);
        assert_eq!(children.len(), 1);
        assert_eq!(remainder.item_count, 1);

        // Exact remainder equality: returned + remainder == total direct children metrics
        assert_eq!(
            children[0].subtree_allocated_bytes + remainder.allocated_bytes,
            12288
        );
        assert_eq!(
            children[0].subtree_logical_bytes + remainder.logical_bytes,
            1700
        );

        // Test pruning
        let gen2 = StorageTreeGeneration {
            generation_id: "stg_test_2".to_string(),
            observed_at: 2000,
            ..gen.clone()
        };
        StorageTreeEngine::publish_generation(&mut conn, &gen2, &nodes).unwrap();

        let gen3 = StorageTreeGeneration {
            generation_id: "stg_test_3".to_string(),
            observed_at: 3000,
            ..gen.clone()
        };
        StorageTreeEngine::publish_generation(&mut conn, &gen3, &nodes).unwrap();

        // Keep 2, should prune 1 (stg_test_1)
        let pruned = StorageTreeEngine::prune_older_generations(&mut conn, root_id, 2).unwrap();
        assert_eq!(pruned, 1);

        // Verify stg_test_1 is gone
        assert!(StorageTreeEngine::get_generation(&conn, "stg_test_1").is_err());
        // Verify stg_test_2 and stg_test_3 remain
        assert!(StorageTreeEngine::get_generation(&conn, "stg_test_2").is_ok());
        assert!(StorageTreeEngine::get_generation(&conn, "stg_test_3").is_ok());
    }
}
