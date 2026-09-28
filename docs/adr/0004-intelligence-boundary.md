# ADR 0004: Intelligence Boundary, Non-Bypassable Policy, and Schema Validation

## Status
Accepted

## Context
As LLMs and on-device Foundation Models proliferate, software utilities often introduce AI agents that execute arbitrary shell scripts or delete paths autonomously. This architectural pattern represents an unacceptable security risk for storage tools:
1. Hallucinations can lead to catastrophic data destruction (`rm -rf /` or deleting user documents).
2. Prompt injection (indirect or direct) can weaponize models into deleting sensitive credentials or private keys.
3. Users lose trust when actions taken by software cannot be audited or explained.

## Decision
1. **AI Output is Strictly Untrusted Input**:
   - Every response from any intelligence provider (Apple On-Device, local Ollama, or Cloud LLM) is treated as unvalidated user input.
   - Outputs must deserialize strictly into `StructuredIntent` governed by `schemas/structured-intent-v1.json`.
2. **AI Never Emits File Paths or Commands**:
   - Models can specify filtering constraints (e.g. `max_risk: "SAFE"`, `excluded_categories: ["CONTAINER_DATA"]`, `target_reclaim_bytes: 10737418240`), but NEVER individual filesystem paths or deletion commands.
3. **Deterministic Safety Policy Engine Controls Execution**:
   - The compiled `StructuredIntent` is passed into the deterministic planner (`vacua-plan`) and invariant checker (`vacua-core::invariants`).
   - If an intent attempts to request an invalid risk or bypass invariants, it is rejected by the policy engine.
4. **Execution Requires Human Approval**:
   - Every plan compiled from an intent requires user confirmation before execution.
   - Deletions are re-validated against live filesystem metadata (TOCTOU defense) immediately before moving to Trash.

## Consequences
- **Positive**:
  - Zero chance of accidental deletion or prompt-injection-driven destruction.
  - Transparent, fully auditable translation from natural language to deterministic execution plans.
- **Negative / Mitigations**:
  - AI cannot autonomously "fix" unexpected storage problems with custom scripts. *Mitigation*: This constraint is an intentional security design choice.
