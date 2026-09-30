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
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.largeTitle)
                        .foregroundStyle(.orange)
                    Text("Storage Analysis Failed")
                        .font(.title3)
                        .fontWeight(.semibold)
                    Text(errorMsg)
                        .font(.body)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 32)

                    Button("Try Again") {
                        Task { await model.analyzeStorageMap(forceRefresh: true) }
                    }
                    .buttonStyle(.borderedProminent)
                    .padding(.top, 8)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(Color(nsColor: .windowBackgroundColor))
    }
}

// MARK: - Idle Welcome Screen

struct StorageMapIdleView: View {
    let onAnalyze: () -> Void

    var body: some View {
        VStack(spacing: 20) {
            Image(systemName: "square.split.2x2")
                .font(.system(size: 56))
                .foregroundStyle(.tint)

            VStack(spacing: 6) {
                Text("Visual Storage Map")
                    .font(.title)
                    .fontWeight(.bold)

                Text("Analyze the selected storage root to build a hierarchical view of logical and allocated storage.")
                    .font(.body)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 460)
            }

            VStack(alignment: .leading, spacing: 6) {
                Label("Deterministic hardlink-aware allocation attribution", systemImage: "link")
                Label("APFS clone extent sharing transparency", systemImage: "square.on.square")
                Label("Interactive Squarified Treemap drill-down", systemImage: "arrow.down.forward.and.arrow.up.backward")
                Label("Point-in-time snapshot growth & shrink overlays", systemImage: "chart.line.uptrend.xyaxis")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            .padding(.vertical, 8)

            Button(action: onAnalyze) {
                Label("Analyze Storage Map", systemImage: "arrow.triangle.2.circlepath")
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding()
    }
}

// MARK: - Snapshot Delta Legend

struct SnapshotDeltaLegendView: View {
    var body: some View {
        HStack(spacing: 16) {
            Text("Delta Legend:")
                .font(.caption2)
                .fontWeight(.bold)
                .foregroundStyle(.secondary)

            LegendItem(color: .red.opacity(0.6), label: "Grown")
            LegendItem(color: .green.opacity(0.6), label: "Shrunk")
            LegendItem(color: .purple.opacity(0.6), label: "New Since Snapshot")
            LegendItem(color: .gray.opacity(0.4), label: "Unchanged")

            Spacer()
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 4)
        .background(Color(nsColor: .controlBackgroundColor).opacity(0.5))
        Divider()
    }
}

struct LegendItem: View {
    let color: Color
    let label: String

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
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
