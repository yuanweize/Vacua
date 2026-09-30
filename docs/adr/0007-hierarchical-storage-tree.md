# ADR 0007: Hierarchical Storage Tree Engine and Squarified Treemap Architecture

## Status
Accepted (v0.7.0)

## Context
Vacua requires an interactive visual storage map to give users visibility into directory hierarchies, large files, and space distribution across scanned directories.

Prior storage tools (e.g. DaisyDisk, GrandPerspective, OmniDiskSweeper) and simple disk visualizers frequently exhibit critical architectural flaws:
1. **Frontend-Guessed Sizes**: The UI recursively sums naive file lengths or ad-hoc file lists, producing incorrect space attribution.
2. **Hardlink Double Counting**: Hardlink aliases are summed as independent allocations, severely inflating directory totals.
3. **Physical Storage Overclaiming**: Tools conflate filesystem allocated blocks (`st_blocks * 512`) with unique physical APFS disk consumption, ignoring APFS copy-on-write clone extents.
4. **Unbounded Queries and Memory Bloat**: Flattening entire 500k-node filesystems into the GUI crashes the renderer.
5. **Path Leakage and Cross-Generation Inconsistency**: Passing mutable raw paths across IPC creates race conditions and privacy hazards.

## Decision

### 1. "Rust Computes Storage Hierarchy; Swift Computes Screen Geometry"
The storage hierarchy is an authoritative filesystem model built and aggregated entirely in the Rust engine (`vacua-tree`, `vacua-index`).
- Swift code **never** traverses the filesystem, never calculates ancestor aggregations, and never determines storage truth.
- Swift operates strictly as a pure visualization client, transforming pre-computed node weights into `CGRect` coordinates via a deterministic Squarified Treemap layout algorithm.

### 2. Dual Accounting: Logical Bytes vs. Allocated Bytes
Every node maintains two distinct storage metrics:
- **Logical Bytes**: The sum of file lengths (`st_size`), representing namespace volume.
- **Allocated Bytes**: The attributed filesystem block allocations (`st_blocks * 512`).
- We explicitly reject labelling allocated bytes as "physical bytes" or "reclaimable bytes" because APFS extent sharing across copy-on-write clones means allocated blocks are not guaranteed unique physical allocations.

### 3. Hardlink Attribution without Double Counting
To prevent hardlink aliases from inflating allocated storage totals:
- For every unique `(device_id, inode)` pair, allocated bytes are attributed to exactly **one deterministic representative path** (the lexicographically smallest canonical relative path).
- All peer hardlink aliases retain their namespace logical bytes, but receive `0` attributed allocated bytes with `hardlink_alias = true`.

### 4. Index-Backed Atomic Tree Generations
Storage trees are persisted in the existing SQLite database under `storage_tree_generations` and `storage_tree_nodes`:
- Build operations execute in a single isolated SQLite transaction (`status = 'building'`).
- Upon invariant verification, the generation transitions atomically to `status = 'ready'`.
- Failed builds leave previous ready generations completely untouched.
- Background generational pruning retains the latest 2 generations per root, avoiding unbounded database growth.

### 5. Stable Opaque Node Identifiers
Node IDs are domain-separated BLAKE3 hashes:
`BLAKE3("VACUA_STORAGE_NODE_V1" || root_id || raw_relative_unix_path_bytes)`
- Node IDs do not leak filesystem paths or usernames.
- Node IDs are deterministic within a tree generation.
- Client queries bind to both `generation_id` and `node_id`, preventing cross-generation desynchronization.

### 6. Bounded Child Queries with Exact Remainder Accounting
Clients query directory children in bounded pages (default 60, maximum 200 items):
- Rather than rendering tens of thousands of sub-nodes, queries return top children sorted deterministically by the requested metric.
- Unreturned children are aggregated into an authoritative `remainder` structure:
  $$\sum \text{Returned Children} + \text{Remainder} = \text{Parent Subtree Metric}$$
- The UI renders `Other` as an aggregate visual rectangle, without fabricating a fake filesystem node identity.

### 7. Rust-Authoritative Snapshot Delta Engine
Comparing tree generations against historical snapshots is computed strictly in Rust (`StorageTreeEngine::compare_generation_with_snapshot`):
- Swift receives typed delta records (`allocated_delta_bytes`, `logical_delta_bytes`, `change_kind`).
- Swift applies semantic diverging palettes (e.g. grown, shrunk, new) without computing historical subtraction math.

## Consequences

### Positive
- **Determinism**: Identical filesystem state generates byte-identical trees, IDs, and remainder totals.
- **Zero Double Counting**: Hardlink aliases cannot artificially multiply disk usage figures.
- **UI Responsiveness**: Bounded paging ensures Swift renders at 60 FPS even on million-node filesystems.
- **Privacy**: MCP clients and external tools interact with opaque node IDs without observing unredacted raw paths.
- **Crash Consistency**: Atomic SQLite publishing guarantees incomplete scans never corrupt visible UI state.

### Negative
- Initial tree build requires metadata aggregation and SQLite persistence overhead (mitigated by bulk transaction commits and single-pass bottom-up rollup).
- APFS clone sharing cannot be uniquely resolved to single files without kernel private extent introspection.
