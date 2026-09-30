import SwiftUI

/// Semantic design tokens and appearance helpers adhering to macOS Human Interface Guidelines.
public enum VacuaTheme {
    // MARK: - Brand & Technical Colors
    public static let brand = Color(red: 0.0, green: 0.65, blue: 0.85) // Technical cyan
    public static let slateBase = Color(red: 0.12, green: 0.14, blue: 0.18)

    // MARK: - Restrained Palette for Hierarchy Grouping
    // Note: Node colors are for hierarchical branch grouping ONLY.
    // They NEVER imply risk, reclaimability, or deletion safety.
    private static let groupingPalette: [Color] = [
        Color(red: 0.20, green: 0.45, blue: 0.70), // Slate Blue
        Color(red: 0.25, green: 0.55, blue: 0.65), // Muted Teal
        Color(red: 0.40, green: 0.45, blue: 0.65), // Indigo Slate
        Color(red: 0.35, green: 0.55, blue: 0.50), // Sage Blue
        Color(red: 0.50, green: 0.40, blue: 0.60), // Dusk Violet
        Color(red: 0.30, green: 0.50, blue: 0.75), // Ocean
        Color(red: 0.45, green: 0.50, blue: 0.55)  // Steel
    ]

    /// Deterministically derive a stable presentation color for a node branch.
    /// Preserves visual continuity during metric switches, hover, and drill-down.
    public static func stableGroupingColor(for identifier: String) -> Color {
        var hash: UInt32 = 5381
        for byte in identifier.utf8 {
            hash = ((hash << 5) &+ hash) &+ UInt32(byte)
        }
        let index = Int(hash % UInt32(groupingPalette.count))
        return groupingPalette[index]
    }

    // MARK: - Snapshot Delta Semantic Colors
    public static func deltaColor(for changeKind: String) -> Color {
        switch changeKind.lowercased() {
        case "grown":
            return Color.red
        case "shrunk":
            return Color.green
        case "new":
            return Color.purple
        case "unchanged":
            return Color.gray
        default:
            return Color.secondary
        }
    }

    public static func deltaSymbol(for changeKind: String) -> String {
        switch changeKind.lowercased() {
        case "grown":
            return VacuaSymbols.deltaGrown
        case "shrunk":
            return VacuaSymbols.deltaShrunk
        case "new":
            return VacuaSymbols.deltaNew
        case "unchanged":
            return VacuaSymbols.deltaUnchanged
        default:
            return VacuaSymbols.info
        }
    }

    public static func deltaLabel(for changeKind: String) -> String {
        switch changeKind.lowercased() {
        case "grown":
            return "Grown"
        case "shrunk":
            return "Shrunk"
        case "new":
            return "New"
        case "unchanged":
            return "Unchanged"
        default:
            return changeKind.capitalized
        }
    }
}
