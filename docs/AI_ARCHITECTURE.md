# AI Integration Architecture

> **The AI Operating Axiom**:  
> *AI may propose.*  
> *The policy engine decides what is allowed.*  
> *The user decides what is executed.*

---

## 1. Decoupled Role of Artificial Intelligence

In Vacua, AI is strictly an **optional advisory layer**. The core engine (`vacua-core`, `vacua-scan`, `vacua-index`, `vacua-rules`, `vacua-plan`) operates with complete autonomy and 100% deterministic correctness without any LLM or network connection.

When enabled, intelligence providers serve three narrow purposes:
1. **Candidate Explanation**: Translating complex developer artifacts and build systems into plain language for non-technical users.
2. **Natural Language Intent Parsing**: Translating natural language queries into typed, bounded cleanup intents (`StructuredIntent`).
3. **Scan Summarization**: Providing concise executive summaries of disk allocations.

---

## 2. Intent-to-Execution Pipeline

Under no circumstances does an LLM issue shell commands or direct filesystem mutation instructions. The execution flow is strictly gated through deterministic schemas:

```
[ User Prompt ]
       |
       v
[ Intelligence Provider (Apple On-Device / Local / Cloud) ]
       |
       v
[ Structured Intent (JSON Schema Validated) ]
  - target_reclaim_bytes: e.g. 10 GB
  - max_risk: "SAFE"
  - excluded_categories: ["CONTAINER_DATA", "USER_DOCUMENT"]
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

## 3. Provider-Neutral Architecture

Vacua supports multiple intelligence providers under an identical, typed contract:
1. **Apple On-Device**: Built natively on macOS Foundation Models (`SystemLanguageModel`).
2. **Local Open-Source (Planned)**: Ollama / llama.cpp on `127.0.0.1:11434`.
3. **Optional Cloud Models (Planned)**: Opt-in cloud providers with local path redaction.
