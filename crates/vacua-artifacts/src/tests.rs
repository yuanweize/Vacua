use super::*;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_fundamental_false_positive_rejection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create misleading generic directories without any project manifest
    let fake_build = root.join("build");
    fs::create_dir_all(&fake_build).unwrap();
    File::create(fake_build.join("my_vacation_photos.jpg")).unwrap();

    let fake_target = root.join("target");
    fs::create_dir_all(&fake_target).unwrap();
    File::create(fake_target.join("important_doc.pdf")).unwrap();

    let fake_dist = root.join("dist");
    fs::create_dir_all(&fake_dist).unwrap();
    File::create(fake_dist.join("notes.txt")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    // Invariant: MUST NOT classify fake build/target/dist as developer artifacts
    assert_eq!(
        gen.projects.len(),
        0,
        "No projects should be found without manifests"
    );
    let total_artifacts: usize = gen.projects.iter().map(|p| p.artifacts.len()).sum();
    assert_eq!(
        total_artifacts, 0,
        "No developer artifacts should be classified"
    );
    // They should be recorded in unclassified candidate directories
    assert!(gen.coverage.unclassified_candidate_directories >= 3);
}

#[test]
fn test_cargo_project_and_source_protection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Create a valid Cargo project
    File::create(root.join("Cargo.toml")).unwrap();
    File::create(root.join("Cargo.lock")).unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let mut main_rs = File::create(src_dir.join("main.rs")).unwrap();
    writeln!(main_rs, "fn main() {{}}").unwrap();

    let target_dir = root.join("target/debug");
    fs::create_dir_all(&target_dir).unwrap();
    let mut bin = File::create(target_dir.join("my_app")).unwrap();
    bin.write_all(&[0u8; 1024]).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(gen.projects.len(), 1);
    let proj = &gen.projects[0];
    assert_eq!(proj.primary_ecosystem, DeveloperEcosystem::RustCargo);
    assert_eq!(proj.artifacts.len(), 1);

    let art = &proj.artifacts[0];
    assert_eq!(art.display_name, "target");
    assert_eq!(art.ecosystem, DeveloperEcosystem::RustCargo);
    assert_eq!(art.artifact_kind, DeveloperArtifactKind::BuildOutput);
    assert!(art.rebuild_evidence.manifest_present);
    assert!(art.rebuild_evidence.lockfile_present);
    assert_eq!(
        art.rebuild_evidence.reconstruction_confidence,
        RebuildConfidence::Strong
    );

    // Invariant: src/ and Cargo.toml must NOT be classified as artifacts
    for p in &gen.projects {
        for a in &p.artifacts {
            assert_ne!(a.display_name, "src");
            assert_ne!(a.display_name, "Cargo.toml");
        }
    }
}

#[test]
fn test_swift_pm_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    File::create(root.join("Package.swift")).unwrap();
    File::create(root.join("Package.resolved")).unwrap();

    let sources = root.join("Sources/App");
    fs::create_dir_all(&sources).unwrap();
    File::create(sources.join("main.swift")).unwrap();

    let build_dir = root.join(".build");
    fs::create_dir_all(&build_dir).unwrap();
    File::create(build_dir.join("build.db")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(gen.projects.len(), 1);
    let proj = &gen.projects[0];
    assert_eq!(proj.primary_ecosystem, DeveloperEcosystem::SwiftPM);
    assert_eq!(proj.artifacts.len(), 1);
    assert_eq!(proj.artifacts[0].display_name, ".build");
    assert!(proj.artifacts[0].rebuild_evidence.lockfile_present);
}

#[test]
fn test_xcode_archives_caution() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("MyApp.xcodeproj")).unwrap();
    let derived_data = root.join("DerivedData");
    fs::create_dir_all(&derived_data).unwrap();
    File::create(derived_data.join("ModuleCache.noindex")).unwrap();

    let archives = root.join("Archives");
    fs::create_dir_all(&archives).unwrap();
    File::create(archives.join("MyApp.xcarchive")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(gen.projects.len(), 1);
    let proj = &gen.projects[0];
    assert_eq!(proj.primary_ecosystem, DeveloperEcosystem::Xcode);
    assert_eq!(proj.artifacts.len(), 2);

    let archive_art = proj
        .artifacts
        .iter()
        .find(|a| a.display_name == "Archives")
        .unwrap();
    // Invariant: Archives must receive CAUTION / Partial confidence
    assert_eq!(
        archive_art.rebuild_evidence.reconstruction_confidence,
        RebuildConfidence::Partial
    );
    assert!(archive_art
        .rebuild_evidence
        .reasons
        .iter()
        .any(|r| r.contains("requires manual review")));
}

