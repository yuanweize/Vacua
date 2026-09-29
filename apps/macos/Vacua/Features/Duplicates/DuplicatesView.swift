import SwiftUI
import VacuaClient

public struct DuplicatesView: View {
    @Bindable var model: AppModel
    @State private var selectedGroupId: String?
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        HSplitView {
            // Duplicate Groups List
            VStack(spacing: 0) {
                HStack {
                    Text("\(model.duplicateGroups.count) Duplicate Groups")
                        .font(.subheadline.weight(.medium))
                    Spacer()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                
                Divider()
                
                if model.duplicateGroups.isEmpty && !model.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "doc.on.doc")
                            .font(.largeTitle)
                            .foregroundStyle(.tertiary)
                        Text("No Duplicates Detected")
                            .font(.headline)
                        Text("No identical file sets found within the active root.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(model.duplicateGroups, selection: $selectedGroupId) { group in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack {
                                Text("\(group.member_count) identical copies")
                                    .font(.headline)
                                Spacer()
                                Text(group.logical_duplicate_bytes.formatted(.byteCount(style: .file)))
                                    .font(.subheadline.weight(.semibold))
                                    .foregroundStyle(.blue)
                            }
                            
                            HStack(spacing: 12) {
                                // APFS Kernel Physical Truth Display
                                if group.kernel_private_bytes_known_members > 0 {
                                    HStack(spacing: 4) {
                                        Text("APFS Private:")
                                            .foregroundStyle(.secondary)
                                        Text(group.kernel_private_bytes.formatted(.byteCount(style: .file)))
                                            .foregroundStyle(.green)
                                    }
                                } else {
                                    HStack(spacing: 4) {
                                        Text("APFS Private:")
                                            .foregroundStyle(.secondary)
                                        Text("Unknown")
                                            .foregroundStyle(.orange)
                                    }
                                    .help("APFS physical private allocation unmeasured without snapshot diff")
                                }
                                
                                Spacer()
                                
                                Text("Lower Bound: \(group.confirmed_reclaim_lower_bound.formatted(.byteCount(style: .file)))")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                            .font(.caption)
                        }
                        .padding(.vertical, 4)
                        .tag(group.id)
                    }
                    .listStyle(.inset(alternatesRowBackgrounds: true))
                    .onChange(of: selectedGroupId) { _, newId in
                        if let grp = model.duplicateGroups.first(where: { $0.id == newId }) {
                            Task { await model.selectDuplicateGroup(grp) }
                        }
                    }
                }
            }
            .frame(minWidth: 320)
            
            // Detail / Member File Inspector
            if let group = currentSelectedGroup {
                groupInspector(group: group, detail: model.selectedDuplicateDetail)
                    .frame(minWidth: 320)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select a duplicate group to inspect individual copy locations")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("Duplicate Files")
    }
    
    private var currentSelectedGroup: DuplicateGroupSummaryV1? {
        model.duplicateGroups.first(where: { $0.id == selectedGroupId })
    }
    
    @ViewBuilder
    private func groupInspector(group: DuplicateGroupSummaryV1, detail: DuplicateGroupDetailV1?) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Group Header
            VStack(alignment: .leading, spacing: 4) {
                Text("Duplicate Group Details")
                    .font(.headline)
                Text("File size: \(group.file_size.formatted(.byteCount(style: .file))) · \(group.member_count) copies")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            
            // Storage Truth Reality Callout
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: "info.circle.fill")
                        .foregroundStyle(.blue)
                    Text("Physical vs Nominal Storage Truth")
                        .font(.subheadline.weight(.semibold))
                }
                Text("APFS supports block-level clone copies (copy-on-write). Multiple identical file paths may share the exact same physical storage blocks on disk.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                
                HStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Logical Duplicate")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text(group.logical_duplicate_bytes.formatted(.byteCount(style: .file)))
                            .font(.subheadline.weight(.medium))
                    }
                    VStack(alignment: .leading, spacing: 2) {
                        Text("APFS Private Blocks")
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                        Text(group.kernel_private_bytes_known_members > 0 ? group.kernel_private_bytes.formatted(.byteCount(style: .file)) : "Unknown")
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(group.kernel_private_bytes_known_members > 0 ? .green : .orange)
                    }
                }
                .padding(.top, 4)
            }
            .padding(12)
            .background(Color.blue.opacity(0.06))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            
            // Duplicate Copies List
            if let detail = detail {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Files in this Set (\(detail.members.count))")
                        .font(.subheadline.weight(.medium))
                    
                    List(detail.members) { member in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(member.physical_relation.uppercased())
                                    .font(.caption2.weight(.bold))
                                    .foregroundStyle(.secondary)
                                Spacer()
                                if member.kernel_private_bytes_known {
                                    Text("Private: \(member.kernel_private_bytes.formatted(.byteCount(style: .file)))")
                                        .font(.caption2)
                                        .foregroundStyle(.green)
                                } else {
                                    Text("Private: Unknown")
                                        .font(.caption2)
                                        .foregroundStyle(.orange)
                                }
                            }
                            Text(member.display_path)
                                .font(.system(.caption, design: .monospaced))
                                .textSelection(.enabled)
                        }
                        .padding(.vertical, 2)
                    }
                    .listStyle(.bordered(alternatesRowBackgrounds: true))
                }
            }
            
            Spacer()
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
    }
}
