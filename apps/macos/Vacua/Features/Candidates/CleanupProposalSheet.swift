import SwiftUI
import AppKit
import VacuaClient

public struct CleanupProposalSheet: View {
    public let proposal: CleanupPlanProposalV1
    @Environment(\.dismiss) private var dismiss
    @State private var copied: Bool = false
    
    public init(proposal: CleanupPlanProposalV1) {
        self.proposal = proposal
    }
    
    public var body: some View {
        VStack(spacing: 20) {
            // Header
            HStack {
                Image(systemName: "doc.text.magnifyingglass")
                    .font(.title)
                    .foregroundStyle(.purple)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Cleanup Proposal Review")
                        .font(.headline)
                    Text("Plan ID: \(proposal.plan_id)")
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button("Done") {
                    dismiss()
                }
            }
            
            Divider()
            
            // STRICT NON-DESTRUCTIVE NOTICE
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: "exclamationmark.shield.fill")
                    .foregroundStyle(.purple)
                    .font(.title3)
                VStack(alignment: .leading, spacing: 4) {
                    Text("Proposal Only — No Files Changed")
                        .font(.subheadline.weight(.semibold))
                    Text("Vacua does not execute cleanup from the native app. Execution authority is strictly disabled in the native client and MCP helper. You may inspect or export this proposal for verification.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
            }
            .padding(12)
            .background(Color.purple.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 8))
            
            // Plan Metrics
            HStack(spacing: 16) {
                metricCard(
                    title: "Candidates Included",
                    value: "\(proposal.item_count)",
                    subtitle: "Items in scope"
                )
                metricCard(
                    title: "Estimated Reclaim",
                    value: proposal.estimated_eventual_reclaim_bytes.formatted(.byteCount(style: .file)),
                    subtitle: "Max eventual space"
                )
                metricCard(
                    title: "Immediate Reclaim",
                    value: proposal.immediate_reclaim_bytes.formatted(.byteCount(style: .file)),
                    subtitle: "Immediate block return"
                )
            }
            
            // Plan Details
            VStack(alignment: .leading, spacing: 8) {
                Text("Verification Integrity")
                    .font(.subheadline.weight(.medium))
                
                HStack {
                    Text("Plan Hash:")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(proposal.plan_hash)
                        .font(.system(.caption, design: .monospaced))
                    Spacer()
                }
                .padding(8)
                .background(Color.secondary.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
            
            Spacer()
            
            // Actions
            HStack {
                Button {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(proposal.plan_id, forType: .string)
                    copied = true
                } label: {
                    Label(copied ? "Copied ID!" : "Copy Plan ID", systemImage: copied ? "checkmark" : "doc.on.doc")
                }
                
                Spacer()
                
                Button("Close") {
                    dismiss()
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(minWidth: 540, minHeight: 440)
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
