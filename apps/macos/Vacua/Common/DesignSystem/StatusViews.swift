import SwiftUI

// MARK: - Safety Guarantee Banner

public struct SafetyGuaranteeBanner: View {
    @Environment(\.colorScheme) private var colorScheme

    public init() {}

    public var body: some View {
        HStack(spacing: 10) {
            Image(systemName: VacuaSymbols.safetyShield)
                .foregroundStyle(.green)
                .font(.title3)

            VStack(alignment: .leading, spacing: 2) {
                Text("Analysis & proposal only")
                    .font(.subheadline.weight(.semibold))

                Text("This app does not modify or delete files.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(10)
        .background(Color.green.opacity(colorScheme == .dark ? 0.12 : 0.06))
        .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusMedium))
        .overlay(
            RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusMedium)
                .stroke(Color.green.opacity(0.25), lineWidth: 1)
        )
    }
}

// MARK: - Engine Status Badge

public struct EngineStatusBadge: View {
    public let isReady: Bool
    public let isStarting: Bool

    public init(isReady: Bool, isStarting: Bool = false) {
        self.isReady = isReady
        self.isStarting = isStarting
    }

    public var body: some View {
        HStack(spacing: 5) {
            if isStarting {
                ProgressView()
                    .controlSize(.mini)
                Text("Starting")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            } else if isReady {
                Circle()
                    .fill(Color.green)
                    .frame(width: 7, height: 7)
                Text("Engine Ready")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            } else {
                Circle()
                    .fill(Color.red)
                    .frame(width: 7, height: 7)
                Text("Engine Unavailable")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(Color.secondary.opacity(0.08))
        .clipShape(Capsule())
    }
}
