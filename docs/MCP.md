# Vacua MCP Interface (v0.5.0)

> **Status**: Verified implementation shipped in Vacua v0.5.0. Built on official `rmcp` (Rust SDK for Model Context Protocol).

`vacua-mcp` exposes Vacua's deterministic storage intelligence engine as a stable, versioned, strictly capability-isolated, agent-safe machine interface for local AI environments such as **Claude Desktop**, **Cursor**, **Codex**, **VS Code**, and compatible MCP hosts.

---

## 1. Architecture & Security Invariant

```text
MCP Host (Claude / Cursor / VS Code / Codex)
   │ (stdio transport, JSON-RPC 2.0)
   ▼
vacua-mcp (official rmcp 3.5.0)
   │
   ▼
McpPolicy (path disclosure, allowed roots, bounded pagination, sanitization)
   │
   ▼
Vacua Domain Services (vacua-api public DTO boundary)
   │
   ├── vacua-core (APFS volumes, storage pressure, invariant safety)
   ├── vacua-index (SQLite snapshot storage, FSEvents index, audit journal)
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

## 2. Transport & Stdio Purity

- **Transport**: `stdio` exclusively in v0.5.0. No HTTP listeners, SSE ports, or network exposure.
- **Stdout Purity**: All `stdout` traffic is strictly framed JSON-RPC protocol messages. All logs, diagnostics, and tracing output are redirected to `stderr`.

---

## 3. Server Capability Tiers

Vacua MCP categorizes operations into three non-destructive capability tiers:

1. **`READ_ONLY`**: Reads existing deterministic state (SQLite snapshots, index freshness, volume pressure, audit journal).
2. **`ANALYZE_ONLY`**: Performs bounded CPU/metadata analysis (hash verification, clonefile detection). Does not mutate filesystem state.
3. **`PROPOSE_ONLY`**: Compiles an immutable, cryptographically sealed `CleanupPlan` v2 with SHA-256 integrity hash. Does not save or execute the plan.

---

## 4. Path Disclosure & Allowed Roots Policy

Configured at server startup:

- `--path-disclosure <MODE>`:
  - `home-relative` (default): Replaces `/Users/<user>` with `~` (e.g., `~/Downloads/test.zip`).
  - `full`: Discloses absolute filesystem paths.
  - `redacted`: Replaces path components with opaque tokens (`<path-redacted>`).
- `--allow-root <PATH>`:
  - Constrains ad-hoc analysis to pre-configured paths. By default, queries utilize pre-indexed roots only.
- **Untrusted Metadata Handling**: All filenames and path strings returned by the server are treated as untrusted data. Control characters, newlines, and ANSI escape codes are scrubbed to prevent terminal injection or log forging.

---

## 5. Tool Catalog (14 Tools)

| Tool Name | Tier | Description |
| :--- | :--- | :--- |
| `vacua_get_capabilities` | `READ_ONLY` | Reports server features, schema versions, path disclosure mode, and asserts `mutation_authority: false` and `executor_linked: false`. |
| `vacua_storage_summary` | `READ_ONLY` | Reports APFS container metrics, logical used, free vs available space, storage pressure, and candidate reclaim lower bounds. |
| `vacua_list_snapshots` | `READ_ONLY` | Lists recorded storage snapshots with pagination (`limit`, `cursor`). |
| `vacua_diff_snapshots` | `READ_ONLY` | Computes storage delta between two snapshots, highlighting growing and shrinking subtrees. |
| `vacua_list_candidates` | `READ_ONLY` | Lists classified cleanup candidates with filtering (`max_risk`, `category`, `min_reclaim_bytes`). |
| `vacua_explain_candidate` | `READ_ONLY` | Explains why an item was classified, detailing its evidence vector, rebuild consequence, and experimental cost model. |
| `vacua_list_applications` | `READ_ONLY` | Lists detected applications and associated artifact counts from the evidence graph. |
| `vacua_get_application` | `READ_ONLY` | Retrieves bundle identity, installed status, and child artifacts for a specific application. |
| `vacua_list_duplicates` | `ANALYZE_ONLY` | Lists content-verified BLAKE3 exact duplicate groups with APFS clone and hardlink metrics. |
| `vacua_get_duplicate_group` | `ANALYZE_ONLY` | Retrieves detailed member-level physical allocation and kernel private bytes for a duplicate group. |
| `vacua_simulate_cleanup` | `ANALYZE_ONLY` | Simulates cleanup for selected candidate/duplicate IDs, verifying that immediate Trash reclaim is 0 bytes. |
| `vacua_propose_cleanup_plan` | `PROPOSE_ONLY` | Compiles an immutable `CleanupPlan` v2 with SHA-256 seal and preservation guards. Cannot execute. |
| `vacua_history_summary` | `READ_ONLY` | Summarizes past local execution transactions and verified journal records. |
| `vacua_verify_history` | `READ_ONLY` | Verifies cryptographic hash chain integrity of the local execution journal. |

---

## 6. Resources & Resource Templates

### Direct Resources
- `vacua://capabilities` (`application/json`): Server version, schema generation, and capability invariants.
- `vacua://storage/summary` (`application/json`): Real-time APFS volume and container status.

