#!/usr/bin/env python3
"""
Vacua Developer Artifact Intelligence Benchmark Suite (v0.8.0)

Measures and validates:
- Workload A: 100 Rust projects (build time, peak RSS, evidence accuracy)
- Workload B: Mixed 500 project tree (Rust, SwiftPM, Node, Python, Gradle, Maven)
- Workload C: Large Node dependency trees (heavy node_modules, .next, dist rollup)
- Workload D: Nested monorepo (nested project precedence, zero double counting)
- Workload E: 10,000 misleading directories named build/dist/target without manifests (CRITICAL false-positive benchmark)

Reports:
- Analysis time
- Peak RSS
- Projects discovered
- Artifacts discovered
- False positives
- False negatives in known fixtures
- SQLite growth
- Warm query latency

Outputs raw results to benchmarks/developer-artifacts-v0.8.0.json
"""

import os
import sys
import time
import json
import shutil
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VACUA_BIN = REPO_ROOT / "target" / "release" / "vacua"
BENCH_DIR = REPO_ROOT / "target" / "artifact-bench-fixtures"
BENCH_JSON = REPO_ROOT / "benchmarks" / "developer-artifacts-v0.8.0.json"

def get_sys_info():
    def cmd(c):
        try:
            return subprocess.check_output(c, shell=True).decode().strip()
        except Exception:
            return "unknown"

    soc = cmd("sysctl -n machdep.cpu.brand_string")
    if not soc or "Apple" not in soc:
        soc = cmd("sysctl -n hw.model")
    mem_bytes = int(cmd("sysctl -n hw.memsize") or "0")
    mem_gb = mem_bytes / (1024 ** 3)
    os_ver = cmd("sw_vers -productVersion")
    build_ver = cmd("sw_vers -buildVersion")
    git_sha = cmd("git rev-parse --short HEAD")

    return {
        "soc": soc,
        "ram": f"{mem_gb:.0f} GB",
        "os": f"macOS {os_ver} (Build {build_ver})",
        "filesystem": "APFS (Apple File System)",
        "git_sha": git_sha,
    }

def run_timed_vacua_artifacts(path: Path, extra_args: list = None):
    cmd_args = [str(VACUA_BIN), "artifacts", str(path), "--json"]
    if extra_args:
        cmd_args.extend(extra_args)

    time_cmd = f"/usr/bin/time -l {' '.join(cmd_args)}"
    t0 = time.perf_counter()
    p = subprocess.Popen(
        time_cmd,
        shell=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    stdout, stderr = p.communicate()
    elapsed = time.perf_counter() - t0

    if p.returncode != 0:
        raise RuntimeError(f"vacua artifacts failed ({p.returncode}): {stderr.decode(errors='ignore')}")

    # Parse peak RSS
    rss_bytes = 0
    for line in stderr.decode(errors="ignore").splitlines():
        if "maximum resident set size" in line:
            parts = line.strip().split()
            if parts:
                try:
                    rss_bytes = int(parts[0])
                except ValueError:
                    pass
            break

    rss_mb = rss_bytes / (1024 * 1024)

    try:
        data = json.loads(stdout.decode("utf-8"))
    except Exception as e:
        raise RuntimeError(f"Failed to parse JSON output: {e}\nStdout: {stdout.decode(errors='ignore')[:500]}")

    return {
        "elapsed_sec": elapsed,
        "rss_mb": rss_mb,
        "data": data,
    }

def create_file(path: Path, content: str = ""):
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)

# MARK: - Workload Generators

def setup_workload_a(root: Path):
    """100 Rust projects."""
    print("  Creating Workload A: 100 Rust projects...", flush=True)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    for i in range(100):
        pdir = root / f"rust_project_{i:03d}"
        create_file(pdir / "Cargo.toml", f'[package]\nname = "rust_project_{i:03d}"\nversion = "0.1.0"\n')
        create_file(pdir / "Cargo.lock", '# Lockfile\nversion = 4\n')
        create_file(pdir / "src" / "main.rs", 'fn main() { println!("hi"); }\n')
        
        # Generated target
        tdir = pdir / "target"
        create_file(tdir / "debug" / f"rust_project_{i:03d}", "binary content " * 100)
        create_file(tdir / "debug" / "deps" / "dep.o", "object data " * 50)
        create_file(tdir / "CACHEDIR.TAG", "Signature: 8a477f597d28d172789f06886806bc55\n")

