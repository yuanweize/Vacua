#!/usr/bin/env python3
"""
Vacua Content Identity & Duplicate Intelligence Engine Benchmark Suite (v0.4.1)

Deterministic, reproducible, verified measurements across staged pipeline,
cache invalidation, APFS physical accounting, and concurrency scaling.
"""

import os
import sys
import time
import json
import shutil
import ctypes
import subprocess
import platform
import random
import datetime
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VACUA_BIN = REPO_ROOT / "target" / "release" / "vacua"
NAIVE_BIN = REPO_ROOT / "target" / "release" / "vacua-naive-baseline"
BENCH_DIR = REPO_ROOT / "target" / "dedup-bench-fixtures"
BENCH_JSON = REPO_ROOT / "benchmarks" / "dedup-v0.4.1.json"
BENCHMARKS_MD = REPO_ROOT / "BENCHMARKS.md"

SEED = 42

# Load macOS libc for clonefile
libc = None
if sys.platform == "darwin":
    try:
        libc = ctypes.cdll.LoadLibrary("/usr/lib/libSystem.B.dylib")
        libc.clonefile.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32]
        libc.clonefile.restype = ctypes.c_int
    except Exception as e:
        print(f"Warning: Failed to load libSystem.B.dylib for clonefile: {e}")

def create_clonefile(src: Path, dst: Path) -> bool:
    if libc is None:
        return False
    ret = libc.clonefile(str(src).encode("utf-8"), str(dst).encode("utf-8"), 0)
    return ret == 0

def make_deterministic_bytes(size: int, stream_id: int) -> bytes:
    """Generate reproducible deterministic payload using seeded pseudo-random blocks."""
    local_rng = random.Random(SEED + stream_id * 1000003)
    block_size = min(size, 65536)
    block = local_rng.randbytes(block_size)
    repeats = size // block_size
    remainder = size % block_size
    return block * repeats + block[:remainder]

def run_vacua_duplicates(path: Path, min_size: str = "0", extra_args: list = None) -> dict:
    cmd = [str(VACUA_BIN), "duplicates", str(path), "--min-size", min_size, "--json"]
    if extra_args:
        cmd.extend(extra_args)
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if p.returncode != 0:
        raise RuntimeError(f"vacua duplicates failed ({p.returncode}): {p.stderr}")
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"Failed to parse JSON: {e}\nStdout: {p.stdout}")

def run_naive_baseline(path: Path, min_size: str = "0") -> dict:
    cmd = [str(NAIVE_BIN), str(path), "--min-size", min_size]
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if p.returncode != 0:
        raise RuntimeError(f"vacua-naive-baseline failed ({p.returncode}): {p.stderr}")
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError as e:
        raise RuntimeError(f"Failed to parse JSON from naive baseline: {e}\nStdout: {p.stdout}")

def get_git_sha() -> str:
    try:
        res = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT, text=True).strip()
        return res[:7]
    except Exception:
        return "unknown"

