import Foundation
import SwiftUI
import VacuaClient
import os

public enum StorageTreeMetricType: String, CaseIterable, Identifiable, Sendable {
    case allocated = "Allocated"
    case logical = "Logical"

    public var id: String { rawValue }

    public var apiKey: String {
        switch self {
        case .allocated: return "allocated"
        case .logical: return "logical"
        }
    }

    public var descriptionTitle: String {
        switch self {
        case .allocated: return "Allocated filesystem blocks"
        case .logical: return "Logical byte size"
        }
    }

    public var tooltip: String {
        switch self {
        case .allocated:
            return "Filesystem allocation attributed to this namespace tree. APFS clone sharing may cause physical overlap; not guaranteed unique physical storage."
        case .logical:
            return "Logical file length in bytes without considering filesystem block allocations or compression."
        }
    }
}

public enum TreemapColorMode: String, CaseIterable, Identifiable, Sendable {
    case type = "Type"
    case snapshotDelta = "Change Since Snapshot"

    public var id: String { rawValue }
}

public enum StorageMapState: Sendable, Equatable {
    case idle
    case loading(message: String?)
    case loaded
    case error(message: String)
}

public enum TreemapPresentationMode: String, CaseIterable, Identifiable, Sendable {
    case treemap = "Treemap"
    case list = "List View"

    public var id: String { rawValue }
}