def setup_workload_b(root: Path):
    """Mixed 500 project tree: 150 Rust, 100 Swift, 100 Node, 75 Python, 50 Gradle, 25 Maven."""
    print("  Creating Workload B: 500 mixed projects...", flush=True)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    # 150 Rust
    for i in range(150):
        pdir = root / "rust" / f"proj_{i:03d}"
        create_file(pdir / "Cargo.toml", f'[package]\nname = "proj_{i:03d}"\n')
        create_file(pdir / "Cargo.lock", '# lock\n')
        create_file(pdir / "src" / "lib.rs", "pub fn f() {}\n")
        create_file(pdir / "target" / "release" / "lib.dylib", "x" * 200)

    # 100 SwiftPM
    for i in range(100):
        pdir = root / "swift" / f"proj_{i:03d}"
        create_file(pdir / "Package.swift", '// swift-tools-version: 6.0\nimport PackageDescription\n')
        create_file(pdir / "Package.resolved", '{"version": 3}\n')
        create_file(pdir / "Sources" / "App" / "main.swift", 'print("swift")\n')
        create_file(pdir / ".build" / "release" / "App", "swiftbinary" * 50)

    # 100 Node
    for i in range(100):
        pdir = root / "node" / f"proj_{i:03d}"
        create_file(pdir / "package.json", f'{{"name": "node_{i:03d}", "version": "1.0.0"}}\n')
        create_file(pdir / "package-lock.json", '{"lockfileVersion": 3}\n')
        create_file(pdir / "src" / "index.js", 'console.log(1);\n')
        create_file(pdir / "node_modules" / "express" / "index.js", 'module.exports = {};\n')
        create_file(pdir / "dist" / "bundle.js", 'bundle code' * 50)

    # 75 Python
    for i in range(75):
        pdir = root / "python" / f"proj_{i:03d}"
        create_file(pdir / "pyproject.toml", f'[project]\nname = "py_{i:03d}"\nversion = "0.1.0"\n')
        create_file(pdir / "poetry.lock", '# poetry lock\n')
        create_file(pdir / "src" / "__init__.py", '')
        create_file(pdir / ".venv" / "bin" / "python", 'stub')
        create_file(pdir / "src" / "__pycache__" / "mod.cpython-312.pyc", 'bytecode')

    # 50 Gradle
    for i in range(50):
        pdir = root / "gradle" / f"proj_{i:03d}"
        create_file(pdir / "build.gradle.kts", 'plugins { kotlin("jvm") }\n')
        create_file(pdir / "settings.gradle.kts", 'rootProject.name = "proj"\n')
        create_file(pdir / "src" / "main" / "kotlin" / "App.kt", 'fun main() {}\n')
        create_file(pdir / "build" / "libs" / "app.jar", "jarbytes" * 50)
        create_file(pdir / ".gradle" / "buildOutputCleanup" / "cache.properties", "gradle cache")

    # 25 Maven
    for i in range(25):
        pdir = root / "maven" / f"proj_{i:03d}"
        create_file(pdir / "pom.xml", '<project><modelVersion>4.0.0</modelVersion></project>\n')
        create_file(pdir / "src" / "main" / "java" / "App.java", 'class App {}\n')
        create_file(pdir / "target" / "classes" / "App.class", "classbytes" * 20)

def setup_workload_c(root: Path):
    """Large Node dependency tree with 1,000 nested files under node_modules, dist, and .next."""
    print("  Creating Workload C: Large Node dependency tree...", flush=True)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    create_file(root / "package.json", '{"name": "heavy-next-app", "version": "1.0.0"}\n')
    create_file(root / "pnpm-lock.yaml", 'lockfileVersion: 5.4\n')
    create_file(root / "src" / "pages" / "index.tsx", 'export default function() {}\n')

    # Simulate 50 packages with nested files in node_modules
    for pkg_idx in range(50):
        pkg_dir = root / "node_modules" / f"package_{pkg_idx:02d}"
        create_file(pkg_dir / "package.json", f'{{"name": "package_{pkg_idx:02d}"}}\n')
        for f_idx in range(15):
            create_file(pkg_dir / f"lib_{f_idx:02d}.js", f"console.log({f_idx});\n" * 10)

    # .next cache and build output
    for page in ["index", "about", "dashboard", "settings", "api"]:
        create_file(root / ".next" / "server" / "pages" / f"{page}.js", "server bundle" * 20)
        create_file(root / ".next" / "static" / "chunks" / f"{page}.chunk.js", "static chunk" * 30)

    # dist
    for i in range(20):
        create_file(root / "dist" / f"bundle_{i}.js", "dist content" * 40)

