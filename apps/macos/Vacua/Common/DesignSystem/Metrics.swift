import SwiftUI

/// Standard layout metrics, spacing rules, and dimensional boundaries.
/// Eliminates magic numbers while adhering strictly to macOS native layout rhythms.
public enum VacuaMetrics {
    // MARK: - Spacing & Margins
    public static let pagePadding: CGFloat = 20
    public static let sectionSpacing: CGFloat = 16
    public static let itemSpacing: CGFloat = 12
    public static let tightSpacing: CGFloat = 6
    public static let microSpacing: CGFloat = 4

    // MARK: - Corner Radii
    public static let cornerRadiusSmall: CGFloat = 4
    public static let cornerRadiusMedium: CGFloat = 8
    public static let cornerRadiusLarge: CGFloat = 12

    // MARK: - Inspector Dimensions
    public static let inspectorMinWidth: CGFloat = 240
    public static let inspectorIdealWidth: CGFloat = 280
    public static let inspectorMaxWidth: CGFloat = 360

    // MARK: - Treemap Constraints
    public static let treemapCellMinLabelWidth: CGFloat = 40
    public static let treemapCellMinLabelHeight: CGFloat = 22
    public static let treemapCellMinDetailWidth: CGFloat = 72
    public static let treemapCellMinDetailHeight: CGFloat = 38
}