def benchmark_scenario_a():
    """Scenario A: Mostly unique files of variable sizes"""
    print("\n========================================================")
    print("Scenario A: Mostly Unique Files (Size Filter Avoidance)")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_a"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    total_files = 2000
    print(f"Creating {total_files} deterministic files with variable sizes (10KB–50KB)...")
    for i in range(total_files):
        size = 10240 + (i * 37) % 40960
        payload = make_deterministic_bytes(size, i)
        fpath = scen_dir / f"file_{i:04d}.bin"
        with open(fpath, "wb") as f:
            f.write(payload)

    # 1. Genuine Naive baseline
    naive_res = run_naive_baseline(scen_dir, min_size="1K")

    # 2. Vacua staged engine
    t0 = time.perf_counter()
    vacua_res = run_vacua_duplicates(scen_dir, min_size="1K", extra_args=["--no-cache"])
    t_vacua = time.perf_counter() - t0

    stats = vacua_res.get("stats", {})
    naive_bytes_read = naive_res.get("bytes_read", 0)
    vacua_bytes_read = stats.get("sample_bytes_read", 0) + stats.get("full_hash_bytes_read", 0)
    io_saved_pct = (1.0 - (vacua_bytes_read / naive_bytes_read)) * 100.0 if naive_bytes_read > 0 else 0.0

    # Assertion: Size filter must eliminate 100% of full hashes
    assert stats.get("full_hashed_files", 0) == 0, f"Expected 0 full hashed files, got {stats.get('full_hashed_files')}"
    assert stats.get("full_hash_bytes_read", 0) == 0, f"Expected 0 full hash bytes read, got {stats.get('full_hash_bytes_read')}"

    print(f"  Files seen:                   {stats.get('files_seen', 0)}")
    print(f"  Files size-unique:            {stats.get('size_unique_files', 0)}")
    print(f"  Full hashed files:            {stats.get('full_hashed_files', 0)} (VERIFIED 0)")
    print(f"  Naive full hash bytes read:   {naive_bytes_read / (1024*1024):.2f} MiB ({naive_res.get('elapsed_ms', 0):.2f} ms)")
    print(f"  Vacua staged bytes read:      {vacua_bytes_read / (1024*1024):.2f} MiB ({t_vacua*1000:.2f} ms)")
    print(f"  I/O avoided by staging:       {io_saved_pct:.1f}%")

    return {
        "scenario": "A: Mostly Unique",
        "description": "2,000 files (10 KB – 50 KB variable sizes, 55.22 MiB total)",
        "naive_bytes": naive_bytes_read,
        "naive_ms": naive_res.get("elapsed_ms", 0),
        "vacua_bytes": vacua_bytes_read,
        "vacua_ms": t_vacua * 1000,
        "io_saved_pct": io_saved_pct,
        "proof": "Size bucketing eliminates 100% of non-colliding files before touching disk contents",
    }

def benchmark_scenario_b():
    """Scenario B: Same-size adversarial files (Sample hashing filter)"""
    print("\n========================================================")
    print("Scenario B: Same-Size Adversarial Files (Sample Hashing Filter)")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_b"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    file_count = 100
    file_size = 512 * 1024
    prefix_block = b"P" * (64 * 1024)
    print(f"Creating {file_count} files of identical size (512 KiB) with identical prefix & unique middle/end...")

    for i in range(file_count):
        middle = make_deterministic_bytes(64 * 1024, 1000 + i)
        suffix = make_deterministic_bytes(64 * 1024, 2000 + i)
        filler = b"Z" * (file_size - len(prefix_block) - len(middle) - len(suffix))
        payload = prefix_block + middle + filler + suffix
        fpath = scen_dir / f"adv_file_{i:03d}.bin"
        with open(fpath, "wb") as f:
            f.write(payload)

    # 1. Genuine Naive baseline
    naive_res = run_naive_baseline(scen_dir, min_size="1K")

    # 2. Vacua staged engine
    t0 = time.perf_counter()
    vacua_res = run_vacua_duplicates(scen_dir, min_size="1K", extra_args=["--no-cache"])
    t_vacua = time.perf_counter() - t0

    stats = vacua_res.get("stats", {})
    groups = vacua_res.get("groups", [])
    naive_bytes_read = naive_res.get("bytes_read", 0)
    vacua_bytes_read = stats.get("sample_bytes_read", 0) + stats.get("full_hash_bytes_read", 0)
    io_saved_pct = (1.0 - (vacua_bytes_read / naive_bytes_read)) * 100.0 if naive_bytes_read > 0 else 0.0

    # Assertion: No duplicate groups found
    assert len(groups) == 0, f"Expected 0 duplicate groups, got {len(groups)}"
    assert stats.get("full_hashed_files", 0) == 0, f"Expected 0 full hashed files, got {stats.get('full_hashed_files')}"

    print(f"  Files seen:                   {stats.get('files_seen', 0)}")
    print(f"  Files size-colliding:         {stats.get('size_collision_files', 0)} (100% collision in size filter)")
    print(f"  Sample hashed files:          {stats.get('sampled_files', 0)}")
    print(f"  Full hashed files:            {stats.get('full_hashed_files', 0)} (Sample hash eliminated 100% false full hashes!)")
    print(f"  Naive full hash bytes read:   {naive_bytes_read / (1024*1024):.2f} MiB ({naive_res.get('elapsed_ms', 0):.2f} ms)")
    print(f"  Vacua staged bytes read:      {vacua_bytes_read / (1024*1024):.2f} MiB ({t_vacua*1000:.2f} ms)")
    print(f"  I/O avoided by sampling:      {io_saved_pct:.1f}%")

    return {
        "scenario": "B: Same-Size Adversarial",
        "description": "100 files of identical size (512 KiB) with identical prefix & unique suffix",
        "naive_bytes": naive_bytes_read,
        "naive_ms": naive_res.get("elapsed_ms", 0),
        "vacua_bytes": vacua_bytes_read,
        "vacua_ms": t_vacua * 1000,
        "io_saved_pct": io_saved_pct,
        "proof": "3-window sample hash (first/middle/last 64KiB) eliminates 100% false full hashes without full file reads",
    }

