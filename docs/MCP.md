# Model Context Protocol (MCP) Server Specification

Project Reclaim provides an optional MCP server (`reclaim-mcp`) allowing AI coding assistants (Claude Desktop, Cursor, Codex, Open-Source Agents) to query macOS storage state and propose safe cleanup plans.

---

## 1. Security Architecture: Read-Only by Default

**Under no circumstances does `reclaim-mcp` expose raw destructive endpoints such as `rm_file`, `delete_path`, or arbitrary shell execution.**

The MCP interface operates strictly on a **Propose-and-Approve** model:
1. The AI agent can read storage summaries, list candidates, and generate an immutable cleanup plan.
2. The generated plan is cryptographically hashed with a plan ID.
3. Execution can **only** occur through out-of-band user approval via the native CLI (`reclaim plan execute <plan_id>`) or the macOS SwiftUI interface.

---

## 2. Exposed MCP Tools

| Tool Name | Access Tier | Description |
| :--- | :--- | :--- |
| `storage_summary` | `READ_ONLY` | Retrieves APFS storage status, free space, and documented storage pressure level. |
| `scan_status` | `READ_ONLY` | Returns progress and summary statistics of active or previous filesystem scans. |
| `list_candidates` | `READ_ONLY` | Queries classified cleanup candidates filtered by max risk level (`safe`, `review`, `caution`). |
| `explain_candidate` | `READ_ONLY` | Explains why an item was classified, displaying its full evidence vector and rebuild effects. |
| `build_cleanup_plan` | `READ_ONLY` | Deterministically compiles a proposed `CleanupPlan` with a SHA-256 integrity hash. |

---

## 3. Tool Schema Example: `build_cleanup_plan`

```json
{
  "name": "build_cleanup_plan",
  "description": "Construct an immutable cleanup plan based on deterministic rules and maximum permitted risk tier.",
  "parameters": {
    "type": "object",
    "properties": {
      "target_path": {
        "type": "string",
        "description": "Path to scan and evaluate"
      },
      "max_risk": {
        "type": "string",
        "enum": ["safe", "review", "caution"],
        "default": "safe",
        "description": "Upper bound of risk allowed in proposed plan"
      }
    },
    "required": ["target_path"]
  }
}
```