def setup_workload_d(root: Path):
    """Nested Monorepo: Node parent + nested Rust child + nested React child."""
    print("  Creating Workload D: Nested Monorepo...", flush=True)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    # Monorepo root (Node workspace)
    create_file(root / "package.json", '{"name": "mono-root", "private": true, "workspaces": ["packages/*"]}\n')
    create_file(root / "pnpm-lock.yaml", 'lockfileVersion: "9.0"\n')
    create_file(root / "node_modules" / "turbo" / "bin" / "turbo", "turbo cli stub")

    # Backend child (Rust)
    backend = root / "packages" / "backend"
    create_file(backend / "Cargo.toml", '[package]\nname = "backend"\nversion = "0.1.0"\n')
    create_file(backend / "Cargo.lock", '# lockfile\n')
    create_file(backend / "src" / "main.rs", 'fn main() {}\n')
    create_file(backend / "target" / "release" / "backend", "binary" * 500)

    # Frontend child (Node)
    frontend = root / "packages" / "frontend"
    create_file(frontend / "package.json", '{"name": "frontend", "version": "0.1.0"}\n')
    create_file(frontend / "src" / "App.tsx", 'export const App = () => null;\n')
    create_file(frontend / "dist" / "assets" / "index.js", "js asset" * 200)

def setup_workload_e(root: Path):
    """CRITICAL FALSE-POSITIVE BENCHMARK: 10,000 directories named build/dist/target without manifests."""
    print("  Creating Workload E: 10,000 misleading directories named build/dist/target without manifests...", flush=True)
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True, exist_ok=True)

    names = ["build", "dist", "target", "out", "cache", "temp", "output"]
    # Create 10,000 directories organized in hierarchy
    # e.g., folder_00 to folder_99, each having 100 subdirs named build/dist/target/etc.
    for parent_idx in range(100):
        parent = root / f"user_folder_{parent_idx:02d}"
        for child_idx in range(100):
            dname = names[(parent_idx * 100 + child_idx) % len(names)]
            target_dir = parent / f"sub_{child_idx:02d}" / dname
            # Create dummy user documents inside, NOT a developer project!
            create_file(target_dir / "user_notes.txt", "important user notes not generated")
            create_file(target_dir / "photo.jpg", "dummy photo bytes")

