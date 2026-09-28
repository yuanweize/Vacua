import Foundation

public struct StructuredIntent: Codable, Equatable, Sendable {
    public let intent_version: Int
    public let target_reclaim_bytes: UInt64?
    public let max_risk: String
    public let preferred_categories: [String]
    public let excluded_categories: [String]
    public let excluded_candidate_ids: [String]
    public let include_developer_artifacts: Bool
    public let prefer_reversible_actions: Bool
    public let explanation_requested: Bool

    public init(
        intent_version: Int = 1,
        target_reclaim_bytes: UInt64? = nil,
        max_risk: String = "SAFE",
        preferred_categories: [String] = [],
        excluded_categories: [String] = [],
        excluded_candidate_ids: [String] = [],
        include_developer_artifacts: Bool = true,
        prefer_reversible_actions: Bool = true,
        explanation_requested: Bool = false
    ) {
        self.intent_version = intent_version
        self.target_reclaim_bytes = target_reclaim_bytes
        self.max_risk = max_risk
        self.preferred_categories = preferred_categories
        self.excluded_categories = excluded_categories
        self.excluded_candidate_ids = excluded_candidate_ids
        self.include_developer_artifacts = include_developer_artifacts
        self.prefer_reversible_actions = prefer_reversible_actions
        self.explanation_requested = explanation_requested
    }
}

public enum ProviderAvailability: String, Codable, Sendable {
    case available
    case deviceNotEligible
    case appleIntelligenceNotEnabled
    case modelNotReady
    case unsupported
}

public struct IPCRequest: Codable {
    public let protocol_version: Int
    public let request_id: String
    public let action: String
    public let prompt: String?
}

public struct IPCResponse: Codable {
    public let protocol_version: Int
    public let request_id: String
    public let status: String
    public let provider_requested: String?
    public let provider_used: String?
    public let apple_model_availability: String?
    public let availability: ProviderAvailability?
    public let intent: StructuredIntent?
    public let error_message: String?

    public init(
        protocol_version: Int = 1,
        request_id: String,
        status: String,
        provider_requested: String? = nil,
        provider_used: String? = nil,
        apple_model_availability: String? = nil,
        availability: ProviderAvailability? = nil,
        intent: StructuredIntent? = nil,
        error_message: String? = nil
    ) {
        self.protocol_version = protocol_version
        self.request_id = request_id
        self.status = status
        self.provider_requested = provider_requested
        self.provider_used = provider_used
        self.apple_model_availability = apple_model_availability
        self.availability = availability
        self.intent = intent
        self.error_message = error_message
    }
}
