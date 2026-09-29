# Vacua MCP Interface (v0.5.1)

> **Status**: Verified implementation shipped in Vacua v0.5.1. Built on official `rmcp` (Rust SDK for Model Context Protocol). Strictly qualified by official `@modelcontextprotocol/inspector` in CI.

`vacua-mcp` exposes Vacua's deterministic storage intelligence engine as a stable, versioned, strictly capability-isolated, agent-safe machine interface for local AI environments such as **Claude Desktop**, **Cursor**, **Codex**, **VS Code**, and compatible MCP hosts.

---

## 1. Architecture & Security Invariant

```text
MCP Host (Claude / Cursor / VS Code / Codex / Inspector)
   │ (stdio transport, JSON-RPC 2.0)
   ▼
vacua-mcp (official rmcp 3.5.0)
   │
   ▼
McpPolicy (canonical AllowedRoot authorization, CursorV2, opaque IDs, sanitization)
   │
   ▼
Vacua Domain Services (vacua-api public DTO boundary)
   │
   ├── vacua-core (APFS volumes, storage pressure, invariant safety, allocation truth)
   ├── vacua-index (read-only SQLite snapshot storage, audit journal reader)
   ├── vacua-scan (bounded filesystem scanning, clonefile detection)
   ├── vacua-rules / vacua-risk (classification & safety invariants)
   ├── vacua-content (BLAKE3 exact duplicate identity & APFS sharing)
   └── vacua-plan (deterministic CleanupPlan v2 compiler with SHA-256 seal)

CRITICAL INVARIANT (Compile-Time Capability Isolation):
vacua-mcp ──X──► vacua-executor (NO DEPENDENCY PATH, ZERO MUTATION CAPABILITY)
```

### Machine-Enforced Invariant: No Executor Linkage

- `vacua-mcp` **does not depend on `vacua-executor`**. This is enforced at the Cargo crate dependency level and verified via `cargo tree -p vacua-mcp` in continuous integration.
- No destructive endpoints (`execute`, `delete`, `trash`, `remove`, `rm`, `purge`, `empty_trash`, `shell`, `sudo`, `write_file`) exist in the tool catalog or binary symbols.
- Execution of cleanup plans requires out-of-band human confirmation through the native CLI:
  ```bash
  vacua execute <plan-file>
  ```

---

## 2. Authoritative Root Policy & Capability Bounds

Configured at server startup:

1. **`AllowedRoot` Model**:
   - Each `--allow-root <PATH>` argument is canonicalized at startup, validated to exist, and verified as a directory.
   - If any root fails canonicalization or validation, **server startup fails closed**.
   - Stable opaque identifiers are assigned: `root-home` for `$HOME`, and `root-<deterministic-hash>` for external roots.
   - An empty allowed roots set strictly fails closed (`VACUA_POLICY_DENIED`).
2. **Symlink Escape Protection**:
   - Every accessed path is canonicalized. Symlinks escaping outside configured allowed roots are immediately rejected with `VACUA_POLICY_DENIED`.
3. **Multi-Root Isolation**:
   - Root-scoped tools accept `root_id`. Candidate caches are isolated per `root_id` with zero cross-root contamination.
4. **Input Budgets**:
   - Maximum proposal candidates: 200.
   - Maximum simulation candidates: 500.
   - Maximum string parameter length: 512 bytes.
   - Maximum pagination cursor length: 512 bytes.
   - Violations return `VACUA_LIMIT_EXCEEDED`.

---

## 3. Path Disclosure & Privacy Hardening

- `--path-disclosure <MODE>`:
  - `home-relative` (default): Formats home paths as `~/...`, and non-home roots as `<root:{root_id}>/...`.
  - `full`: Discloses absolute filesystem paths.
  - `redacted`: Replaces directory hierarchy with `<redacted-path>/<file_name>` and scrubbed strings.
- **Opaque Domain-Separated BLAKE3 IDs**:
  - `candidate_id`: `cand-<16-hex>`
  - `group_id`: `dup-<16-hex>`
  - `member_id`: `mem-<16-hex>` (mapped server-side, never leaks raw path)
  - `artifact_id`: `art-<16-hex>` (never leaks raw path)
- **Serialized CleanupPlan Leak Elimination**:
  - `serialized_plan` is `None` by default.
  - Only populated if `--allow-plan-export` is explicitly passed **AND** `--path-disclosure full` is active.

---

## 4. Storage Reclaim Truth & Accounting

