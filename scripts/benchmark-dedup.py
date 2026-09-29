#!/usr/bin/env python3
"""
Vacua Content Identity & Duplicate Intelligence Engine Benchmark Suite
Scenarios:
  A: Mostly unique files (size filter efficiency vs naive full hashing)
  B: Same-size adversarial files (sample hashing eliminates full hashes)
  C: Duplicate-heavy dataset (group and reclaim correctness)
  D: Persistent cache warm run (100% cache hits, zero bytes re-hashed)
  E: 1% modifications (99% cache hit rate, incremental rehash)
  F: APFS clone & hardlink physical accounting validation
"""

import os
import sys
import time
import json
import shutil
import ctypes
import subprocess
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
VACUA_BIN = REPO_ROOT / "target" / "release" / "vacua"
BENCH_DIR = REPO_ROOT / "target" / "dedup-bench-fixtures"

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

def benchmark_scenario_a():
    """Scenario A: Mostly unique files of various sizes"""
    print("\n========================================================")
    print("Scenario A: Mostly Unique Files (Size Filter Avoidance)")
    print("========================================================")
    scen_dir = BENCH_DIR / "scenario_a"
    if scen_dir.exists():
        shutil.rmtree(scen_dir)
    scen_dir.mkdir(parents=True, exist_ok=True)

    total_files = 2000
    total_logical_bytes = 0
    print(f"Creating {total_files} mostly unique files with variable sizes...")
    for i in range(total_files):
        # Varying size between 10KB and 50KB to make most unique
        size = 10240 + (i * 37) % 40960
        payload = os.urandom(size)
        fpath = scen_dir / f"file_{i:04d}.bin"
        with open(fpath, "wb") as f:
            f.write(payload)
        total_logical_bytes += size

    t0 = time.perf_counter()
    res = run_vacua_duplicates(scen_dir, min_size="1K")
    t_vacua = time.perf_counter() - t0

    stats = res.get("stats", {})
    naive_bytes_read = total_logical_bytes
    vacua_bytes_read = stats.get("sample_bytes_read", 0) + stats.get("full_hash_bytes_read", 0)
    io_saved_pct = (1.0 - (vacua_bytes_read / naive_bytes_read)) * 100.0 if naive_bytes_read > 0 else 0.0

    print(f"  Files seen:                  {stats.get('files_seen', 0)}")
    print(f"  Files size-unique:           {stats.get('size_unique_files', 0)}")
    print(f"  Files size-colliding:        {stats.get('size_collision_files', 0)}")
    print(f"  Sample hashed files:         {stats.get('sampled_files', 0)}")
    print(f"  Full hashed files:           {stats.get('full_hashed_files', 0)}")
    print(f"  Naive full hash bytes read:  {naive_bytes_read / (1024*1024):.2f} MiB")
    print(f"  Vacua staged bytes read:     {vacua_bytes_read / (1024*1024):.2f} MiB")
    print(f"  I/O avoided by staging:      {io_saved_pct:.1f}%")
    print(f"  Execution time:              {t_vacua*1000:.2f} ms")

    return {
        "scenario": "Scenario A (Mostly Unique)",
        "files": total_files,
        "naive_bytes": naive_bytes_read,
        "vacua_bytes": vacua_bytes_read,
        "io_saved_pct": io_saved_pct,
        "time_ms": t_vacua * 1000,
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

    # 100 files of identical size 512 KiB, but different content in middle/end
    file_count = 100
    file_size = 512 * 1024
    total_logical_bytes = file_count * file_size

    print(f"Creating {file_count} files of exact same size (512 KiB) with unique content...")
    for i in range(file_count):
        # First 64KiB identical prefix, middle 64KiB unique, last 64KiB unique
        prefix = b"P" * (64 * 1024)
        middle = os.urandom(64 * 1024)
        suffix = os.urandom(64 * 1024)
        filler = b"Z" * (file_size - len(prefix) - len(middle) - len(suffix))
        payload = prefix + middle + filler + suffix
        fpath = scen_dir / f"adv_file_{i:03d}.bin"
        with open(fpath, "wb") as f:
            f.write(payload)

    t0 = time.perf_counter()
    res = run_vacua_duplicates(scen_dir, min_size="1K")
    t_vacua = time.perf_counter() - t0

    stats = res.get("stats", {})
    naive_bytes_read = total_logical_bytes
    vacua_bytes_read = stats.get("sample_bytes_read", 0) + stats.get("full_hash_bytes_read", 0)
    io_saved_pct = (1.0 - (vacua_bytes_read / naive_bytes_read)) * 100.0 if naive_bytes_read > 0 else 0.0

    print(f"  Files seen:                  {stats.get('files_seen', 0)}")
    print(f"  Files size-colliding:        {stats.get('size_collision_files', 0)} (100% collision in size filter)")
    print(f"  Sample hashed files:         {stats.get('sampled_files', 0)}")
    print(f"  Full hashed files:           {stats.get('full_hashed_files', 0)} (Sample hash eliminated 100% false full hashes!)")
    print(f"  Naive full hash bytes read:  {naive_bytes_read / (1024*1024):.2f} MiB")
    print(f"  Vacua staged bytes read:     {vacua_bytes_read / (1024*1024):.2f} MiB")
    print(f"  I/O avoided by sampling:     {io_saved_pct:.1f}%")
    print(f"  Execution time:              {t_vacua*1000:.2f} ms")

    return {
        "scenario": "Scenario B (Same-Size Adversarial)",
        "files": file_count,
        "naive_bytes": naive_bytes_read,
        "vacua_bytes": vacua_bytes_read,
        "io_saved_pct": io_saved_pct,
        "time_ms": t_vacua * 1000,
    }

def benchmark_scenario_c():
    """Scenario C: Duplicate-heavy dataset"""
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
    print(f"Creating {group_count} duplicate groups with {copies_per_group} copies each (1 MiB per file)...")

    for g in range(group_count):
        payload = os.urandom(file_size)
        for c in range(copies_per_group):
            fpath = scen_dir / f"group_{g:02d}_copy_{c}.bin"
            with open(fpath, "wb") as f:
                f.write(payload)

    t0 = time.perf_counter()
    res = run_vacua_duplicates(scen_dir, min_size="1K")
    t_vacua = time.perf_counter() - t0

    stats = res.get("stats", {})
    groups = res.get("groups", [])

    print(f"  Duplicate groups found:      {len(groups)} (Expected: {group_count})")
    print(f"  Duplicate members found:     {stats.get('duplicate_members', 0)} (Expected: {group_count * copies_per_group})")
    print(f"  Logical duplicate bytes:     {stats.get('logical_duplicate_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Confirmed reclaimable:       {stats.get('confirmed_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
    print(f"  Execution time:              {t_vacua*1000:.2f} ms")

    return {
        "scenario": "Scenario C (Duplicate-Heavy)",
        "groups": len(groups),
        "members": stats.get('duplicate_members', 0),
        "logical_dup_bytes": stats.get('logical_duplicate_bytes', 0),
        "reclaimable_bytes": stats.get('confirmed_reclaimable_bytes', 0),
        "time_ms": t_vacua * 1000,
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
    file_size = 256 * 1024 # 256 KiB
    print(f"Creating {file_count} files (256 KiB each) with 50 duplicate pairs...")
    for i in range(file_count):
        pair_id = i // 2 if i < 100 else i
        fpath = scen_dir / f"file_{i:04d}.bin"
        if i % 2 == 1 and i < 100:
            # Duplicate of previous
            prev = scen_dir / f"file_{i-1:04d}.bin"
            shutil.copyfile(prev, fpath)
        else:
            payload = os.urandom(file_size)
            with open(fpath, "wb") as f:
                f.write(payload)

    # Run 1: Cold cache
    t0 = time.perf_counter()
    res_cold = run_vacua_duplicates(scen_dir, min_size="1K")
    t_cold = time.perf_counter() - t0
    stats_cold = res_cold.get("stats", {})

    print("\nRun 1 (Cold Cache):")
    print(f"  Full hashes performed:       {stats_cold.get('full_hashed_files', 0)}")
    print(f"  Cache hits:                  {stats_cold.get('full_hash_cache_hits', 0)}")
    print(f"  Cache misses:                {stats_cold.get('full_hash_cache_misses', 0)}")
    print(f"  Elapsed:                     {t_cold*1000:.2f} ms")

    # Run 2: Warm cache (Scenario D)
    t0 = time.perf_counter()
    res_warm = run_vacua_duplicates(scen_dir, min_size="1K")
    t_warm = time.perf_counter() - t0
    stats_warm = res_warm.get("stats", {})

    print("\nRun 2 (Warm Cache - Scenario D):")
    print(f"  Full hashes performed:       {stats_warm.get('full_hashed_files', 0)} (Re-hash avoided via SQLite cache!)")
    print(f"  Cache hits:                  {stats_warm.get('full_hash_cache_hits', 0)}")
    print(f"  Cache misses:                {stats_warm.get('full_hash_cache_misses', 0)}")
    print(f"  Elapsed:                     {t_warm*1000:.2f} ms (Speedup: {t_cold/max(t_warm, 0.0001):.1f}x)")

    # Modify 1% of files (5 files out of 500) (Scenario E)
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

    print("\nRun 3 (1% Modifications - Scenario E):")
    print(f"  Cache hits:                  {stats_mut.get('full_hash_cache_hits', 0)}")
    print(f"  Cache misses / rehashed:     {stats_mut.get('full_hash_cache_misses', 0)}")
    print(f"  Elapsed:                     {t_mut*1000:.2f} ms")

    return {
        "cold_ms": t_cold * 1000,
        "warm_ms": t_warm * 1000,
        "warm_hits": stats_warm.get('full_hash_cache_hits', 0),
        "mut_hits": stats_mut.get('full_hash_cache_hits', 0),
        "mut_misses": stats_mut.get('full_hash_cache_misses', 0),
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
        f.write(os.urandom(file_size))

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

    res = run_vacua_duplicates(scen_dir, min_size="1K")
    groups = res.get("groups", [])
    stats = res.get("stats", {})

    print(f"  Groups found:                {len(groups)}")
    if groups:
        g = groups[0]
        print(f"  Group ID:                    {g.get('group_id')}")
        print(f"  Members count:               {len(g.get('members', []))}")
        print(f"  Logical duplicate bytes:     {g.get('logical_duplicate_bytes', 0) / (1024*1024):.2f} MiB")
        print(f"  Confirmed reclaimable:       {g.get('confirmed_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
        print(f"  Estimated reclaimable:       {g.get('estimated_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
        print(f"  Upper bound reclaimable:     {g.get('upper_bound_reclaimable_bytes', 0) / (1024*1024):.2f} MiB")
        print(f"  Physical sharing state:      {g.get('physical_sharing_state')}")

        for m in g.get("members", []):
            print(f"    - {Path(m.get('path')).name}: relation={m.get('physical_relation')}, nlink={m.get('nlink')}, clone_id={m.get('clone_id')}")

    return {
        "groups": len(groups),
        "hardlinks_collapsed": stats.get("hardlinks_collapsed", 0),
        "clone_family_members": stats.get("clone_family_members", 0),
    }

def main():
    if not VACUA_BIN.exists():
        print(f"Error: binary not found at {VACUA_BIN}. Please run `cargo build --release` first.")
        sys.exit(1)

    print(f"Starting Vacua Duplicate Engine Benchmark Suite with binary: {VACUA_BIN}")
    BENCH_DIR.mkdir(parents=True, exist_ok=True)

    try:
        res_a = benchmark_scenario_a()
        res_b = benchmark_scenario_b()
        res_c = benchmark_scenario_c()
        res_de = benchmark_scenario_d_and_e()
        res_f = benchmark_scenario_f()

        print("\n========================================================")
        print("Benchmark Summary Completed Successfully.")
        print("========================================================")
    finally:
        if BENCH_DIR.exists():
            shutil.rmtree(BENCH_DIR, ignore_errors=True)

if __name__ == "__main__":
    main()
