import SwiftUI
import Charts
import VacuaClient

public struct SnapshotsView: View {
    @Bindable var model: AppModel
    @State private var selectedSnapshotId: String?
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        HSplitView {
            VStack(spacing: 0) {
                HStack {
                    Text("\(model.snapshots.count) APFS Snapshots")
                        .font(.subheadline.weight(.medium))
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                
                Divider()
                
                if model.snapshots.isEmpty && !model.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "camera.metering.matrix")
                            .font(.largeTitle)
                            .foregroundStyle(.tertiary)
                        Text("No APFS Snapshots")
                            .font(.headline)
                        Text("No local APFS snapshots found on this volume.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(model.snapshots, selection: $selectedSnapshotId) { snapshot in
                        HStack {
                            Image(systemName: "camera.fill")
                                .foregroundStyle(.blue)
                            
                            VStack(alignment: .leading, spacing: 2) {
                                Text(snapshot.name)
                                    .font(.headline)
                                Text(snapshot.created_at)
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
                }
            }
            .frame(minWidth: 320)
            
            // Snapshot Inspector
            if let snap = currentSelectedSnapshot {
                snapshotDetail(snap: snap)
                    .frame(minWidth: 340)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select a snapshot to inspect details and delta calculations")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("APFS Snapshots")
    }
    
    private var currentSelectedSnapshot: SnapshotSummaryV1? {
        model.snapshots.first(where: { $0.id == selectedSnapshotId })
    }
    
    @ViewBuilder
    private func snapshotDetail(snap: SnapshotSummaryV1) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(snap.name)
                    .font(.headline)
                Text("Created: \(snap.created_at)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text("Root: \(snap.root_path)")
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(.secondary)
            }
            
            Divider()
            
            VStack(alignment: .leading, spacing: 8) {
                Text("Snapshot Allocation")
                    .font(.subheadline.weight(.medium))
                
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Allocated Physical Bytes")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text(snap.allocated_bytes.formatted(.byteCount(style: .file)))
                            .font(.title2.weight(.bold))
                            .foregroundStyle(.purple)
                    }
                    Spacer()
                    VStack(alignment: .trailing, spacing: 2) {
                        Text("Total Files")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text("\(snap.total_files)")
                            .font(.title2.weight(.bold))
                    }
                }
                .padding(12)
                .background(Color.purple.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: "info.circle")
                        .foregroundStyle(.blue)
                    Text("APFS Snapshot Retention")
                        .font(.caption.weight(.semibold))
                }
                Text("APFS snapshots freeze filesystem state. Any files deleted or modified since this snapshot was created will retain their physical disk blocks until the snapshot is deleted by macOS.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .padding(10)
            .background(Color.secondary.opacity(0.06))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            
            Spacer()
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
    }
}
