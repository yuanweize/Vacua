import Foundation

/// Real MCP stdio client implementing VacuaEngineClient.
public final class MCPVacuaEngineClient: VacuaEngineClient, @unchecked Sendable {
    private let transport: MCPStdioTransport
    private let decoder: JSONDecoder

    public init(transport: MCPStdioTransport) {
        self.transport = transport
        self.decoder = JSONDecoder()
    }

    /// Performs the standard MCP initialize handshake and verifies server capability invariants.
    public func initialize() async throws -> ServerCapabilitiesV1 {
        // 1. Send initialize request
        let initParams: [String: Any] = [
            "protocolVersion": "2025-11-25",
            "capabilities": [
                "roots": ["listChanged": true]
            ],
            "clientInfo": [
                "name": "vacua-app",
                "version": "0.6.0"
            ]
        ]

        _ = try await transport.sendRequest(method: "initialize", params: initParams, timeoutSeconds: 15.0)

        // 2. Send initialized notification
        try await transport.sendNotification(method: "notifications/initialized")

        // 3. Query server capabilities and verify safety boundaries
        let caps = try await capabilities()

        guard !caps.mutation_authority else {
            throw VacuaClientError.protocolViolation(message: "Safety violation: engine exposes mutation authority!")
        }
        guard !caps.executor_linked else {
            throw VacuaClientError.protocolViolation(message: "Safety violation: engine links vacua-executor!")
        }

        return caps
    }

    // MARK: - VacuaEngineClient Methods

    public func capabilities() async throws -> ServerCapabilitiesV1 {
        let caps: ServerCapabilitiesV1 = try await callTool(
            name: "vacua_get_capabilities",
            arguments: [:],
            expectedSchema: VacuaSchemas.serverCapabilitiesV1
        )
        return caps
    }

    public func storageSummary(rootId: String? = nil) async throws -> StorageSummaryV1 {
        var args: [String: Any] = [:]
        if let rootId = rootId {
            args["root_id"] = rootId
        }
        return try await callTool(
            name: "vacua_storage_summary",
            arguments: args,
            expectedSchema: VacuaSchemas.storageSummaryV1
        )
    }

    public func listCandidates(
        rootId: String? = nil,
        maxRisk: McpRiskFilter? = nil,
        category: String? = nil,
        minReclaimBytes: UInt64? = nil,
        limit: Int? = nil,
        cursor: String? = nil
    ) async throws -> CandidateListResponseV1 {
        var args: [String: Any] = [:]
        if let rootId = rootId { args["root_id"] = rootId }
        if let maxRisk = maxRisk { args["max_risk"] = maxRisk.rawValue }
        if let category = category { args["category"] = category }
        if let minReclaimBytes = minReclaimBytes { args["min_reclaim_bytes"] = minReclaimBytes }
        if let limit = limit { args["limit"] = limit }
        if let cursor = cursor { args["cursor"] = cursor }

        return try await callTool(
            name: "vacua_list_candidates",
            arguments: args,
            expectedSchema: VacuaSchemas.candidateListV1
        )
    }

    public func candidateDetail(candidateId: String) async throws -> CandidateDetailV1 {
        let args: [String: Any] = ["candidate_id": candidateId]
        return try await callTool(
            name: "vacua_explain_candidate",
            arguments: args,
            expectedSchema: VacuaSchemas.candidateDetailV1
        )
    }

    public func listDuplicates(
        rootId: String? = nil,
        minSizeBytes: UInt64? = nil,
        limit: Int? = nil,
        cursor: String? = nil
    ) async throws -> DuplicateListResponseV1 {
        var args: [String: Any] = [:]
        if let rootId = rootId { args["root_id"] = rootId }
        if let minSizeBytes = minSizeBytes { args["min_size_bytes"] = minSizeBytes }
        if let limit = limit { args["limit"] = limit }
        if let cursor = cursor { args["cursor"] = cursor }

        return try await callTool(
            name: "vacua_list_duplicates",
            arguments: args,
            expectedSchema: VacuaSchemas.duplicateListV1,
            timeoutSeconds: 60.0 // duplicate hashing may take longer
        )
    }

    public func duplicateDetail(groupId: String) async throws -> DuplicateGroupDetailV1 {
        let args: [String: Any] = ["group_id": groupId]
        return try await callTool(
            name: "vacua_get_duplicate_group",
            arguments: args,
            expectedSchema: VacuaSchemas.duplicateGroupV1,
            timeoutSeconds: 60.0
        )
    }

