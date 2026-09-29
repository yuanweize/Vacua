import SwiftUI
import VacuaClient

public struct CleanupSimulationSheet: View {
    public let simulation: CleanupSimulationV1
    @Environment(\.dismiss) private var dismiss
    
    public init(simulation: CleanupSimulationV1) {
        self.simulation = simulation
    }
    
    public var body: some View {
        VStack(spacing: 20) {
            // Header
            HStack {
                Image(systemName: "gauge.with.dots.needle.bottom.50percent")
                    .font(.title)
                    .foregroundStyle(.blue)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Cleanup Simulation")
                        .font(.headline)
                    Text("Dry-run impact evaluation performed by Vacua engine")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Done") {
                    dismiss()
                }
            }
            
            Divider()
            
            // Simulation Metrics
            HStack(spacing: 16) {
                metricCard(
                    title: "Candidates",
                    value: "\(simulation.candidate_count)",
                    subtitle: "Targeted items"
                )
                metricCard(
                    title: "Confirmed Lower Bound",
                    value: simulation.confirmed_lower_bound_bytes.formatted(.byteCount(style: .file)),
                    subtitle: "Guaranteed reclaimable"
                )
                metricCard(
                    title: "Estimated Upper Bound",
                    value: simulation.reclaim_upper_bound_bytes.formatted(.byteCount(style: .file)),
                    subtitle: "Max potential physical space"
                )
            }
            
            // Immediate Reclaim Explainer (Reality-First)
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: "info.circle.fill")
                    .foregroundStyle(.orange)
                    .font(.title3)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Immediate Reclaim: \(simulation.immediate_reclaim_bytes.formatted(.byteCount(style: .file)))")
                        .font(.subheadline.weight(.semibold))
                    Text("Deleting or unlinking APFS files does not immediately return free blocks to the container if active snapshots or shared extents exist. macOS kernel reclaims physical blocks during system idle or snapshot pruning.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding(12)
            .background(Color.orange.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            
            // Highest Risk and Consequences
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Highest Risk:")
                        .font(.subheadline.weight(.medium))
                    RiskBadge(risk: simulation.highest_risk)
                }
                
                if !simulation.rebuild_consequences.isEmpty {
                    Text("Rebuild Consequences:")
                        .font(.caption.weight(.semibold))
                    ForEach(simulation.rebuild_consequences, id: \.self) { item in
                        Text("• \(item)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
            .background(Color.secondary.opacity(0.06))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            
            Spacer()
        }
        .padding(24)
        .frame(minWidth: 540, minHeight: 460)
    }
    
    private func metricCard(title: String, value: String, subtitle: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption2)
                .foregroundStyle(.secondary)
            Text(value)
                .font(.title3.weight(.bold))
            Text(subtitle)
                .font(.caption2)
                .foregroundStyle(.secondary)
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.secondary.opacity(0.06))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
