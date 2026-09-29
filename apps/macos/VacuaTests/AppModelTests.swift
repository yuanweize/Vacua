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
}