@Observable
@MainActor
public final class StorageMapModel {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "StorageMapModel")

    // MARK: - State
    public var state: StorageMapState = .idle
    public var metric: StorageTreeMetricType = .allocated
    public var colorMode: TreemapColorMode = .type
    public var presentationMode: TreemapPresentationMode = .treemap

    public var currentAnalysis: StorageTreeAnalysisV1?
    public var currentPage: StorageTreePageV1?
    public var navigationStack: [StorageTreeNodeV1] = []

    public var selectedNodeId: String?
    public var selectedNodeDetail: StorageTreeNodeDetailV1?
    public var isLoadingNodeDetail: Bool = false

    public var availableSnapshots: [SnapshotSummaryV1] = []
    public var selectedSnapshotId: String?

    // MARK: - Dependencies
    private weak var appModel: AppModel?

    public init(appModel: AppModel? = nil) {
        self.appModel = appModel
    }

    public func setAppModel(_ appModel: AppModel) {
        self.appModel = appModel
    }

    // MARK: - Current Node
    public var currentNode: StorageTreeNodeV1? {
        navigationStack.last ?? currentAnalysis?.root_node
    }

    // MARK: - Actions

    /// Trigger metadata analysis to build an immutable READY tree generation.
    public func analyzeStorageMap(forceRefresh: Bool = false) async {
        guard let appModel else {
            state = .error(message: "App model unavailable")
            return
        }

        state = .loading(message: "Analyzing filesystem hierarchy...")
        do {
            let client = try appModel.supervisor.getClient()
            let rootId = appModel.activeRootID
            let analysis = try await client.analyzeStorageMap(rootId: rootId, forceRefresh: forceRefresh)
            self.currentAnalysis = analysis
            self.navigationStack = [analysis.root_node]
            self.selectedNodeId = nil
            self.selectedNodeDetail = nil

            // Load initial root page
            await loadPage(
                rootId: analysis.root_id,
                generationId: analysis.generation_id,
                nodeId: nil,
                metric: metric.apiKey
            )

            // Also refresh available snapshots for delta comparison
            await loadSnapshots()

            state = .loaded
        } catch let VacuaClientError.serverError(code, message) {
            logger.error("Analyze storage map failed: \(code) - \(message)")
            state = .error(message: message)
        } catch {
            logger.error("Analyze storage map failed: \(error.localizedDescription)")
            state = .error(message: error.localizedDescription)
        }
    }

    /// Load child page for a specific parent node.
    public func loadPage(
        rootId: String,
        generationId: String,
        nodeId: String?,
        metric: String
    ) async {
        guard let appModel else { return }

        do {
            let client = try appModel.supervisor.getClient()
            let page = try await client.storageMap(
                rootId: rootId,
                generationId: generationId,
                nodeId: nodeId,
                metric: metric,
                limit: 100,
                offset: 0
            )
            self.currentPage = page
        } catch let VacuaClientError.serverError(code, message) where code == "VACUA_STALE_STATE" {
            logger.warning("Tree generation expired or pruned")
            state = .error(message: "Storage Map analysis is no longer available. Please analyze again.")
        } catch {
            logger.error("Failed to load page: \(error.localizedDescription)")
        }
    }

    /// Drill down into a directory node.
    public func drillDown(to node: StorageTreeNodeV1) async {
        guard node.isDirectory, let analysis = currentAnalysis else { return }

        navigationStack.append(node)
        selectedNodeId = nil
        selectedNodeDetail = nil

        await loadPage(
            rootId: analysis.root_id,
            generationId: analysis.generation_id,
            nodeId: node.node_id,
            metric: metric.apiKey
        )
    }

    /// Navigate back up to a specific index in the breadcrumb stack.
    public func navigateToBreadcrumb(at index: Int) async {
        guard index >= 0, index < navigationStack.count, let analysis = currentAnalysis else { return }

        navigationStack = Array(navigationStack.prefix(index + 1))
        selectedNodeId = nil
        selectedNodeDetail = nil

        let targetNode = navigationStack.last
        let targetId = targetNode?.node_id == analysis.root_node.node_id ? nil : targetNode?.node_id

        await loadPage(
            rootId: analysis.root_id,
            generationId: analysis.generation_id,
            nodeId: targetId,
            metric: metric.apiKey
        )
    }

    /// Go back one level in the hierarchy.
    public func goBack() async {
        guard navigationStack.count > 1 else { return }
        await navigateToBreadcrumb(at: navigationStack.count - 2)
    }

    /// Select a node for inspector display.
    public func selectNode(id: String?) async {
        selectedNodeId = id
        guard let id, let analysis = currentAnalysis, let appModel else {
            selectedNodeDetail = nil
            return
        }

        isLoadingNodeDetail = true
        defer { isLoadingNodeDetail = false }

        do {
            let client = try appModel.supervisor.getClient()
            let detail = try await client.storageNode(
                rootId: analysis.root_id,
                generationId: analysis.generation_id,
                nodeId: id,
                compareSnapshotId: selectedSnapshotId
            )
            self.selectedNodeDetail = detail
        } catch {
            logger.error("Failed to query node detail: \(error.localizedDescription)")
            self.selectedNodeDetail = nil
        }
    }

    /// Switch metric between Allocated and Logical.
    public func switchMetric(to newMetric: StorageTreeMetricType) async {
        guard metric != newMetric else { return }
        metric = newMetric

        guard let analysis = currentAnalysis else { return }
        let targetId = currentNode?.node_id == analysis.root_node.node_id ? nil : currentNode?.node_id

        await loadPage(
            rootId: analysis.root_id,
            generationId: analysis.generation_id,
            nodeId: targetId,
            metric: newMetric.apiKey
        )

        if let selectedNodeId {
            await selectNode(id: selectedNodeId)
        }
    }

    /// Compare with a historical snapshot.
    public func selectSnapshotForComparison(_ snapshotId: String?) async {
        self.selectedSnapshotId = snapshotId
        if snapshotId != nil {
            self.colorMode = .snapshotDelta
        } else {
            self.colorMode = .type
        }

        if let selectedNodeId {
            await selectNode(id: selectedNodeId)
        }
    }

    /// Load list of available snapshots for the compare dropdown.
    private func loadSnapshots() async {
        guard let appModel else { return }
        do {
            let client = try appModel.supervisor.getClient()
            let res = try await client.listSnapshots(limit: 20, cursor: nil)
            self.availableSnapshots = res.items
        } catch {
            logger.warning("Failed to load snapshots for comparison: \(error.localizedDescription)")
        }
    }

    /// Clear all state when root is switched in the app.
    public func resetForRootSwitch() {
        state = .idle
        currentAnalysis = nil
        currentPage = nil
        navigationStack = []
        selectedNodeId = nil
        selectedNodeDetail = nil
        selectedSnapshotId = nil
    }
}
