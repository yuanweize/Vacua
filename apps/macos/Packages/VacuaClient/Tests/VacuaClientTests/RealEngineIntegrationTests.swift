import Testing
import Foundation
@testable import VacuaClient

@Suite("Real Engine Process Integration Tests")
struct RealEngineIntegrationTests {
    private func findWorkspaceRoot() -> URL? {
        var current = URL(fileURLWithPath: #filePath)
        while current.pathComponents.count > 1 {
            let cargoToml = current.appendingPathComponent("Cargo.toml")
            if FileManager.default.fileExists(atPath: cargoToml.path) {
                return current
            }
            current.deleteLastPathComponent()
        }
        return nil
    }

    @Test("Spawn real vacua-mcp child process over stdio and query capabilities and storage")
    func testRealEngineSubprocessHandshake() async throws {
        guard let workspaceRoot = findWorkspaceRoot() else {
            return
        }

        let mcpBin = workspaceRoot.appendingPathComponent("target/debug/vacua-mcp")
        guard FileManager.default.isExecutableFile(atPath: mcpBin.path) else {
            // If binary not pre-built in test environment, skip gracefully
            return
        }

        let tempFixtureDir = FileManager.default.temporaryDirectory.appendingPathComponent("vacua_swift_test_\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tempFixtureDir, withIntermediateDirectories: true)
        defer {
            try? FileManager.default.removeItem(at: tempFixtureDir)
        }

        // Create a dummy cache file inside fixture
        let cacheDir = tempFixtureDir.appendingPathComponent("Library/Caches/test_app", isDirectory: true)
        try FileManager.default.createDirectory(at: cacheDir, withIntermediateDirectories: true)
        try "test data cache".data(using: .utf8)?.write(to: cacheDir.appendingPathComponent("data.bin"))

        let transport = MCPStdioTransport(
            executableURL: mcpBin,
            arguments: ["--allow-root", tempFixtureDir.path]
        )

        try await transport.start()

        do {
            let client = MCPVacuaEngineClient(transport: transport)
            let caps = try await client.initialize()

            #expect(caps.mutation_authority == false)
            #expect(caps.executor_linked == false)
            #expect(caps.server_version == "0.9.0")

            let summary = try await client.storageSummary(rootId: nil)
            #expect(summary.schema_version == VacuaSchemas.storageSummaryV1)
            #expect(summary.total_space_bytes > 0)

            let candidates = try await client.listCandidates(
                rootId: nil,
                maxRisk: .safe,
                category: nil,
                minReclaimBytes: nil,
                limit: 10,
                cursor: nil
            )
            #expect(candidates.schema_version == VacuaSchemas.candidateListV1)
        }

        await transport.shutdown()
    }
}