### Resource Templates
- `vacua://candidate/{candidate_id}`: Deep inspect specific candidate classification facts.
- `vacua://snapshot/{snapshot_id}`: Storage snapshot metadata.
- `vacua://duplicate/{group_id}`: Duplicate group allocation details.
- `vacua://application/{application_id}`: Application evidence graph node.

---

## 7. Prompts

1. `review_storage_growth`:
   - Arguments: `base_snapshot_id` (optional), `target_snapshot_id` (optional).
   - Generates structured guidance for analyzing storage deltas without executing destructive operations.
2. `review_cleanup_proposal`:
   - Arguments: `plan_id` (optional).
   - Generates structured guidance for verifying plan safety, preservation guards, and rebuild consequences.

---

## 8. Schema Versioning & Stability

Every top-level DTO in `crates/vacua-api` includes an explicit `schema_version`:
- `vacua.mcp.capabilities.v1`
- `vacua.mcp.storage-summary.v1`
- `vacua.mcp.candidate-list.v1`
- `vacua.mcp.candidate-detail.v1`
- `vacua.mcp.duplicate-list.v1`
- `vacua.mcp.duplicate-detail.v1`
- `vacua.mcp.snapshot-list.v1`
- `vacua.mcp.snapshot-diff.v1`
- `vacua.mcp.plan-proposal.v1`
- `vacua.mcp.history-summary.v1`
- `vacua.mcp.history-verification.v1`

Generated JSON schemas are tracked under `schemas/mcp/*.schema.json` and verified against accidental drift in CI via `crates/vacua-api/tests/schema_drift.rs`.

---

## 9. Client Configuration

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

### Cursor (`~/.cursor/mcp.json` or `.cursor/mcp.json`)
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

### VS Code (`.vscode/mcp.json`)
```json
{
  "mcp": {
    "servers": {
      "vacua": {
        "command": "/opt/homebrew/bin/vacua-mcp",
        "args": ["--path-disclosure", "home-relative"],
        "type": "stdio"
      }
    }
  }
}
```

---

## 10. Threat Model & Known Limitations

- **Threat Model**:
  - Prompt injection via malicious file paths is mitigated by treating all filenames as untrusted structured data and applying control-character escaping.
  - Resource exhaustion is prevented via clamped pagination (`max_results = 200`) and a concurrency-limiting semaphore on expensive operations.
  - Compile-time absence of `vacua-executor` guarantees that no agent can invoke filesystem deletion or mutation.
- **Limitations in v0.5.0**:
  - Transport is strictly local `stdio`; remote HTTP/SSE is not implemented.
  - Fresh arbitrary root scanning is restricted; queries must target configured or indexed roots.
  - Apple Foundation Models helper is not chained into MCP to prevent multi-hop hallucination and latency overhead.
