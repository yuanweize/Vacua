import SwiftUI
import VacuaClient

public struct TreemapView: View {
    @Bindable var model: StorageMapModel
    @State private var hoveredNodeId: String?

    public init(model: StorageMapModel) {
        self.model = model
    }

    public var body: some View {
        GeometryReader { geometry in
            let bounds = CGRect(origin: .zero, size: geometry.size)
            let items = computeItems()
            let rects = SquarifiedTreemapLayout.layout(items: items, bounds: bounds)

            ZStack(alignment: .topLeading) {
                // Background
                Color(nsColor: .controlBackgroundColor)
                    .edgesIgnoringSafeArea(.all)

                if rects.isEmpty {
                    VStack(spacing: 8) {
                        Image(systemName: "square.split.2x2")
                            .font(.largeTitle)
                            .foregroundStyle(.secondary)
                        Text("No items to display in this folder")
                            .font(.headline)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    ForEach(rects) { r in
                        let node = findNode(by: r.id)
                        let isOther = (r.id == "other_remainder")
                        let isSelected = (model.selectedNodeId == r.id)
                        let isHovered = (hoveredNodeId == r.id)

                        TreemapCellView(
                            rect: r.rect,
                            node: node,
                            isOther: isOther,
                            otherRemainder: isOther ? model.currentPage?.remainder : nil,
                            metric: model.metric,
                            colorMode: model.colorMode,
                            isSelected: isSelected,
                            isHovered: isHovered,
                            delta: model.delta(for: r.id)
                        )
                        .onHover { hovering in
                            hoveredNodeId = hovering ? r.id : nil
                        }
                        .onTapGesture {
                            if !isOther {
                                Task { await model.selectNode(id: r.id) }
                            }
                        }
                        .simultaneousGesture(
                            TapGesture(count: 2).onEnded {
                                if let node, node.isDirectory {
                                    Task { await model.drillDown(to: node) }
                                }
                            }
                        )
                    }
                }
            }
        }
        .focusable()
        .onKeyPress(.return) {
            if let selected = model.selectedNodeDetail?.node, selected.isDirectory {
                Task { await model.drillDown(to: selected) }
                return .handled
            }
            return .ignored
        }
        .onKeyPress(.escape) {
            Task { await model.goBack() }
            return .handled
        }
    }

    private func computeItems() -> [TreemapItem] {
        guard let page = model.currentPage else { return [] }
        var items: [TreemapItem] = []

        for node in page.items {
            let weight = (model.metric == .allocated)
                ? Double(node.subtree_allocated_bytes)
                : Double(node.subtree_logical_bytes)
            if weight > 0 {
                items.append(TreemapItem(id: node.node_id, weight: weight, kind: node.kind))
            }
        }

        // Remainder ("Other") representation
        let remainderWeight = (model.metric == .allocated)
            ? Double(page.remainder.allocated_bytes)
            : Double(page.remainder.logical_bytes)

        if remainderWeight > 0 {
            items.append(TreemapItem(id: "other_remainder", weight: remainderWeight, kind: "other"))
        }

        return items
    }

    private func findNode(by id: String) -> StorageTreeNodeV1? {
        model.currentPage?.items.first(where: { $0.node_id == id })
    }
}

// MARK: - Individual Treemap Rectangle Cell

struct TreemapCellView: View {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityDifferentiateWithoutColor) private var differentiateWithoutColor
    @Environment(\.colorSchemeContrast) private var colorSchemeContrast

    let rect: CGRect
    let node: StorageTreeNodeV1?
    let isOther: Bool
    let otherRemainder: StorageTreeRemainderV1?
    let metric: StorageTreeMetricType
    let colorMode: TreemapColorMode
    let isSelected: Bool
    let isHovered: Bool
    let delta: StorageTreeDeltaV1?

