import SwiftUI
import VacuaClient

// MARK: - Risk Badge

public struct RiskBadge: View {
    public let risk: String
    
    public init(risk: String) {
        self.risk = risk
    }
    
    public var body: some View {
        let normalized = risk.lowercased()
        let (color, text): (Color, String) = {
            switch normalized {
            case "safe":
                return (.green, "Safe")
            case "caution", "cautious":
                return (.orange, "Caution")
            case "review":
                return (.blue, "Review")
            case "protected":
                return (.purple, "Protected")
            case "unknown":
                return (.secondary, "Unknown")
            default:
                return (.secondary, risk.capitalized)
            }
        }()
        
        Text(text)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(color.opacity(0.15))
            .foregroundStyle(color)
            .clipShape(Capsule())
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
            .clipShape(RoundedRectangle(cornerRadius: 4))
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
        VStack(alignment: .trailing, spacing: 1) {
            HStack(spacing: 4) {
                Text(lowerBoundBytes.formatted(.byteCount(style: .file)))
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.green)
            }
            if upperBoundBytes > lowerBoundBytes {
                Text("up to \(upperBoundBytes.formatted(.byteCount(style: .file)))")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
    }
}

// MARK: - Safety Guarantee Banner

public struct SafetyGuaranteeBanner: View {
    public init() {}
    
    public var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark.shield.fill")
                .foregroundStyle(.green)
                .font(.title3)
            
            VStack(alignment: .leading, spacing: 2) {
                Text("Proposal-Only Interface (v0.6.0)")
                    .font(.subheadline.weight(.medium))
                Text("Vacua native client has strictly zero mutation authority. No deletions or modifications will ever be performed from this interface.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(10)
        .background(Color.green.opacity(0.08))
        .clipShape(RoundedRectangle(cornerRadius: 8))
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.green.opacity(0.2), lineWidth: 1)
        )
    }
}
