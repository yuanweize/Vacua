import SwiftUI
import VacuaClient

public struct StorageMapToolbar: View {
    @Bindable var model: StorageMapModel

    public init(model: StorageMapModel) {
        self.model = model
    }

    public var body: some View {
        VStack(spacing: 0) {
            // Upper row: Controls
            HStack(spacing: 12) {
                // Back button
                Button(action: {
                    Task { await model.goBack() }
                }) {
                    Image(systemName: "chevron.left")
                }
                .disabled(model.navigationStack.count <= 1)
                .help("Navigate up one level (Escape)")

                // Metric Toggle
                Picker("Metric", selection: Binding(
                    get: { model.metric },
                    set: { newMetric in
                        Task { await model.switchMetric(to: newMetric) }
                    }
                )) {
                    ForEach(StorageTreeMetricType.allCases) { m in
                        Text(m.rawValue).tag(m)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 170)
                .help(model.metric.tooltip)

                // Snapshot Compare Selector
                if !model.availableSnapshots.isEmpty {
                    Menu {
                        Button("None (Current Size)") {
                            Task { await model.selectSnapshotForComparison(nil) }
                        }
                        Divider()
                        ForEach(model.availableSnapshots) { snap in
                            Button(snap.name) {
                                Task { await model.selectSnapshotForComparison(snap.id) }
                            }
                        }
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: "camera.metering.matrix")
                            Text(currentSnapshotName)
                                .lineLimit(1)
                        }
                    }
                    .menuStyle(.borderedButton)
                    .help("Compare storage growth against a historical snapshot")
                }

                // Color Mode Picker
                Picker("Color Mode", selection: $model.colorMode) {
                    ForEach(TreemapColorMode.allCases) { mode in
                        Text(mode.rawValue).tag(mode)
                    }
                }
                .pickerStyle(.menu)
                .frame(width: 150)

                Spacer()

                // View Mode: Treemap vs List Fallback
                Picker("Presentation", selection: $model.presentationMode) {
                    Label("Treemap", systemImage: "square.split.2x2").tag(TreemapPresentationMode.treemap)
                    Label("List View", systemImage: "list.bullet").tag(TreemapPresentationMode.list)
                }
                .pickerStyle(.segmented)
                .frame(width: 140)
                .help("Switch between visual Squarified Treemap and accessible List View")

                // Refresh Button
                Button(action: {
                    Task { await model.analyzeStorageMap(forceRefresh: true) }
                }) {
                    Image(systemName: "arrow.clockwise")
                }
                .help("Refresh Storage Map analysis (Command+R)")
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color(nsColor: .controlBackgroundColor))

            Divider()

            // Lower row: Breadcrumbs
            HStack(spacing: 6) {
                ForEach(Array(model.navigationStack.enumerated()), id: \.offset) { index, node in
                    if index > 0 {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 9))
                            .foregroundStyle(.tertiary)
                    }

                    Button(action: {
                        Task { await model.navigateToBreadcrumb(at: index) }
                    }) {
                        Text(node.display_name.isEmpty ? "Root" : node.display_name)
                            .font(.subheadline)
                            .fontWeight(index == model.navigationStack.count - 1 ? .semibold : .regular)
                            .foregroundStyle(index == model.navigationStack.count - 1 ? .primary : .secondary)
                    }
                    .buttonStyle(.plain)
                }

                Spacer()

                // Coverage summary indicators
                if let cov = model.currentAnalysis?.coverage {
                    Text("\(cov.files_observed) files · \(cov.directories_observed) folders")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(Color(nsColor: .windowBackgroundColor).opacity(0.6))

            Divider()
        }
    }

    private var currentSnapshotName: String {
        if let snapId = model.selectedSnapshotId,
           let snap = model.availableSnapshots.first(where: { $0.id == snapId }) {
            return "Diff: \(snap.name)"
        }
        return "Compare Snapshot"
    }
}
