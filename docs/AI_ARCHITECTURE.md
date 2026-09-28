# AI Integration Architecture

> **The AI Operating Axiom**:  
> *AI may propose.*  
> *The policy engine decides what is allowed.*  
> *The user decides what is executed.*

---

## 1. Decoupled Role of Artificial Intelligence

In Project Reclaim, AI is strictly an **optional advisory layer**. The core engine (`reclaim-core`, `reclaim-scan`, `reclaim-rules`, `reclaim-plan`) operates with complete autonomy and 100% deterministic correctness without any LLM or network connection.

When enabled, AI serves three narrow purposes:
1. **Candidate Explanation**: Translating complex developer artifacts and build systems into plain language for non-technical users.
2. **Natural Language Intent Parsing**: Translating natural language queries into typed, bounded cleanup intents.
3. **Scan Summarization**: Providing concise executive summaries of disk allocations.

---

## 2. Intent-to-Execution Pipeline

Under no circumstances does an LLM issue shell commands or direct filesystem mutation instructions. The execution flow is strictly gated through deterministic schemas:

```
[ User Prompt ]
       |
       v
[ LLM / Foundation Model ]
       |
       v
[ Structured Intent (JSON) ]
  - target_bytes: e.g. 20 GB
  - max_risk: "SAFE"
  - exclude_categories: ["VIRTUAL_MACHINE", "USER_DOCUMENT"]
       |
       v
[ Deterministic Planner ]
       |
       v
[ Hard Safety Policy Engine ]
  (Validates Invariants: PROTECTED & UNKNOWN cannot be bypassed)
       |
       v
[ Immutable Cleanup Plan ] (SHA-256 Hashed)
       |
       v
[ Human Approval Required ]
       |
       v
[ Executor ]
  (Pre-execution TOCTOU Check -> macOS Trash)
```

---

## 3. Graceful Degradation & On-Device Execution

- **Tier 1 (Default)**: Fully local, zero-AI mode. All explanations use pre-compiled rule metadata.
- **Tier 2 (macOS Foundation Models)**: If supported by hardware and macOS 15+, on-device Apple Intelligence models are queried via native system frameworks without network egress.
- **Tier 3 (Optional Cloud LLM)**: If configured explicitly by the user, only sanitized, high-level aggregated numbers are sent to third-party endpoints.
