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
        case .candidates: return "list.bullet.rectangle"
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
    
    public var toMcpFilter: ApplicationFilter {
        switch self {
        case .all: return .all
        case .installed: return .installedOnly
        case .potentialResidual: return .orphansOnly
        }
    }
}

@Observable
@MainActor
public final class AppModel {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "AppModel")
    
    // MARK: - Navigation & Engine
    public var selectedNavigation: NavigationItem = .overview
    public let supervisor: EngineProcessSupervisor
    public var engineState: EngineState = .starting
    
    // MARK: - Explicit Root Identity
    public var activeRootID: String? = nil
    public var activeRootPath: String = NSHomeDirectory()
    public var activeRootDisplayName: String = "~"
    
    // MARK: - Feature-Local Load States
    public var overviewState: LoadState<StorageSummaryV1> = .idle
    public var candidatesState: LoadState<[CandidateSummaryV1]> = .idle
    public var duplicatesState: LoadState<[DuplicateGroupSummaryV1]> = .idle
    public var applicationsState: LoadState<[ApplicationSummaryV1]> = .idle
    public var snapshotsState: LoadState<[SnapshotSummaryV1]> = .idle
    public var snapshotDiffState: LoadState<SnapshotDiffV1> = .idle
    
    // Convenience Accessors
    public var storageSummary: StorageSummaryV1? { overviewState.value }
    public var candidates: [CandidateSummaryV1] { candidatesState.value ?? [] }
    public var duplicateGroups: [DuplicateGroupSummaryV1] { duplicatesState.value ?? [] }
    public var applications: [ApplicationSummaryV1] { applicationsState.value ?? [] }
    public var snapshots: [SnapshotSummaryV1] { snapshotsState.value ?? [] }
    public var selectedSnapshotDiff: SnapshotDiffV1? { snapshotDiffState.value }
    
    // Overall / Active Loading helper
    public var isLoading: Bool {
        overviewState.isLoading || candidatesState.isLoading || duplicatesState.isLoading ||
        applicationsState.isLoading || snapshotsState.isLoading || isSimulating || isProposing
    }
    
    // MARK: - Domain Selections & Filters
    public var selectedCandidate: CandidateSummaryV1?
    public var selectedCandidateDetail: CandidateDetailV1?
    public var candidateRiskFilter: McpRiskFilter? = nil
    public var candidateCategoryFilter: String? = nil
    
    public var selectedDuplicateGroup: DuplicateGroupSummaryV1?
    public var selectedDuplicateDetail: DuplicateGroupDetailV1?
    
    public var appFilter: ApplicationFilterMode = .all
    public var selectedApplication: ApplicationSummaryV1?
    public var selectedAppDetail: ApplicationDetailV1?
    
    public var selectedSnapshot: SnapshotSummaryV1?
    public var selectedSnapshotDetail: SnapshotDetailV1?
    
    // MARK: - Cursor-Aware Pagination States
    public var candidateNextCursor: String? = nil
    public var hasMoreCandidates: Bool = false
    public var isLoadingNextCandidatesPage: Bool = false
    
    public var duplicateNextCursor: String? = nil
    public var hasMoreDuplicates: Bool = false
    public var isLoadingNextDuplicatesPage: Bool = false
    
    public var applicationNextCursor: String? = nil
    public var hasMoreApplications: Bool = false
    public var isLoadingNextApplicationsPage: Bool = false
    
    public var snapshotNextCursor: String? = nil
    public var hasMoreSnapshots: Bool = false
    public var isLoadingNextSnapshotsPage: Bool = false
    
    // MARK: - Modals & In-Flight Operations
    public var activeSimulation: CleanupSimulationV1?
    public var activeProposal: CleanupPlanProposalV1?
    public var isSimulating: Bool = false
    public var isProposing: Bool = false
    
    // MARK: - In-Flight Tasks (For Prompt Cancellation)
    private var candidateDetailTask: Task<Void, Never>?
    private var duplicateDetailTask: Task<Void, Never>?
    private var applicationDetailTask: Task<Void, Never>?
    private var snapshotDiffTask: Task<Void, Never>?
    
    // UI Feedback
    public var errorMessage: String?
    public var lastRefreshDate: Date?
    
    public init(supervisor: EngineProcessSupervisor = EngineProcessSupervisor()) {
        self.supervisor = supervisor
        self.activeRootPath = supervisor.currentRootPath
        self.activeRootDisplayName = (supervisor.currentRootPath as NSString).lastPathComponent
    }
    
    private var hasStartedEngine = false
    
    // MARK: - Engine Lifecycle
    
    /// Starts the engine and only loads cheap overview data on startup.
    public func startEngine() async {
        guard !hasStartedEngine else { return }
        hasStartedEngine = true
        await restartEngine()
    }
    
    /// Explicitly restarts the engine process and re-runs cheap overview startup.
    public func restartEngine() async {
        overviewState = .loading(previous: nil)
        errorMessage = nil
        await supervisor.start()
        self.engineState = supervisor.state
        
        if case .ready = supervisor.state {
            // Cheap Startup: Only load Overview, never run expensive features!
            await loadOverview(force: true)
        } else if case .unavailable(let err) = supervisor.state {
            self.errorMessage = err.localizedDescription
            self.overviewState = .failed(message: err.localizedDescription, previous: nil)
        }
    }
    
    // MARK: - Visible Feature Refresh
    
    /// Refreshes ONLY the currently visible feature view (never a giant refresh-all).
    public func refreshCurrentView() async {
        switch selectedNavigation {
        case .overview:
            await loadOverview(force: true)
        case .candidates:
            await loadCandidates(force: true)
        case .duplicates:
            await loadDuplicates(force: true)
        case .applications:
            await loadApplications(force: true)
        case .snapshots:
            await loadSnapshots(force: true)
        case .settings:
            break
        }
    }
    
    // Backward compatibility shim
    public func refreshAll() async {
        await refreshCurrentView()
    }
    
    // MARK: - Overview Feature (Cheap)
    
    public func loadOverview(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        if !force && (overviewState.isLoading || overviewState.value != nil) { return }
        
        overviewState = .loading(previous: overviewState.value)
        errorMessage = nil
        
        do {
            let client = try supervisor.getClient()
            let summary = try await client.storageSummary(rootId: activeRootID)
            self.activeRootPath = summary.target_path
            self.activeRootDisplayName = (summary.target_path as NSString).lastPathComponent
            self.overviewState = .loaded(summary)
            self.lastRefreshDate = Date()
        } catch {
            self.errorMessage = error.localizedDescription
            self.overviewState = .failed(message: error.localizedDescription, previous: overviewState.value)
            logger.error("Error loading storage summary: \(error.localizedDescription, privacy: .private)")
        }
    }
    
    // MARK: - Candidates Feature (Lazy + Paginated)
    
    public func loadCandidatesIfNeeded(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        if !force && (candidatesState.isLoading || candidatesState.value != nil) { return }
        await loadCandidates(force: true)
    }
    
    public func loadCandidates(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        candidatesState = .loading(previous: candidatesState.value)
        candidateNextCursor = nil
        
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listCandidates(
                rootId: activeRootID,
                maxRisk: candidateRiskFilter,
                category: candidateCategoryFilter,
                minReclaimBytes: nil,
                limit: 50,
                cursor: nil
            )
            self.candidatesState = .loaded(resp.items)
            self.candidateNextCursor = resp.next_cursor
            self.hasMoreCandidates = (resp.next_cursor != nil)
        } catch {
            self.errorMessage = error.localizedDescription
            self.candidatesState = .failed(message: error.localizedDescription, previous: candidatesState.value)
            logger.error("Error loading candidates: \(error.localizedDescription, privacy: .private)")
        }
    }
    
    public func loadMoreCandidates() async {
        guard let cursor = candidateNextCursor, !isLoadingNextCandidatesPage else { return }
        guard case .ready = supervisor.state else { return }
        
        isLoadingNextCandidatesPage = true
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listCandidates(
                rootId: activeRootID,
                maxRisk: candidateRiskFilter,
                category: candidateCategoryFilter,
                minReclaimBytes: nil,
                limit: 50,
                cursor: cursor
            )
            var current = candidatesState.value ?? []
            current.append(contentsOf: resp.items)
            self.candidatesState = .loaded(current)
            self.candidateNextCursor = resp.next_cursor
            self.hasMoreCandidates = (resp.next_cursor != nil)
        } catch {
            logger.error("Error fetching more candidates: \(error.localizedDescription, privacy: .private)")
        }
        isLoadingNextCandidatesPage = false
    }
    
    public func selectCandidate(_ candidate: CandidateSummaryV1) async {
        self.selectedCandidate = candidate
        candidateDetailTask?.cancel()
        
        candidateDetailTask = Task {
            do {
                let client = try supervisor.getClient()
                let detail = try await client.candidateDetail(candidateId: candidate.candidate_id)
                guard !Task.isCancelled else { return }
                self.selectedCandidateDetail = detail
            } catch {
                guard !Task.isCancelled else { return }
                logger.error("Failed to load candidate detail: \(error.localizedDescription, privacy: .private)")
            }
        }
        await candidateDetailTask?.value
    }
    
    // MARK: - Duplicates Feature (Explicit Analysis + Paginated)
    
    /// Duplicates analysis is NEVER executed at app launch. Only triggered explicitly by user action.
    public func loadDuplicates(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        duplicatesState = .loading(previous: duplicatesState.value)
        duplicateNextCursor = nil
        
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listDuplicates(
                rootId: activeRootID,
                minSizeBytes: nil,
                limit: 50,
                cursor: nil
            )
            self.duplicatesState = .loaded(resp.items)
            self.duplicateNextCursor = resp.next_cursor
            self.hasMoreDuplicates = (resp.next_cursor != nil)
        } catch {
            self.errorMessage = error.localizedDescription
            self.duplicatesState = .failed(message: error.localizedDescription, previous: duplicatesState.value)
            logger.error("Error loading duplicates: \(error.localizedDescription, privacy: .private)")
        }
    }
    
    public func loadMoreDuplicates() async {
        guard let cursor = duplicateNextCursor, !isLoadingNextDuplicatesPage else { return }
        guard case .ready = supervisor.state else { return }
        
        isLoadingNextDuplicatesPage = true
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listDuplicates(
                rootId: activeRootID,
                minSizeBytes: nil,
                limit: 50,
                cursor: cursor
            )
            var current = duplicatesState.value ?? []
            current.append(contentsOf: resp.items)
            self.duplicatesState = .loaded(current)
            self.duplicateNextCursor = resp.next_cursor
            self.hasMoreDuplicates = (resp.next_cursor != nil)
        } catch {
            logger.error("Error fetching more duplicates: \(error.localizedDescription, privacy: .private)")
        }
        isLoadingNextDuplicatesPage = false
    }
    
    public func selectDuplicateGroup(_ group: DuplicateGroupSummaryV1) async {
        self.selectedDuplicateGroup = group
        duplicateDetailTask?.cancel()
        
        duplicateDetailTask = Task {
            do {
                let client = try supervisor.getClient()
                let detail = try await client.duplicateDetail(groupId: group.group_id)
                guard !Task.isCancelled else { return }
                self.selectedDuplicateDetail = detail
            } catch {
                guard !Task.isCancelled else { return }
                logger.error("Failed to load duplicate group detail: \(error.localizedDescription, privacy: .private)")
            }
        }
        await duplicateDetailTask?.value
    }
    
    // MARK: - Applications Feature (Lazy + Server Filter + Paginated)
    
    public func loadApplicationsIfNeeded(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        if !force && (applicationsState.isLoading || applicationsState.value != nil) { return }
        await loadApplications(force: true)
    }
    
    public func loadApplications(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        applicationsState = .loading(previous: applicationsState.value)
        applicationNextCursor = nil
        
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listApplications(
                filter: appFilter.toMcpFilter,
                limit: 50,
                cursor: nil
            )
            self.applicationsState = .loaded(resp.items)
            self.applicationNextCursor = resp.next_cursor
            self.hasMoreApplications = (resp.next_cursor != nil)
        } catch {
            self.errorMessage = error.localizedDescription
            self.applicationsState = .failed(message: error.localizedDescription, previous: applicationsState.value)
            logger.error("Error loading applications: \(error.localizedDescription, privacy: .private)")
        }
    }
    
    public func loadMoreApplications() async {
        guard let cursor = applicationNextCursor, !isLoadingNextApplicationsPage else { return }
        guard case .ready = supervisor.state else { return }
        
        isLoadingNextApplicationsPage = true
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listApplications(
                filter: appFilter.toMcpFilter,
                limit: 50,
                cursor: cursor
            )
            var current = applicationsState.value ?? []
            current.append(contentsOf: resp.items)
            self.applicationsState = .loaded(current)
            self.applicationNextCursor = resp.next_cursor
            self.hasMoreApplications = (resp.next_cursor != nil)
        } catch {
            logger.error("Error fetching more applications: \(error.localizedDescription, privacy: .private)")
        }
        isLoadingNextApplicationsPage = false
    }
    
    public func selectApplication(_ app: ApplicationSummaryV1) async {
        self.selectedApplication = app
        applicationDetailTask?.cancel()
        
        applicationDetailTask = Task {
            do {
                let client = try supervisor.getClient()
                let detail = try await client.applicationDetail(applicationId: app.application_id)
                guard !Task.isCancelled else { return }
                self.selectedAppDetail = detail
            } catch {
                guard !Task.isCancelled else { return }
                logger.error("Failed to load application detail: \(error.localizedDescription, privacy: .private)")
            }
        }
        await applicationDetailTask?.value
    }
    
    // MARK: - Snapshots & Diff Feature (Lazy + Real Diff)
    
    public func loadSnapshotsIfNeeded(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        if !force && (snapshotsState.isLoading || snapshotsState.value != nil) { return }
        await loadSnapshots(force: true)
    }
    
    public func loadSnapshots(force: Bool = false) async {
        guard case .ready = supervisor.state else { return }
        snapshotsState = .loading(previous: snapshotsState.value)
        snapshotNextCursor = nil
        
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listSnapshots(limit: 50, cursor: nil)
            self.snapshotsState = .loaded(resp.items)
            self.snapshotNextCursor = resp.next_cursor
            self.hasMoreSnapshots = (resp.next_cursor != nil)
        } catch {
            self.errorMessage = error.localizedDescription
            self.snapshotsState = .failed(message: error.localizedDescription, previous: snapshotsState.value)
            logger.error("Error loading snapshots: \(error.localizedDescription, privacy: .private)")
        }
    }
    
    public func loadMoreSnapshots() async {
        guard let cursor = snapshotNextCursor, !isLoadingNextSnapshotsPage else { return }
        guard case .ready = supervisor.state else { return }
        
        isLoadingNextSnapshotsPage = true
        do {
            let client = try supervisor.getClient()
            let resp = try await client.listSnapshots(limit: 50, cursor: cursor)
            var current = snapshotsState.value ?? []
            current.append(contentsOf: resp.items)
            self.snapshotsState = .loaded(current)
            self.snapshotNextCursor = resp.next_cursor
            self.hasMoreSnapshots = (resp.next_cursor != nil)
        } catch {
            logger.error("Error fetching more snapshots: \(error.localizedDescription, privacy: .private)")
        }
        isLoadingNextSnapshotsPage = false
    }
    
    public func selectSnapshot(_ snapshot: SnapshotSummaryV1, compareWithTarget targetName: String? = nil) async {
        self.selectedSnapshot = snapshot
        await computeSnapshotDiff(base: snapshot.name, target: targetName)
    }
    
    public func computeSnapshotDiff(base: String, target: String?) async {
        snapshotDiffTask?.cancel()
        snapshotDiffState = .loading(previous: snapshotDiffState.value)
        
        snapshotDiffTask = Task {
            do {
                let client = try supervisor.getClient()
                let diff = try await client.diffSnapshots(base: base, target: target)
                guard !Task.isCancelled else { return }
                self.snapshotDiffState = .loaded(diff)
            } catch {
                guard !Task.isCancelled else { return }
                self.snapshotDiffState = .failed(message: error.localizedDescription, previous: snapshotDiffState.value)
                logger.error("Failed to diff snapshots: \(error.localizedDescription, privacy: .private)")
            }
        }
        await snapshotDiffTask?.value
    }
    
    // MARK: - Simulation & Proposals
    
    public func simulateCleanup(candidateIds: [String]) async {
        guard case .ready = supervisor.state else { return }
        isSimulating = true
        do {
            let client = try supervisor.getClient()
            let sim = try await client.simulateCleanup(candidateIds: candidateIds)
            self.activeSimulation = sim
        } catch {
            self.errorMessage = error.localizedDescription
            logger.error("Simulation failed: \(error.localizedDescription, privacy: .private)")
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
            logger.error("Proposal failed: \(error.localizedDescription, privacy: .private)")
        }
        isProposing = false
    }
    
    // MARK: - Root Switching & Safety Invalidation
    
    public func updateRoot(path: String) async {
        // Cancel all in-flight detail/diff tasks immediately
        candidateDetailTask?.cancel()
        duplicateDetailTask?.cancel()
        applicationDetailTask?.cancel()
        snapshotDiffTask?.cancel()
        
        // Invalidate all root-scoped state
        self.activeRootID = nil
        self.activeRootPath = path
        self.activeRootDisplayName = (path as NSString).lastPathComponent
        
        self.overviewState = .loading(previous: nil)
        self.candidatesState = .idle
        self.duplicatesState = .idle
        self.applicationsState = .idle
        self.snapshotsState = .idle
        self.snapshotDiffState = .idle
        
        self.selectedCandidate = nil
        self.selectedCandidateDetail = nil
        self.selectedDuplicateGroup = nil
        self.selectedDuplicateDetail = nil
        self.selectedApplication = nil
        self.selectedAppDetail = nil
        self.selectedSnapshot = nil
        self.selectedSnapshotDetail = nil
        
        self.candidateNextCursor = nil
        self.duplicateNextCursor = nil
        self.applicationNextCursor = nil
        self.snapshotNextCursor = nil
        
        self.activeSimulation = nil
        self.activeProposal = nil
        self.errorMessage = nil
        
        await supervisor.updateRoot(newPath: path)
        self.engineState = supervisor.state
        if case .ready = supervisor.state {
            await loadOverview(force: true)
        }
    }
}