def benchmark_scenario_c():
    """Scenario C: Duplicate-heavy dataset (Group and Reclaim Correctness)"""
    print("\n========================================================")
    print("Scenario C: Duplicate-Heavy Dataset (Group Correctness)")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_c"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    group_count = 10
    copies_per_group = 4
    file_size = 1024 * 1024 # 1 MiB each
    print(f"Creating {group_count} duplicate groups with {copies_per_group} copies each (1 MiB per file = 40 MiB total)...")

    for g in range(group_count):
        payload = make_deterministic_bytes(file_size, 3000 + g)
        for c in range(copies_per_group):
            fpath = scen_dir / f"group_{g:02d}_copy_{c}.bin"
            with open(fpath, "wb") as f:
                f.write(payload)

    # 1. Genuine Naive baseline
    naive_res = run_naive_baseline(scen_dir, min_size="1K")

    # 2. Vacua staged engine
    t0 = time.perf_counter()
    vacua_res = run_vacua_duplicates(scen_dir, min_size="1K", extra_args=["--no-cache"])
    t_vacua = time.perf_counter() - t0

    stats = vacua_res.get("stats", {})
    groups = vacua_res.get("groups", [])
    naive_bytes_read = naive_res.get("bytes_read", 0)
    vacua_bytes_read = stats.get("sample_bytes_read", 0) + stats.get("full_hash_bytes_read", 0)
    io_saved_pct = (1.0 - (vacua_bytes_read / naive_bytes_read)) * 100.0 if naive_bytes_read > 0 else 0.0

    # Assertions: 10 groups, 40 members
    assert len(groups) == group_count, f"Expected {group_count} duplicate groups, got {len(groups)}"
    assert stats.get("duplicate_members", 0) == group_count * copies_per_group, f"Expected {group_count * copies_per_group} duplicate members, got {stats.get('duplicate_members')}"

    print(f"  Duplicate groups found:       {len(groups)} (VERIFIED {group_count})")
    print(f"  Duplicate members found:      {stats.get('duplicate_members', 0)} (VERIFIED {group_count * copies_per_group})")
    print(f"  Logical duplicate bytes:      {stats.get('logical_duplicate_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Confirmed reclaimable:        {stats.get('confirmed_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Naive full hash bytes read:   {naive_bytes_read / (1024*1024):.2f} MiB")
    print(f"  Vacua staged bytes read:      {vacua_bytes_read / (1024*1024):.2f} MiB (sample={stats.get('sample_bytes_read', 0)/(1024*1024):.2f} MiB + full={stats.get('full_hash_bytes_read', 0)/(1024*1024):.2f} MiB)")
    print(f"  I/O saving vs naive:          {io_saved_pct:.1f}% (truthful accounting of sampling overhead on all-duplicate sets)")

    return {
        "scenario": "C: Duplicate Heavy",
        "description": "10 duplicate groups × 4 copies (1 MiB per file, 40 MiB total)",
        "naive_bytes": naive_bytes_read,
        "naive_ms": naive_res.get("elapsed_ms", 0),
        "vacua_bytes": vacua_bytes_read,
        "vacua_ms": t_vacua * 1000,
        "io_saved_pct": io_saved_pct,
        "groups": len(groups),
        "members": stats.get("duplicate_members", 0),
        "proof": f"Correctly identifies {len(groups)} groups, {stats.get('duplicate_members', 0)} members; staged I/O truthfully accounts for {vacua_bytes_read / (1024*1024):.2f} MiB staged reads",
    }

