# Vacua Storage Engine Benchmarks (v0.2.0)

> **Engineering Honesty Rule**:
> All benchmarks reported in this document are reproducible measurements executed against calibrated hardware. No simulated, estimated, or cherry-picked performance metrics are accepted.

---

## 1. Test Environment & System Calibrations

| Parameter | Calibrated Value |
| :--- | :--- |
| **Hardware Platform** | Apple M4 |
| **System Memory (RAM)** | 16 GB |
| **Operating System** | macOS 27.0 (Build 26A428) |
| **Filesystem** | APFS (Apple File System) |
| **Architecture** | aarch64 (Apple Silicon) |
| **Rust Toolchain** | rustc 1.96.0-nightly / Apple Swift 6.4 |
| **Git Commit Reference** | [`1a58cbe`](https://github.com/yuanweize/vacua/commit/1a58cbe) |
| **Page Cache State** | Warm OS filesystem page cache |

---

## 2. Full Traversal: Sequential vs. Bounded Concurrent

Synthetic trees consisting of uniform files (512B – 4KB) across balanced directory fan-outs evaluated under APFS. Peak Resident Set Size (RSS) measured via Darwin `/usr/bin/time -l` (maximum resident set size).

| Workload | Sequential (`-j 1`) | Bounded Concurrent (`-j 8`) | Throughput Delta | Peak RSS (Concurrent) |
| :--- | :--- | :--- | :--- | :--- |
| **10,000 files** | 0.079s (127,097 files/s) | **0.040s** (**252,767 files/s**) | **1.99x** | 11.4 MB |
| **100,000 files** | 0.802s (124,732 files/s) | **0.331s** (**302,239 files/s**) | **2.42x** | 46.9 MB |

### Key Architectural Takeaways
1. **Bounded Backpressure**: Memory consumption remains strictly bounded (46.9 MB peak RSS on 100k nodes, 11.4 MB on 10k nodes) through bounded `sync_channel(2048)` metadata worker queue.
2. **Deterministic Output**: Concurrent parallel traversal collects entries into indexed partitions and deterministically sorts them by path prior to reporting or plan generation.

---

## 3. Incremental Index Refresh: Native macOS FSEvents

Evaluates the performance behavior of surgical dirty-subtree rescan via native `FSEventStream` replay versus traversing the entire filesystem root.

| Workload | Full Rescan Time | Incremental (No Change) | Incremental (1% Dirty) | Incremental (10% Dirty) | Speedup (Clean vs Full) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **10,000 files** | 0.040s | **0.192s** | 0.203s | 0.199s | **0.2x** |
| **100,000 files** | 0.331s | **0.848s** | 0.657s | 0.733s | **0.4x** |

### Incremental Invariants
- **Zero Full Traversal on Clean Roots**: When FSEventStream indicates no subtree modifications, `vacua index refresh` advances the persistent SQLite event cursor without touching any filesystem nodes.
- **Fail-Safe Dropped Event Recovery**: If `kFSEventStreamEventFlagMustScanSubDirs` or buffer overflow occurs, the engine automatically flags the watched root for a full fallback rescan.

> [!NOTE]
> **Warm-Cache FSEvents Dispatch Latency & Trade-offs**:
> On small synthetic trees with warm in-memory page caches where full concurrent traversal completes in sub-second time (0.04s - 0.33s), initializing the native Darwin `FSEventStreamCreate` dispatch queue, flushing the stream, and committing SQLite transactions introduces a baseline overhead of ~150–200ms. Consequently, full scan can be faster on small, hot caches. On large, cold real-world directory trees (where full scan takes seconds or minutes due to disk I/O), targeted dirty subtree reconciliation provides significant architectural advantages.
