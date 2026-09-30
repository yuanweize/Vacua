import SwiftUI

// MARK: - Risk Badge

public struct RiskBadge: View {
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityDifferentiateWithoutColor) private var differentiateWithoutColor
    
    public let risk: String

    public init(risk: String) {
        self.risk = risk
    }

    public var body: some View {
        let normalized = risk.lowercased()
        let (color, text, symbol): (Color, String, String) = {
            switch normalized {
            case "safe":
                return (.green, "Safe", VacuaSymbols.riskSafe)
            case "caution", "cautious":
                return (.orange, "Caution", VacuaSymbols.riskCaution)
            case "review":
                return (.blue, "Review", VacuaSymbols.riskReview)
            case "protected":
                return (.purple, "Protected", VacuaSymbols.riskProtected)
            case "unknown":
                return (.secondary, "Unknown", VacuaSymbols.riskUnknown)
            default:
                return (.secondary, risk.capitalized, VacuaSymbols.info)
            }
        }()

        HStack(spacing: 3) {
            Image(systemName: symbol)
                .font(.system(size: 9, weight: .semibold))
            Text(text)
                .font(.caption2.weight(.semibold))
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 2.5)
        .background(color.opacity(colorScheme == .dark ? 0.22 : 0.12))
        .foregroundStyle(color)
        .clipShape(Capsule())
        .overlay(
            Capsule()
                .stroke(differentiateWithoutColor ? color : color.opacity(0.3), lineWidth: 1)
        )
    }
}

// MARK: - Category Badge

public struct CategoryBadge: View {
    public let category: String

    public init(category: String) {
        self.category = category
    }

    public var body: some View {
        Text(category.capitalized)
            .font(.caption2.weight(.medium))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(Color.secondary.opacity(0.12))
            .foregroundStyle(.secondary)
            .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusSmall))
    }
}

// MARK: - Byte Count View

public struct ByteCountText: View {
    public let bytes: Int64
    public let font: Font
    public let weight: Font.Weight

    public init(bytes: Int64, font: Font = .body, weight: Font.Weight = .regular) {
        self.bytes = bytes
        self.font = font
        self.weight = weight
    }

    public var body: some View {
        Text(bytes.formatted(.byteCount(style: .file)))
            .font(font.weight(weight))
            .help("\(bytes) bytes")
    }
}

// MARK: - Reclaim Bounds Display

public struct ReclaimBoundsView: View {
    public let lowerBoundBytes: Int64
    public let upperBoundBytes: Int64

    public init(lowerBoundBytes: Int64, upperBoundBytes: Int64) {
        self.lowerBoundBytes = lowerBoundBytes
        self.upperBoundBytes = upperBoundBytes
    }

    public var body: some View {
        VStack(alignment: .trailing, spacing: 2) {
            HStack(spacing: 4) {
                Text("Confirmed:")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                Text(lowerBoundBytes.formatted(.byteCount(style: .file)))
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.green)
            }
            if upperBoundBytes > lowerBoundBytes {
                Text("Estimated up to \(upperBoundBytes.formatted(.byteCount(style: .file)))")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }
}
