# Vacua Intelligence Layer

> **The Intelligence Axiom**:  
> *AI output is untrusted input.*  
> *AI may interpret.*  
> *AI may explain.*  
> *AI may propose.*  
> *AI may never lower risk.*  
> *AI may never bypass policy.*  
> *AI may never directly delete.*

---

## 1. Architectural Role

Artificial Intelligence in Vacua is strictly an **optional interpretation and advisory layer**. The core engine (`vacua-core`, `vacua-scan`, `vacua-index`, `vacua-rules`, `vacua-plan`) is 100% autonomous, local, and complete without any AI provider.

When an intelligence provider is engaged, it performs exactly one translation step:

```
[ User Natural Language ]
       ↓
[ Intelligence Provider (Apple On-Device / Local / Cloud) ]
       ↓
[ Typed StructuredIntent (JSON Schema Validated) ]
       ↓
[ Deterministic Planner & Hard Invariant Policy Engine ]
       ↓
[ Immutable CleanupPlan (SHA-256 Hashed) ]
       ↓
[ Explicit Human User Approval ]
       ↓
[ Safe Executor (TOCTOU Re-validation -> macOS Trash) ]
```

---

## 2. Inviolable Security & Policy Invariants

1. **Zero Direct Execution Authority**: Under no circumstances can an AI model output shell commands (`rm`, `sudo`, `mv`), arbitrary file paths, or execute system commands.
2. **Untrusted Input Invariant**: All structured output from any model is parsed strictly against `schemas/structured-intent-v1.json`. Any extra or malformed property is rejected.
3. **Non-Escalation Rule**: An AI model cannot alter or elevate the risk classification of any candidate:
   - `PROTECTED` can NEVER be changed to `SAFE`.
   - `UNKNOWN` can NEVER be changed to `SAFE`.
   - `REVIEW` can NEVER be changed to `SAFE`.
4. **Data Minimization**: Intelligence providers are provided only sanitized high-level metadata (categories, sizes, age buckets) rather than sensitive paths or file contents.

---

## 3. Supported & Planned Provider Tiers

### 3.1 Apple On-Device (`apple/VacuaIntelligence`)
- **Status**: Implemented prototype (`AppleOnDeviceProvider`).
- **Mechanism**: Integrates with Apple Foundation Models (`SystemLanguageModel`) on macOS.
- **Locality**: 100% on-device on Apple Silicon Neural Engine. Zero network traffic.
- **Availability Probe**: Gracefully inspects `SystemLanguageModel.default.availability` (`available`, `deviceNotEligible`, `appleIntelligenceNotEnabled`, `modelNotReady`).

### 3.2 Local Third-Party Models (Planned)
- **Target**: Ollama / llama.cpp on `http://127.0.0.1:11434`.
- **Policy**: Must run locally; zero internet access.

### 3.3 Optional Cloud Models (Planned)
- **Target**: OpenAI-compatible, Anthropic, or Gemini APIs.
- **Policy**: Disabled by default. Requires explicit user opt-in and API key stored in macOS Keychain. Full path redaction performed prior to transmission.
