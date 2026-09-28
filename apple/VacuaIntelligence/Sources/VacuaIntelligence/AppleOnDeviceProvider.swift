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

    /// Parse a natural language cleanup prompt into a typed StructuredIntent.
    /// Real Apple Foundation Models inference is executed ONLY when the model is actually available.
    public func parseIntent(prompt: String) async -> ParsedIntentResult {
        let (availability, reasonString) = checkAvailability()

        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            if availability == .available {
                // Real Apple Foundation Models Inference Session
                let model = SystemLanguageModel.default
                let session = LanguageModelSession(model: model)

                let systemInstruction = """
                Translate the user storage cleanup request into a JSON object strictly matching this schema:
                {
                  "intent_version": 1,
                  "target_reclaim_bytes": <integer or null>,
                  "max_risk": "SAFE" | "REVIEW" | "CAUTION",
                  "preferred_categories": [<string>],
                  "excluded_categories": [<string>],
                  "include_developer_artifacts": true,
                  "prefer_reversible_actions": true,
                  "explanation_requested": <boolean>
                }
                Allowed categories: BUILD_ARTIFACT, PACKAGE_MANAGER_CACHE, CONTAINER_DATA, USER_DOCUMENT, SOURCE_CODE, VIRTUAL_MACHINE.
                Respond with ONLY the raw JSON object. Do not include markdown codeblocks or extra text.
                """

                do {
                    let promptWithContext = "\(systemInstruction)\n\nUser request: \"\(prompt)\""
                    let response = try await session.respond(to: promptWithContext)
                    let responseText = response.content.trimmingCharacters(in: .whitespacesAndNewlines)

                    // Attempt decoding model output into StructuredIntent
                    if let data = responseText.data(using: .utf8),
                       let intent = try? JSONDecoder().decode(StructuredIntent.self, from: data) {
                        return ParsedIntentResult(
                            intent: intent,
                            providerRequested: "apple-system",
                            providerUsed: "apple-system",
                            modelAvailability: reasonString,
                            generationSucceeded: true,
                            fallbackUsed: false
                        )
                    }
                } catch {
                    // Inference failed; fall through to verified deterministic fallback
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
            generationSucceeded: false,
            fallbackUsed: true
        )
    }
}