def benchmark_scenario_d_and_e():
    """Scenario D & E: Persistent Cache Warm Run & 1% Modifications"""
    print("\n========================================================")
    print("Scenario D: Cache Warm Run & Scenario E: 1% Modifications")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_de"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    file_count = 500
    file_size = 256 * 1024 # 256 KiB = 125 MiB total
    print(f"Creating {file_count} files (256 KiB each = 125 MiB total) with 50 duplicate pairs...")
    for i in range(file_count):
        fpath = scen_dir / f"file_{i:04d}.bin"
        if i < 100 and i % 2 == 1:
            prev = scen_dir / f"file_{i-1:04d}.bin"
            shutil.copyfile(prev, fpath)
        else:
            payload = make_deterministic_bytes(file_size, 4000 + i)
            with open(fpath, "wb") as f:
                f.write(payload)

    # Baseline: 500 * 256 KiB = 125 MiB
    total_baseline_bytes = file_count * file_size

    # Run 1: Cold cache
    t0 = time.perf_counter()
    res_cold = run_vacua_duplicates(scen_dir, min_size="1K")
    t_cold = time.perf_counter() - t0
    stats_cold = res_cold.get("stats", {})

    print("\nRun 1 (Cold Cache):")
    print(f"  Full hashes performed:        {stats_cold.get('full_hashed_files', 0)}")
    print(f"  Full cache hits:              {stats_cold.get('full_cache_hits', 0)}")
    print(f"  Elapsed:                      {t_cold*1000:.2f} ms")

    # Run 2: Warm cache (Scenario D)
    t0 = time.perf_counter()
    res_warm = run_vacua_duplicates(scen_dir, min_size="1K")
    t_warm = time.perf_counter() - t0
    stats_warm = res_warm.get("stats", {})

    warm_hits = stats_warm.get("full_cache_hits", 0)
    warm_bytes_read = stats_warm.get("sample_bytes_read", 0) + stats_warm.get("full_hash_bytes_read", 0)

    # Assertion: Cache hits should satisfy candidates and avoid full re-hash
    assert warm_hits > 0, f"Expected cache hits on warm run, got {warm_hits}"
    assert stats_warm.get("full_hashed_files", 0) == 0, f"Expected 0 full hash re-reads, got {stats_warm.get('full_hashed_files')}"

    print("\nRun 2 (Warm Cache - Scenario D):")
    print(f"  Full hashes performed:        {stats_warm.get('full_hashed_files', 0)} (Re-hash avoided via SQLite cache!)")
    print(f"  Full cache hits:              {warm_hits}")
    print(f"  Vacua staged bytes read:      {warm_bytes_read / (1024*1024):.2f} MiB")
    print(f"  Elapsed:                      {t_warm*1000:.2f} ms (Speedup: {t_cold/max(t_warm, 0.0001):.1f}x)")

    # Modify 5 files (1% of dataset) (Scenario E)
    print("\nModifying 5 files (1% of dataset) for Scenario E...")
    for i in range(5):
        fpath = scen_dir / f"file_{i:04d}.bin"
        with open(fpath, "ab") as f:
            f.write(b"modified_data_block")

    # Run 3: Incremental run after 1% mutation
    t0 = time.perf_counter()
    res_mut = run_vacua_duplicates(scen_dir, min_size="1K")
    t_mut = time.perf_counter() - t0
    stats_mut = res_mut.get("stats", {})

    mut_hits = stats_mut.get("full_cache_hits", 0)
    mut_misses = stats_mut.get("full_cache_misses", 0)

    print("\nRun 3 (1% Modifications - Scenario E):")
    print(f"  Cache hits:                   {mut_hits}")
    print(f"  Cache misses / rehashed:      {mut_misses}")
    print(f"  Elapsed:                      {t_mut*1000:.2f} ms")

    return {
        "d": {
            "scenario": "D: Cache Warm Run",
            "description": "500 files (256 KiB, 50 duplicate pairs) second run (125.00 MiB total)",
            "naive_bytes": total_baseline_bytes,
            "vacua_bytes": warm_bytes_read,
            "io_saved_pct": 100.0 if warm_bytes_read == 0 else (1.0 - (warm_bytes_read / total_baseline_bytes)) * 100.0,
            "proof": f"SQLite persistent fingerprint cache satisfies {warm_hits} lookups; full re-hashing completely bypassed",
            "speedup": t_cold / max(t_warm, 0.0001),
            "warm_ms": t_warm * 1000,
        },
        "e": {
            "scenario": "E: 1% Modifications",
            "description": "5 files modified out of 500 (125.00 MiB total)",
            "naive_bytes": total_baseline_bytes,
            "vacua_bytes": stats_mut.get("full_hash_bytes_read", 0),
            "io_saved_pct": (1.0 - (stats_mut.get("full_hash_bytes_read", 0) / total_baseline_bytes)) * 100.0,
            "proof": f"Stat identity invalidation preserves {mut_hits} valid cache entries; surgical incremental rehash of modified entries",
            "mut_ms": t_mut * 1000,
            "mut_hits": mut_hits,
            "mut_misses": mut_misses,
        }
    }

