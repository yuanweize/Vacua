# Vacua Storage Engine Benchmarks (v0.3.0)

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

---

## 4. Content Identity & Duplicate Intelligence Engine (v0.4.0)

Evaluates the staged content pipeline (Stage 0: Eligibility -> Stage 1: Size Buckets -> Stage 2: Hardlink Inode Collapse -> Stage 3: APFS Clone Metadata -> Stage 4: Domain-Separated Sample Hashing -> Stage 5: Bounded BLAKE3 Streaming -> Stage 6: Destructive Confirmation) compared against naive full-content hashing. Measured using [`scripts/benchmark-dedup.py`](file:///Users/yuanweize/我的文档/服务器/GITHUB/vacua/scripts/benchmark-dedup.py).

### Workload Comparisons & I/O Reduction

| Scenario | Workload Specification | Naive Full Hashing (Bytes Read) | Vacua Staged Engine (Bytes Read) | I/O Reduction | Key Algorithmic Proof |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **A: Mostly Unique** | 2,000 files (10 KB – 50 KB variable sizes, 55.22 MiB total) | 55.22 MiB | **0.00 MiB** | **100.0%** | Size bucketing eliminates 100% of non-colliding files before touching disk contents |
| **B: Same-Size Adversarial** | 100 files of identical size (512 KiB) with identical prefix & unique suffix | 50.00 MiB | **18.75 MiB** | **62.5%** | 3-window sample hash (first/middle/last 64KiB) eliminates 100% false full hashes without full file reads |
| **C: Duplicate Heavy** | 10 duplicate groups × 4 copies (1 MiB per file, 40 MiB total) | 40.00 MiB | 40.00 MiB | 0.0% | Correctly identifies 10 groups, 40 members, 30.00 MiB logical duplicate bytes, 30.00 MiB confirmed reclaim |
| **D: Cache Warm Run** | 500 files (256 KiB, 50 duplicate pairs) second run | 25.00 MiB | **0.00 MiB re-read** | **100.0%** | SQLite persistent fingerprint cache satisfies 400 lookups; re-hashing completely bypassed |
| **E: 1% Modifications** | 5 files modified out of 500 | 25.00 MiB | **1.25 MiB** | **95.0%** | Stat identity invalidation preserves 400 valid cache entries; surgical incremental rehash |

### Physical Sharing Awareness vs. Logical Duplicates (Scenario F)

Synthetic APFS dataset containing 4 identical 5 MiB files:
1. `original.bin` (Master candidate)
2. `hardlink_copy.bin` (Direct `os.link` hardlink to `original.bin`, same inode)
3. `apfs_clone.bin` (APFS copy-on-write clone via `clonefile(2)`)
4. `independent_copy.bin` (Independent byte copy)

| Metric | Naive Duplicate Tool | Vacua APFS-Aware Engine | Physical Reality |
| :--- | :--- | :--- | :--- |
| **Detected Duplicate Copies** | 4 files | 4 files (1 Group: `dup-2a0285ddc3a9eec5`) | Correct |
| **Logical Duplicate Waste** | 15.00 MiB | **15.00 MiB** | Correct |
| **Hardlink Reclaim** | Overclaimed (+5.00 MiB) | **0.00 MiB** | Zero physical blocks reclaimed while peer link exists |
| **APFS Clone Reclaim** | Overclaimed (+5.00 MiB) | **0.00 MiB confirmed** / **0.00 MiB estimated** | Copy-on-write extents remain shared with master |
| **Independent Copy Reclaim** | 5.00 MiB | **5.00 MiB confirmed** | True independent storage blocks |
| **Total Group Reclaimable** | False claim: 15.00 MiB | **5.00 MiB confirmed** / **10.00 MiB upper bound** | Prevents user expectation mismatch |

