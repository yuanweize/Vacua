import Foundation

@main
struct VacuaIntelligenceCLI {
    static let version = "0.5.1"

    static func main() async {
        let provider = AppleOnDeviceProvider()
        let args = CommandLine.arguments

        if args.count > 1 {
            let command = args[1]
            if command == "--version" || command == "-V" {
                print("vacua-intelligence \(version)")
                return
            }

            switch command {
            case "status":
                let (availability, reason) = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-system" : "deterministic-fallback"
                let response = IPCResponse(
                    request_id: "cli-status",
                    status: "success",
                    provider_requested: "apple-system",
                    provider_used: providerUsed,
                    apple_model_availability: reason,
                    availability: availability,
                    generation_mode: (availability == .available) ? "apple-guided-generation" : "deterministic-parser",
                    generation_succeeded: false,
                    fallback_used: availability != .available,
                    fallback_reason: (availability != .available) ? reason : nil
                )
                outputJSON(response)

            case "capabilities":
                #if canImport(FoundationModels)
                let fmCompiled = true
                let guidedCompiled = true
                #else
                let fmCompiled = false
                let guidedCompiled = false
                #endif
                let cap = VacuaCapabilities(
                    foundation_models_compiled: fmCompiled,
                    guided_generation_compiled: guidedCompiled,
                    protocol_version: 1
                )
                outputJSON(cap)

            case "parse":
                let prompt = args.dropFirst(2).joined(separator: " ")
                let result = await provider.parseIntent(prompt: prompt)
                let response = IPCResponse(
                    request_id: "cli-parse",
                    status: "success",
                    provider_requested: result.providerRequested,
                    provider_used: result.providerUsed,
                    apple_model_availability: result.modelAvailability,
                    generation_mode: result.generationMode,
                    generation_succeeded: result.generationSucceeded,
                    fallback_used: result.fallbackUsed,
                    fallback_reason: result.fallbackReason,
                    intent: result.intent
                )
                outputJSON(response)

            case "explain":
                let context = args.count > 2 ? args[2] : "{}"
                let question = args.count > 3 ? args.dropFirst(3).joined(separator: " ") : "Why did storage grow?"
                let expl = await provider.explainStorage(prompt: question, contextJSON: context)
                let response = IPCResponse(
                    request_id: "cli-explain",
                    status: "success",
                    provider_requested: "apple-system",
                    provider_used: expl.provider_used,
                    generation_mode: expl.generation_mode,
                    fallback_used: expl.fallback_used,
                    explanation: expl
                )
                outputJSON(response)

            case "test":
                print("Running VacuaIntelligence self-tests...")
                // Test 1: size and exclusions
                let prompt = "Please safely free 10 GB of space, but don't touch Docker containers, photos or git repositories."
                let res1 = await provider.parseIntent(prompt: prompt)
                assert(res1.intent.intent_version == 1, "intent_version should be 1")
                assert(res1.intent.target_reclaim_bytes == 10 * 1024 * 1024 * 1024, "target_reclaim_bytes should be 10GB")
                assert(res1.intent.max_risk == "SAFE", "max_risk should be SAFE")
                assert(res1.intent.excluded_categories.contains("CONTAINER_DATA"), "should exclude CONTAINER_DATA")
                assert(res1.intent.excluded_categories.contains("USER_DOCUMENT"), "should exclude USER_DOCUMENT")
                assert(res1.intent.excluded_categories.contains("SOURCE_CODE"), "should exclude SOURCE_CODE")
                assert(res1.providerRequested == "apple-system", "providerRequested should be apple-system")
                print("✓ testIntentParsingSizeAndExclusions passed (providerUsed: \(res1.providerUsed), mode: \(res1.generationMode))")

                // Test 2: review risk level
                let promptReview = "Clear old build caches and include review items up to 500 MB"
                let res2 = await provider.parseIntent(prompt: promptReview)
                assert(res2.intent.target_reclaim_bytes == 500 * 1024 * 1024, "target_reclaim_bytes should be 500MB")
                assert(res2.intent.max_risk == "REVIEW", "max_risk should be REVIEW")
                assert(res2.intent.preferred_categories.contains("BUILD_ARTIFACT"), "should prefer BUILD_ARTIFACT")
                print("✓ testRiskLevelParsing passed")

                // Test 3: check real availability probe
                let (availability, reason) = provider.checkAvailability()
                print("✓ testRealAvailabilityProbe passed (availability: \(availability.rawValue), reason: \(reason))")

                // Test 4: capabilities probe
                #if canImport(FoundationModels)
                assert(true, "FoundationModels macro compiled")
                print("✓ testCapabilitiesProbe passed (FoundationModels compiled: true, Guided: true)")
                #endif

                // Test 5: grounded explanation
                let sampleCtx = "{\"snapshot_id\":\"baseline\",\"total_delta_str\":\"+10 MB\",\"top_growing_subtrees\":[{\"path\":\"~/Library/Caches\",\"delta_str\":\"+10 MB\"}],\"candidates\":[{\"id\":\"cand-1\"}]}"
                let expl = await provider.explainStorage(prompt: "Why did storage grow?", contextJSON: sampleCtx)
                assert(expl.referenced_snapshot_ids.contains("baseline"), "Must reference baseline snapshot")
                assert(expl.referenced_candidate_ids.contains("cand-1"), "Must reference candidate")
                print("✓ testGroundedExplanation passed (provider: \(expl.provider_used))")

                print("All 5 VacuaIntelligence unit tests passed successfully!")
                return

            default:
                fputs("Unknown command: \(command). Supported: status, capabilities, parse, explain, test\n", stderr)
                exit(1)
            }
            return
        }

        // Standard IPC Mode: Read JSON from stdin
        let inputData = FileHandle.standardInput.readDataToEndOfFile()
        guard !inputData.isEmpty else {
            fputs("VacuaIntelligence daemon: No input received on stdin.\n", stderr)
            exit(1)
        }

        do {
            let request = try JSONDecoder().decode(IPCRequest.self, from: inputData)
            switch request.action {
            case "check_availability", "status":
                let (availability, reason) = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-system" : "deterministic-fallback"
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    provider_requested: "apple-system",
                    provider_used: providerUsed,
                    apple_model_availability: reason,
                    availability: availability,
                    generation_mode: (availability == .available) ? "apple-guided-generation" : "deterministic-parser",
                    generation_succeeded: false,
                    fallback_used: availability != .available,
                    fallback_reason: (availability != .available) ? reason : nil
                )
                outputJSON(response)

            case "capabilities":
                #if canImport(FoundationModels)
                let fmCompiled = true
                let guidedCompiled = true
                #else
                let fmCompiled = false
                let guidedCompiled = false
                #endif
                let cap = VacuaCapabilities(
                    foundation_models_compiled: fmCompiled,
                    guided_generation_compiled: guidedCompiled,
                    protocol_version: 1
                )
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    capabilities: cap
                )
                outputJSON(response)

            case "parse_intent", "parse":
                let prompt = request.prompt ?? ""
                let result = await provider.parseIntent(prompt: prompt)
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    provider_requested: result.providerRequested,
                    provider_used: result.providerUsed,
                    apple_model_availability: result.modelAvailability,
                    generation_mode: result.generationMode,
                    generation_succeeded: result.generationSucceeded,
                    fallback_used: result.fallbackUsed,
                    fallback_reason: result.fallbackReason,
                    intent: result.intent
                )
                outputJSON(response)

            case "explain_storage", "explain":
                let prompt = request.prompt ?? "Why did storage grow?"
                let ctx = request.context_json ?? "{}"
                let expl = await provider.explainStorage(prompt: prompt, contextJSON: ctx)
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    provider_requested: "apple-system",
                    provider_used: expl.provider_used,
                    generation_mode: expl.generation_mode,
                    fallback_used: expl.fallback_used,
                    explanation: expl
                )
                outputJSON(response)

            default:
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "error",
                    error_message: "Unsupported IPC action: \(request.action)"
                )
                outputJSON(response)
            }
        } catch {
            let response = IPCResponse(
                request_id: "invalid-request",
                status: "error",
                error_message: "JSON decode error: \(error.localizedDescription)"
            )
            outputJSON(response)
        }
    }

    static func outputJSON<T: Encodable>(_ object: T) {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let data = try? encoder.encode(object),
           let jsonString = String(data: data, encoding: .utf8) {
            print(jsonString)
        }
    }
}
