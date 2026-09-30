import Foundation

/// Core domain client protocol for communicating with the Vacua storage intelligence engine.
public protocol VacuaEngineClient: Sendable {
    func capabilities() async throws -> ServerCapabilitiesV1
    func storageSummary(rootId: String?) async throws -> StorageSummaryV1
    func listCandidates(
        rootId: String?,
        maxRisk: McpRiskFilter?,
        category: String?,
        minReclaimBytes: UInt64?,
        limit: Int?,
        cursor: String?
    ) async throws -> CandidateListResponseV1
    func candidateDetail(candidateId: String) async throws -> CandidateDetailV1
    func listDuplicates(
        rootId: String?,
        minSizeBytes: UInt64?,
        limit: Int?,
        cursor: String?
    ) async throws -> DuplicateListResponseV1
    func duplicateDetail(groupId: String) async throws -> DuplicateGroupDetailV1
    func listApplications(
        filter: ApplicationFilter?,
        limit: Int?,
        cursor: String?
    ) async throws -> ApplicationListResponseV1
    func applicationDetail(applicationId: String) async throws -> ApplicationDetailV1
    func listSnapshots(
        limit: Int?,
        cursor: String?
    ) async throws -> SnapshotListResponseV1
    func diffSnapshots(
        base: String,
        target: String?
    ) async throws -> SnapshotDiffV1
    func simulateCleanup(
        candidateIds: [String]
    ) async throws -> CleanupSimulationV1
    func proposeCleanupPlan(
        candidateIds: [String]
    ) async throws -> CleanupPlanProposalV1
    func historySummary() async throws -> HistorySummaryV1

    // MARK: - Storage Tree Map
    func analyzeStorageMap(
        rootId: String?,
        forceRefresh: Bool?
    ) async throws -> StorageTreeAnalysisV1

    func storageMap(
        rootId: String,
        generationId: String,
        nodeId: String?,
        metric: String?,
        limit: Int?,
        offset: Int?,
        compareSnapshotId: String?
    ) async throws -> StorageTreePageV1

    func storageNode(
        rootId: String,
        generationId: String,
        nodeId: String,
        compareSnapshotId: String?
    ) async throws -> StorageTreeNodeDetailV1

    // MARK: - Developer Artifacts
    func analyzeDeveloperArtifacts(
        rootId: String?,
        forceRefresh: Bool?
    ) async throws -> DeveloperArtifactAnalysisV1

    func listDeveloperArtifacts(
        rootId: String?,
        generationId: String?,
        ecosystem: String?,
        kind: String?,
        confidence: String?,
        projectId: String?,
        minAllocatedBytes: UInt64?,
        limit: Int?,
        offset: Int?
    ) async throws -> DeveloperArtifactPageV1

    func developerArtifactDetail(
        artifactId: String
    ) async throws -> DeveloperArtifactDetailV1
}

/// Whitelisted, narrow command service for non-destructive operations.
/// Strictly forbids arbitrary CLI arguments or shell execution.
public protocol VacuaCommandService: Sendable {
    func createSnapshot(rootPath: String, name: String) async throws -> String
}
