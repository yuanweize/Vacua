# Vacua Hierarchical Storage Map & Treemap Intelligence Engine

Vacua v0.7.0 introduces the **Hierarchical Storage Map**: a deterministic, index-backed storage intelligence engine built in Rust (`vacua-tree`) paired with a native Squarified Treemap interface in macOS SwiftUI.

---

## 1. Core Architecture

The Storage Map strictly enforces the principle:
> **"Rust computes storage hierarchy; Swift computes screen geometry."**

```text
Filesystem / Index
        │
        ▼
   vacua-tree (Rollup, Invariants, Hardlink Attribution)
        │
        ▼
   vacua-api  (Versioned DTOs & Schemas)
     │     │
     ├── CLI (`vacua tree`)
     └── MCP (`vacua_analyze_storage_map`, `vacua_get_storage_map`, `vacua_get_storage_node`)
        │
        ▼
   VacuaClient (Swift Engine Client)
        │
        ▼
Native SwiftUI Treemap (Squarified Layout, Selection, Drill-down, Snapshot Delta)
```

---

## 2. Storage Semantics & Accounting Caveats

Storage Map provides an **observed filesystem namespace attribution**, not a freeable space or physical disk map:

| Dimension | Logical Bytes (`logical_bytes`) | Allocated Bytes (`allocated_bytes`) |
| :--- | :--- | :--- |
| **Definition** | Nominal file length from `st_size` | Allocated filesystem blocks from `st_blocks * 512` |
| **Subtree Aggregation** | $\sum \text{child logical bytes} + \text{dir entry size}$ | $\sum \text{child attributed allocated bytes}$ |
| **Hardlinks** | Counts in all aliases | **Attributed to 1 representative path** (aliases receive 0) |
| **APFS Clones** | Counts in each clone | Shows allocated blocks; extent sharing is flagged |
| **Physical Truth** | Namespace representation | **Allocated blocks $\neq$ unique physical storage** |
| **Reclaimability** | **Not reclaimable bytes** | **Not reclaimable bytes** |

### Hardlink Attribution Algorithm
When multiple directory entries share the same `(device_id, inode)`:
1. The engine deterministically selects the **lexicographically smallest canonical relative path** as the primary representative.
2. The representative path receives 100% of the allocated blocks.
3. All other alias paths receive `attributed_allocated_bytes = 0` with `hardlink_alias = true`.
4. In the Inspector, aliases are marked: *"Hardlink alias — allocated storage is attributed to another path."*

### APFS Clone Extent Uncertainty
APFS allows files with different inodes to share extent blocks via copy-on-write `clonefile(2)`. The engine flags `physical_sharing_uncertainty: true` in analysis metadata, alerting users that allocated blocks do not imply isolated disk ownership.

---

## 3. Tree Build & Aggregation Algorithm

Naive recursion walks ancestors for every file, creating $O(N \times D)$ degradation. Vacua employs a single-pass iterative bottom-up rollup:
1. **Entry Ingestion**: Ingest scanned entries and initialize direct leaf nodes.
2. **Hardlink Inode Mapping**: Group paths by `(dev, ino)` to assign representative attribution.
3. **Parent Index Resolution**: Group children under parent path keys.
4. **Iterative Bottom-Up Rollup**: Process directory levels from maximum observed depth down to root (tested safely up to depth 256).
5. **Atomic SQLite Publication**: Insert nodes inside an isolated SQLite transaction and prune generations older than the latest 2 ready generations.

Complexity: **$O(N \log N)$** time, bounded memory footprint.

---

## 4. Bounded Queries & Remainder Invariants

Treemap queries do not return unbounded child arrays. Each directory query accepts `limit` (default 60, max 200) and `offset`:
- Returns top children ordered deterministically by the requested metric.
- Computes an authoritative `remainder` aggregate:
  $$\text{Parent Subtree Bytes} = \sum_{i=1}^{k} \text{Child}_i\text{ Bytes} + \text{Remainder Bytes}$$
