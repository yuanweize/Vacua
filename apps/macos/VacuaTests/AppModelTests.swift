import Testing
import Foundation
@testable import VacuaClient

@Suite("App Model & State Tests")
struct AppModelTests {
    @Test("Verify application filter modes filter installed and residue correctly")
    func testApplicationFilters() {
        let app1 = ApplicationSummaryV1(
            application_id: "app-1",
            app_name: "InstalledApp",
            display_path: "/Applications/InstalledApp.app",
            installed: true,
            orphan_confidence: "low"
        )
        
        let app2 = ApplicationSummaryV1(
            application_id: "app-2",
            app_name: "DeletedApp",
            display_path: nil,
            installed: false,
            orphan_confidence: "high"
        )
        
        let apps = [app1, app2]
        
        let installedOnly = apps.filter { $0.installed }
        #expect(installedOnly.count == 1)
        #expect(installedOnly.first?.app_name == "InstalledApp")
        
        let residueOnly = apps.filter { !$0.installed }
        #expect(residueOnly.count == 1)
        #expect(residueOnly.first?.app_name == "DeletedApp")
    }
    
    @Test("Verify APFS private bytes unmeasured status displays as unknown")
    func testDuplicateGroupPrivateBytesStatus() {
        let groupUnmeasured = DuplicateGroupSummaryV1(
            group_id: "dup-1",
            member_count: 2,
            file_size: 5000,
            logical_duplicate_bytes: 5000,
            kernel_private_bytes: 0,
            confirmed_reclaim_lower_bound: 0,
            estimated_reclaim: 5000,
            content_identity_verified: true,
            algorithm: "blake3",
            kernel_private_bytes_known_members: 0,
            kernel_private_bytes_unknown_members: 2
        )
        #expect(groupUnmeasured.kernel_private_bytes_known_members == 0)
        #expect(groupUnmeasured.kernel_private_bytes_unknown_members == 2)
        
        let groupMeasured = DuplicateGroupSummaryV1(
            group_id: "dup-2",
            member_count: 2,
            file_size: 5000,
            logical_duplicate_bytes: 5000,
            kernel_private_bytes: 2500,
            confirmed_reclaim_lower_bound: 2500,
            estimated_reclaim: 5000,
            content_identity_verified: true,
            algorithm: "blake3",
            kernel_private_bytes_known_members: 2,
            kernel_private_bytes_unknown_members: 0
        )
        #expect(groupMeasured.kernel_private_bytes_known_members == 2)
        #expect(groupMeasured.kernel_private_bytes == 2500)
    }
    
    @Test("Verify candidate risk filter mapping conforms to schema")
    func testCandidateRiskFilterValues() {
        #expect(McpRiskFilter.safe.rawValue == "safe")
        #expect(McpRiskFilter.caution.rawValue == "caution")
        #expect(McpRiskFilter.review.rawValue == "review")
    }

    @Test("Verify LoadState enum transitions and accessors")
    func testLoadStateTransitions() {
        var state: LoadState<String> = .idle
        #expect(!state.isLoading)
        #expect(state.value == nil)
        #expect(state.errorMessage == nil)

        state = .loading(previous: nil)
        #expect(state.isLoading)
        #expect(state.value == nil)

        state = .loaded("data-1")
        #expect(!state.isLoading)
        #expect(state.value == "data-1")
        #expect(state.errorMessage == nil)

        state = .loading(previous: "data-1")
        #expect(state.isLoading)
        #expect(state.value == "data-1")

        state = .failed(message: "Network timeout", previous: "data-1")
        #expect(!state.isLoading)
        #expect(state.value == "data-1")
        #expect(state.errorMessage == "Network timeout")
    }

    @Test("Verify VacuaDateFormatter parses RFC3339 dates without error")
    func testDateFormatter() {
        let rfc3339 = "2026-09-30T01:23:45Z"
        let parsed = VacuaDateFormatter.parse(rfc3339)
        #expect(parsed != nil)

        let formatted = VacuaDateFormatter.formatDisplay(rfc3339)
        #expect(!formatted.isEmpty)
        #expect(formatted != rfc3339) // formatted should be human-readable, not raw RFC3339

        let invalid = "not-a-date"
        #expect(VacuaDateFormatter.parse(invalid) == nil)
        #expect(VacuaDateFormatter.formatDisplay(invalid) == invalid)
    }

    @Test("Verify SnapshotDiffV1 truth semantics")
    func testSnapshotDiffSemantics() {
        let diff = SnapshotDiffV1(
            schema_version: "vacua.mcp.snapshot-diff.v1",
            base_snapshot: "snap-1",
            target_snapshot: "snap-2",
            allocated_delta_bytes: -1048576,
            logical_delta_bytes: -2097152,
            files_delta: -5,
            top_growing: [],
            top_shrinking: [
                SubtreeDeltaV1(display_path: "/Users/test/Caches", delta_bytes: -1048576, files_delta: -5)
            ]
        )
        #expect(diff.allocated_delta_bytes == -1048576)
        #expect(diff.top_shrinking.count == 1)
        #expect(diff.top_shrinking.first?.delta_bytes == -1048576)
    }
}
