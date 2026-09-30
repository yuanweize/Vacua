import SwiftUI
import VacuaClient

public struct StorageNodeInspector: View {
    let detail: StorageTreeNodeDetailV1?
    let isLoading: Bool

    public init(detail: StorageTreeNodeDetailV1?, isLoading: Bool) {
        self.detail = detail
        self.isLoading = isLoading
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if isLoading {
                VStack(spacing: 8) {
                    ProgressView()
                    Text("Loading node details...")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let detail {
                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        // Header
                        HStack(spacing: 8) {
                            Image(systemName: detail.node.isDirectory ? "folder.fill" : "doc.fill")
                                .font(.title2)
                                .foregroundStyle(detail.node.isDirectory ? .blue : .secondary)

                            VStack(alignment: .leading, spacing: 2) {
                                Text(detail.node.display_name)
                                    .font(.title3)
                                    .fontWeight(.semibold)
                                    .lineLimit(2)

                                Text(detail.node.kind.capitalized)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        Divider()

                        // Storage Metrics
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Storage Metrics")
                                .font(.caption)
                                .fontWeight(.bold)
                                .foregroundStyle(.secondary)
                                .textCase(.uppercase)

                            InspectorRow(label: "Allocated Blocks", value: Formatters.formatBytes(detail.node.subtree_allocated_bytes))
                            InspectorRow(label: "Logical Bytes", value: Formatters.formatBytes(detail.node.subtree_logical_bytes))

                            if let pctParent = detail.percentage_of_parent {
                                InspectorRow(label: "Share of Parent", value: String(format: "%.1f%%", pctParent))
                            }
                            InspectorRow(label: "Share of Root", value: String(format: "%.1f%%", detail.percentage_of_root))
                        }

                        Divider()

                        // Item Counts
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Descendant Breakdown")
                                .font(.caption)
                                .fontWeight(.bold)
                                .foregroundStyle(.secondary)
                                .textCase(.uppercase)

                            InspectorRow(label: "Files", value: "\(detail.node.file_count)")
                            InspectorRow(label: "Directories", value: "\(detail.node.directory_count)")
                            if detail.node.isDirectory {
                                InspectorRow(label: "Direct Children", value: "\(detail.node.child_count)")
                            }
                        }

                        // Snapshot Delta if available
                        if let delta = detail.delta {
                            Divider()
                            VStack(alignment: .leading, spacing: 8) {
                                Text("Snapshot Comparison")
                                    .font(.caption)
                                    .fontWeight(.bold)
                                    .foregroundStyle(.secondary)
                                    .textCase(.uppercase)

                                HStack {
                                    Text("Change Status")
                                        .font(.subheadline)
                                        .foregroundStyle(.secondary)
                                    Spacer()
                                    Text(delta.change_kind.capitalized)
                                        .font(.subheadline)
                                        .fontWeight(.semibold)
                                        .foregroundStyle(deltaColor(delta.change_kind))
                                }

                                InspectorRow(
                                    label: "Allocated Delta",
                                    value: Formatters.formatBytesSigned(delta.allocated_delta_bytes)
                                )
                                InspectorRow(
                                    label: "Logical Delta",
                                    value: Formatters.formatBytesSigned(delta.logical_delta_bytes)
                                )
                                InspectorRow(
                                    label: "File Count Delta",
                                    value: delta.file_count_delta > 0 ? "+\(delta.file_count_delta)" : "\(delta.file_count_delta)"
                                )
                            }
                        }

                        // Hardlink Attribution Info
                        if let hardlinkInfo = detail.hardlink_info {
                            Divider()
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Hardlink Attribution")
                                    .font(.caption)
                                    .fontWeight(.bold)
                                    .foregroundStyle(.secondary)
                                    .textCase(.uppercase)

                                Text(hardlinkInfo)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                        }

                        // Allocation Semantics Caveat
                        Divider()
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Allocation Semantics")
                                .font(.caption)
                                .fontWeight(.bold)
                                .foregroundStyle(.secondary)
                                .textCase(.uppercase)

                            Text(detail.allocation_semantics)
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }

                        // Path & Identity
                        Divider()
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Node Identity")
                                .font(.caption)
                                .fontWeight(.bold)
                                .foregroundStyle(.secondary)
                                .textCase(.uppercase)

                            Text("Opaque ID: \(detail.node.node_id)")
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)

                            Text("Path: \(detail.node.display_path)")
                                .font(.system(size: 10, design: .monospaced))
                                .foregroundStyle(.secondary)
                                .textSelection(.enabled)
                        }

                        // Scope notice
                        Divider()
                        Text("The Storage Map represents the selected root namespace, not all space used on the volume.")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                            .padding(.top, 4)
                    }
                    .padding()
                }
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.title2)
                        .foregroundStyle(.tertiary)
                    Text("Select an item to inspect details")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .padding()
            }
        }
        .frame(minWidth: 240, idealWidth: 280, maxWidth: 320)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private func deltaColor(_ kind: String) -> Color {
        switch kind {
        case "grown": return .red
        case "shrunk": return .green
        case "new": return .purple
        default: return .secondary
        }
    }
}

struct InspectorRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline)
                .fontWeight(.medium)
        }
    }
}
