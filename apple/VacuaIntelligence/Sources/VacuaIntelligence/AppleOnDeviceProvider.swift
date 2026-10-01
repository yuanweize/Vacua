import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

public struct AppleOnDeviceProvider: Sendable {
    private let deterministicParser = DeterministicIntentParser()

    public init() {}

    /// Probe the availability of Apple Foundation Models on this Mac using real Apple OS APIs.
    public func checkAvailability() -> (ProviderAvailability, String) {
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            let model = SystemLanguageModel.default
            switch model.availability {
            case .available:
                return (.available, "available")
            case .unavailable(let reason):
                switch reason {
                case .deviceNotEligible:
                    return (.deviceNotEligible, "deviceNotEligible")
                case .appleIntelligenceNotEnabled:
                    return (.appleIntelligenceNotEnabled, "appleIntelligenceNotEnabled")
                case .modelNotReady:
                    return (.modelNotReady, "modelNotReady")
                @unknown default:
                    return (.modelNotReady, "unavailableReasonUnknown")
                }
            @unknown default:
                return (.modelNotReady, "unknownAvailability")
            }
        } else {
            return (.deviceNotEligible, "macOSVersionBelow26")
        }
        #else
        return (.unsupported, "foundationModelsModuleNotBundled")
        #endif
    }

    /// Parse a natural language cleanup prompt into a typed StructuredIntent using Apple Foundation Models.
    /// Real Apple Foundation Models inference is executed ONLY when the model is actually available.
    public func parseIntent(prompt: String) async -> ParsedIntentResult {
        let (availability, reasonString) = checkAvailability()

        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            if availability == .available {
                let model = SystemLanguageModel.default
                let session = LanguageModelSession(model: model)

                let schemaPrompt = """
                You are a storage cleanup intent classifier for Vacua.
                Output ONLY a JSON object matching this schema:
                {
                  "targetReclaimBytes": null,
                  "maxRisk": "SAFE",
                  "preferredCategories": [],
                  "excludedCategories": [],
                  "preferReversibleActions": true,
                  "includeDeveloperArtifacts": true,
                  "explanationRequested": false
                }
                Valid maxRisk values are "SAFE", "REVIEW", "CAUTION".
                Never output markdown fences, only the JSON string.

                User query: "\(prompt)"
                """

                do {
                    let response = try await session.respond(to: schemaPrompt)
                    if let generated = extractJSON(GeneratedCleanupIntent.self, from: response.content) {
                        let validated = validate(generated: generated, prompt: prompt)
                        return ParsedIntentResult(
                            intent: validated,
                            providerRequested: "apple-system",
                            providerUsed: "apple-system",
                            modelAvailability: reasonString,
                            generationMode: "apple-on-device-foundation-models",
                            generationSucceeded: true,
                            fallbackUsed: false,
                            fallbackReason: nil
                        )
                    }
                } catch {
                    // Model was available but inference failed; accurately report error provenance
                    let fallbackIntent = deterministicParser.parse(prompt: prompt)
                    return ParsedIntentResult(
                        intent: fallbackIntent,
                        providerRequested: "apple-system",
                        providerUsed: "deterministic-fallback",
                        modelAvailability: reasonString,
                        generationMode: "deterministic-parser",
                        generationSucceeded: false,
                        fallbackUsed: true,
                        fallbackReason: "inference_error"
                    )
                }
            }
        }
        #endif

        // Verified Deterministic Fallback
        let fallbackIntent = deterministicParser.parse(prompt: prompt)
        return ParsedIntentResult(
            intent: fallbackIntent,
            providerRequested: "apple-system",
            providerUsed: "deterministic-fallback",
            modelAvailability: reasonString,
            generationMode: "deterministic-parser",
            generationSucceeded: false,
            fallbackUsed: true,
            fallbackReason: (availability != .available) ? reasonString : nil
        )
    }

    private func extractJSON<T: Decodable>(_ type: T.Type, from text: String) -> T? {
        if let data = text.data(using: .utf8), let result = try? JSONDecoder().decode(type, from: data) {
            return result
        }
        if let start = text.firstIndex(of: "{"), let end = text.lastIndex(of: "}") {
            let jsonSub = String(text[start...end])
            if let data = jsonSub.data(using: .utf8), let result = try? JSONDecoder().decode(type, from: data) {
                return result
            }
        }
        return nil
    }

    private func validate(generated: GeneratedCleanupIntent, prompt: String) -> StructuredIntent {
        let lower = prompt.lowercased()
        let risk: String
        if lower.contains("review") {
            risk = "REVIEW"
        } else if lower.contains("caution") {
            risk = "CAUTION"
        } else {
            let allowedRisks = ["SAFE", "REVIEW", "CAUTION"]
            risk = allowedRisks.contains(generated.maxRisk.uppercased()) ? generated.maxRisk.uppercased() : "SAFE"
        }

        let allowedCategories = [
            "BUILD_ARTIFACT", "PACKAGE_MANAGER_CACHE", "CONTAINER_DATA",
            "USER_DOCUMENT", "SOURCE_CODE", "VIRTUAL_MACHINE"
        ]
        var preferred = generated.preferredCategories.filter { allowedCategories.contains($0) }
        var excluded = generated.excludedCategories.filter { allowedCategories.contains($0) }
        if (lower.contains("docker") || lower.contains("container")) && !excluded.contains("CONTAINER_DATA") {
            excluded.append("CONTAINER_DATA")
        }
        if (lower.contains("photo") || lower.contains("document")) && !excluded.contains("USER_DOCUMENT") {
            excluded.append("USER_DOCUMENT")
        }
        if (lower.contains("git") || lower.contains("repo")) && !excluded.contains("SOURCE_CODE") {
            excluded.append("SOURCE_CODE")
        }
        if (lower.contains("build") || lower.contains("target") || lower.contains("cache")) && !preferred.contains("BUILD_ARTIFACT") {
            preferred.append("BUILD_ARTIFACT")
        }

        var targetBytes: UInt64? = nil
        if let tb = generated.targetReclaimBytes, tb > 0 && tb < (1024 * 1024 * 1024 * 1024 * 1024) { // < 1 PB
            if tb <= 1024 {
                if lower.contains("gb") || lower.contains("gig") {
                    targetBytes = UInt64(tb) * 1024 * 1024 * 1024
                } else if lower.contains("mb") || lower.contains("meg") {
                    targetBytes = UInt64(tb) * 1024 * 1024
                } else {
                    targetBytes = UInt64(tb)
                }
            } else {
                targetBytes = UInt64(tb)
            }
        }

        if targetBytes == nil {
            let det = deterministicParser.parse(prompt: prompt)
            targetBytes = det.target_reclaim_bytes
        }

        return StructuredIntent(
            intent_version: 1,
            target_reclaim_bytes: targetBytes,
            max_risk: risk,
            preferred_categories: preferred,
            excluded_categories: excluded,
            include_developer_artifacts: generated.includeDeveloperArtifacts,
            prefer_reversible_actions: generated.preferReversibleActions,
            explanation_requested: generated.explanationRequested
        )
    }

    /// Grounded storage reasoning: generate explanation referencing only verified context IDs.
    public func explainStorage(prompt: String, contextJSON: String) async -> StorageExplanationResult {
        let (availability, _) = checkAvailability()

        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            if availability == .available {
                let model = SystemLanguageModel.default
                let session = LanguageModelSession(model: model)
                let fullPrompt = """
                You are explaining Vacua's deterministic macOS storage evidence.
                GROUNDING INVARIANTS:
                1. Only reference facts and candidate/snapshot IDs present in the context below.
                2. Do NOT invent IDs, paths, or byte numbers.
                3. Do NOT authorize deletion.
                4. Output ONLY a valid JSON object matching:
                {
                  "summary": "...",
                  "causes": ["..."],
                  "referencedCandidateIDs": ["..."],
                  "referencedSnapshotIDs": ["..."],
                  "caution": "..."
                }

                Context:
                \(contextJSON)

                User Question:
                \(prompt)
                """
                do {
                    let response = try await session.respond(to: fullPrompt)
                    if let gen = extractJSON(GeneratedStorageExplanation.self, from: response.content) {
                        let filteredCandidates = gen.referencedCandidateIDs.filter { contextJSON.contains($0) }
                        let filteredSnapshots = gen.referencedSnapshotIDs.filter { contextJSON.contains($0) }

                        return StorageExplanationResult(
                            summary: gen.summary,
                            causes: gen.causes,
                            referenced_candidate_ids: filteredCandidates,
                            referenced_snapshot_ids: filteredSnapshots,
                            caution: gen.caution ?? "Safety invariants active: deletion requires explicit human approval.",
                            provider_used: "apple-system",
                            generation_mode: "apple-on-device-foundation-models",
                            fallback_used: false
                        )
                    }
                } catch {
                    // Fall through to deterministic summarizer
                }
            }
        }
        #endif

        // Grounded deterministic summarizer based on contextJSON
        return generateDeterministicExplanation(prompt: prompt, contextJSON: contextJSON)
    }

    private func generateDeterministicExplanation(prompt: String, contextJSON: String) -> StorageExplanationResult {
        var summary = "Storage analysis grounded in differential snapshots and evidence graph."
        var causes: [String] = []
        var refCandidates: [String] = []
        var refSnapshots: [String] = []

        // Parse rudimentary context facts if JSON is provided
        if let data = contextJSON.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            if let snapshotId = obj["snapshot_id"] as? String {
                refSnapshots.append(snapshotId)
            }
            if let deltaStr = obj["total_delta_str"] as? String {
                summary = "Storage changed by \(deltaStr) according to snapshot baselines."
            }
            if let topLocations = obj["top_growing_subtrees"] as? [[String: Any]] {
                for loc in topLocations.prefix(3) {
                    if let path = loc["path"] as? String, let delta = loc["delta_str"] as? String {
                        causes.append("\(path) expanded by \(delta)")
                    }
                }
            }
            if let candidates = obj["candidates"] as? [[String: Any]] {
                for c in candidates.prefix(5) {
                    if let cid = c["id"] as? String {
                        refCandidates.append(cid)
                    }
                }
            }
        }

        return StorageExplanationResult(
            summary: summary,
            causes: causes,
            referenced_candidate_ids: refCandidates,
            referenced_snapshot_ids: refSnapshots,
            caution: "Safety invariants active: deletion requires explicit human approval.",
            provider_used: "deterministic-grounded-summarizer",
            generation_mode: "deterministic-parser",
            fallback_used: true
        )
    }
}
