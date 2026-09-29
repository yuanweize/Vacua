import Foundation
import SwiftUI
import VacuaClient
import os

public enum NavigationItem: String, CaseIterable, Identifiable, Sendable {
    case overview = "Overview"
    case candidates = "Candidates"
    case duplicates = "Duplicates"
    case applications = "Applications"
    case snapshots = "Snapshots"
    case settings = "Settings"

    public var id: String { rawValue }
    
    public var iconName: String {
        switch self {
        case .overview: return "gauge.with.needle"
        case .candidates: return "trash"
        case .duplicates: return "doc.on.doc"
        case .applications: return "app.badge"
        case .snapshots: return "camera.metering.matrix"
        case .settings: return "gearshape"
        }
    }
}

public enum ApplicationFilterMode: String, CaseIterable, Identifiable, Sendable {
    case all = "All"
    case installed = "Installed"
    case potentialResidual = "Potential Residual"
    
    public var id: String { rawValue }
}

@Observable
@MainActor
public final class AppModel {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "AppModel")
    
    // MARK: - Navigation & Engine
    public var selectedNavigation: NavigationItem = .overview
    public let supervisor: EngineProcessSupervisor
    public var engineState: EngineState = .starting
    
    // MARK: - Domain Data
    public var storageSummary: StorageSummaryV1?
    public var candidates: [CandidateSummaryV1] = []
    public var selectedCandidate: CandidateSummaryV1?
    public var selectedCandidateDetail: CandidateDetailV1?
    public var candidateRiskFilter: McpRiskFilter? = nil
    public var candidateCategoryFilter: String? = nil
    
    public var duplicateGroups: [DuplicateGroupSummaryV1] = []
    public var selectedDuplicateGroup: DuplicateGroupSummaryV1?
    public var selectedDuplicateDetail: DuplicateGroupDetailV1?
    
    public var applications: [ApplicationSummaryV1] = []
    public var appFilter: ApplicationFilterMode = .all
    public var selectedApplication: ApplicationSummaryV1?
    public var selectedAppDetail: ApplicationDetailV1?
    
    public var snapshots: [SnapshotSummaryV1] = []
    public var selectedSnapshot: SnapshotSummaryV1?
    public var selectedSnapshotDetail: SnapshotDetailV1?
    public var selectedSnapshotDiff: SnapshotDiffV1?
    
    // MARK: - Modals & Overlays
    public var activeSimulation: CleanupSimulationV1?
    public var activeProposal: CleanupPlanProposalV1?
    public var isSimulating: Bool = false
    public var isProposing: Bool = false
    
    // MARK: - UI Feedback
    public var isLoading: Bool = false
    public var errorMessage: String?
    public var lastRefreshDate: Date?
    
    public init(supervisor: EngineProcessSupervisor = EngineProcessSupervisor()) {
        self.supervisor = supervisor
    }
    
    private var hasStartedEngine = false
    
    public func startEngine() async {
        guard !hasStartedEngine else { return }
        hasStartedEngine = true
        
        isLoading = true
        errorMessage = nil
        await supervisor.start()
        self.engineState = supervisor.state
        
        if case .ready = supervisor.state {
            await refreshAll()
        } else if case .unavailable(let err) = supervisor.state {
            self.errorMessage = err.localizedDescription
        }
        isLoading = false
    }
    
    public func refreshAll() async {
        guard case .ready = supervisor.state else { return }
        isLoading = true
        errorMessage = nil
        
        do {
            let client = try supervisor.getClient()
            
            async let summaryTask = client.storageSummary(rootId: nil)
            async let candidatesTask = client.listCandidates(
                rootId: nil,
                maxRisk: candidateRiskFilter,
                category: candidateCategoryFilter,
                minReclaimBytes: nil,
                limit: 100,
                cursor: nil
            )
            async let duplicatesTask = client.listDuplicates(rootId: nil, minSizeBytes: nil, limit: 100, cursor: nil)
            async let appsTask = client.listApplications(filter: .all, limit: 100, cursor: nil)
            async let snapshotsTask = client.listSnapshots(limit: 100, cursor: nil)
            
            let (summary, candidateResp, duplicateResp, appResp, snapshotResp) = try await (
                summaryTask,
                candidatesTask,
                duplicatesTask,
                appsTask,
                snapshotsTask
            )
            
            self.storageSummary = summary
            self.candidates = candidateResp.items
            self.duplicateGroups = duplicateResp.items
            self.applications = appResp.items
            self.snapshots = snapshotResp.items
            self.lastRefreshDate = Date()
        } catch {
            self.errorMessage = error.localizedDescription
            logger.error("Error refreshing engine data: \(error.localizedDescription, privacy: .public)")
        }
        
        isLoading = false
    }
    
    public func refreshCandidates() async {
        guard case .ready = supervisor.state else { return }
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listCandidates(
                rootId: nil,
                maxRisk: candidateRiskFilter,
                category: candidateCategoryFilter,
                minReclaimBytes: nil,
                limit: 100,
                cursor: nil
            )
            self.candidates = resp.items
        } catch {
            self.errorMessage = error.localizedDescription
        }
    }
    
    public func selectCandidate(_ candidate: CandidateSummaryV1) async {
        self.selectedCandidate = candidate
        do {
            let client = try supervisor.getClient()
            self.selectedCandidateDetail = try await client.candidateDetail(candidateId: candidate.candidate_id)
        } catch {
            logger.error("Failed to load candidate detail: \(error.localizedDescription, privacy: .public)")
        }
    }
    
    public func selectDuplicateGroup(_ group: DuplicateGroupSummaryV1) async {
        self.selectedDuplicateGroup = group
        do {
            let client = try supervisor.getClient()
            self.selectedDuplicateDetail = try await client.duplicateDetail(groupId: group.group_id)
        } catch {
            logger.error("Failed to load duplicate group detail: \(error.localizedDescription, privacy: .public)")
        }
    }
    
    public func selectApplication(_ app: ApplicationSummaryV1) async {
        self.selectedApplication = app
        do {
            let client = try supervisor.getClient()
            self.selectedAppDetail = try await client.applicationDetail(applicationId: app.application_id)
        } catch {
            logger.error("Failed to load application detail: \(error.localizedDescription, privacy: .public)")
        }
    }
    
    public func simulateCleanup(candidateIds: [String]) async {
        guard case .ready = supervisor.state else { return }
        isSimulating = true
        do {
            let client = try supervisor.getClient()
            let sim = try await client.simulateCleanup(candidateIds: candidateIds)
            self.activeSimulation = sim
        } catch {
            self.errorMessage = error.localizedDescription
        }
        isSimulating = false
    }
    
    public func proposeCleanup(candidateIds: [String]) async {
        guard case .ready = supervisor.state else { return }
        isProposing = true
        do {
            let client = try supervisor.getClient()
            let prop = try await client.proposeCleanupPlan(candidateIds: candidateIds)
            self.activeProposal = prop
        } catch {
            self.errorMessage = error.localizedDescription
        }
        isProposing = false
    }
    
    public func updateRoot(path: String) async {
        await supervisor.updateRoot(newPath: path)
        self.engineState = supervisor.state
        if case .ready = supervisor.state {
            await refreshAll()
        }
    }
}
