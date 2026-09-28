> **Status**: Planned Roadmap Specification (Not shipped in v0.1.0). In v0.1.0, machine agents should use the `--json` CLI interface.

Vacua specifies an optional MCP server interface (`vacua-mcp`) allowing external AI coding assistants (Claude Desktop, Cursor, Codex, Open-Source Agents) to query macOS storage state and propose safe cleanup plans.

---

## 1. Security Architecture: Read-Only by Default

**Under no circumstances does `vacua-mcp` expose raw destructive endpoints such as `rm_file`, `delete_path`, or arbitrary shell execution.**

The MCP interface operates strictly on a **Propose-and-Approve** model:
1. The AI agent can read storage summaries, list candidates, and generate an immutable cleanup plan.
2. The generated plan is cryptographically hashed with a plan ID.
3. Execution can **only** occur through out-of-band user approval via the native CLI (`vacua plan execute <plan_id>`) or the macOS desktop interface.

---

## 2. Exposed MCP Tools

| Tool Name | Access Tier | Description |
| :--- | :--- | :--- |
| `storage_summary` | `READ_ONLY` | Retrieves APFS storage status, free space, and documented storage pressure level. |
| `scan_status` | `READ_ONLY` | Returns progress and summary statistics of active or previous filesystem scans. |
| `list_candidates` | `READ_ONLY` | Queries classified cleanup candidates filtered by max risk level (`safe`, `review`, `caution`). |
| `explain_candidate` | `READ_ONLY` | Explains why an item was classified, displaying its full evidence vector and rebuild effects. |
| `build_cleanup_plan` | `READ_ONLY` | Deterministically compiles a proposed `CleanupPlan` with a SHA-256 integrity hash. |
