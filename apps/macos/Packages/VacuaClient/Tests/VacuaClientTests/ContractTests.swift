import Testing
import Foundation
@testable import VacuaClient

@Suite("Cross-Language API Contract Tests")
struct ContractTests {
    private func loadFixtureData(named filename: String) throws -> Data {
        // Find workspace root by traversing upwards from source file
        var current = URL(fileURLWithPath: #filePath)
        while current.pathComponents.count > 1 {
            let candidate = current.appendingPathComponent("fixtures").appendingPathComponent("api").appendingPathComponent(filename)
            if FileManager.default.fileExists(atPath: candidate.path) {
                return try Data(contentsOf: candidate)
            }
            current.deleteLastPathComponent()
        }
        throw VacuaClientError.engineNotFound(path: "fixtures/api/\(filename)")
    }

    @Test("Decode ServerCapabilitiesV1 fixture")
    func testCapabilitiesFixture() throws {
        let data = try loadFixtureData(named: "capabilities-v1.json")
        let caps = try JSONDecoder().decode(ServerCapabilitiesV1.self, from: data)
        #expect(caps.schema_version == VacuaSchemas.serverCapabilitiesV1)
        #expect(caps.mutation_authority == false)
        #expect(caps.executor_linked == false)
        #expect(caps.transport == "stdio")
    }

    @Test("Decode StorageSummaryV1 fixture")
    func testStorageSummaryFixture() throws {
        let data = try loadFixtureData(named: "storage-summary-v1.json")
        let sum = try JSONDecoder().decode(StorageSummaryV1.self, from: data)
        #expect(sum.schema_version == VacuaSchemas.storageSummaryV1)
        #expect(sum.filesystem_type == "apfs")
        #expect(sum.candidate_confirmed_reclaim_bytes == 8589934592)
        #expect(sum.candidate_count == 42)
        #expect(sum.usedSpaceBytes > 0)
    }

    @Test("Decode CandidateListResponseV1 fixture")
    func testCandidateListFixture() throws {
        let data = try loadFixtureData(named: "candidate-list-v1.json")
        let list = try JSONDecoder().decode(CandidateListResponseV1.self, from: data)
        #expect(list.schema_version == VacuaSchemas.candidateListV1)
        #expect(list.items.count == 2)
        #expect(list.items[0].category == "build-cache")
        #expect(list.items[0].risk == "safe")
        #expect(list.items[1].confirmed_reclaim_lower_bound == 0)
    }

    @Test("Decode CandidateDetailV1 fixture")
    func testCandidateDetailFixture() throws {
        let data = try loadFixtureData(named: "candidate-detail-v1.json")
        let detail = try JSONDecoder().decode(CandidateDetailV1.self, from: data)
        #expect(detail.schema_version == VacuaSchemas.candidateDetailV1)
        #expect(detail.candidate_id == "cand-8a9b7c6d5e4f3a2b")
        #expect(detail.reconstructable == true)
        #expect(detail.evidence_signals.count == 3)
    }

    @Test("Decode DuplicateListResponseV1 fixture")
    func testDuplicateListFixture() throws {
        let data = try loadFixtureData(named: "duplicate-list-v1.json")
        let list = try JSONDecoder().decode(DuplicateListResponseV1.self, from: data)
        #expect(list.schema_version == VacuaSchemas.duplicateListV1)
        #expect(list.items.count == 1)
        #expect(list.items[0].member_count == 3)
        #expect(list.items[0].algorithm == "BLAKE3")
    }

    @Test("Decode DuplicateGroupDetailV1 fixture")
    func testDuplicateDetailFixture() throws {
        let data = try loadFixtureData(named: "duplicate-detail-v1.json")
        let detail = try JSONDecoder().decode(DuplicateGroupDetailV1.self, from: data)
        #expect(detail.schema_version == VacuaSchemas.duplicateGroupV1)
        #expect(detail.members.count == 3)
        #expect(detail.members[0].physical_relation == "IndependentExtent")
        #expect(detail.members[2].physical_relation == "SharedClone")
        #expect(detail.members[2].kernel_private_bytes == 0)
    }

    @Test("Decode ApplicationListResponseV1 fixture")
    func testApplicationListFixture() throws {
        let data = try loadFixtureData(named: "application-list-v1.json")
        let list = try JSONDecoder().decode(ApplicationListResponseV1.self, from: data)
        #expect(list.schema_version == VacuaSchemas.applicationListV1)
        #expect(list.items.count == 2)
        #expect(list.items[0].installed == true)
        #expect(list.items[1].orphan_confidence == "high")
    }

    @Test("Decode ApplicationDetailV1 fixture")
    func testApplicationDetailFixture() throws {
        let data = try loadFixtureData(named: "application-detail-v1.json")
        let detail = try JSONDecoder().decode(ApplicationDetailV1.self, from: data)
        #expect(detail.schema_version == VacuaSchemas.applicationDetailV1)
        #expect(detail.artifact_count == 2)
        #expect(detail.artifacts[0].kind == "application-support")
        #expect(detail.artifacts[0].risk == "caution")
    }

    @Test("Decode SnapshotListResponseV1 fixture")
    func testSnapshotListFixture() throws {
        let data = try loadFixtureData(named: "snapshot-list-v1.json")
        let list = try JSONDecoder().decode(SnapshotListResponseV1.self, from: data)
        #expect(list.schema_version == VacuaSchemas.snapshotListV1)
        #expect(list.items.count == 1)
        #expect(list.items[0].name == "pre-cleanup-baseline")
    }

    @Test("Decode SnapshotDiffV1 fixture")
    func testSnapshotDiffFixture() throws {
        let data = try loadFixtureData(named: "snapshot-diff-v1.json")
        let diff = try JSONDecoder().decode(SnapshotDiffV1.self, from: data)
        #expect(diff.schema_version == VacuaSchemas.snapshotDiffV1)
        #expect(diff.allocated_delta_bytes == 1073741824)
        #expect(diff.top_growing.count == 1)
        #expect(diff.top_shrinking.count == 1)
    }

    @Test("Decode CleanupSimulationV1 fixture")
    func testCleanupSimulationFixture() throws {
        let data = try loadFixtureData(named: "cleanup-simulation-v1.json")
        let sim = try JSONDecoder().decode(CleanupSimulationV1.self, from: data)
        #expect(sim.schema_version == VacuaSchemas.cleanupSimulationV1)
        #expect(sim.immediate_reclaim_bytes == 0)
        #expect(sim.highest_risk == "review")
        #expect(sim.rebuild_consequences.count == 2)
    }

    @Test("Decode CleanupPlanProposalV1 fixture")
    func testCleanupProposalFixture() throws {
        let data = try loadFixtureData(named: "cleanup-proposal-v1.json")
        let prop = try JSONDecoder().decode(CleanupPlanProposalV1.self, from: data)
        #expect(prop.schema_version == VacuaSchemas.planProposalV1)
        #expect(prop.proposal_status == "PROPOSAL_ONLY_NOT_EXECUTABLE_VIA_MCP")
        #expect(prop.serialized_plan == nil)
        #expect(prop.items.count == 2)
    }
}