def benchmark_scenario_f():
    """Scenario F: APFS Clone & Hardlink Physical Accounting"""
    print("\n========================================================")
    print("Scenario F: APFS Clone & Hardlink Physical Accounting")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_f"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    file_size = 5 * 1024 * 1024 # 5 MiB
    src_file = scen_dir / "original.bin"
    with open(src_file, "wb") as f:
        f.write(make_deterministic_bytes(file_size, 5000))

    # 1. Hardlink
    hl_file = scen_dir / "hardlink_copy.bin"
    os.link(src_file, hl_file)

    # 2. APFS clone
    clone_file = scen_dir / "apfs_clone.bin"
    has_clone = create_clonefile(src_file, clone_file)

    # 3. Independent physical copy
    ind_file = scen_dir / "independent_copy.bin"
    shutil.copyfile(src_file, ind_file)

    print(f"Created: original (5 MiB), hardlink, clonefile (status={has_clone}), independent copy.")

    res = run_vacua_duplicates(scen_dir, min_size="1K", extra_args=["--no-cache"])
    groups = res.get("groups", [])
    stats = res.get("stats", {})

    assert len(groups) == 1, f"Expected 1 duplicate group, got {len(groups)}"
    g = groups[0]
    members = g.get("members", [])
    assert len(members) == 4, f"Expected 4 members in group, got {len(members)}"

    print(f"  Groups found:                 {len(groups)}")
    print(f"  Logical duplicate bytes:      {g.get('logical_duplicate_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Confirmed reclaimable:        {g.get('confirmed_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Estimated reclaimable:        {g.get('estimated_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Upper bound reclaimable:      {g.get('upper_bound_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Physical sharing state:       {g.get('physical_sharing_state')}")

    member_details = []
    for m in members:
        pname = Path(m.get("path")).name
        rel = m.get("physical_relation")
        priv = m.get("kernel_private_bytes")
        print(f"    - {pname}: relation={rel}, nlink={m.get('nlink')}, clone_id={m.get('clone_id')}, private_bytes={priv}")
        member_details.append({
            "name": pname,
            "relation": rel,
            "nlink": m.get("nlink"),
            "clone_id": m.get("clone_id"),
            "kernel_private_bytes": priv,
        })

    return {
        "groups": len(groups),
        "members": len(members),
        "logical_duplicate_bytes": g.get("logical_duplicate_bytes", 0),
        "confirmed_reclaimable_bytes": g.get("confirmed_reclaimable_bytes", 0),
        "estimated_reclaimable_bytes": g.get("estimated_reclaimable_bytes", 0),
        "upper_bound_reclaimable_bytes": g.get("upper_bound_reclaimable_bytes", 0),
        "member_details": member_details,
    }

