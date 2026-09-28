use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::risk::RiskLevel;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    ApplicationBundle,
    BundleIdentifier,
    FilesystemArtifact,
    PackageReceipt,
    LaunchAgent,
    LaunchDaemon,
    Container,
    GroupContainer,
    ApplicationSupport,
    Cache,
    Preferences,
    SavedState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub path: Option<PathBuf>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeKind {
    OwnedBy,
    AssociatedWith,
    GeneratedBy,
    ReceiptFor,
    LaunchedBy,
    ContainerOf,
    PossibleMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub kind: EdgeKind,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum OrphanConfidence {
    NotOrphan,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanEvaluation {
    pub bundle_id: String,
    pub app_name: String,
    pub bundle_installed: bool,
    pub bundle_path: Option<PathBuf>,
    pub receipt_present: bool,
    pub launch_agents_present: usize,
    pub associated_artifacts_bytes: u64,
    pub artifact_paths: Vec<PathBuf>,
    pub orphan_confidence: OrphanConfidence,
    pub risk_level: RiskLevel,
    pub explanation: String,
}

/// In-memory graph tracking dependencies, bundle ownership, and leftover artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplicationEvidenceGraph {
    pub nodes: HashMap<String, GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl ApplicationEvidenceGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Discover application bundles and system residue for standard macOS user directories.
    pub fn build_from_system() -> Self {
        let mut graph = Self::new();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/Users/unknown"));

        // 1. Scan /Applications for installed bundles
        let app_dirs = [
            PathBuf::from("/Applications"),
            home.join("Applications"),
            PathBuf::from("/System/Applications"),
        ];

        let mut installed_bundle_ids: HashSet<String> = HashSet::new();

        for app_dir in &app_dirs {
            if let Ok(entries) = std::fs::read_dir(app_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("app") {
                        let name = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let bundle_id = read_bundle_id(&path)
                            .unwrap_or_else(|| format!("com.apple.app.{}", name.to_lowercase()));
                        installed_bundle_ids.insert(bundle_id.clone());

                        let node_id = format!("bundle:{}", bundle_id);
                        graph.add_node(GraphNode {
                            id: node_id.clone(),
                            kind: NodeKind::ApplicationBundle,
                            path: Some(path),
                            metadata: HashMap::from([
                                ("name".into(), name),
                                ("bundle_id".into(), bundle_id.clone()),
                            ]),
                        });

                        let bid_node = format!("bid:{}", bundle_id);
                        graph.add_node(GraphNode {
                            id: bid_node.clone(),
                            kind: NodeKind::BundleIdentifier,
                            path: None,
                            metadata: HashMap::new(),
                        });

                        graph.add_edge(GraphEdge {
                            source_id: node_id,
                            target_id: bid_node,
                            kind: EdgeKind::AssociatedWith,
                            confidence: 1.0,
                            reason: "App bundle declares CFBundleIdentifier".into(),
                        });
                    }
                }
            }
        }

        // 2. Scan standard Application Support and Containers directories for residue
        let residue_roots = [
            (
                home.join("Library/Application Support"),
                NodeKind::ApplicationSupport,
            ),
            (home.join("Library/Containers"), NodeKind::Container),
            (
                home.join("Library/Group Containers"),
                NodeKind::GroupContainer,
            ),
            (home.join("Library/Caches"), NodeKind::Cache),
            (
                home.join("Library/Saved Application State"),
                NodeKind::SavedState,
            ),
        ];

        for (dir, kind) in &residue_roots {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if name.starts_with('.') {
                        continue;
                    }

                    let node_id = format!("artifact:{}", path.display());
                    let mut meta = HashMap::new();
                    meta.insert("name".into(), name.clone());

                    // Check if artifact name matches or contains known bundle IDs
                    let matched_bundle = installed_bundle_ids.iter().find(|bid| {
                        name.eq_ignore_ascii_case(bid)
                            || name.contains(bid.as_str())
                            || bid.contains(&name)
                    });

                    let target_bid = matched_bundle
                        .cloned()
                        .unwrap_or_else(|| format!("unknown.{}", name));

                    graph.add_node(GraphNode {
                        id: node_id.clone(),
                        kind: kind.clone(),
                        path: Some(path),
                        metadata: meta,
                    });

                    let bid_node = format!("bid:{}", target_bid);
                    let edge_kind = if *kind == NodeKind::Container {
                        EdgeKind::ContainerOf
                    } else {
                        EdgeKind::GeneratedBy
                    };

                    graph.add_edge(GraphEdge {
                        source_id: node_id,
                        target_id: bid_node,
                        kind: edge_kind,
                        confidence: if matched_bundle.is_some() { 0.95 } else { 0.60 },
                        reason: format!("Filesystem residue under {}", dir.display()),
                    });
                }
            }
        }

        graph
    }

    /// Evaluates if a given bundle ID represents an uninstalled orphan application.
    pub fn evaluate_orphan(&self, bundle_id: &str) -> OrphanEvaluation {
        let bundle_key = format!("bundle:{}", bundle_id);
        let bundle_node = self.nodes.get(&bundle_key);
        let bundle_installed = bundle_node.is_some();
        let bundle_path = bundle_node.and_then(|n| n.path.clone());

        let bid_key = format!("bid:{}", bundle_id);
        let mut associated_paths = Vec::new();
        let mut total_bytes = 0u64;

        for edge in &self.edges {
            if edge.target_id == bid_key {
                if let Some(node) = self.nodes.get(&edge.source_id) {
                    if let Some(ref p) = node.path {
                        if p.exists() {
                            associated_paths.push(p.clone());
                            if let Ok(meta) = std::fs::symlink_metadata(p) {
                                total_bytes = total_bytes.saturating_add(meta.len());
                            }
                        }
                    }
                }
            }
        }

        let orphan_confidence = if bundle_installed {
            OrphanConfidence::NotOrphan
        } else if !associated_paths.is_empty() {
            OrphanConfidence::High
        } else {
            OrphanConfidence::Low
        };

        let risk_level = if orphan_confidence == OrphanConfidence::High {
            RiskLevel::Review // Residual orphans require user review, never silent destruction
        } else if bundle_installed {
            RiskLevel::Protected // Installed app residue is protected by default
        } else {
            RiskLevel::Review
        };

        let explanation = if bundle_installed {
            format!(
                "Application bundle is currently installed at {:?}",
                bundle_path
            )
        } else if orphan_confidence == OrphanConfidence::High {
            format!(
                "Application bundle '{}' is uninstalled, but {} associated residue artifacts remain ({} bytes)",
                bundle_id,
                associated_paths.len(),
                total_bytes
            )
        } else {
            "No associated residue found".to_string()
        };

        let app_name = bundle_node
            .and_then(|n| n.metadata.get("name").cloned())
            .unwrap_or_else(|| bundle_id.to_string());

        OrphanEvaluation {
            bundle_id: bundle_id.to_string(),
            app_name,
            bundle_installed,
            bundle_path,
            receipt_present: false,
            launch_agents_present: 0,
            associated_artifacts_bytes: total_bytes,
            artifact_paths: associated_paths,
            orphan_confidence,
            risk_level,
            explanation,
        }
    }
}

