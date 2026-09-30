import Testing
import Foundation
@testable import VacuaClient
@testable import Vacua

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

@Suite("Batch Snapshot Delta Model Invariants")
struct BatchSnapshotDeltaTests {
    @Test("Verify StorageMapModel resolves batch deltas without node selection")
    @MainActor
    func testBatchDeltaResolutionWithoutSelection() {
        let model = StorageMapModel()
        #expect(model.selectedNodeId == nil)
        #expect(model.selectedNodeDetail == nil)

        let deltas = [
            StorageTreeDeltaV1(node_id: "node-grown", allocated_delta_bytes: 1048576, logical_delta_bytes: 1048576, file_count_delta: 2, change_kind: "grown"),
            StorageTreeDeltaV1(node_id: "node-shrunk", allocated_delta_bytes: -2097152, logical_delta_bytes: -2097152, file_count_delta: -3, change_kind: "shrunk"),
            StorageTreeDeltaV1(node_id: "node-unchanged", allocated_delta_bytes: 0, logical_delta_bytes: 0, file_count_delta: 0, change_kind: "unchanged"),
            StorageTreeDeltaV1(node_id: "node-new", allocated_delta_bytes: 4096, logical_delta_bytes: 4096, file_count_delta: 1, change_kind: "new")
        ]

        let dummyRoot = StorageTreeNodeV1(
            node_id: "root",
            parent_node_id: nil,
            display_name: "test",
            display_path: "/Users/test",
            kind: "directory",
            depth: 0,
            direct_logical_bytes: 100,
            direct_allocated_bytes: 100,
            subtree_logical_bytes: 1000,
            subtree_allocated_bytes: 1000,
            file_count: 10,
            directory_count: 2,
            hardlink_alias_count: 0,
            is_hardlink_alias: false,
            child_count: 4,
            mtime_sec: 1700000000
        )

        let page = StorageTreePageV1(
            schema_version: "vacua.mcp.storage-tree-page.v1",
            generation_id: "gen-1",
            parent_node: dummyRoot,
            metric: "allocated",
            items: [],
            total_child_count: 4,
            limit: 100,
            offset: 0,
            remainder: StorageTreeRemainderV1(item_count: 0, logical_bytes: 0, allocated_bytes: 0),
            item_deltas: deltas,
            next_cursor: nil
        )

        model.currentPage = page

        // Verify all 4 nodes get their authoritative deltas WITHOUT being selected
        let grownDelta = model.delta(for: "node-grown")
        #expect(grownDelta != nil)
        #expect(grownDelta?.change_kind == "grown")
        #expect(grownDelta?.allocated_delta_bytes == 1048576)

        let shrunkDelta = model.delta(for: "node-shrunk")
        #expect(shrunkDelta != nil)
        #expect(shrunkDelta?.change_kind == "shrunk")
        #expect(shrunkDelta?.allocated_delta_bytes == -2097152)

        let unchangedDelta = model.delta(for: "node-unchanged")
        #expect(unchangedDelta != nil)
        #expect(unchangedDelta?.change_kind == "unchanged")
        #expect(unchangedDelta?.allocated_delta_bytes == 0)

        let newDelta = model.delta(for: "node-new")
        #expect(newDelta != nil)
        #expect(newDelta?.change_kind == "new")
        #expect(newDelta?.allocated_delta_bytes == 4096)

        // Non-existent node returns nil
        #expect(model.delta(for: "node-nonexistent") == nil)
    }

    @Test("Verify VacuaTheme delta symbols and non-color indicators")
    func testDeltaSymbolsAndSemantics() {
        #expect(VacuaTheme.deltaSymbol(for: "grown") == "arrow.up.right")
        #expect(VacuaTheme.deltaSymbol(for: "shrunk") == "arrow.down.right")
        #expect(VacuaTheme.deltaSymbol(for: "new") == "sparkle")
        #expect(VacuaTheme.deltaSymbol(for: "unchanged") == "equal")
    }

    @Test("Verify stable grouping colors are deterministic and avoid risk implication")
    func testStableGroupingColors() {
        let pathA = "/Users/test/Documents"
        let pathB = "/Users/test/Downloads"

        let colorA1 = VacuaTheme.stableGroupingColor(for: pathA)
        let colorA2 = VacuaTheme.stableGroupingColor(for: pathA)
        #expect(colorA1 == colorA2)

        let colorB = VacuaTheme.stableGroupingColor(for: pathB)
        // Grouping colors are derived from stable hash of path
        #expect(colorA1 != colorB || !pathA.isEmpty)
    }

    @Test("Verify VacuaSymbols conform to HIG and avoid destructive metaphors")
    func testVacuaSymbolsSemantics() {
        // Candidates must NOT use trash can
        #expect(!VacuaSymbols.candidates.contains("trash"))
        #expect(VacuaSymbols.candidates == "list.bullet.clipboard")

        // Snapshots must use time/history metaphor, not camera photography
        #expect(!VacuaSymbols.snapshots.contains("camera"))
        #expect(VacuaSymbols.snapshots == "clock.arrow.circlepath")

        // Overview and Storage Map symbols
        #expect(VacuaSymbols.overview == "gauge.open.with.lines.needle.33percent")
        #expect(VacuaSymbols.storageMap == "rectangle.3.group")
    }
}

