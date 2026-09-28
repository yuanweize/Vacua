# Benchmarks & Performance Methodology

> **Engineering Honesty Rule**:  
> Vacua rejects fabricated benchmarks and marketing metrics. Benchmark suites will only be reported once the synthetic filesystem generator and cargo bench targets are compiled and executed against calibrated hardware.

---

## 1. Planned Benchmark Methodology (Phase 3)

The following benchmark metrics are designed to evaluate real-world performance without risking user data:

- **Key Metrics Tracked**:
  - **Wall Time**: Real elapsed time for traversal, metadata evaluation, and SQLite insertion.
  - **CPU Time**: User + System CPU consumption across streaming workers.
  - **Peak RSS (Resident Set Size)**: Maximum memory allocated during deep walks (target: bounded < 64 MB RSS for 100k files).
  - **Incremental Rescan Time**: Comparison of FSEvents-driven incremental rescans against full cold disk traversals.

- **Required Reporting Metadata**:
  Every published benchmark run MUST include:
  - Exact Mac Hardware Model (e.g. `MacBook Pro (16-inch, Nov 2023)`)
  - SoC (e.g. `Apple M3 Max, 16-core CPU`)
  - Installed Memory (RAM)
  - macOS Version & Build
  - Filesystem Format (APFS, block size)
  - Rust & Toolchain Versions
  - Git Commit SHA
  - Cold vs Warm Page Cache status

---

## 2. Synthetic Test Harness Specification

To safely evaluate scale at 10k, 100k, and 1,000,000 files, Phase 3 will introduce `vacua-bench` (a synthetic directory generator):

```bash
# Generate synthetic tree in disposable sandbox (Planned Phase 3)
cargo run --bin vacua-bench -- --size 100k --out /tmp/vacua-bench-100k

# Execute traversal benchmark
cargo bench --bench scanner_traversal
```
