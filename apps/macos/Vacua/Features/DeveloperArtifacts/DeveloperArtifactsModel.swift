import Foundation
import SwiftUI
import VacuaClient
import os

@Observable
@MainActor
public final class DeveloperArtifactsModel {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "DeveloperArtifactsModel")

    public weak var appModel: AppModel?

    // MARK: - State
    public var analysisState: LoadState<DeveloperArtifactAnalysisV1> = .idle
    public var selectedEcosystem: String? = nil // nil = All
    public var selectedProjectId: String? = nil
    public var selectedArtifactId: String? = nil
    public var selectedArtifactDetail: DeveloperArtifactDetailV1? = nil
    public var isLoadingDetail: Bool = false

    // Cached project artifacts map: projectId -> [DeveloperArtifactSummaryV1]
    public var projectArtifacts: [String: [DeveloperArtifactSummaryV1]] = [:]
    public var loadingProjectIds: Set<String> = []

    public var analysis: DeveloperArtifactAnalysisV1? {
        analysisState.value
    }

    public var isAnalyzing: Bool {
        analysisState.isLoading
    }

    public var filteredProjects: [DeveloperProjectSummaryV1] {
        guard let analysis = analysis else { return [] }
        guard let ecosystem = selectedEcosystem, !ecosystem.isEmpty, ecosystem != "All" else {
            return analysis.projects
        }
        return analysis.projects.filter { proj in
            proj.primary_ecosystem.caseInsensitiveCompare(ecosystem) == .orderedSame ||
            proj.all_ecosystems.contains { $0.caseInsensitiveCompare(ecosystem) == .orderedSame }
        }
    }

    public var availableEcosystems: [String] {
        guard let analysis = analysis else {
            return ["All", "Xcode", "Swift", "Rust", "Node", "Python", "Gradle", "Maven", "CMake"]
        }
        var set = Set<String>()
        for p in analysis.projects {
            set.insert(p.primary_ecosystem)
            for e in p.all_ecosystems {
                set.insert(e)
            }
        }
        var list = Array(set).sorted()
        list.insert("All", at: 0)
        return list
    }

    public init() {}

    public func setAppModel(_ model: AppModel) {
        self.appModel = model
    }

    // MARK: - Analysis Action

    public func analyzeDeveloperArtifacts(forceRefresh: Bool = false) async {
        guard let appModel = appModel else { return }
        guard case .ready = appModel.supervisor.state else { return }

        if !forceRefresh && (analysisState.isLoading || analysisState.value != nil) {
            return
        }

        analysisState = .loading(previous: analysisState.value)
        selectedArtifactDetail = nil
        selectedArtifactId = nil

        do {
            let client = try appModel.supervisor.getClient()
            let result = try await client.analyzeDeveloperArtifacts(
                rootId: appModel.activeRootID,
                forceRefresh: forceRefresh
            )
            self.analysisState = .loaded(result)

            // If a project was selected, ensure it's still valid
            if let pid = selectedProjectId, !result.projects.contains(where: { $0.project_id == pid }) {
                selectedProjectId = nil
            }
            if selectedProjectId == nil, let first = result.projects.first {
                selectedProjectId = first.project_id
                await loadArtifactsForProject(projectId: first.project_id)
            }
        } catch {
            self.analysisState = .failed(message: error.localizedDescription, previous: analysisState.value)
            logger.error("Developer artifact analysis failed: \(error.localizedDescription, privacy: .private)")
        }
    }

    // MARK: - Project Selection & Artifacts

    public func selectProject(projectId: String) async {
        self.selectedProjectId = projectId
        await loadArtifactsForProject(projectId: projectId)
    }

    public func loadArtifactsForProject(projectId: String) async {
        if projectArtifacts[projectId] != nil {
            return
        }
        guard let appModel = appModel else { return }
        guard case .ready = appModel.supervisor.state else { return }

        loadingProjectIds.insert(projectId)
        defer { loadingProjectIds.remove(projectId) }

        do {
            let client = try appModel.supervisor.getClient()
            let page = try await client.listDeveloperArtifacts(
                rootId: appModel.activeRootID,
                generationId: nil,
                ecosystem: nil,
                kind: nil,
                confidence: nil,
                projectId: projectId,
                minAllocatedBytes: nil,
                limit: 100,
                offset: 0
            )
            projectArtifacts[projectId] = page.artifacts
        } catch {
            logger.error("Failed to load artifacts for project \(projectId): \(error.localizedDescription, privacy: .private)")
        }
    }

    // MARK: - Artifact Selection & Detail

    public func selectArtifact(artifactId: String) async {
        self.selectedArtifactId = artifactId
        guard let appModel = appModel else { return }
        guard case .ready = appModel.supervisor.state else { return }

        isLoadingDetail = true
        selectedArtifactDetail = nil
        defer { isLoadingDetail = false }

        do {
            let client = try appModel.supervisor.getClient()
            let detail = try await client.developerArtifactDetail(artifactId: artifactId)
            self.selectedArtifactDetail = detail
        } catch {
            logger.error("Failed to load artifact detail \(artifactId): \(error.localizedDescription, privacy: .private)")
        }
    }

    public func navigateToCandidate(candidateId: String) {
        guard let appModel = appModel else { return }
        appModel.selectedNavigation = .candidates
        Task {
            await appModel.loadCandidatesIfNeeded()
            if let cand = appModel.candidates.first(where: { $0.candidate_id == candidateId }) {
                await appModel.selectCandidate(cand)
            }
        }
    }
}
