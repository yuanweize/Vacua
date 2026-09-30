import SwiftUI

/// Centralized, audited SF Symbols verified for macOS 15+ deployment target.
/// Prevents symbol fragmentation, typo bugs, or semantic drift across the application.
public enum VacuaSymbols {
    // MARK: - Navigation & Feature Destinations
    public static let overview = "gauge.open.with.lines.needle.33percent"
    public static let storageMap = "rectangle.3.group"
    public static let candidates = "list.bullet.clipboard"
    public static let duplicates = "doc.on.doc"
    public static let applications = "app.badge"
    public static let snapshots = "clock.arrow.circlepath"
    public static let settings = "gearshape"
    public static let diagnostics = "waveform.path.ecg"

    // MARK: - Actions & Controls
    public static let refresh = "arrow.clockwise"
    public static let drillDown = "arrow.turn.down.right"
    public static let back = "chevron.left"
    public static let forward = "chevron.right"
    public static let folder = "folder.fill"
    public static let file = "doc.fill"
    public static let search = "magnifyingglass"
    public static let filter = "line.3.horizontal.decrease.circle"
    public static let info = "info.circle"
    public static let activeRoot = "externaldrive.fill"

    // MARK: - Safety & Risk Badges
    public static let safetyShield = "shield.lefthalf.filled"
    public static let riskSafe = "checkmark.circle.fill"
    public static let riskReview = "eye.fill"
    public static let riskCaution = "exclamationmark.triangle.fill"
    public static let riskProtected = "lock.shield.fill"
    public static let riskUnknown = "questionmark.circle.fill"

    // MARK: - Snapshot Delta Indications (Non-color accessible indicators)
    public static let deltaGrown = "arrow.up.right"
    public static let deltaShrunk = "arrow.down.right"
    public static let deltaNew = "sparkle"
    public static let deltaUnchanged = "equal"

    // MARK: - Engine States
    public static let engineReady = "checkmark.circle.fill"
    public static let engineStarting = "progress.indicator"
    public static let engineUnavailable = "exclamationmark.circle.fill"
}
