# Privacy & Data Isolation Policy

> **Core Commitment**:  
> Vacua core operations are 100% offline-first and perform **zero telemetry**.  
> Networked intelligence is strictly optional, explicitly opt-in, and disabled by default.

---

## 1. Zero Telemetry in Core Engine

By default, the core engine (`vacua-core`, `vacua-scan`, `vacua-index`, `vacua-rules`, `vacua-plan`, `vacua-cli`):
- Transmits **no data** over the network.
- Collects **no telemetry**, analytics, user metrics, or error beacons.
- Uploads **no file paths**, file contents, hashes, or directory trees.
- Stores SQLite index databases and transaction logs **strictly locally** on your device.

---

## 2. Intelligence Provider Tiers & Network Boundaries

Vacua clearly delineates network boundaries across all supported and planned intelligence providers:

| Provider Tier | Locality | Network Egress | Description |
| :--- | :--- | :--- | :--- |
| **Core Engine** | Local | **NONE** | Standard rule evaluation, indexing, planning, and scanning. |
| **Apple On-Device** | Local | **NONE** | System Foundation Models running locally on Apple Silicon NPU. |
| **Local Third-Party (Ollama / MLX)** | Local (`127.0.0.1`) | **NONE** | Open-source local models running on localhost. |
| **Apple Private Cloud Compute (PCC)** | Cloud (Apple Silicon) | End-to-End Encrypted | Optional future Apple cloud intelligence with cryptographic privacy attestations. |
| **Third-Party Cloud APIs** | External Cloud | Explicit Outbound | Optional opt-in API integration (OpenAI, Anthropic, Gemini). Requires explicit API key stored in macOS Keychain. Metadata is redacted locally before dispatch. |

---

## 3. Data Sanitization & Minimization

Even when an on-device or cloud intelligence provider is active:
- Raw file paths containing sensitive username or project names are sanitized prior to model input.
- File contents are **never** provided to models during scanning or planning.
- The intelligence provider only receives sanitized metadata (category, approximate allocated size, age bucket, and rebuildability status).
