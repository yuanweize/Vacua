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
    pub process_running: bool,
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

    /// Discover application bundles and system residue across standard macOS locations.
    pub fn build_from_system() -> Self {
        let mut graph = Self::new();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/Users/unknown"));

        // 1. Scan Application Bundles (/Applications, ~/Applications, /System/Applications)
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

        // 2. Discover Package Receipts from /var/db/receipts
        let receipts_dir = PathBuf::from("/var/db/receipts");
        if let Ok(entries) = std::fs::read_dir(receipts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let fname = path.file_name().unwrap_or_default().to_string_lossy();
                if fname.ends_with(".bom") || fname.ends_with(".plist") {
                    let pkg_id = fname
                        .trim_end_matches(".bom")
                        .trim_end_matches(".plist")
                        .to_string();
                    let node_id = format!("receipt:{}", pkg_id);
                    graph.add_node(GraphNode {
                        id: node_id.clone(),
                        kind: NodeKind::PackageReceipt,
                        path: Some(path),
                        metadata: HashMap::from([("package_id".into(), pkg_id.clone())]),
                    });
                    let bid_node = format!("bid:{}", pkg_id);
                    graph.add_edge(GraphEdge {
                        source_id: node_id,
                        target_id: bid_node,
                        kind: EdgeKind::ReceiptFor,
                        confidence: 0.95,
                        reason: "Apple package receipt registered in /var/db/receipts".into(),
                    });
                }
            }
        }

        // 3. Discover LaunchAgents & LaunchDaemons
        let launch_dirs = [
            (home.join("Library/LaunchAgents"), NodeKind::LaunchAgent),
            (
                PathBuf::from("/Library/LaunchAgents"),
                NodeKind::LaunchAgent,
            ),
            (
                PathBuf::from("/Library/LaunchDaemons"),
                NodeKind::LaunchDaemon,
            ),
        ];

        for (ldir, kind) in &launch_dirs {
            if let Ok(entries) = std::fs::read_dir(ldir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("plist") {
                        let label = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        let node_id = format!("launch:{}", path.display());
                        graph.add_node(GraphNode {
                            id: node_id.clone(),
                            kind: kind.clone(),
                            path: Some(path),
                            metadata: HashMap::from([("label".into(), label.clone())]),
                        });

                        let bid_node = format!("bid:{}", label);
                        graph.add_edge(GraphEdge {
                            source_id: node_id,
                            target_id: bid_node,
                            kind: EdgeKind::LaunchedBy,
                            confidence: 0.90,
                            reason: format!(
                                "Launch plist service registered under {}",
                                ldir.display()
                            ),
                        });
                    }
                }
            }
        }

        // 4. Discover Preferences (~/Library/Preferences/*.plist)
        let pref_dir = home.join("Library/Preferences");
        if let Ok(entries) = std::fs::read_dir(pref_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("plist") {
                    let bid = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let node_id = format!("pref:{}", path.display());
                    graph.add_node(GraphNode {
                        id: node_id.clone(),
                        kind: NodeKind::Preferences,
                        path: Some(path),
                        metadata: HashMap::from([("bundle_id".into(), bid.clone())]),
                    });

                    let bid_node = format!("bid:{}", bid);
                    graph.add_edge(GraphEdge {
                        source_id: node_id,
                        target_id: bid_node,
                        kind: EdgeKind::AssociatedWith,
                        confidence: 0.85,
                        reason: "User domain preference plist".into(),
                    });
                }
            }
        }

        // 5. Scan Application Support, Containers, Caches, Saved Application State
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

    /// Evaluates if a given bundle ID represents an uninstalled orphan application using multi-signal scoring.
    pub fn evaluate_orphan(&self, bundle_id: &str) -> OrphanEvaluation {
        let bundle_key = format!("bundle:{}", bundle_id);
        let bundle_node = self.nodes.get(&bundle_key);
        let bundle_installed = bundle_node.is_some();
        let bundle_path = bundle_node.and_then(|n| n.path.clone());

        let bid_key = format!("bid:{}", bundle_id);
        let mut associated_paths = Vec::new();
        let mut total_bytes = 0u64;

        let mut receipt_present = false;
        let mut launch_agents_present = 0usize;
        let mut has_app_support_or_container = false;
        let mut has_preferences = false;
        let mut has_saved_state = false;
        let mut has_cache = false;

        for edge in &self.edges {
            if edge.target_id == bid_key {
                if let Some(node) = self.nodes.get(&edge.source_id) {
                    match node.kind {
                        NodeKind::PackageReceipt => receipt_present = true,
                        NodeKind::LaunchAgent | NodeKind::LaunchDaemon => {
                            launch_agents_present += 1;
                        }
                        NodeKind::ApplicationSupport
                        | NodeKind::Container
                        | NodeKind::GroupContainer => {
                            has_app_support_or_container = true;
                        }
                        NodeKind::Preferences => has_preferences = true,
                        NodeKind::SavedState => has_saved_state = true,
                        NodeKind::Cache => has_cache = true,
                        _ => {}
                    }

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

        let app_name = bundle_node
            .and_then(|n| n.metadata.get("name").cloned())
            .unwrap_or_else(|| bundle_id.to_string());

        let process_running = is_process_running(bundle_id, &app_name);

        // Deterministic multi-factor scoring
        let mut orphan_score: i32 = 0;
        if bundle_installed {
            orphan_score -= 100;
        }
        if process_running {
            orphan_score -= 100;
        }
        if launch_agents_present > 0 {
            orphan_score -= 50;
        }
        if receipt_present {
            orphan_score -= 30;
        }

        if !bundle_installed && !process_running {
            if has_app_support_or_container {
                orphan_score += 40;
            }
            if has_preferences {
                orphan_score += 15;
            }
            if has_saved_state {
                orphan_score += 15;
            }
            if has_cache {
                orphan_score += 10;
            }
            if !associated_paths.is_empty() {
                orphan_score += 10;
            }
        }

        let orphan_confidence = if orphan_score >= 50 {
            OrphanConfidence::High
        } else if orphan_score >= 25 {
            OrphanConfidence::Medium
        } else if orphan_score > 0 {
            OrphanConfidence::Low
        } else {
            OrphanConfidence::NotOrphan
        };

        // Safety invariant: Active or installed apps are Protected. Uninstalled residue is REVIEW (never Safe).
        let risk_level = if bundle_installed || process_running {
            RiskLevel::Protected
        } else if orphan_confidence != OrphanConfidence::NotOrphan {
            RiskLevel::Review
        } else {
            RiskLevel::Protected
        };

        let explanation = if bundle_installed {
            format!(
                "Application bundle is currently installed at {:?}",
                bundle_path
            )
        } else if process_running {
            format!(
                "Process for '{}' is actively running in macOS memory",
                app_name
            )
        } else if orphan_confidence != OrphanConfidence::NotOrphan {
            format!(
                "Application bundle '{}' is absent, but {} residue signals remain ({} bytes, receipt: {}, launch_agents: {})",
                bundle_id,
                associated_paths.len(),
                total_bytes,
                receipt_present,
                launch_agents_present
            )
        } else {
            "No active residue signals found".to_string()
        };

        OrphanEvaluation {
            bundle_id: bundle_id.to_string(),
            app_name,
            bundle_installed,
            bundle_path,
            process_running,
            receipt_present,
            launch_agents_present,
            associated_artifacts_bytes: total_bytes,
            artifact_paths: associated_paths,
            orphan_confidence,
            risk_level,
            explanation,
        }
    }
}

/// Helper to check if a process matching bundle ID or app name is running.
fn is_process_running(bundle_id: &str, app_name: &str) -> bool {
    if let Ok(output) = std::process::Command::new("/bin/ps")
        .args(["-axo", "comm"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line_trimmed = line.trim();
            if !line_trimmed.is_empty()
                && (line_trimmed.contains(bundle_id)
                    || (!app_name.is_empty() && line_trimmed.contains(app_name)))
            {
                return true;
            }
        }
    }
    false
}

/// Helper to read CFBundleIdentifier from Info.plist of an app bundle.
fn read_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    if !plist_path.exists() {
        return None;
    }
    if let Ok(content) = std::fs::read_to_string(&plist_path) {
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

    #[test]
    fn test_multi_signal_graph_discovery() {
        let mut graph = ApplicationEvidenceGraph::new();

        // Add receipt and launch agent nodes
        graph.add_node(GraphNode {
            id: "receipt:com.test.daemon".into(),
            kind: NodeKind::PackageReceipt,
            path: None,
            metadata: HashMap::new(),
        });
        graph.add_edge(GraphEdge {
            source_id: "receipt:com.test.daemon".into(),
            target_id: "bid:com.test.daemon".into(),
            kind: EdgeKind::ReceiptFor,
            confidence: 0.95,
            reason: "Registered package receipt".into(),
        });

        graph.add_node(GraphNode {
            id: "launch:com.test.daemon".into(),
            kind: NodeKind::LaunchDaemon,
            path: None,
            metadata: HashMap::new(),
        });
        graph.add_edge(GraphEdge {
            source_id: "launch:com.test.daemon".into(),
            target_id: "bid:com.test.daemon".into(),
            kind: EdgeKind::LaunchedBy,
            confidence: 0.90,
            reason: "Launch daemon plist".into(),
        });

        let eval = graph.evaluate_orphan("com.test.daemon");
        assert!(eval.receipt_present);
        assert_eq!(eval.launch_agents_present, 1);
        // Because of launch agent and receipt without artifacts, score <= 0 -> NotOrphan
        assert_eq!(eval.orphan_confidence, OrphanConfidence::NotOrphan);
    }
}