1. **Candidate Reclaim Truth**:
   - `allocated_bytes`: True physical storage allocation (`st_blocks * 512`).
   - `confirmed_reclaim_lower_bound`: Conservative minimum reclaimable space (0 for APFS clones and hardlinks).
   - `reclaim_estimate_bytes`: Truthful estimate based on private bytes and extent models.
   - `reclaim_upper_bound`: Upper bound (allocated bytes).
2. **Simulation Truth**:
   - `immediate_reclaim_bytes`: Always 0 (moving files to Trash does NOT reclaim storage until emptied).
   - `eventual_reclaim_estimate_bytes`: Eventual space reclaimed after purge.
3. **Kernel Private Bytes**:
   - Unknown private bytes are never replaced with allocated bytes.
   - Explicitly tracked via `kernel_private_bytes_known` and member counters.
4. **Application Residual Allocation**:
   - Measured via `st_blocks * 512`, not logical length.

---

## 5. Tool Catalog & Detailed Property Matrix

| Tool Name | Tier | Persistent Cache Mutation? | User Data Mutation? | Expensive? | Root Scoped? | Path Disclosure Filtered? |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| `vacua_get_capabilities` | `READ_ONLY` | No | No | No | No | Yes |
| `vacua_storage_summary` | `READ_ONLY` | No | No | No | Yes | Yes |
| `vacua_list_snapshots` | `READ_ONLY` | No | No | No | Yes | Yes |
| `vacua_diff_snapshots` | `READ_ONLY` | No | No | No | Yes | Yes |
| `vacua_list_candidates` | `READ_ONLY` | No | No | No | Yes | Yes |
| `vacua_explain_candidate` | `READ_ONLY` | No | No | No | Yes | Yes |
| `vacua_list_applications` | `READ_ONLY` | No | No | No | Scoped | Yes |
| `vacua_get_application` | `READ_ONLY` | No | No | No | Scoped | Yes |
| `vacua_list_duplicates` | `ANALYZE_ONLY` | **Yes (Fingerprint DB)** | No | **Yes (Worker)** | Yes | Yes |
| `vacua_get_duplicate_group` | `ANALYZE_ONLY` | **Yes (Fingerprint DB)** | No | **Yes (Worker)** | Yes | Yes |
| `vacua_simulate_cleanup` | `ANALYZE_ONLY` | No | No | No | Yes | Yes |
| `vacua_propose_cleanup_plan` | `PROPOSE_ONLY` | No | No | No | Yes | Yes |
| `vacua_history_summary` | `READ_ONLY` | No | No | No | No | Yes |
| `vacua_verify_history` | `READ_ONLY` | No | No | No | No | Yes |

*Note on Tool Annotations*: `vacua_list_duplicates` and `vacua_get_duplicate_group` truthfully declare `read_only_hint = false, destructive_hint = false, idempotent_hint = true, open_world_hint = false` to reflect local fingerprint cache writes. All local closed-domain tools declare `open_world_hint = false`.

---

## 6. Resources & Resource Templates

### Direct Resources
- `vacua://capabilities` (`application/json`): Server capabilities, protocol generation, and capability invariants.
- `vacua://storage/summary` (`application/json`): Real-time APFS volume and container status.

### Resource Templates
- `vacua://candidate/{candidate_id}`: Deep inspect specific candidate classification facts (`vacua.mcp.candidate-detail.v1`).
- `vacua://snapshot/{snapshot_id}`: Point-in-time storage snapshot state (`vacua.mcp.snapshot-detail.v1`).
- `vacua://duplicate/{group_id}`: Duplicate group allocation and extent sharing (`vacua.mcp.duplicate-detail.v1`).
- `vacua://application/{application_id}`: Application bundle residue and evidence (`vacua.mcp.application-detail.v1`).

---

## 7. Prompts

1. `review_storage_growth`:
   - Arguments: `base_snapshot_id` (optional), `target_snapshot_id` (optional).
   - Generates structured guidance for analyzing storage deltas without executing destructive operations.
2. `review_cleanup_proposal`:
   - Arguments: `plan_id` (optional).
   - Generates structured guidance for verifying plan safety, preservation guards, and rebuild consequences.

---

## 8. Client Configuration

### Claude Desktop (`~/Library/Application Support/Claude/claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "vacua": {
      "command": "/opt/homebrew/bin/vacua-mcp",
      "args": ["--path-disclosure", "home-relative"]
    }
  }
}
```

### Cursor (`.cursor/mcp.json`)
```json
{
  "mcpServers": {
    "vacua": {
      "command": "vacua-mcp",
      "args": ["--path-disclosure", "home-relative"]
    }
  }
}
```