#[test]
fn test_node_lockfile_confidence_distinction() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Node project WITH lockfile
    let node_with_lock = root.join("proj_locked");
    fs::create_dir_all(&node_with_lock).unwrap();
    File::create(node_with_lock.join("package.json")).unwrap();
    File::create(node_with_lock.join("package-lock.json")).unwrap();
    let nm1 = node_with_lock.join("node_modules");
    fs::create_dir_all(&nm1).unwrap();
    File::create(nm1.join("dep.js")).unwrap();

    // Node project WITHOUT lockfile
    let node_no_lock = root.join("proj_unlocked");
    fs::create_dir_all(&node_no_lock).unwrap();
    File::create(node_no_lock.join("package.json")).unwrap();
    let nm2 = node_no_lock.join("node_modules");
    fs::create_dir_all(&nm2).unwrap();
    File::create(nm2.join("dep.js")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(gen.projects.len(), 2);

    let locked_proj = gen
        .projects
        .iter()
        .find(|p| p.display_name == "proj_locked")
        .unwrap();
    assert_eq!(
        locked_proj.artifacts[0]
            .rebuild_evidence
            .reconstruction_confidence,
        RebuildConfidence::Verified
    );

    let unlocked_proj = gen
        .projects
        .iter()
        .find(|p| p.display_name == "proj_unlocked")
        .unwrap();
    assert_eq!(
        unlocked_proj.artifacts[0]
            .rebuild_evidence
            .reconstruction_confidence,
        RebuildConfidence::Partial
    );
}

#[test]
fn test_python_virtualenv_and_pycache() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    File::create(root.join("pyproject.toml")).unwrap();
    File::create(root.join("uv.lock")).unwrap();

    let venv = root.join(".venv");
    fs::create_dir_all(venv.join("bin")).unwrap();
    File::create(venv.join("pyvenv.cfg")).unwrap();
    File::create(venv.join("bin/python")).unwrap();

    let pycache = root.join("__pycache__");
    fs::create_dir_all(&pycache).unwrap();
    File::create(pycache.join("mod.cpython-312.pyc")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(gen.projects.len(), 1);
    let proj = &gen.projects[0];
    assert_eq!(proj.primary_ecosystem, DeveloperEcosystem::Python);

    let venv_art = proj
        .artifacts
        .iter()
        .find(|a| a.display_name == ".venv")
        .unwrap();
    assert_eq!(
        venv_art.artifact_kind,
        DeveloperArtifactKind::VirtualEnvironment
    );
    assert_eq!(
        venv_art.rebuild_evidence.reconstruction_confidence,
        RebuildConfidence::Strong
    );

    let pycache_art = proj
        .artifacts
        .iter()
        .find(|a| a.display_name == "__pycache__")
        .unwrap();
    assert_eq!(
        pycache_art.artifact_kind,
        DeveloperArtifactKind::CompilerCache
    );
}

#[test]
fn test_monorepo_nested_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Parent: Node project
    File::create(root.join("package.json")).unwrap();
    File::create(root.join("package-lock.json")).unwrap();
    let parent_nm = root.join("node_modules");
    fs::create_dir_all(&parent_nm).unwrap();
    File::create(parent_nm.join("parent_dep.js")).unwrap();

    // Nested child: Rust Cargo project
    let rust_child = root.join("rust_service");
    fs::create_dir_all(&rust_child).unwrap();
    File::create(rust_child.join("Cargo.toml")).unwrap();
    File::create(rust_child.join("Cargo.lock")).unwrap();
    let child_target = rust_child.join("target");
    fs::create_dir_all(&child_target).unwrap();
    File::create(child_target.join("service.bin")).unwrap();

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    assert_eq!(
        gen.projects.len(),
        2,
        "Both parent and child projects must be recognized"
    );

    let node_proj = gen
        .projects
        .iter()
        .find(|p| p.primary_ecosystem == DeveloperEcosystem::Node)
        .unwrap();
    assert_eq!(node_proj.artifacts.len(), 1);
    assert_eq!(node_proj.artifacts[0].display_name, "node_modules");

    let rust_proj = gen
        .projects
        .iter()
        .find(|p| p.primary_ecosystem == DeveloperEcosystem::RustCargo)
        .unwrap();
    assert_eq!(rust_proj.artifacts.len(), 1);
    assert_eq!(rust_proj.artifacts[0].display_name, "target");
}

