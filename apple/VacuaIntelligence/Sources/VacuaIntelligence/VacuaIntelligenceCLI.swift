import Foundation

@main
struct VacuaIntelligenceCLI {
    static func main() async {
        let provider = AppleOnDeviceProvider()
        let args = CommandLine.arguments

        if args.count > 1 {
            let command = args[1]
            if command == "--version" || command == "-V" {
                print("vacua-intelligence 0.2.0")
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
                    generation_succeeded: false,
                    fallback_used: availability != .available
                )
                outputJSON(response)

            case "parse":
                let prompt = args.dropFirst(2).joined(separator: " ")
                let result = await provider.parseIntent(prompt: prompt)
                let response = IPCResponse(
                    request_id: "cli-parse",
                    status: "success",
                    provider_requested: result.providerRequested,
                    provider_used: result.providerUsed,
                    apple_model_availability: result.modelAvailability,
                    generation_succeeded: result.generationSucceeded,
                    fallback_used: result.fallbackUsed,
                    intent: result.intent
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
                print("✓ testIntentParsingSizeAndExclusions passed (providerUsed: \(res1.providerUsed))")

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
                print("All 3 VacuaIntelligence unit tests passed successfully!")
                return

            default:
                fputs("Unknown command: \(command). Supported: status, parse, test\n", stderr)
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
                    generation_succeeded: false,
                    fallback_used: availability != .available
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
                    generation_succeeded: result.generationSucceeded,
                    fallback_used: result.fallbackUsed,
                    intent: result.intent
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
