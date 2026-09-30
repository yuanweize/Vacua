import SwiftUI
import Charts
import VacuaClient

public struct SnapshotsView: View {
    @Bindable var model: AppModel
    @State private var selectedSnapshotId: String?
    @State private var compareMode: SnapshotCompareTarget = .current
    @State private var customTargetSnapshotName: String = ""
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public enum SnapshotCompareTarget: String, CaseIterable, Identifiable {
        case current = "Current Storage"
        case anotherSnapshot = "Another Snapshot"
        
        public var id: String { rawValue }
    }
    
    public var body: some View {
        HSplitView {
            // Left: Snapshots List
            VStack(spacing: 0) {
                HStack {
                    Text("\(model.snapshots.count) Storage Snapshots\(model.hasMoreSnapshots ? " (more available)" : "")")
                        .font(.subheadline.weight(.medium))
                    Spacer()
                    if model.snapshotsState.isLoading {
                        ProgressView()
                            .controlSize(.small)
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                
                Divider()
                
                if model.snapshotsState.isLoading && model.snapshots.isEmpty {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text("Reading storage snapshots…")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if model.snapshots.isEmpty {
                    VacuaEmptyState(
                        symbol: VacuaSymbols.snapshots,
                        title: "No Storage Snapshots",
                        message: "Create a point-in-time storage snapshot to compare filesystem allocation growth and shrink over time.",
                        actionTitle: "Refresh Snapshots",
                        action: {
                            Task { await model.loadSnapshots(force: true) }
                        }
                    )
                } else {
                    VStack(spacing: 0) {
                        List(model.snapshots, selection: $selectedSnapshotId) { snapshot in
                            HStack {
                                Image(systemName: VacuaSymbols.snapshots)
                                    .foregroundStyle(.purple)
                                
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(snapshot.name)
                                        .font(.headline)
                                    Text(VacuaDateFormatter.formatDisplay(snapshot.created_at))
                                        .font(.caption)
                                        .foregroundStyle(.secondary)
                                }
                                
                                Spacer()
                                
                                Text(snapshot.allocated_bytes.formatted(.byteCount(style: .file)))
                                    .font(.subheadline.weight(.semibold))
                            }
                            .padding(.vertical, 4)
                            .tag(snapshot.id)
                        }
                        .listStyle(.inset(alternatesRowBackgrounds: true))
                        .onChange(of: selectedSnapshotId) { _, newId in
                            if let snap = model.snapshots.first(where: { $0.id == newId }) {
                                triggerDiff(for: snap)
                            }
                        }
                        .onChange(of: model.snapshots) { _, newSnaps in
                            if let selId = selectedSnapshotId, !newSnaps.contains(where: { $0.id == selId }) {
                                selectedSnapshotId = nil
                                model.selectedSnapshot = nil
                            }
                        }
                        
                        // Pagination Footer
                        if model.hasMoreSnapshots {
                            Divider()
                            HStack {
                                Spacer()
                                Button {
                                    Task { await model.loadMoreSnapshots() }
                                } label: {
                                    if model.isLoadingNextSnapshotsPage {
                                        ProgressView()
                                            .controlSize(.small)
                                    } else {
                                        Text("Load More Snapshots…")
                                            .font(.caption)
                                    }
                                }
                                .buttonStyle(.link)
                                .disabled(model.isLoadingNextSnapshotsPage)
                                Spacer()
                            }
                            .padding(.vertical, 8)
                            .background(Color(NSColor.controlBackgroundColor))
                        }
                    }
                }
            }
            .frame(minWidth: 320)
            
            // Right: Snapshot Details & Real Diff Inspector
            if let snap = currentSelectedSnapshot {
                snapshotComparisonView(snap: snap)
                    .frame(minWidth: 420)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select a storage snapshot to view details and compare storage deltas")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("Storage Snapshots")
        .task {
            // Lazy load when entering snapshots view
            await model.loadSnapshotsIfNeeded()
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.loadSnapshots(force: true) }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.snapshotsState.isLoading)
            }
        }
    }
    
    private var currentSelectedSnapshot: SnapshotSummaryV1? {
        model.snapshots.first(where: { $0.id == selectedSnapshotId })
    }
    
    private func triggerDiff(for snap: SnapshotSummaryV1) {
        let target: String?
        switch compareMode {
        case .current:
            target = nil
        case .anotherSnapshot:
            target = customTargetSnapshotName.isEmpty ? nil : customTargetSnapshotName
        }
        Task {
            await model.selectSnapshot(snap, compareWithTarget: target)
        }
    }
    
    // MARK: - Snapshot Comparison View
    
    @ViewBuilder
    private func snapshotComparisonView(snap: SnapshotSummaryV1) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Header
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text(snap.name)
                            .font(.title2.weight(.bold))
                        Spacer()
                        Text(snap.allocated_bytes.formatted(.byteCount(style: .file)))
                            .font(.headline)
                            .foregroundStyle(.purple)
                    }
                    Text("Observed: \(VacuaDateFormatter.formatDisplay(snap.created_at))")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text("Root: \(snap.root_path)")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                
                Divider()
                
