# ADR 0002: Deterministic Risk Scoring, Inviolable Invariants, and Value Separation

## Status
Accepted

## Context
Traditional disk cleaning utilities frequently suffer from two fatal failure modes:
1. **Opaque Aggressive Heuristics**: Deleting files based solely on superficial directory names (e.g., `~/Library/Caches/*` or `~/Library/Application Support/*`), resulting in broken application states, lost browser sessions, corrupted email indices, or missing user documents.
2. **Conflating Size with Safety**: Treating huge directories (e.g., an 18 GB Docker image or a 20 GB local virtual machine disk) as "prime candidates for immediate automatic cleanup" simply because reclaiming them increases marketing numbers, even though doing so could destroy critical user workflows.

We need a formal decision on how storage items are evaluated, classified, and approved for cleanup in Vacua.

## Decision

### 1. Deterministic Multi-Signal Scoring over Opaque ML
We reject opaque black-box machine learning models or probabilistic neural networks for safety decisions. Instead, all classifications must be computed via a deterministic scoring pipeline over an explicit `Evidence` vector.
Signals include:
- Path semantics and parent directory classification.
- Bundle identifier presence and application registration.
- Package receipt verification (`pkgutil` / receipt database).
- Active process presence (e.g., is `xcodebuild` actively compiling?).
- File age and access patterns (mtime, ctime).
- Reconstructability consequence (can this artifact be rebuilt automatically, with network cost, or is it non-restorable?).

### 2. Five Discrete Risk Levels
Every candidate is mapped into one of five mutually exclusive levels:
- `SAFE`: Zero user data, completely reproducible, producing application idle.
- `REVIEW`: Reconstructable, but incurs user friction, compilation latency, or network bandwidth.
- `CAUTION`: Stale leftovers with partial ambiguity or possible user settings.
- `PROTECTED`: Hard safety boundaries (SIP, credentials, user documents, active git repos).
- `UNKNOWN`: Unrecognized or ambiguous candidates.

### 3. Absolute Non-Bypassable Invariants
- `PROTECTED` items can NEVER be cleaned or proposed for deletion.
- `UNKNOWN` items can NEVER be automatically cleaned.
- Neither AI agent prompts nor user preference models may lower the risk classification of an item.
- Safe automated cleanup proposals can only include items that strictly qualify as `SAFE`.

### 4. Separation of Risk and Recommendation Value
Risk and Value are orthogonal dimensions:
- **Risk (`RiskLevel`)**: Measures the danger of data loss or workflow disruption.
- **Value (`RecommendationValue`)**: Measures the reclaim impact based on allocated physical bytes, disk pressure, and age.
A 30 GB Docker volume might have `Value: HIGH` and `Risk: REVIEW`. Its high value will bring it to the user's attention, but its risk level strictly prevents automated deletion.

### 5. Terminology Honesty
Non-calibrated scoring metrics must be exposed in CLI and UI as `confidence score` (e.g. `Score: 0.95`), never as a falsified `95% probability`.
