import Foundation

/// Offline, deterministic keyword and regex intent parser.
/// Serves as the guaranteed, reproducible fallback when on-device neural models are not available.
public struct DeterministicIntentParser: Sendable {
    public init() {}

    public func parse(prompt: String) -> StructuredIntent {
        let lower = prompt.lowercased()

        // 1. Target reclaim bytes extraction
        var targetBytes: UInt64? = nil
        let gib: UInt64 = 1024 * 1024 * 1024
        let mib: UInt64 = 1024 * 1024

        if let match = lower.range(of: #"\b(\d+)\s*(gb|g|gib)\b"#, options: .regularExpression) {
            let matchedStr = String(lower[match])
            let numStr = matchedStr.filter { $0.isNumber }
            if let num = UInt64(numStr) {
                targetBytes = num * gib
            }
        } else if let match = lower.range(of: #"\b(\d+)\s*(mb|m|mib)\b"#, options: .regularExpression) {
            let matchedStr = String(lower[match])
            let numStr = matchedStr.filter { $0.isNumber }
            if let num = UInt64(numStr) {
                targetBytes = num * mib
            }
        }

        // 2. Risk determination
        var maxRisk = "SAFE"
        if lower.contains("review") {
            maxRisk = "REVIEW"
        } else if lower.contains("caution") || lower.contains("aggressive") {
            maxRisk = "CAUTION"
        }

        // 3. Excluded categories
        var excluded: [String] = []
        if lower.contains("docker") || lower.contains("container") {
            excluded.append("CONTAINER_DATA")
        }
        if lower.contains("photo") || lower.contains("document") || lower.contains("desktop") {
            excluded.append("USER_DOCUMENT")
        }
        if lower.contains("git") || lower.contains("source") {
            excluded.append("SOURCE_CODE")
        }
        if lower.contains("vm") || lower.contains("virtual machine") {
            excluded.append("VIRTUAL_MACHINE")
        }

        // 4. Preferred categories
        var preferred: [String] = []
        if lower.contains("xcode") || lower.contains("deriveddata") || lower.contains("build") {
            preferred.append("BUILD_ARTIFACT")
        }
        if lower.contains("homebrew") || lower.contains("brew") || lower.contains("cache") || lower.contains("npm") {
            preferred.append("PACKAGE_MANAGER_CACHE")
        }

        return StructuredIntent(
            intent_version: 1,
            target_reclaim_bytes: targetBytes,
            max_risk: maxRisk,
            preferred_categories: preferred,
            excluded_categories: excluded,
            excluded_candidate_ids: [],
            include_developer_artifacts: true,
            prefer_reversible_actions: true,
            explanation_requested: lower.contains("explain") || lower.contains("why")
        )
    }
}
