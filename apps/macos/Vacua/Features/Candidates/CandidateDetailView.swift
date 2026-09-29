import SwiftUI
import VacuaClient

public struct CandidateDetailView: View {
    public let candidate: CandidateSummaryV1
    public let detail: CandidateDetailV1?
    public let onSimulate: () -> Void
    public let onPropose: () -> Void
    
    public init(
        candidate: CandidateSummaryV1,
        detail: CandidateDetailV1? = nil,
        onSimulate: @escaping () -> Void,
        onPropose: @escaping () -> Void
    ) {
        self.candidate = candidate
        self.detail = detail
        self.onSimulate = onSimulate
        self.onPropose = onPropose
    }
    
    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                // Header
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        CategoryBadge(category: candidate.category)
                        RiskBadge(risk: candidate.risk)
                        Spacer()
                    }
                    Text(candidate.display_path)
                        .font(.headline)
                        .textSelection(.enabled)
                }
                
                Divider()
                
                // Reclaim Bounds Card
                VStack(alignment: .leading, spacing: 8) {
                    Text("Reclaim Potential")
                        .font(.subheadline.weight(.semibold))
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Confirmed Lower Bound")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(candidate.confirmed_reclaim_lower_bound.formatted(.byteCount(style: .file)))
                                .font(.title3.weight(.bold))
                                .foregroundStyle(.green)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 2) {
                            Text("Estimated Upper Bound")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(candidate.reclaim_upper_bound.formatted(.byteCount(style: .file)))
                                .font(.title3.weight(.bold))
                                .foregroundStyle(.blue)
                        }
                    }
                }
                .padding(12)
                .background(Color.secondary.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                
                // Reconstructable / Cost Model Status
                if let detail = detail {
                    if let consequence = detail.rebuild_consequence {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("Rebuild Consequence")
                                .font(.subheadline.weight(.semibold))
                            Text(consequence)
                                .font(.callout)
                                .foregroundStyle(.secondary)
                        }
                    }
                    
                    // Evidence Vector
                    if !detail.evidence_signals.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Evidence Graph (\(detail.evidence_signals.count))")
                                .font(.subheadline.weight(.semibold))
                            
                            ForEach(Array(detail.evidence_signals.enumerated()), id: \.offset) { _, ev in
                                HStack(alignment: .top, spacing: 8) {
                                    Image(systemName: "checkmark.circle.fill")
                                        .foregroundStyle(.blue)
                                        .font(.caption)
                                        .padding(.top, 2)
                                    Text(ev)
                                        .font(.caption)
                                        .foregroundStyle(.primary)
                                }
                            }
                        }
                        .padding(12)
                        .background(Color.secondary.opacity(0.06))
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                    }
                }
                
                Spacer()
                
                // Action Buttons
                VStack(spacing: 8) {
                    Button {
                        onSimulate()
                    } label: {
                        Label("Simulate Cleanup", systemImage: "play.circle")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                    
                    Button {
                        onPropose()
                    } label: {
                        Label("Generate Proposal Plan", systemImage: "doc.badge.plus")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                }
                .padding(.top, 12)
            }
            .padding(16)
        }
        .frame(minWidth: 280, maxWidth: 360)
    }
}
