import Foundation
import CoreGraphics

/// Input item for the Squarified Treemap layout algorithm.
public struct TreemapItem: Identifiable, Sendable {
    public let id: String
    public let weight: Double
    public let kind: String

    public init(id: String, weight: Double, kind: String = "item") {
        self.id = id
        self.weight = weight
        self.kind = kind
    }
}

/// Output positioned rectangle computed by the layout algorithm.
public struct TreemapRect: Identifiable, Sendable, Equatable {
    public let id: String
    public let rect: CGRect
    public let kind: String

    public init(id: String, rect: CGRect, kind: String = "item") {
        self.id = id
        self.rect = rect
        self.kind = kind
    }
}

/// Deterministic, pure Swift implementation of the Squarified Treemap layout algorithm
/// based on the seminal paper by Bruls, Huizing, and van Wijk (2000).
/// Swift computes ONLY display geometry; storage metrics are computed exclusively by Rust.
public struct SquarifiedTreemapLayout: Sendable {

    /// Computes layout for a list of weighted items inside a bounding rectangle.
    /// Invariants:
    /// - All output rectangles fit strictly inside `bounds`.
    /// - Rectangles do not overlap beyond floating point epsilon.
    /// - Total area equals bounds area (proportional to total weight).
    /// - Items with non-positive weight are omitted.
    /// - Deterministic for identical inputs.
    public static func layout(
        items: [TreemapItem],
        bounds: CGRect
    ) -> [TreemapRect] {
        guard bounds.width > 0, bounds.height > 0 else {
            return []
        }

        // Filter and sort items descending by weight for squarified packing
        let validItems = items
            .filter { $0.weight > 0 && $0.weight.isFinite }
            .sorted { (a, b) -> Bool in
                if a.weight != b.weight {
                    return a.weight > b.weight
                }
                return a.id < b.id // Deterministic tie-breaker
            }

        guard !validItems.isEmpty else {
            return []
        }

        let totalWeight = validItems.reduce(0.0) { $0 + $1.weight }
        guard totalWeight > 0, totalWeight.isFinite else {
            return []
        }

        // Normalize weights such that sum of normalized areas == container area
        let containerArea = Double(bounds.width * bounds.height)
        let normalized = validItems.map { item in
            (item: item, area: (item.weight / totalWeight) * containerArea)
        }

        var results: [TreemapRect] = []
        var remainingBounds = bounds
        var currentChildren: [(item: TreemapItem, area: Double)] = []

        var queue = normalized

        while !queue.isEmpty {
            let next = queue[0]
            let candidateChildren = currentChildren + [next]

            let currentWorst = worstAspectRatio(row: currentChildren, bounds: remainingBounds)
            let candidateWorst = worstAspectRatio(row: candidateChildren, bounds: remainingBounds)

            if currentChildren.isEmpty || candidateWorst <= currentWorst {
                // Aspect ratio improves or stays acceptable: append to current row
                currentChildren.append(next)
                queue.removeFirst()
            } else {
                // Aspect ratio would worsen: lay out current row and begin new one
                let (rects, newBounds) = layoutRow(row: currentChildren, bounds: remainingBounds)
                results.append(contentsOf: rects)
                remainingBounds = newBounds
                currentChildren = []
            }
        }

        // Layout any remaining row
        if !currentChildren.isEmpty {
            let (rects, _) = layoutRow(row: currentChildren, bounds: remainingBounds)
            results.append(contentsOf: rects)
        }

        return results
    }

    // MARK: - Internal Squarified Math

    /// Calculates worst aspect ratio for a row in the current container slice.
    private static func worstAspectRatio(
        row: [(item: TreemapItem, area: Double)],
        bounds: CGRect
    ) -> Double {
        guard !row.isEmpty else { return Double.infinity }

        let side = Double(min(bounds.width, bounds.height))
        guard side > 0 else { return Double.infinity }

        let rowArea = row.reduce(0.0) { $0 + $1.area }
        guard rowArea > 0 else { return Double.infinity }

        var maxAspect = 0.0
        let s2 = side * side

        for elem in row {
            let a = elem.area
            guard a > 0 else { continue }
            // Aspect ratio calculation: max(s^2 * a / rowArea^2, rowArea^2 / (s^2 * a))
            let aspect1 = (s2 * a) / (rowArea * rowArea)
            let aspect2 = (rowArea * rowArea) / (s2 * a)
            let aspect = max(aspect1, aspect2)
            if aspect > maxAspect {
                maxAspect = aspect
            }
        }

        return maxAspect
    }

    /// Lays out a row along the shortest side of `bounds` and returns the remaining container.
    private static func layoutRow(
        row: [(item: TreemapItem, area: Double)],
        bounds: CGRect
    ) -> ([TreemapRect], CGRect) {
        guard !row.isEmpty else { return ([], bounds) }

        let rowArea = row.reduce(0.0) { $0 + $1.area }
        guard rowArea > 0 else { return ([], bounds) }

        var rects: [TreemapRect] = []
        let width = Double(bounds.width)
        let height = Double(bounds.height)

        if width >= height {
            // Cut vertical strip of width = rowArea / height
            let rowWidth = rowArea / height
            var currentY = Double(bounds.minY)

            for elem in row {
                let elemHeight = elem.area / rowWidth
                let rect = CGRect(
                    x: Double(bounds.minX),
                    y: currentY,
                    width: rowWidth,
                    height: elemHeight
                )
                rects.append(TreemapRect(id: elem.item.id, rect: rect, kind: elem.item.kind))
                currentY += elemHeight
            }

            let newBounds = CGRect(
                x: Double(bounds.minX) + rowWidth,
                y: Double(bounds.minY),
                width: max(0, width - rowWidth),
                height: height
            )
            return (rects, newBounds)
        } else {
            // Cut horizontal strip of height = rowArea / width
            let rowHeight = rowArea / width
            var currentX = Double(bounds.minX)

            for elem in row {
                let elemWidth = elem.area / rowHeight
                let rect = CGRect(
                    x: currentX,
                    y: Double(bounds.minY),
                    width: elemWidth,
                    height: rowHeight
                )
                rects.append(TreemapRect(id: elem.item.id, rect: rect, kind: elem.item.kind))
                currentX += elemWidth
            }

            let newBounds = CGRect(
                x: Double(bounds.minX),
                y: Double(bounds.minY) + rowHeight,
                width: width,
                height: max(0, height - rowHeight)
            )
            return (rects, newBounds)
        }
    }
}