    public func listApplications(
        filter: ApplicationFilter? = nil,
        limit: Int? = nil,
        cursor: String? = nil
    ) async throws -> ApplicationListResponseV1 {
        var args: [String: Any] = [:]
        if let filter = filter { args["filter"] = filter.rawValue }
        if let limit = limit { args["limit"] = limit }
        if let cursor = cursor { args["cursor"] = cursor }

        return try await callTool(
            name: "vacua_list_applications",
            arguments: args,
            expectedSchema: VacuaSchemas.applicationListV1
        )
    }

    public func applicationDetail(applicationId: String) async throws -> ApplicationDetailV1 {
        let args: [String: Any] = ["application_id": applicationId]
        return try await callTool(
            name: "vacua_get_application",
            arguments: args,
            expectedSchema: VacuaSchemas.applicationDetailV1
        )
    }

    public func listSnapshots(
        limit: Int? = nil,
        cursor: String? = nil
    ) async throws -> SnapshotListResponseV1 {
        var args: [String: Any] = [:]
        if let limit = limit { args["limit"] = limit }
        if let cursor = cursor { args["cursor"] = cursor }

        return try await callTool(
            name: "vacua_list_snapshots",
            arguments: args,
            expectedSchema: VacuaSchemas.snapshotListV1
        )
    }

    public func diffSnapshots(
        base: String,
        target: String? = nil
    ) async throws -> SnapshotDiffV1 {
        var args: [String: Any] = ["base": base]
        if let target = target { args["target"] = target }

        return try await callTool(
            name: "vacua_diff_snapshots",
            arguments: args,
            expectedSchema: VacuaSchemas.snapshotDiffV1
        )
    }

    public func simulateCleanup(candidateIds: [String]) async throws -> CleanupSimulationV1 {
        let args: [String: Any] = ["candidate_ids": candidateIds]
        return try await callTool(
            name: "vacua_simulate_cleanup",
            arguments: args,
            expectedSchema: VacuaSchemas.cleanupSimulationV1
        )
    }

    public func proposeCleanupPlan(candidateIds: [String]) async throws -> CleanupPlanProposalV1 {
        let args: [String: Any] = ["candidate_ids": candidateIds]
        return try await callTool(
            name: "vacua_propose_cleanup_plan",
            arguments: args,
            expectedSchema: VacuaSchemas.planProposalV1
        )
    }

    public func historySummary() async throws -> HistorySummaryV1 {
        return try await callTool(
            name: "vacua_history_summary",
            arguments: [:],
            expectedSchema: VacuaSchemas.historySummaryV1
        )
    }

    // MARK: - Private Helper

    private func callTool<T: Decodable>(
        name: String,
        arguments: [String: Any],
        expectedSchema: String,
        timeoutSeconds: Double = 30.0
    ) async throws -> T {
        let params: [String: Any] = [
            "name": name,
            "arguments": arguments
        ]

        let rawResponse = try await transport.sendRequest(
            method: "tools/call",
            params: params,
            timeoutSeconds: timeoutSeconds
        )

        guard let json = try? JSONSerialization.jsonObject(with: rawResponse) as? [String: Any],
              let result = json["result"] as? [String: Any] else {
            throw VacuaClientError.protocolViolation(message: "Malformed tool response envelope")
        }

        if let isError = result["isError"] as? Bool, isError {
            let msg = (result["content"] as? [[String: Any]])?.first?["text"] as? String ?? "Tool execution failed"
            throw VacuaClientError.serverError(code: "TOOL_ERROR", message: msg)
        }

        guard let contentArray = result["content"] as? [[String: Any]],
              let firstContent = contentArray.first,
              let textPayload = firstContent["text"] as? String,
              let textData = textPayload.data(using: .utf8) else {
            throw VacuaClientError.protocolViolation(message: "Tool result missing text content payload")
        }

        // Validate schema version
        if let rawObj = try? JSONSerialization.jsonObject(with: textData) as? [String: Any],
           let schema = rawObj["schema_version"] as? String {
            guard schema == expectedSchema else {
                throw VacuaClientError.unsupportedSchema(expected: expectedSchema, actual: schema)
            }
        }

        do {
            return try decoder.decode(T.self, from: textData)
        } catch {
            throw VacuaClientError.decodingFailure(details: "\(error)")
        }
    }
}
