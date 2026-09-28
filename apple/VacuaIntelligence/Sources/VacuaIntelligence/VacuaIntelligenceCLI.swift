import Foundation

@main
struct VacuaIntelligenceCLI {
    static func main() async {
        let provider = AppleOnDeviceProvider()
        let args = CommandLine.arguments

        if args.count > 1 {
            let command = args[1]
            if command == "--version" || command == "-V" {
                print("vacua-intelligence 0.1.0")
                return
            }

            switch command {
            case "status":
                let availability = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-on-device" : "deterministic-fallback"
                let response = IPCResponse(
                    request_id: "cli-status",
                    status: "success",
                    provider_requested: "apple-on-device",
                    provider_used: providerUsed,
                    apple_model_availability: availability.rawValue,
                    availability: availability
                )
                outputJSON(response)

            case "parse":
                let prompt = args.dropFirst(2).joined(separator: " ")
                let availability = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-on-device" : "deterministic-fallback"
                let intent = await provider.parseIntent(prompt: prompt)
                let response = IPCResponse(
                    request_id: "cli-parse",
                    status: "success",
                    provider_requested: "apple-on-device",
                    provider_used: providerUsed,
                    apple_model_availability: availability.rawValue,
                    availability: availability,
                    intent: intent
                )
                outputJSON(response)

            case "test":
                print("Running VacuaIntelligence self-tests...")
                // Test 1: size and exclusions
                let prompt = "Please safely free 10 GB of space, but don't touch Docker containers, photos or git repositories."
                let intent = await provider.parseIntent(prompt: prompt)
                assert(intent.intent_version == 1, "intent_version should be 1")
                assert(intent.target_reclaim_bytes == 10 * 1024 * 1024 * 1024, "target_reclaim_bytes should be 10GB")
                assert(intent.max_risk == "SAFE", "max_risk should be SAFE")
                assert(intent.excluded_categories.contains("CONTAINER_DATA"), "should exclude CONTAINER_DATA")
                assert(intent.excluded_categories.contains("USER_DOCUMENT"), "should exclude USER_DOCUMENT")
                assert(intent.excluded_categories.contains("SOURCE_CODE"), "should exclude SOURCE_CODE")
                print("✓ testIntentParsingSizeAndExclusions passed")

                // Test 2: review risk level
                let promptReview = "Clear old build caches and include review items up to 500 MB"
                let intentReview = await provider.parseIntent(prompt: promptReview)
                assert(intentReview.target_reclaim_bytes == 500 * 1024 * 1024, "target_reclaim_bytes should be 500MB")
                assert(intentReview.max_risk == "REVIEW", "max_risk should be REVIEW")
                assert(intentReview.preferred_categories.contains("BUILD_ARTIFACT"), "should prefer BUILD_ARTIFACT")
                print("✓ testRiskLevelParsing passed")

                // Test 3: check availability
                let availability = provider.checkAvailability()
                print("✓ testAvailabilityProbe passed (availability: \(availability.rawValue))")
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
                let availability = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-on-device" : "deterministic-fallback"
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    provider_requested: "apple-on-device",
                    provider_used: providerUsed,
                    apple_model_availability: availability.rawValue,
                    availability: availability
                )
                outputJSON(response)

            case "parse_intent", "parse":
                let prompt = request.prompt ?? ""
                let availability = provider.checkAvailability()
                let providerUsed = (availability == .available) ? "apple-on-device" : "deterministic-fallback"
                let intent = await provider.parseIntent(prompt: prompt)
                let response = IPCResponse(
                    request_id: request.request_id,
                    status: "success",
                    provider_requested: "apple-on-device",
                    provider_used: providerUsed,
                    apple_model_availability: availability.rawValue,
                    availability: availability,
                    intent: intent
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