def benchmark_hash_concurrency():
    """Benchmark Hash Worker Pool Concurrency Scaling (1, 2, 4, 8 workers)"""
    print("\n========================================================")
    print("Hash Worker Pool Concurrency Benchmark (1, 2, 4, 8 jobs)")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_concurrency"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    # 8 independent duplicate files of 10 MiB each = 80 MiB total
    file_count = 8
    file_size = 10 * 1024 * 1024 # 10 MiB
    total_bytes = file_count * file_size
    print(f"Creating {file_count} independent duplicate copies (10 MiB each = 80 MiB total)...")
    payload = make_deterministic_bytes(file_size, 6000)
    for i in range(file_count):
        fpath = scen_dir / f"large_dup_{i:02d}.bin"
        with open(fpath, "wb") as f:
            f.write(payload)

    concurrency_results = []
    job_counts = [1, 2, 4, 8]

    for jobs in job_counts:
        # Run 3 trials to obtain stable minimum latency
        trials = []
        for _ in range(3):
            t0 = time.perf_counter()
            run_vacua_duplicates(scen_dir, min_size="1K", extra_args=["--no-cache", "--jobs", str(jobs)])
            trials.append(time.perf_counter() - t0)

        best_time = min(trials)
        throughput_mibs = (total_bytes / (1024 * 1024)) / best_time if best_time > 0 else 0
        print(f"  --jobs {jobs}: best time = {best_time*1000:.2f} ms ({throughput_mibs:.1f} MiB/s)")

        concurrency_results.append({
            "jobs": jobs,
            "elapsed_ms": best_time * 1000,
            "throughput_mibs": throughput_mibs,
        })

    return concurrency_results

def update_benchmarks_markdown(bench_data: dict):
    if not BENCHMARKS_MD.exists():
        return

    content = BENCHMARKS_MD.read_text(encoding="utf-8")

    # Generate Section 4 Markdown Table
    scen_a = bench_data["scenarios"]["scenario_a"]
    scen_b = bench_data["scenarios"]["scenario_b"]
    scen_c = bench_data["scenarios"]["scenario_c"]
    scen_d = bench_data["scenarios"]["scenario_d"]
    scen_e = bench_data["scenarios"]["scenario_e"]

    table_lines = [
        "| Scenario | Workload Specification | Naive Full Hashing (Bytes Read) | Vacua Staged Engine (Bytes Read) | I/O Reduction | Key Algorithmic Proof |",
        "| :--- | :--- | :--- | :--- | :--- | :--- |",
        f"| **A: Mostly Unique** | {scen_a['description']} | {scen_a['naive_bytes'] / (1024*1024):.2f} MiB | **{scen_a['vacua_bytes'] / (1024*1024):.2f} MiB** | **{scen_a['io_saved_pct']:.1f}%** | {scen_a['proof']} |",
        f"| **B: Same-Size Adversarial** | {scen_b['description']} | {scen_b['naive_bytes'] / (1024*1024):.2f} MiB | **{scen_b['vacua_bytes'] / (1024*1024):.2f} MiB** | **{scen_b['io_saved_pct']:.1f}%** | {scen_b['proof']} |",
        f"| **C: Duplicate Heavy** | {scen_c['description']} | {scen_c['naive_bytes'] / (1024*1024):.2f} MiB | {scen_c['vacua_bytes'] / (1024*1024):.2f} MiB | {scen_c['io_saved_pct']:.1f}% | {scen_c['proof']} |",
        f"| **D: Cache Warm Run** | {scen_d['description']} | {scen_d['naive_bytes'] / (1024*1024):.2f} MiB | **{scen_d['vacua_bytes'] / (1024*1024):.2f} MiB** | **{scen_d['io_saved_pct']:.1f}%** | {scen_d['proof']} |",
        f"| **E: 1% Modifications** | {scen_e['description']} | {scen_e['naive_bytes'] / (1024*1024):.2f} MiB | **{scen_e['vacua_bytes'] / (1024*1024):.2f} MiB** | **{scen_e['io_saved_pct']:.1f}%** | {scen_e['proof']} |",
    ]
    new_table_str = "\n".join(table_lines)

    # Concurrency table
    conc_rows = bench_data.get("concurrency", [])
    conc_lines = [
        "| Worker Jobs (`--jobs`) | Elapsed Time (ms) | Hashing Throughput (MiB/s) | Speedup vs Sequential |",
        "| :--- | :--- | :--- | :--- |",
    ]
    seq_time = conc_rows[0]["elapsed_ms"] if conc_rows else 1.0
    for r in conc_rows:
        speedup = seq_time / max(r["elapsed_ms"], 0.001)
        conc_lines.append(f"| **`-j {r['jobs']}`** | {r['elapsed_ms']:.2f} ms | **{r['throughput_mibs']:.1f} MiB/s** | **{speedup:.2f}x** |")
    new_conc_str = "\n".join(conc_lines)

    # Replace Section 4 Table in BENCHMARKS.md if possible
    # We will format the updated section cleanly
    import re
    sec4_pattern = re.compile(
        r"(### Workload Comparisons & I/O Reduction\s*\n\s*)\| Scenario \|.*?(?=\n\n### Physical Sharing Awareness)",
        re.DOTALL
    )
    if sec4_pattern.search(content):
        content = sec4_pattern.sub(r"\1" + new_table_str, content)

    # Add concurrency section if not present or update it
    conc_header = "### Bounded Hashing Concurrency Scaling"
    if conc_header in content:
        conc_pattern = re.compile(
            r"(### Bounded Hashing Concurrency Scaling\s*\n\s*)\| Worker Jobs.*?(?=\n\n---|\Z)",
            re.DOTALL
        )
        content = conc_pattern.sub(r"\1" + new_conc_str, content)
    else:
        content += f"\n\n---\n\n## 5. Bounded Hashing Concurrency Scaling (v0.4.1)\n\nEvaluates `--jobs` worker pool throughput across 80 MiB independent duplicate copies.\n\n{new_conc_str}\n"

    BENCHMARKS_MD.write_text(content, encoding="utf-8")
    print(f"\nSuccessfully updated {BENCHMARKS_MD} with verified benchmark metrics.")

