import SwiftUI
import VacuaClient

public struct StorageMapView: View {
    @Bindable var model: StorageMapModel

    public init(model: StorageMapModel) {
        self.model = model
    }

    public var body: some View {
        VStack(spacing: 0) {
            switch model.state {
            case .idle:
                StorageMapIdleView {
                    Task { await model.analyzeStorageMap(forceRefresh: false) }
                }

            case .loading(let message):
                VStack(spacing: 16) {
                    ProgressView()
                        .controlSize(.large)
                    Text(message ?? "Analyzing storage hierarchy...")
                        .font(.headline)
                        .foregroundStyle(.secondary)
                    Text("Computing hardlink attribution and bottom-up rollups in Rust engine")
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)

            case .loaded:
                VStack(spacing: 0) {
                    StorageMapToolbar(model: model)

                    if model.colorMode == .snapshotDelta {
                        SnapshotDeltaLegendView()
                    }

                    HSplitView {
                        // Main Visual or Accessible Presentation
                        Group {
                            if model.presentationMode == .treemap {
                                TreemapView(model: model)
                            } else {
                                StorageMapListView(model: model)
                            }
                        }
                        .frame(minWidth: 400, maxWidth: .infinity, maxHeight: .infinity)

                        // Inspector Panel
                        StorageNodeInspector(
                            detail: model.selectedNodeDetail,
                            isLoading: model.isLoadingNodeDetail
                        )
                    }
                }

            case .error(let errorMsg):
                VacuaErrorState(
                    title: "Storage Analysis Failed",
                    message: errorMsg,
                    retryAction: {
                        Task { await model.analyzeStorageMap(forceRefresh: true) }
                    }
                )
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

// MARK: - Idle Welcome Screen

struct StorageMapIdleView: View {
    let onAnalyze: () -> Void

    var body: some View {
        VacuaEmptyState(
            symbol: VacuaSymbols.storageMap,
            title: "Storage Map",
            message: "Analyze the active storage root to build an interactive treemap explaining where disk capacity is actually allocated.",
            actionTitle: "Analyze Storage Map",
            action: onAnalyze
        )
    }
}

// MARK: - Snapshot Delta Legend

struct SnapshotDeltaLegendView: View {
    @Environment(\.accessibilityDifferentiateWithoutColor) private var differentiateWithoutColor

    var body: some View {
        HStack(spacing: 16) {
            Text("Delta Overlay:")
                .font(.caption2.weight(.bold))
                .foregroundStyle(.secondary)

            LegendItem(
                color: VacuaTheme.deltaColor(for: "grown"),
                symbol: VacuaSymbols.deltaGrown,
                label: "Grown"
            )
            LegendItem(
                color: VacuaTheme.deltaColor(for: "shrunk"),
                symbol: VacuaSymbols.deltaShrunk,
                label: "Shrunk"
            )
            LegendItem(
                color: VacuaTheme.deltaColor(for: "new"),
                symbol: VacuaSymbols.deltaNew,
                label: "New"
            )
            LegendItem(
                color: VacuaTheme.deltaColor(for: "unchanged"),
                symbol: VacuaSymbols.deltaUnchanged,
                label: "Unchanged"
            )

            Spacer()
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 6)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.6))
        Divider()
    }
}

struct LegendItem: View {
    let color: Color
    let symbol: String
    let label: String

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: symbol)
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(color)
            Text(label)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
    }
}

// MARK: - Accessible List View Fallback

struct StorageMapListView: View {
    @Bindable var model: StorageMapModel

    var body: some View {
        Table(model.currentPage?.items ?? [], selection: Binding(
            get: { model.selectedNodeId },
            set: { id in Task { await model.selectNode(id: id) } }
        )) {
            TableColumn("Name") { node in
                HStack(spacing: 6) {
                    Image(systemName: node.isDirectory ? "folder.fill" : "doc.fill")
                        .foregroundStyle(node.isDirectory ? .blue : .secondary)
                    Text(node.display_name)
                }
            }

            TableColumn("Allocated") { node in
                Text(Formatters.formatBytes(node.subtree_allocated_bytes))
            }
            .width(min: 80, ideal: 100)

            TableColumn("Logical") { node in
                Text(Formatters.formatBytes(node.subtree_logical_bytes))
            }
            .width(min: 80, ideal: 100)

            TableColumn("Files") { node in
                Text("\(node.file_count)")
            }
            .width(min: 50, ideal: 70)

            TableColumn("Folders") { node in
                Text("\(node.directory_count)")
            }
            .width(min: 50, ideal: 70)
        }
        .contextMenu(forSelectionType: String.self) { selection in
            if let id = selection.first,
               let node = model.currentPage?.items.first(where: { $0.node_id == id }),
               node.isDirectory {
                Button("Drill Down") {
                    Task { await model.drillDown(to: node) }
                }
            }
        } primaryAction: { selection in
            if let id = selection.first,
               let node = model.currentPage?.items.first(where: { $0.node_id == id }),
               node.isDirectory {
                Task { await model.drillDown(to: node) }
            }
        }
    }
}
