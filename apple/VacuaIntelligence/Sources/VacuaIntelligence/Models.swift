import Foundation

public struct GeneratedCleanupIntent: Codable, Sendable {
    public var targetReclaimBytes: Int?
    public var maxRisk: String
    public var preferredCategories: [String]
    public var excludedCategories: [String]
    public var preferReversibleActions: Bool
    public var includeDeveloperArtifacts: Bool
    public var explanationRequested: Bool

    public init(
        targetReclaimBytes: Int? = nil,
        maxRisk: String = "SAFE",
        preferredCategories: [String] = [],
        excludedCategories: [String] = [],
        preferReversibleActions: Bool = true,
        includeDeveloperArtifacts: Bool = true,
        explanationRequested: Bool = false
    ) {
        self.targetReclaimBytes = targetReclaimBytes
        self.maxRisk = maxRisk
        self.preferredCategories = preferredCategories
        self.excludedCategories = excludedCategories
        self.preferReversibleActions = preferReversibleActions
        self.includeDeveloperArtifacts = includeDeveloperArtifacts
        self.explanationRequested = explanationRequested
    }
}

public struct GeneratedStorageExplanation: Codable, Sendable {
    public var summary: String
    public var causes: [String]
    public var referencedCandidateIDs: [String]
    public var referencedSnapshotIDs: [String]
    public var caution: String?

    public init(
        summary: String = "",
        causes: [String] = [],
        referencedCandidateIDs: [String] = [],
        referencedSnapshotIDs: [String] = [],
        caution: String? = nil
    ) {
        self.summary = summary
        self.causes = causes
        self.referencedCandidateIDs = referencedCandidateIDs
        self.referencedSnapshotIDs = referencedSnapshotIDs
        self.caution = caution
    }
}

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

public struct ParsedIntentResult: Sendable {
    public let intent: StructuredIntent
    public let providerRequested: String
    public let providerUsed: String
    public let modelAvailability: String
    public let generationMode: String
    public let generationSucceeded: Bool
    public let fallbackUsed: Bool
    public let fallbackReason: String?

    public init(
        intent: StructuredIntent,
        providerRequested: String,
        providerUsed: String,
        modelAvailability: String,
        generationMode: String = "deterministic-parser",
        generationSucceeded: Bool,
        fallbackUsed: Bool,
        fallbackReason: String? = nil
    ) {
        self.intent = intent
        self.providerRequested = providerRequested
        self.providerUsed = providerUsed
        self.modelAvailability = modelAvailability
        self.generationMode = generationMode
        self.generationSucceeded = generationSucceeded
        self.fallbackUsed = fallbackUsed
        self.fallbackReason = fallbackReason
    }
}

public struct VacuaCapabilities: Codable, Sendable {
    public let foundation_models_compiled: Bool
    public let guided_generation_compiled: Bool
    public let protocol_version: Int

    public init(
        foundation_models_compiled: Bool,
        guided_generation_compiled: Bool,
        protocol_version: Int = 1
    ) {
        self.foundation_models_compiled = foundation_models_compiled
        self.guided_generation_compiled = guided_generation_compiled
        self.protocol_version = protocol_version
    }
}

public struct StorageExplanationResult: Codable, Sendable {
    public let summary: String
    public let causes: [String]
    public let referenced_candidate_ids: [String]
    public let referenced_snapshot_ids: [String]
    public let caution: String?
    public let provider_used: String
    public let generation_mode: String
    public let fallback_used: Bool

    public init(
        summary: String,
        causes: [String],
        referenced_candidate_ids: [String],
        referenced_snapshot_ids: [String],
        caution: String? = nil,
        provider_used: String,
        generation_mode: String,
        fallback_used: Bool
    ) {
        self.summary = summary
        self.causes = causes
        self.referenced_candidate_ids = referenced_candidate_ids
        self.referenced_snapshot_ids = referenced_snapshot_ids
        self.caution = caution
        self.provider_used = provider_used
        self.generation_mode = generation_mode
        self.fallback_used = fallback_used
    }
}

public struct IPCRequest: Codable {
    public let protocol_version: Int
    public let request_id: String
    public let action: String
    public let prompt: String?
    public let context_json: String?
}

public struct IPCResponse: Codable {
    public let protocol_version: Int
    public let request_id: String
    public let status: String
    public let provider_requested: String?
    public let provider_used: String?
    public let apple_model_availability: String?
    public let availability: ProviderAvailability?
    public let generation_mode: String?
    public let generation_succeeded: Bool?
    public let fallback_used: Bool?
    public let fallback_reason: String?
    public let intent: StructuredIntent?
    public let explanation: StorageExplanationResult?
    public let capabilities: VacuaCapabilities?
    public let error_message: String?

    public init(
        protocol_version: Int = 1,
        request_id: String,
        status: String,
        provider_requested: String? = nil,
        provider_used: String? = nil,
        apple_model_availability: String? = nil,
        availability: ProviderAvailability? = nil,
        generation_mode: String? = nil,
        generation_succeeded: Bool? = nil,
        fallback_used: Bool? = nil,
        fallback_reason: String? = nil,
        intent: StructuredIntent? = nil,
        explanation: StorageExplanationResult? = nil,
        capabilities: VacuaCapabilities? = nil,
        error_message: String? = nil
    ) {
        self.protocol_version = protocol_version
        self.request_id = request_id
        self.status = status
        self.provider_requested = provider_requested
        self.provider_used = provider_used
        self.apple_model_availability = apple_model_availability
        self.availability = availability
        self.generation_mode = generation_mode
        self.generation_succeeded = generation_succeeded
        self.fallback_used = fallback_used
        self.fallback_reason = fallback_reason
        self.intent = intent
        self.explanation = explanation
        self.capabilities = capabilities
        self.error_message = error_message
    }
}