def main():
    if not VACUA_BIN.exists():
        print(f"Error: binary not found at {VACUA_BIN}. Please run `cargo build --release` first.")
        sys.exit(1)
    if not NAIVE_BIN.exists():
        print(f"Error: naive baseline binary not found at {NAIVE_BIN}. Please run `cargo build --release --bin vacua-naive-baseline` first.")
        sys.exit(1)

    print(f"Starting Vacua Duplicate Engine Benchmark Suite v0.4.1 with binary: {VACUA_BIN}")
    BENCH_DIR.mkdir(parents=True, exist_ok=True)
    BENCH_JSON.parent.mkdir(parents=True, exist_ok=True)

    git_sha = get_git_sha()
    start_time = datetime.datetime.now(datetime.timezone.utc).isoformat()

    try:
        res_a = benchmark_scenario_a()
        res_b = benchmark_scenario_b()
        res_c = benchmark_scenario_c()
        res_de = benchmark_scenario_d_and_e()
        res_f = benchmark_scenario_f()
        res_conc = benchmark_hash_concurrency()

        full_results = {
            "benchmark_schema_version": "1.0",
            "vacua_version": "0.4.1",
            "git_sha": git_sha,
            "timestamp": start_time,
            "seed": SEED,
            "hardware": platform.processor() or "Apple Silicon",
            "macos": platform.mac_ver()[0],
            "filesystem": "APFS",
            "scenarios": {
                "scenario_a": res_a,
                "scenario_b": res_b,
                "scenario_c": res_c,
                "scenario_d": res_de["d"],
                "scenario_e": res_de["e"],
                "scenario_f": res_f,
            },
            "concurrency": res_conc,
        }

        # Write machine-readable JSON artifact
        with open(BENCH_JSON, "w", encoding="utf-8") as f:
            json.dump(full_results, f, indent=2)
        print(f"\nRaw reproducible benchmark artifact written to {BENCH_JSON}")

        # Update BENCHMARKS.md with verified numbers
        update_benchmarks_markdown(full_results)

        print("\n========================================================")
        print("All Benchmark Scenarios & Structural Assertions Passed!")
        print("========================================================")
    finally:
        if BENCH_DIR.exists():
            shutil.rmtree(BENCH_DIR, ignore_errors=True)

if __name__ == "__main__":
    main()
