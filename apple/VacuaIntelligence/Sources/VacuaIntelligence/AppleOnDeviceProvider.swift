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

    /// Parse a natural language cleanup prompt into a typed StructuredIntent using @Generable guided generation.
    /// Real Apple Foundation Models inference is executed ONLY when the model is actually available.
    public func parseIntent(prompt: String) async -> ParsedIntentResult {
        let (availability, reasonString) = checkAvailability()

        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            if availability == .available {
                let model = SystemLanguageModel.default
                let session = LanguageModelSession(model: model)

                do {
                    let response = try await session.respond(
                        to: prompt,
                        generating: GeneratedCleanupIntent.self
                    )
                    let generated = response.content
                    let validated = validate(generated: generated)

                    return ParsedIntentResult(
                        intent: validated,
                        providerRequested: "apple-system",
                        providerUsed: "apple-system",
                        modelAvailability: reasonString,
                        generationMode: "apple-guided-generation",
                        generationSucceeded: true,
                        fallbackUsed: false,
                        fallbackReason: nil
                    )
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

    #if canImport(FoundationModels)
    @available(macOS 26.0, *)
    private func validate(generated: GeneratedCleanupIntent) -> StructuredIntent {
        let allowedRisks = ["SAFE", "REVIEW", "CAUTION"]
        let risk = allowedRisks.contains(generated.maxRisk.uppercased()) ? generated.maxRisk.uppercased() : "SAFE"

        let allowedCategories = [
            "BUILD_ARTIFACT", "PACKAGE_MANAGER_CACHE", "CONTAINER_DATA",
            "USER_DOCUMENT", "SOURCE_CODE", "VIRTUAL_MACHINE"
        ]
        let preferred = generated.preferredCategories.filter { allowedCategories.contains($0) }
        let excluded = generated.excludedCategories.filter { allowedCategories.contains($0) }

        var targetBytes: UInt64? = nil
        if let tb = generated.targetReclaimBytes, tb > 0 && tb < (1024 * 1024 * 1024 * 1024 * 1024) { // < 1 PB
            targetBytes = UInt64(tb)
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
    #endif

    /// Grounded storage reasoning: generate explanation referencing only verified context IDs.
    public func explainStorage(prompt: String, contextJSON: String) async -> StorageExplanationResult {
        let (availability, _) = checkAvailability()

        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            if availability == .available {
                let model = SystemLanguageModel.default
                let session = LanguageModelSession(model: model)
                let fullPrompt = "Explain the storage changes using only the provided context facts. Context: \(contextJSON)\nUser question: \(prompt)"
                do {
                    let response = try await session.respond(to: fullPrompt, generating: GeneratedStorageExplanation.self)
                    let gen = response.content
                    return StorageExplanationResult(
                        summary: gen.summary,
                        causes: gen.causes,
                        referenced_candidate_ids: gen.referencedCandidateIDs,
                        referenced_snapshot_ids: gen.referencedSnapshotIDs,
                        caution: gen.caution,
                        provider_used: "apple-system",
                        generation_mode: "apple-guided-generation",
                        fallback_used: false
                    )
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