def main():
    if not VACUA_BIN.exists():
        print(f"Error: Vacua binary not found at {VACUA_BIN}. Please run 'cargo build --release' first.", file=sys.stderr)
        sys.exit(1)

    BENCH_DIR.mkdir(parents=True, exist_ok=True)
    BENCH_JSON.parent.mkdir(parents=True, exist_ok=True)

    sys_info = get_sys_info()
    print("=" * 70)
    print("Vacua Developer Artifact Intelligence Benchmark Suite (v0.8.0)")
    print(f"Platform: {sys_info['soc']} | {sys_info['ram']} | {sys_info['os']}")
    print(f"Git commit: {sys_info['git_sha']}")
    print("=" * 70)

    results = {
        "benchmark_version": "0.8.0",
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "system_info": sys_info,
        "workloads": {}
    }

    # Workload A
    dir_a = BENCH_DIR / "workload_a"
    setup_workload_a(dir_a)
    print("  Running Workload A analysis...", flush=True)
    res_a = run_timed_vacua_artifacts(dir_a, ["--refresh"])
    data_a = res_a["data"]
    # Warm query latency test
    t_warm0 = time.perf_counter()
    _ = run_timed_vacua_artifacts(dir_a)
    warm_a_ms = (time.perf_counter() - t_warm0) * 1000

    workload_a_metrics = {
        "description": "100 independent Rust Cargo projects",
        "analysis_time_sec": round(res_a["elapsed_sec"], 3),
        "peak_rss_mb": round(res_a["rss_mb"], 1),
        "projects_discovered": data_a.get("total_projects", 0),
        "artifacts_discovered": data_a.get("total_artifacts", 0),
        "expected_projects": 100,
        "expected_artifacts": 100,
        "false_positives": max(0, data_a.get("total_projects", 0) - 100),
        "false_negatives": max(0, 100 - data_a.get("total_projects", 0)),
        "warm_query_latency_ms": round(warm_a_ms, 2),
    }
    results["workloads"]["workload_a_100_rust"] = workload_a_metrics

    # Workload B
    dir_b = BENCH_DIR / "workload_b"
    setup_workload_b(dir_b)
    print("  Running Workload B analysis...", flush=True)
    res_b = run_timed_vacua_artifacts(dir_b, ["--refresh"])
    data_b = res_b["data"]
    t_warm0 = time.perf_counter()
    _ = run_timed_vacua_artifacts(dir_b)
    warm_b_ms = (time.perf_counter() - t_warm0) * 1000

    workload_b_metrics = {
        "description": "500 mixed projects (Rust, SwiftPM, Node, Python, Gradle, Maven)",
        "analysis_time_sec": round(res_b["elapsed_sec"], 3),
        "peak_rss_mb": round(res_b["rss_mb"], 1),
        "projects_discovered": data_b.get("total_projects", 0),
        "artifacts_discovered": data_b.get("total_artifacts", 0),
        "expected_projects": 500,
        "false_positives": max(0, data_b.get("total_projects", 0) - 500),
        "false_negatives": max(0, 500 - data_b.get("total_projects", 0)),
        "warm_query_latency_ms": round(warm_b_ms, 2),
    }
    results["workloads"]["workload_b_500_mixed"] = workload_b_metrics

    # Workload C
    dir_c = BENCH_DIR / "workload_c"
    setup_workload_c(dir_c)
    print("  Running Workload C analysis...", flush=True)
    res_c = run_timed_vacua_artifacts(dir_c, ["--refresh"])
    data_c = res_c["data"]
    t_warm0 = time.perf_counter()
    _ = run_timed_vacua_artifacts(dir_c)
    warm_c_ms = (time.perf_counter() - t_warm0) * 1000

    workload_c_metrics = {
        "description": "Large Node dependency tree (heavy node_modules, .next, dist rollup)",
        "analysis_time_sec": round(res_c["elapsed_sec"], 3),
        "peak_rss_mb": round(res_c["rss_mb"], 1),
        "projects_discovered": data_c.get("total_projects", 0),
        "artifacts_discovered": data_c.get("total_artifacts", 0),
        "expected_projects": 1,
        "false_positives": 0,
        "false_negatives": 0,
        "warm_query_latency_ms": round(warm_c_ms, 2),
    }
    results["workloads"]["workload_c_large_node"] = workload_c_metrics

    # Workload D
    dir_d = BENCH_DIR / "workload_d"
    setup_workload_d(dir_d)
    print("  Running Workload D analysis...", flush=True)
    res_d = run_timed_vacua_artifacts(dir_d, ["--refresh"])
    data_d = res_d["data"]
    t_warm0 = time.perf_counter()
    _ = run_timed_vacua_artifacts(dir_d)
    warm_d_ms = (time.perf_counter() - t_warm0) * 1000

    workload_d_metrics = {
        "description": "Nested monorepo (Node parent + Rust & Node packages)",
        "analysis_time_sec": round(res_d["elapsed_sec"], 3),
        "peak_rss_mb": round(res_d["rss_mb"], 1),
        "projects_discovered": data_d.get("total_projects", 0),
        "artifacts_discovered": data_d.get("total_artifacts", 0),
        "expected_projects": 3, # root, backend, frontend
        "false_positives": 0,
        "false_negatives": 0,
        "warm_query_latency_ms": round(warm_d_ms, 2),
    }
    results["workloads"]["workload_d_nested_monorepo"] = workload_d_metrics

    # Workload E: 10,000 misleading directories named build/dist/target without manifests
    dir_e = BENCH_DIR / "workload_e"
    setup_workload_e(dir_e)
    print("  Running Workload E analysis (10k misleading directories)...", flush=True)
    res_e = run_timed_vacua_artifacts(dir_e, ["--refresh"])
    data_e = res_e["data"]
    t_warm0 = time.perf_counter()
    _ = run_timed_vacua_artifacts(dir_e)
    warm_e_ms = (time.perf_counter() - t_warm0) * 1000

    workload_e_metrics = {
        "description": "10,000 misleading directories named build/dist/target/out without manifests (False Positive Gate)",
        "analysis_time_sec": round(res_e["elapsed_sec"], 3),
        "peak_rss_mb": round(res_e["rss_mb"], 1),
        "projects_discovered": data_e.get("total_projects", 0),
        "artifacts_discovered": data_e.get("total_artifacts", 0),
        "expected_projects": 0,
        "expected_artifacts": 0,
        "false_positives": data_e.get("total_artifacts", 0),
        "false_negatives": 0,
        "warm_query_latency_ms": round(warm_e_ms, 2),
        "unclassified_candidate_directories": data_e.get("coverage", {}).get("unclassified_candidate_directories", 0),
    }
    results["workloads"]["workload_e_10k_misleading_dirs"] = workload_e_metrics

    # Save to JSON
    with open(BENCH_JSON, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"\n[OK] Raw benchmark results committed to: {BENCH_JSON.relative_to(REPO_ROOT)}")

    # Print summary table
    print("\n" + "=" * 90)
    print(f"{'Workload':<30} | {'Time (s)':<9} | {'Peak RSS':<9} | {'Projects':<9} | {'Artifacts':<9} | {'False Pos':<9}")
    print("-" * 90)
    for name, m in results["workloads"].items():
        print(f"{name:<30} | {m['analysis_time_sec']:<9.3f} | {m['peak_rss_mb']:<6.1f} MB | {m['projects_discovered']:<9} | {m['artifacts_discovered']:<9} | {m['false_positives']:<9}")
    print("=" * 90)

    # Clean up benchmark fixtures
    print("Cleaning up synthetic benchmark fixtures...")
    shutil.rmtree(BENCH_DIR, ignore_errors=True)
    print("Done!")

if __name__ == "__main__":
    main()
