# ADR 0003: SQLite-backed Incremental Metadata Index & Migration Architecture

## Status
Accepted

## Context
Full filesystem traversal of large directory hierarchies (e.g. hundreds of thousands of files across `~/Library` and developer repositories) incurs significant I/O latency, battery consumption, and kernel buffer churn if repeated on every invocation.

To achieve instant startup and incremental responsiveness, Vacua requires a fast, local, embedded persistent storage layer to:
1. Cache metadata fingerprints (`device_id`, `inode`, `canonical_path`, `mtime`, `allocated_bytes`, `classification`).
2. Correlate live changes detected via macOS `FSEvents` with previously scanned directory subtrees.
3. Track historical scan sessions and transaction audit logs without network transmission.

## Considered Options
1. **Flat JSON / MessagePack files**:
   - *Pros*: Simple to serialize.
   - *Cons*: Must load the entire index into memory on every run; no atomic writes without full file rewrites; high memory footprint at scale; query filtering requires scanning all records.
2. **Key-Value Store (Sled / RocksDB)**:
   - *Pros*: High key-value write throughput.
   - *Cons*: Heavy binary footprint; complex C++ dependencies (RocksDB); relational parent-child path queries and schema migrations are awkward; cross-compilation friction.
3. **Embedded SQLite via `rusqlite` (Bundled)**:
   - *Pros*:
     - Zero configuration, zero external service dependency.
     - Single-file database with ACID transactions, atomic commits, and crash recovery.
     - Fast B-Tree indexing on `(device_id, inode)` and `canonical_path`.
     - `rusqlite` with `bundled` feature compiles SQLite amalgamation directly into the binary, ensuring consistent SQLite version across macOS releases without relying on system dynamic library differences.
     - Predictable memory footprint using PRAGMAs (`journal_mode = WAL`, `synchronous = NORMAL`, `temp_store = MEMORY`).
     - Schema migration via `PRAGMA user_version` is deterministic and battle-tested.

## Decision
We select **SQLite** via `rusqlite` with the `bundled` feature for `vacua-index`.

### Schema Architecture
The index database maintains 6 structured tables:
- `schema_version`: Tracks migration state via `PRAGMA user_version`.
- `volumes`: Discovered filesystem volumes and mount points.
- `scan_sessions`: Audit record of scans (timestamp, target path, total files, allocated bytes).
- `entries`: Metadata fingerprints for all observed filesystem nodes.
- `classifications`: Semantic classification and evaluated risk for candidates.
- `evidence_snapshots`: Corroborating signals associated with candidate entries.

### Migration Strategy
- Every database connection queries `PRAGMA user_version`.
- If `user_version == 0`, initial schema is constructed transactionally, and `user_version` is set to `CURRENT_VERSION`.
- Future migrations execute sequential upgrade steps inside an explicit transaction (`user_version = 1 -> 2`, etc.).
- If an unsupported future schema version is encountered, Vacua fails closed and refuses to corrupt the database.

## Consequences
- **Positive**:
  - Incremental rescans can check mtime/inode deltas in sub-millisecond queries.
  - Safe concurrent reads using SQLite Write-Ahead Logging (WAL).
  - Database file can be inspected using standard tools (`sqlite3`) for debugging and transparency.
- **Negative / Mitigations**:
  - Adds ~1 MB to CLI binary size. *Mitigation*: Negligible compared to Electron/Node-based tools; zero external runtime dependencies.