#[test]
fn test_symlink_escape_protection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // External target directory outside the root
    let outside_dir = tempdir().unwrap();
    let outside_secret = outside_dir.path().join("secret_data");
    fs::create_dir_all(&outside_secret).unwrap();
    File::create(outside_secret.join("password.txt")).unwrap();

    File::create(root.join("Cargo.toml")).unwrap();

    // Symlink inside project pointing outside allowed root
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let _ = symlink(&outside_secret, root.join("target"));
    }

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();

    // The escaping target symlink must NOT be traversed or counted as an artifact
    for p in &gen.projects {
        for a in &p.artifacts {
            assert_ne!(a.display_path, outside_secret.to_string_lossy());
        }
    }
}

#[test]
fn test_static_shell_safety_boundary() {
    // Read all Rust source files in crates/vacua-artifacts
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in walkdir::WalkDir::new(&src_dir) {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|ext| ext == "rs")
            && !entry.path().ends_with("tests.rs")
        {
            let content = fs::read_to_string(entry.path()).unwrap();
            assert!(
                !content.contains("std::process::Command"),
                "Forbidden Command found in {}",
                entry.path().display()
            );
            assert!(
                !content.contains("Command::new"),
                "Forbidden Command::new found in {}",
                entry.path().display()
            );
            assert!(
                !content.contains("/bin/sh"),
                "Forbidden shell string found in {}",
                entry.path().display()
            );
            assert!(
                !content.contains("/bin/bash"),
                "Forbidden shell string found in {}",
                entry.path().display()
            );
            assert!(
                !content.contains("/bin/zsh"),
                "Forbidden shell string found in {}",
                entry.path().display()
            );
            assert!(
                !content.contains("system("),
                "Forbidden system() call found in {}",
                entry.path().display()
            );
        }
    }
}

#[test]
fn test_agent_development_active_process_guard_scenario() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Helper to backdate mtime so recent-mtime guard does not mask the process check
    fn backdate_mtime(path: &Path) {
        use std::ffi::CString;
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        let old_time = libc::timeval {
            tv_sec: 1_700_000_000,
            tv_usec: 0,
        };
        let times = [old_time, old_time];
        unsafe {
            libc::utimes(c_path.as_ptr(), times.as_ptr());
        }
    }

    // Create Cargo project: Cargo.toml, src/main.rs, target/debug/vacua
    let cargo_toml = root.join("Cargo.toml");
    File::create(&cargo_toml).unwrap();
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let main_rs = src_dir.join("main.rs");
    File::create(&main_rs).unwrap();

    let target_dir = root.join("target");
    let debug_dir = target_dir.join("debug");
    fs::create_dir_all(&debug_dir).unwrap();
    let bin_file = debug_dir.join("vacua");
    File::create(&bin_file).unwrap();

    // Backdate project files and directories to 10 days ago
    backdate_mtime(&bin_file);
    backdate_mtime(&debug_dir);
    backdate_mtime(&target_dir);
    backdate_mtime(&cargo_toml);
    backdate_mtime(&main_rs);
    backdate_mtime(&src_dir);
    backdate_mtime(root);

    // 1. Simulate active cargo/rustc build process on target directory
    std::env::set_var(
        "VACUA_SIMULATED_ACTIVE_PROCESS_PATH",
        target_dir.to_str().unwrap(),
    );

    let scanner = DeveloperArtifactScanner::new(root, "test_root");
    let gen = scanner.scan().unwrap();
    assert_eq!(gen.projects.len(), 1);
    let artifact = &gen.projects[0].artifacts[0];
    assert!(
        artifact.rebuild_evidence.active_guard_deferred,
        "Active build artifact must be deferred!"
    );

    // 2. Simulate process completion / idle state
    std::env::remove_var("VACUA_SIMULATED_ACTIVE_PROCESS_PATH");
    let scanner_idle = DeveloperArtifactScanner::new(root, "test_root");
    let gen_idle = scanner_idle.scan().unwrap();
    let artifact_idle = &gen_idle.projects[0].artifacts[0];
    assert!(
        !artifact_idle.rebuild_evidence.active_guard_deferred,
        "Idle build artifact should no longer be deferred!"
    );
}