                // Compare Selector
                VStack(alignment: .leading, spacing: 8) {
                    Text("Snapshot Comparison")
                        .font(.subheadline.weight(.semibold))
                    
                    Picker("Compare With", selection: $compareMode) {
                        ForEach(SnapshotCompareTarget.allCases) { target in
                            Text(target.rawValue).tag(target)
                        }
                    }
                    .pickerStyle(.segmented)
                    .onChange(of: compareMode) { _, _ in
                        triggerDiff(for: snap)
                    }
                    
                    if compareMode == .anotherSnapshot {
                        let otherSnapshots = model.snapshots.filter { $0.id != snap.id }
                        if otherSnapshots.isEmpty {
                            Text("No other snapshots available to compare against.")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        } else {
                            Picker("Target Snapshot", selection: $customTargetSnapshotName) {
                                ForEach(otherSnapshots) { s in
                                    Text(s.name).tag(s.name)
                                }
                            }
                            .onChange(of: customTargetSnapshotName) { _, _ in
                                triggerDiff(for: snap)
                            }
                        }
                    }
                }
                .padding(12)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                
                // Diff Results
                if model.snapshotDiffState.isLoading {
                    VStack(spacing: 8) {
                        ProgressView()
                        Text("Computing storage allocation delta…")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(20)
                } else if let diff = model.selectedSnapshotDiff {
                    diffResultsView(diff: diff)
                } else if let err = model.snapshotDiffState.errorMessage {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Differential Calculation Failed")
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.red)
                        Text(err)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    .padding(12)
                    .background(Color.red.opacity(0.08))
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                
                // About Reality Card
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: "info.circle")
                            .foregroundStyle(.blue)
                        Text("About Vacua Storage Snapshots")
                            .font(.caption.weight(.semibold))
                    }
                    Text("Vacua storage snapshots save point-in-time filesystem allocation metadata without altering or retaining physical disk blocks. Differential calculations compute exact recursive subtree deltas via the Rust engine.")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .padding(16)
        }
        .background(Color(NSColor.controlBackgroundColor))
    }
    
    // MARK: - Diff Results Breakdown
    
    @ViewBuilder
    private func diffResultsView(diff: SnapshotDiffV1) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            // Delta Metrics Cards
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Allocated Delta")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Text(formatSignedBytes(diff.allocated_delta_bytes))
                        .font(.title3.weight(.bold))
                        .foregroundStyle(diff.allocated_delta_bytes > 0 ? .red : (diff.allocated_delta_bytes < 0 ? .green : .primary))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6))
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Logical Delta")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Text(formatSignedBytes(diff.logical_delta_bytes))
                        .font(.title3.weight(.bold))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6))
                
                VStack(alignment: .leading, spacing: 2) {
                    Text("Files Delta")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                    Text(formatSignedCount(diff.files_delta))
                        .font(.title3.weight(.bold))
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(10)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
            
            // Swift Charts: Delta Distribution
            let chartItems = buildChartItems(diff: diff)
            if !chartItems.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Storage Delta by Subtree")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    
                    Chart(chartItems) { item in
                        BarMark(
                            x: .value("Delta Bytes", item.deltaBytes),
                            y: .value("Location", item.label)
                        )
                        .foregroundStyle(item.isGrowth ? Color.red : Color.green)
                    }
                    .frame(height: CGFloat(chartItems.count * 28 + 30))
                }
                .padding(12)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            
            // Top Growing Subtrees
            if !diff.top_growing.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "arrow.up.right")
                            .foregroundStyle(.red)
                        Text("Top Growing Locations (\(diff.top_growing.count))")
                            .font(.caption.weight(.bold))
                    }
                    
                    ForEach(diff.top_growing) { item in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text((item.display_path as NSString).lastPathComponent)
                                    .font(.caption.weight(.semibold))
                                Text(item.display_path)
                                    .font(.system(.caption2, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 2) {
                                Text("+\(abs(item.delta_bytes).formatted(.byteCount(style: .file)))")
                                    .font(.caption.weight(.bold))
                                    .foregroundStyle(.red)
                                Text("+\(item.files_delta) files")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(.vertical, 3)
                        Divider()
                    }
                }
            }
            
            // Top Shrinking Subtrees
            if !diff.top_shrinking.isEmpty {
                VStack(alignment: .leading, spacing: 8) {
                    HStack {
                        Image(systemName: "arrow.down.right")
                            .foregroundStyle(.green)
                        Text("Top Shrinking Locations (\(diff.top_shrinking.count))")
                            .font(.caption.weight(.bold))
                    }
                    
                    ForEach(diff.top_shrinking) { item in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text((item.display_path as NSString).lastPathComponent)
                                    .font(.caption.weight(.semibold))
                                Text(item.display_path)
                                    .font(.system(.caption2, design: .monospaced))
                                    .foregroundStyle(.secondary)
                                    .lineLimit(1)
                            }
                            Spacer()
                            VStack(alignment: .trailing, spacing: 2) {
                                Text("-\(abs(item.delta_bytes).formatted(.byteCount(style: .file)))")
                                    .font(.caption.weight(.bold))
                                    .foregroundStyle(.green)
                                Text("\(item.files_delta) files")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                        .padding(.vertical, 3)
                        Divider()
                    }
                }
            }
        }
    }
    
    // MARK: - Helpers
    
    private struct ChartDeltaItem: Identifiable {
        let id = UUID()
        let label: String
        let deltaBytes: Int64
        let isGrowth: Bool
    }
    
    private func buildChartItems(diff: SnapshotDiffV1) -> [ChartDeltaItem] {
        var items: [ChartDeltaItem] = []
        for g in diff.top_growing.prefix(4) {
            items.append(ChartDeltaItem(
                label: (g.display_path as NSString).lastPathComponent,
                deltaBytes: g.delta_bytes,
                isGrowth: true
            ))
        }
        for s in diff.top_shrinking.prefix(4) {
            items.append(ChartDeltaItem(
                label: (s.display_path as NSString).lastPathComponent,
                deltaBytes: s.delta_bytes,
                isGrowth: false
            ))
        }
        return items
    }
    
    private func formatSignedBytes(_ bytes: Int64) -> String {
        let absFormatted = abs(bytes).formatted(.byteCount(style: .file))
        if bytes > 0 {
            return "+\(absFormatted)"
        } else if bytes < 0 {
            return "-\(absFormatted)"
        } else {
            return "0 B"
        }
    }
    
    private func formatSignedCount(_ count: Int64) -> String {
        if count > 0 {
            return "+\(count)"
        } else {
            return "\(count)"
        }
    }
}