    var body: some View {
        let displayName = isOther ? "Other (\(otherRemainder?.item_count ?? 0) items)" : (node?.display_name ?? "")
        let byteCount: UInt64 = {
            if isOther {
                return (metric == .allocated) ? (otherRemainder?.allocated_bytes ?? 0) : (otherRemainder?.logical_bytes ?? 0)
            }
            guard let node else { return 0 }
            return (metric == .allocated) ? node.subtree_allocated_bytes : node.subtree_logical_bytes
        }()

        let isHighContrast = (colorSchemeContrast == .increased)
        let fillColor = computeFillColor()
        let borderColor = computeBorderColor(isHighContrast: isHighContrast)
        let borderWidth: CGFloat = isSelected ? 2.5 : (isHovered ? 1.5 : (isHighContrast ? 1.0 : 0.5))

        ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusSmall)
                .fill(fillColor)
                .overlay(
                    RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusSmall)
                        .stroke(borderColor, lineWidth: borderWidth)
                )

            // Content label inside rectangle if there is enough space
            if rect.width > VacuaMetrics.treemapCellMinLabelWidth && rect.height > VacuaMetrics.treemapCellMinLabelHeight {
                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 4) {
                        if colorMode == .snapshotDelta, let delta {
                            Image(systemName: VacuaTheme.deltaSymbol(for: delta.change_kind))
                                .font(.system(size: 9, weight: .bold))
                                .foregroundStyle(VacuaTheme.deltaColor(for: delta.change_kind))
                        } else if !isOther {
                            Image(systemName: (node?.isDirectory ?? false) ? VacuaSymbols.folder : VacuaSymbols.file)
                                .font(.system(size: 9))
                                .opacity(0.7)
                        }
                        Text(displayName)
                            .font(.system(size: 11, weight: .medium))
                            .lineLimit(1)
                    }

                    if rect.width > VacuaMetrics.treemapCellMinDetailWidth && rect.height > VacuaMetrics.treemapCellMinDetailHeight {
                        if colorMode == .snapshotDelta, let delta {
                            let sign = delta.allocated_delta_bytes >= 0 ? "+" : ""
                            Text("\(sign)\(delta.allocated_delta_bytes.formatted(.byteCount(style: .file)))")
                                .font(.system(size: 9, weight: .semibold))
                                .foregroundStyle(VacuaTheme.deltaColor(for: delta.change_kind))
                                .lineLimit(1)
                        } else {
                            Text(Formatters.formatBytes(byteCount))
                                .font(.system(size: 9))
                                .foregroundStyle(.secondary)
                                .lineLimit(1)
                        }
                    }
                }
                .padding(4)
                .frame(maxWidth: rect.width - 8, maxHeight: rect.height - 8, alignment: .topLeading)
            }
        }
        .frame(width: max(0, rect.width - 2), height: max(0, rect.height - 2))
        .position(x: rect.midX, y: rect.midY)
        .help(tooltipText(displayName: displayName, byteCount: byteCount))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(accessibilityDescription(displayName: displayName, byteCount: byteCount))
    }

    private func computeFillColor() -> Color {
        if isOther {
            return Color(nsColor: .windowBackgroundColor).opacity(0.8)
        }

        if colorMode == .snapshotDelta {
            if let delta {
                let opacity: Double = (colorScheme == .dark ? 0.35 : 0.22)
                return VacuaTheme.deltaColor(for: delta.change_kind).opacity(opacity)
            } else {
                return Color.gray.opacity(0.18)
            }
        }

        // Type / Hierarchical Grouping mode
        guard let node else { return Color.gray.opacity(0.2) }
        let baseColor = VacuaTheme.stableGroupingColor(for: node.node_id)
        let opacity: Double = node.isDirectory
            ? (colorScheme == .dark ? 0.38 : 0.25)
            : (colorScheme == .dark ? 0.22 : 0.15)
        return baseColor.opacity(opacity)
    }

    private func computeBorderColor(isHighContrast: Bool) -> Color {
        if isSelected {
            return Color.accentColor
        }
        if isHovered {
            return colorScheme == .dark
                ? Color.white.opacity(0.7)
                : Color.black.opacity(0.55)
        }
        if isHighContrast {
            return colorScheme == .dark
                ? Color.white.opacity(0.35)
                : Color.black.opacity(0.4)
        }
        return colorScheme == .dark
            ? Color.white.opacity(0.12)
            : Color.black.opacity(0.12)
    }

    private func tooltipText(displayName: String, byteCount: UInt64) -> String {
        var base = "\(displayName)\n\(Formatters.formatBytes(byteCount)) (\(metric.rawValue))"
        if colorMode == .snapshotDelta, let delta {
            let sign = delta.allocated_delta_bytes >= 0 ? "+" : ""
            base += "\nChange: \(sign)\(delta.allocated_delta_bytes.formatted(.byteCount(style: .file))) (\(VacuaTheme.deltaLabel(for: delta.change_kind)))"
        }
        return base
    }

    private func accessibilityDescription(displayName: String, byteCount: UInt64) -> String {
        let kind = isOther ? "Remainder aggregate" : (node?.isDirectory == true ? "Folder" : "File")
        var desc = "\(displayName), \(kind), \(Formatters.formatBytes(byteCount))"
        if colorMode == .snapshotDelta, let delta {
            desc += ", Snapshot change: \(VacuaTheme.deltaLabel(for: delta.change_kind)), \(delta.allocated_delta_bytes) bytes"
        }
        return desc
    }
}
