# Benchmarks & Performance Methodology

Project Reclaim rejects fabricated benchmarks and marketing claims. Every performance measurement recorded here must be reproducible using our open-source synthetic filesystem generator.

---

## 1. Benchmarking Principles

- **No Fake Numbers**: All metrics report real hardware, specific macOS versions, filesystem formats (APFS), and commit SHAs.
- **Cold vs Warm Cache Distinction**: Traversal benchmarks must explicitly document whether the page cache was dropped or warm.
- **Key Metrics Tracked**:
  - **Wall Time**: Real elapsed time for traversal and metadata evaluation.
  - **CPU Time**: User + System CPU consumption.
  - **Peak RSS (Resident Set Size)**: Maximum memory allocated during deep walks.
  - **Incremental Rescan Time**: Comparison of SQLite/FSEvents differential scans against full sweeps.

---

## 2. Synthetic Test Harness (Phase 3 Planned)

To test directory sizes of 10k, 100k, and 1,000,000 files without risking user data, we provide a synthetic directory generator tool:

```bash
# Generate synthetic 10k file hierarchy inside disposable ramdisk/sandbox
cargo run --bin reclaim-bench -- --size 10k --out /tmp/reclaim-bench-10k

# Run traversal benchmark
cargo bench --bench scanner_traversal
```

---

## 3. Preliminary Baseline (Apple Silicon)

*Environment*:
- Hardware: Apple Silicon (M-series)
- OS: macOS 27.0.0 (Darwin Kernel)
- Filesystem: APFS (Encrypted, 4KB Block Size)
- Toolchain: rustc 1.96.1

*Verification*:
- `crates/reclaim-scan` performs streaming traversal using POSIX `lstat`/`symlink_metadata` with zero full-content file reads during the scan phase.
- Memory consumption is strictly bounded: entries are processed iteratively with backpressure, and hard link deduplication uses compact `(u64, u64)` integer tuple sets.