/// Helper to read CFBundleIdentifier from Info.plist of an app bundle.
fn read_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }
    if let Ok(content) = std::fs::read_to_string(&plist_path) {
        // Lightweight XML / string parse for CFBundleIdentifier
        if let Some(pos) = content.find("CFBundleIdentifier") {
            let rest = &content[pos..];
            if let Some(start) = rest.find("<string>") {
                if let Some(end) = rest[start + 8..].find("</string>") {
                    let bid = &rest[start + 8..start + 8 + end];
                    return Some(bid.trim().to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orphan_evaluation_with_uninstalled_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let residue_path = dir.path().join("Application Support/com.example.oldapp");
        std::fs::create_dir_all(&residue_path).unwrap();
        std::fs::write(residue_path.join("data.db"), b"app state data").unwrap();

        let mut graph = ApplicationEvidenceGraph::new();

        // 1. Add bundle identifier node without an installed application bundle
        graph.add_node(GraphNode {
            id: "bid:com.example.oldapp".into(),
            kind: NodeKind::BundleIdentifier,
            path: None,
            metadata: HashMap::new(),
        });

        // 2. Add orphaned Application Support directory
        let node_id = format!("artifact:{}", residue_path.display());
        graph.add_node(GraphNode {
            id: node_id.clone(),
            kind: NodeKind::ApplicationSupport,
            path: Some(residue_path),
            metadata: HashMap::new(),
        });

        graph.add_edge(GraphEdge {
            source_id: node_id,
            target_id: "bid:com.example.oldapp".into(),
            kind: EdgeKind::GeneratedBy,
            confidence: 0.95,
            reason: "Residue directory matches bundle name".into(),
        });

        let eval = graph.evaluate_orphan("com.example.oldapp");
        assert!(!eval.bundle_installed);
        assert_eq!(eval.orphan_confidence, OrphanConfidence::High);
        assert_eq!(eval.risk_level, RiskLevel::Review);
        assert!(eval.associated_artifacts_bytes > 0);
    }
}