- The UI renders `Other` as an aggregate rectangle. `Other` is purely a presentation aggregate and does **not** fabricate a fake filesystem node ID.

---

## 5. Machine API & MCP Tools

### Versioned Schemas (`vacua-api`)
- `vacua.mcp.storage-tree-analysis.v1`: Generation metadata, coverage statistics, and root node summary.
- `vacua.mcp.storage-tree-page.v1`: Bounded child nodes, pagination offsets, and remainder totals.
- `vacua.mcp.storage-tree-node-detail.v1`: Detailed single-node metrics, hardlink info, and APFS clone caveats.

### MCP Tools
1. `vacua_analyze_storage_map`: Analyzes the storage root and publishes a new READY tree generation.
2. `vacua_get_storage_map`: Queries bounded child nodes and remainder for any directory node.
3. `vacua_get_storage_node`: Retrieves full metadata and attribution details for an individual node.

All tools enforce strict capability isolation, allowed-root security policies, and expensive-operation concurrency permits.

---

## 6. Native macOS SwiftUI Treemap

### Squarified Treemap Algorithm
Implemented natively in `TreemapLayout.swift` following the algorithm of Bruls, Huizing, and van Wijk (2000):
- Recursively divides bounding rectangles to maintain aspect ratios close to 1.0 (avoiding thin, illegible slivers).
- Pre-normalizes weights to prevent division-by-zero or floating-point anomalies on huge byte counts.
- O(N) layout time for up to 200 nodes per view.

### User Experience
- **Interactive Drill-down**: Click to inspect; double-click or press Return to navigate into subdirectories.
- **Breadcrumb Bar**: Fast ancestor traversal with home-relative paths.
- **Metric Toggle**: Switch between **Allocated filesystem blocks** and **Logical file sizes**.
- **Snapshot Delta Overlay**: Colors rectangles by historical change:
  - Blue: New items
  - Red / Orange: Growth
  - Green: Shrinkage
  - Gray: Unchanged
- **Accessibility Fallback**: Switch freely between the Visual Treemap and a VoiceOver-accessible tabular List view.

---

## 7. Verified Benchmarks (Apple Silicon M4, macOS 27.0 APFS)

Executed using [`scripts/benchmark-storage-tree.py`](../scripts/benchmark-storage-tree.py):

| Workload | Specification | Build Time | Peak RSS | Query Latency | Invariant Proof |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **A: Balanced 10k** | 10,000 files across 408 directories | **236.08 ms** | 29.3 MB | 4.64 ms | Exact file count and byte rollup verified |
| **A: Balanced 50k** | 50,000 files across 2,040 directories | **1555.68 ms** | 102.6 MB | 4.58 ms | Stable linear rollup scaling |
| **B: Extreme Fanout** | 20,000 files in 1 directory (Limit 30) | **463.26 ms** | 43.8 MB | 4.86 ms | Remainder exact match: 30 items + 19,970 remainder = 20,000 |
| **C: Deep Hierarchy** | Depth 256 nested directories | **57.38 ms** | 10.9 MB | 4.50 ms | Iterative rollup; zero stack overflow |
| **D: Hardlinks** | 1,000 files $\times$ 5 aliases (5,000 entries) | **83.10 ms** | 15.2 MB | 4.60 ms | Logical: 51.3 MB; Allocated: 12.28 MB (4.18x ratio, zero double-count) |
| **E: Tiny Files** | 20,000 files (0 – 4 KB) | **353.92 ms** | 44.4 MB | 4.70 ms | APFS 4KB block allocation faithfully reflected |
| **F: Warm Index Query** | 25 iterations on warm SQLite index | — | — | **p50: 4.66 ms / p95: 5.71 ms** | Sub-6ms UI drill-down response |

---

## 8. Limitations & Boundaries

1. **No Content Reads**: The Storage Map never opens, hashes, or reads file contents. Content bytes read remains strictly 0.
2. **No Deletion Authority**: The Storage Map has zero authority to remove, trash, or unlink files.
3. **No Volume-Wide Physical Claim**: The tree represents the scanned root directory, not the entire APFS container.
