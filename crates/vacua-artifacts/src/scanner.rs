use crate::ecosystem::DeveloperEcosystem;
use crate::evidence::{ActiveProcessGuard, ActiveProjectState, RebuildConfidence, RebuildEvidence};
use crate::id::{DeveloperArtifactId, DeveloperProjectId};
use crate::kind::DeveloperArtifactKind;
use crate::model::{
    ArtifactAnalysisGeneration, DeveloperArtifact, DeveloperArtifactCoverage, DeveloperProject,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Maximum manifest size we are willing to read for inspection (1 MiB)
const MAX_MANIFEST_READ_BYTES: u64 = 1024 * 1024;
const ACTIVE_PROJECT_THRESHOLD_DAYS: u64 = 7;

#[derive(Debug, Clone)]
struct DiscoveredProject {
    root_path: PathBuf,
    root_relative_path: Vec<u8>,
    ecosystems: HashSet<DeveloperEcosystem>,
    primary_ecosystem: DeveloperEcosystem,
    manifests: Vec<PathBuf>,
    lockfiles: Vec<PathBuf>,
    most_recent_mtime: u64,
}

#[derive(Debug, Clone)]
struct CandidateArtifactPath {
    path: PathBuf,
    kind: DeveloperArtifactKind,
    ecosystem: DeveloperEcosystem,
    convention_name: String,
    caution_required: bool,
}

pub struct DeveloperArtifactScanner {
    root_path: PathBuf,
    root_id: String,
    active_guard: ActiveProcessGuard,
}

impl DeveloperArtifactScanner {
    pub fn new(root_path: impl Into<PathBuf>, root_id: impl Into<String>) -> Self {
        Self {
            root_path: root_path.into(),
            root_id: root_id.into(),
            active_guard: ActiveProcessGuard::new(),
        }
    }

    /// Primary scan execution returning an immutable generation
    pub fn scan(&self) -> Result<ArtifactAnalysisGeneration, crate::error::ArtifactError> {
        let root_canon = self
            .root_path
            .canonicalize()
            .unwrap_or_else(|_| self.root_path.clone());
        let duration = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let observed_at = duration.as_secs() as i64;
        let generation_id = format!("gen_devart_{:x}", duration.as_nanos());

        // Step 1: Discover all project roots within the root path
        let mut discovered_projects = self.discover_projects(&root_canon)?;

        // Sort projects by path depth descending so nested projects can be matched first
        discovered_projects.sort_by(|a, b| b.root_path.cmp(&a.root_path));

        let mut projects = Vec::new();
        let mut total_gen_logical = 0u64;
        let mut total_gen_allocated = 0u64;
        let mut unclassified_count = 0usize;
        let mut skipped_count = 0usize;
        let mut global_seen_inodes: HashSet<(u64, u64)> = HashSet::new();

        // Step 2: For each discovered project, inspect and collect artifacts
        for proj in discovered_projects {
            let project_id = DeveloperProjectId::from_raw_relative(
                &self.root_id,
                &proj.root_relative_path,
                proj.primary_ecosystem,
            );

            let display_name = proj
                .root_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "project".to_string());

            let display_path = match proj.root_path.strip_prefix(&root_canon) {
                Ok(rel) if rel.as_os_str().is_empty() => ".".to_string(),
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => proj.root_path.to_string_lossy().to_string(),
            };

            let active_state = self.determine_active_state(proj.most_recent_mtime);

            // Find artifacts belonging directly to this project (not belonging to sub-projects)
            let candidate_artifacts = self.find_project_artifacts(&proj);

            let mut project_artifacts = Vec::new();
            let mut project_logical = 0u64;
            let mut project_allocated = 0u64;

            for cand in candidate_artifacts {
                // Ensure candidate does not escape root (symlink guard)
                if !self.is_safe_artifact_path(&cand.path, &root_canon) {
                    skipped_count += 1;
                    continue;
                }

                let rel_path = match cand.path.strip_prefix(&root_canon) {
                    Ok(r) => crate::id::path_to_raw_bytes(r),
                    Err(_) => crate::id::path_to_raw_bytes(&cand.path),
                };

                let artifact_id =
                    DeveloperArtifactId::from_raw_relative(&project_id, &rel_path, cand.kind);

                // Calculate storage metrics with hardlink deduplication
                let (logical, allocated) =
                    self.measure_storage(&cand.path, &mut global_seen_inodes);
                if logical == 0 && allocated == 0 && !cand.path.exists() {
                    continue;
                }

                // Build evidence
                let evidence = self.build_rebuild_evidence(&proj, &cand, active_state);

                let art_display_name = cand
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| cand.convention_name.clone());

                let art_display_path = match cand.path.strip_prefix(&root_canon) {
                    Ok(rel) => rel.to_string_lossy().to_string(),
                    Err(_) => cand.path.to_string_lossy().to_string(),
                };

                let dev_art = DeveloperArtifact {
                    artifact_id,
                    project_id: project_id.clone(),
                    raw_relative_path: rel_path,
                    display_name: art_display_name,
                    display_path: art_display_path,
                    ecosystem: cand.ecosystem,
                    artifact_kind: cand.kind,
                    logical_bytes: logical,
                    allocated_bytes: allocated,
                    confirmed_reclaim_lower_bound: allocated,
                    estimated_reclaim: allocated,
                    physical_sharing_uncertainty: true,
                    rebuild_evidence: evidence,
                    candidate_id: None,
                };

                project_logical += logical;
                project_allocated += allocated;
                project_artifacts.push(dev_art);
            }

            // Determine aggregate rebuild confidence for project
            let proj_confidence = if project_artifacts.is_empty() {
                RebuildConfidence::Unknown
            } else {
                project_artifacts
                    .iter()
                    .map(|a| a.rebuild_evidence.reconstruction_confidence)
                    .min()
                    .unwrap_or(RebuildConfidence::Unknown)
            };

            let manifest_paths: Vec<String> = proj
                .manifests
                .iter()
                .map(|p| match p.strip_prefix(&root_canon) {
                    Ok(r) => r.to_string_lossy().to_string(),
                    Err(_) => p.to_string_lossy().to_string(),
                })
                .collect();

            let lockfile_paths: Vec<String> = proj
                .lockfiles
                .iter()
                .map(|p| match p.strip_prefix(&root_canon) {
                    Ok(r) => r.to_string_lossy().to_string(),
                    Err(_) => p.to_string_lossy().to_string(),
                })
                .collect();

            let mut all_ecosystems: Vec<DeveloperEcosystem> = proj.ecosystems.into_iter().collect();
            all_ecosystems.sort();

            total_gen_logical += project_logical;
            total_gen_allocated += project_allocated;

            projects.push(DeveloperProject {
                project_id,
                raw_relative_path: proj.root_relative_path,
                display_name,
                display_path,
                primary_ecosystem: proj.primary_ecosystem,
                all_ecosystems,
                manifest_paths,
                lockfile_paths,
                artifacts: project_artifacts,
                total_logical_bytes: project_logical,
                total_allocated_bytes: project_allocated,
                rebuild_confidence: proj_confidence,
                active_state,
            });
        }

        // Sort projects by total_allocated_bytes DESC, then display_name
        projects.sort_by(|a, b| {
            b.total_allocated_bytes
                .cmp(&a.total_allocated_bytes)
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        // Step 3: Count unclassified candidate directories
        unclassified_count += self.count_unclassified_directories(&root_canon, &projects);

        let coverage = DeveloperArtifactCoverage {
            supported_ecosystems: vec![
                DeveloperEcosystem::Xcode,
                DeveloperEcosystem::SwiftPM,
                DeveloperEcosystem::RustCargo,
                DeveloperEcosystem::Node,
                DeveloperEcosystem::Python,
                DeveloperEcosystem::Gradle,
                DeveloperEcosystem::Maven,
                DeveloperEcosystem::CMake,
            ],
            unclassified_candidate_directories: unclassified_count,
            skipped_items: skipped_count,
        };

        Ok(ArtifactAnalysisGeneration {
            generation_id,
            root_id: self.root_id.clone(),
            root_path: root_canon.to_string_lossy().to_string(),
            observed_at,
            projects,
            total_logical_bytes: total_gen_logical,
            total_allocated_bytes: total_gen_allocated,
            coverage,
        })
    }

    /// Walk the tree and locate project roots based on authoritative manifests
    fn discover_projects(
        &self,
        root: &Path,
    ) -> Result<Vec<DiscoveredProject>, crate::error::ArtifactError> {
        let mut map: HashMap<PathBuf, DiscoveredProject> = HashMap::new();

        // Walk tree with bounded depth and symlink prevention
        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(10)
            .into_iter();

        for entry in walker.filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Never enter .git, Trash, or system folders
            if name == ".git" || name == ".Trash" || name == "Library" {
                return false;
            }
            // Skip common large artifact folders during project discovery
            if name == "node_modules"
                || name == "target"
                || name == ".build"
                || name == "DerivedData"
                || name == ".venv"
                || name == "venv"
            {
                return false;
            }
            true
        }) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let file_type = entry.file_type();
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy();

            // Detect project markers
            if file_type.is_file() {
                if let Ok(meta) = path.symlink_metadata() {
                    if meta.len() > MAX_MANIFEST_READ_BYTES {
                        continue;
                    }
                }
                if let Some((eco, is_lock)) = self.classify_project_file(&file_name) {
                    if let Some(parent) = path.parent() {
                        let parent_buf = parent.to_path_buf();
                        let rel_bytes = match parent.strip_prefix(root) {
                            Ok(rel) => crate::id::path_to_raw_bytes(rel),
                            Err(_) => crate::id::path_to_raw_bytes(parent),
                        };

                        let mtime = self.get_mtime(path);

                        let proj =
                            map.entry(parent_buf.clone())
                                .or_insert_with(|| DiscoveredProject {
                                    root_path: parent_buf,
                                    root_relative_path: rel_bytes,
                                    ecosystems: HashSet::new(),
                                    primary_ecosystem: eco,
                                    manifests: Vec::new(),
                                    lockfiles: Vec::new(),
                                    most_recent_mtime: 0,
                                });

                        proj.ecosystems.insert(eco);
                        if is_lock {
                            proj.lockfiles.push(path.to_path_buf());
                        } else {
                            proj.manifests.push(path.to_path_buf());
                        }
                        if mtime > proj.most_recent_mtime {
                            proj.most_recent_mtime = mtime;
                        }
                    }
                }
            } else if file_type.is_dir() {
                // Xcode project bundle directory (.xcodeproj / .xcworkspace)
                if file_name.ends_with(".xcodeproj") || file_name.ends_with(".xcworkspace") {
                    if let Some(parent) = path.parent() {
                        let parent_buf = parent.to_path_buf();
                        let rel_bytes = match parent.strip_prefix(root) {
                            Ok(rel) => crate::id::path_to_raw_bytes(rel),
                            Err(_) => crate::id::path_to_raw_bytes(parent),
                        };
                        let mtime = self.get_mtime(path);
                        let proj =
                            map.entry(parent_buf.clone())
                                .or_insert_with(|| DiscoveredProject {
                                    root_path: parent_buf,
                                    root_relative_path: rel_bytes,
                                    ecosystems: HashSet::new(),
                                    primary_ecosystem: DeveloperEcosystem::Xcode,
                                    manifests: Vec::new(),
                                    lockfiles: Vec::new(),
                                    most_recent_mtime: 0,
                                });
                        proj.ecosystems.insert(DeveloperEcosystem::Xcode);
                        proj.manifests.push(path.to_path_buf());
                        if mtime > proj.most_recent_mtime {
                            proj.most_recent_mtime = mtime;
                        }
                    }
                }
            }
        }

        // Refine primary ecosystem preference if multiple exist
        for proj in map.values_mut() {
            if proj.ecosystems.contains(&DeveloperEcosystem::RustCargo) {
                proj.primary_ecosystem = DeveloperEcosystem::RustCargo;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::SwiftPM) {
                proj.primary_ecosystem = DeveloperEcosystem::SwiftPM;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::Xcode) {
                proj.primary_ecosystem = DeveloperEcosystem::Xcode;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::Node) {
                proj.primary_ecosystem = DeveloperEcosystem::Node;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::Python) {
                proj.primary_ecosystem = DeveloperEcosystem::Python;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::Gradle) {
                proj.primary_ecosystem = DeveloperEcosystem::Gradle;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::Maven) {
                proj.primary_ecosystem = DeveloperEcosystem::Maven;
            } else if proj.ecosystems.contains(&DeveloperEcosystem::CMake) {
                proj.primary_ecosystem = DeveloperEcosystem::CMake;
            }
        }

        Ok(map.into_values().collect())
    }

    /// Classify a file into an ecosystem manifest or lockfile
    fn classify_project_file(&self, name: &str) -> Option<(DeveloperEcosystem, bool)> {
        match name {
            // Rust Cargo
            "Cargo.toml" => Some((DeveloperEcosystem::RustCargo, false)),
            "Cargo.lock" => Some((DeveloperEcosystem::RustCargo, true)),

            // SwiftPM
            "Package.swift" => Some((DeveloperEcosystem::SwiftPM, false)),
            "Package.resolved" => Some((DeveloperEcosystem::SwiftPM, true)),

            // Node.js
            "package.json" => Some((DeveloperEcosystem::Node, false)),
            "package-lock.json"
            | "npm-shrinkwrap.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb" => Some((DeveloperEcosystem::Node, true)),

            // Python
            "pyproject.toml" | "requirements.txt" | "Pipfile" | "setup.py" | "setup.cfg" => {
                Some((DeveloperEcosystem::Python, false))
            }
            "poetry.lock" | "uv.lock" | "Pipfile.lock" => Some((DeveloperEcosystem::Python, true)),

            // Gradle
            "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "gradle.properties" => Some((DeveloperEcosystem::Gradle, false)),
            "gradle.lockfile" => Some((DeveloperEcosystem::Gradle, true)),

            // Maven
            "pom.xml" => Some((DeveloperEcosystem::Maven, false)),

            // CMake
            "CMakeLists.txt" => Some((DeveloperEcosystem::CMake, false)),
            "CMakeCache.txt" | "compile_commands.json" => Some((DeveloperEcosystem::CMake, true)),

            _ => {
                if name.starts_with("requirements-") && name.ends_with(".txt") {
                    Some((DeveloperEcosystem::Python, false))
                } else {
                    None
                }
            }
        }
    }

    /// Find known artifacts that belong to this specific project root
    fn find_project_artifacts(&self, proj: &DiscoveredProject) -> Vec<CandidateArtifactPath> {
        let mut candidates = Vec::new();
        let root = &proj.root_path;

        for eco in &proj.ecosystems {
            match eco {
                DeveloperEcosystem::RustCargo => {
                    let target_dir = root.join("target");
                    if target_dir.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: target_dir,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::RustCargo,
                            convention_name: "target".to_string(),
                            caution_required: false,
                        });
                    }
                }
                DeveloperEcosystem::SwiftPM => {
                    let build_dir = root.join(".build");
                    if build_dir.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: build_dir,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::SwiftPM,
                            convention_name: ".build".to_string(),
                            caution_required: false,
                        });
                    }
                }
                DeveloperEcosystem::Xcode => {
                    // Project-local DerivedData or ModuleCache if present
                    let dd = root.join("DerivedData");
                    if dd.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: dd,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::Xcode,
                            convention_name: "DerivedData".to_string(),
                            caution_required: false,
                        });
                    }
                    let archives = root.join("Archives");
                    if archives.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: archives,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::Xcode,
                            convention_name: "Archives".to_string(),
                            caution_required: true, // Archives require REVIEW/CAUTION
                        });
                    }
                }
                DeveloperEcosystem::Node => {
                    let node_modules = root.join("node_modules");
                    if node_modules.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: node_modules,
                            kind: DeveloperArtifactKind::DependencyTree,
                            ecosystem: DeveloperEcosystem::Node,
                            convention_name: "node_modules".to_string(),
                            caution_required: false,
                        });
                    }
                    for name in &[".next", ".nuxt", "dist", "build", "coverage", ".cache"] {
                        let path = root.join(name);
                        if path.is_dir() {
                            let kind = match *name {
                                "coverage" => DeveloperArtifactKind::CoverageOutput,
                                ".cache" => DeveloperArtifactKind::CompilerCache,
                                _ => DeveloperArtifactKind::BuildOutput,
                            };
                            candidates.push(CandidateArtifactPath {
                                path,
                                kind,
                                ecosystem: DeveloperEcosystem::Node,
                                convention_name: name.to_string(),
                                caution_required: false,
                            });
                        }
                    }
                }
                DeveloperEcosystem::Python => {
                    for venv_name in &[".venv", "venv", ".env", "env"] {
                        let venv_dir = root.join(venv_name);
                        if venv_dir.is_dir()
                            && (venv_dir.join("bin/python").exists()
                                || venv_dir.join("pyvenv.cfg").exists())
                        {
                            candidates.push(CandidateArtifactPath {
                                path: venv_dir,
                                kind: DeveloperArtifactKind::VirtualEnvironment,
                                ecosystem: DeveloperEcosystem::Python,
                                convention_name: venv_name.to_string(),
                                caution_required: false,
                            });
                        }
                    }
                    for name in &[
                        "__pycache__",
                        ".pytest_cache",
                        ".mypy_cache",
                        ".ruff_cache",
                        ".coverage",
                        "htmlcov",
                        "dist",
                        "build",
                    ] {
                        let path = root.join(name);
                        if path.exists() {
                            let kind = match *name {
                                ".pytest_cache" => DeveloperArtifactKind::TestOutput,
                                ".coverage" | "htmlcov" => DeveloperArtifactKind::CoverageOutput,
                                ".mypy_cache" | ".ruff_cache" => {
                                    DeveloperArtifactKind::CompilerCache
                                }
                                "__pycache__" => DeveloperArtifactKind::CompilerCache,
                                _ => DeveloperArtifactKind::BuildOutput,
                            };
                            candidates.push(CandidateArtifactPath {
                                path,
                                kind,
                                ecosystem: DeveloperEcosystem::Python,
                                convention_name: name.to_string(),
                                caution_required: false,
                            });
                        }
                    }
                }
                DeveloperEcosystem::Gradle => {
                    let build_dir = root.join("build");
                    if build_dir.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: build_dir,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::Gradle,
                            convention_name: "build".to_string(),
                            caution_required: false,
                        });
                    }
                    let gradle_dir = root.join(".gradle");
                    if gradle_dir.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: gradle_dir,
                            kind: DeveloperArtifactKind::CompilerCache,
                            ecosystem: DeveloperEcosystem::Gradle,
                            convention_name: ".gradle".to_string(),
                            caution_required: false,
                        });
                    }
                }
                DeveloperEcosystem::Maven => {
                    let target_dir = root.join("target");
                    if target_dir.is_dir() {
                        candidates.push(CandidateArtifactPath {
                            path: target_dir,
                            kind: DeveloperArtifactKind::BuildOutput,
                            ecosystem: DeveloperEcosystem::Maven,
                            convention_name: "target".to_string(),
                            caution_required: false,
                        });
                    }
                }
                DeveloperEcosystem::CMake => {
                    for name in &["build", "cmake-build-debug", "cmake-build-release"] {
                        let bdir = root.join(name);
                        if bdir.is_dir()
                            && (bdir.join("CMakeCache.txt").exists()
                                || bdir.join("CMakeFiles").exists())
                        {
                            candidates.push(CandidateArtifactPath {
                                path: bdir,
                                kind: DeveloperArtifactKind::BuildOutput,
                                ecosystem: DeveloperEcosystem::CMake,
                                convention_name: name.to_string(),
                                caution_required: false,
                            });
                        }
                    }
                }
                DeveloperEcosystem::Unknown => {}
            }
        }

        // Deduplicate by path
        let mut seen = HashSet::new();
        candidates.retain(|c| seen.insert(c.path.clone()));
        candidates
    }

    /// Construct evidence model for a specific artifact candidate
    fn build_rebuild_evidence(
        &self,
        proj: &DiscoveredProject,
        cand: &CandidateArtifactPath,
        active_state: ActiveProjectState,
    ) -> RebuildEvidence {
        let mut evidence = RebuildEvidence::new();
        evidence.manifest_present = !proj.manifests.is_empty();
        evidence.manifest_path = proj
            .manifests
            .first()
            .map(|p| p.to_string_lossy().to_string());
        evidence.lockfile_present = !proj.lockfiles.is_empty();
        evidence.lockfile_path = proj
            .lockfiles
            .first()
            .map(|p| p.to_string_lossy().to_string());
        evidence.known_artifact_convention = true;
        evidence.project_root_known = true;
        evidence.toolchain_identified = Some(cand.ecosystem.display_name().to_string());
        evidence.active_project_state = active_state;
        evidence.rebuild_command_template =
            Some(cand.ecosystem.default_rebuild_template().to_string());

        let mut reasons = Vec::new();
        reasons.push(format!(
            "Associated with {} project at {}",
            cand.ecosystem.display_name(),
            proj.root_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));

        if evidence.manifest_present {
            reasons.push(format!(
                "Manifest present: {}",
                proj.manifests
                    .first()
                    .unwrap()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }

        if evidence.lockfile_present {
            reasons.push(format!(
                "Lockfile detected: {}",
                proj.lockfiles
                    .first()
                    .unwrap()
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
        }

        // Calculate confidence
        if cand.caution_required {
            evidence.reconstruction_confidence = RebuildConfidence::Partial;
            reasons.push(
                "Archive contains distribution or signed products; requires manual review."
                    .to_string(),
            );
        } else if cand.kind == DeveloperArtifactKind::VirtualEnvironment {
            // Virtual environments might contain locally editable packages or unpinned wheels
            if evidence.lockfile_present {
                evidence.reconstruction_confidence = RebuildConfidence::Strong;
                reasons.push("Virtual environment backed by deterministic lockfile.".to_string());
            } else {
                evidence.reconstruction_confidence = RebuildConfidence::Partial;
                reasons.push("Virtual environment has no accompanying lockfile; editable packages may exist.".to_string());
            }
        } else if cand.kind == DeveloperArtifactKind::DependencyTree {
            if evidence.lockfile_present {
                evidence.reconstruction_confidence = RebuildConfidence::Verified;
                reasons.push("Dependency tree backed by exact lockfile.".to_string());
            } else {
                evidence.reconstruction_confidence = RebuildConfidence::Partial;
                reasons.push(
                    "Dependencies present without lockfile; reconstruction may vary.".to_string(),
                );
            }
        } else if evidence.manifest_present && evidence.lockfile_present {
            evidence.reconstruction_confidence = RebuildConfidence::Strong;
            reasons.push(
                "Workflow artifacts fully reconstructable via project toolchain and lockfile."
                    .to_string(),
            );
        } else if evidence.manifest_present {
            evidence.reconstruction_confidence = RebuildConfidence::Strong;
            reasons.push("Workflow artifacts reconstructable via project manifest.".to_string());
        } else {
            evidence.reconstruction_confidence = RebuildConfidence::Partial;
            reasons.push(
                "Artifact follows standard naming conventions with partial project evidence."
                    .to_string(),
            );
        }

        let cand_mtime = self.get_mtime(&cand.path);
        if self
            .active_guard
            .is_active_target(&proj.root_path, &cand.path, cand_mtime)
        {
            evidence.active_guard_deferred = true;
            reasons.push(
                "Active build process or recent file modification detected; artifact deferred from automatic cleanup."
                    .to_string(),
            );
        }

        evidence.reasons = reasons;
        evidence
    }

    /// Measures logical and allocated storage of an artifact directory with hardlink deduplication
    fn measure_storage(&self, path: &Path, seen_inodes: &mut HashSet<(u64, u64)>) -> (u64, u64) {
        if !path.exists() {
            return (0, 0);
        }

        let mut logical_total = 0u64;
        let mut allocated_total = 0u64;

        let walker = walkdir::WalkDir::new(path).follow_links(false).into_iter();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let meta = match entry.path().symlink_metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Dataless guard: do not hydrate
            if is_dataless_file(entry.path()) {
                continue;
            }

            #[cfg(unix)]
            {
                let dev = meta.dev();
                let ino = meta.ino();
                let file_logical = meta.size();
                let file_allocated = meta.blocks() * 512;

                logical_total += file_logical;

                // If this is a hardlink alias already counted elsewhere in this generation, don't double count
                if meta.nlink() > 1 {
                    if seen_inodes.insert((dev, ino)) {
                        allocated_total += file_allocated;
                    }
                } else {
                    allocated_total += file_allocated;
                }
            }

            #[cfg(not(unix))]
            {
                let file_logical = meta.len();
                logical_total += file_logical;
                allocated_total += file_logical;
            }
        }

        (logical_total, allocated_total)
    }

    /// Count candidate directories named "build", "target", "dist", etc. that have NO project association
    fn count_unclassified_directories(&self, root: &Path, projects: &[DeveloperProject]) -> usize {
        let mut unclassified = 0;
        let candidate_names: HashSet<&str> = [
            "build",
            "target",
            "dist",
            "out",
            "DerivedData",
            "node_modules",
            ".venv",
            "venv",
        ]
        .iter()
        .copied()
        .collect();

        let walker = walkdir::WalkDir::new(root)
            .follow_links(false)
            .max_depth(5)
            .into_iter();

        for entry in walker.filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            name != ".git" && name != ".Trash" && name != "Library"
        }) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_dir() {
                let name = entry.file_name().to_string_lossy();
                if candidate_names.contains(name.as_ref()) {
                    let canon_path = entry
                        .path()
                        .canonicalize()
                        .unwrap_or_else(|_| entry.path().to_path_buf());
                    // Check if this directory is already recognized as an artifact of any project
                    let is_recognized = projects.iter().any(|p| {
                        p.artifacts.iter().any(|a| {
                            let art_path = root.join(&a.display_path);
                            art_path == canon_path || art_path == entry.path()
                        })
                    });

                    if !is_recognized {
                        unclassified += 1;
                    }
                }
            }
        }

        unclassified
    }

    /// Safety check: ensuring path does not escape the allowed root directory
    fn is_safe_artifact_path(&self, path: &Path, root: &Path) -> bool {
        if let Ok(canon) = path.canonicalize() {
            canon.starts_with(root)
        } else {
            path.starts_with(root)
        }
    }

    fn determine_active_state(&self, most_recent_mtime: u64) -> ActiveProjectState {
        if most_recent_mtime == 0 {
            return ActiveProjectState::Unknown;
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        let threshold_secs = ACTIVE_PROJECT_THRESHOLD_DAYS * 24 * 3600;
        if now.saturating_sub(most_recent_mtime) < threshold_secs {
            ActiveProjectState::Active
        } else {
            ActiveProjectState::Dormant
        }
    }

    fn get_mtime(&self, path: &Path) -> u64 {
        if let Ok(meta) = path.symlink_metadata() {
            if let Ok(mtime) = meta.modified() {
                if let Ok(dur) = mtime.duration_since(SystemTime::UNIX_EPOCH) {
                    return dur.as_secs();
                }
            }
        }
        0
    }
}

/// Helper: check if file is dataless (e.g. iCloud/OneDrive placeholder) without hydration
fn is_dataless_file(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        if let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) {
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::lstat(c_path.as_ptr(), &mut st) } == 0 {
                const DATALESS_FLAG: u32 = 0x40000000;
                return (st.st_flags & DATALESS_FLAG) != 0;
            }
        }
    }
    let _ = path;
    false
}
