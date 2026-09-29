import SwiftUI
import VacuaClient

public struct CandidatesView: View {
    @Bindable var model: AppModel
    @State private var selectedCandidateId: String?
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        HSplitView {
            // Main Candidates Table
            VStack(spacing: 0) {
                // Filter Toolbar
                filterBar
                
                Divider()
                
                // Table
                if model.candidates.isEmpty && !model.isLoading {
                    emptyState
                } else {
                    Table(model.candidates, selection: $selectedCandidateId) {
                        TableColumn("Target Path") { candidate in
                            Text(candidate.display_path)
                                .font(.system(.body, design: .monospaced))
                                .lineLimit(1)
                                .help(candidate.display_path)
                        }
                        .width(min: 240, ideal: 360)
                        
                        TableColumn("Category") { candidate in
                            CategoryBadge(category: candidate.category)
                        }
                        .width(min: 80, ideal: 100)
                        
                        TableColumn("Risk") { candidate in
                            RiskBadge(risk: candidate.risk)
                        }
                        .width(min: 70, ideal: 80)
                        
                        TableColumn("Confirmed") { candidate in
                            Text(candidate.confirmed_reclaim_lower_bound.formatted(.byteCount(style: .file)))
                                .font(.body.weight(.medium))
                                .foregroundStyle(.green)
                        }
                        .width(min: 80, ideal: 100)
                        
                        TableColumn("Estimated") { candidate in
                            Text(candidate.reclaim_upper_bound.formatted(.byteCount(style: .file)))
                                .font(.body)
                                .foregroundStyle(.secondary)
                        }
                        .width(min: 80, ideal: 100)
                    }
                    .tableStyle(.inset(alternatesRowBackgrounds: true))
                    .onChange(of: selectedCandidateId) { _, newId in
                        if let cand = model.candidates.first(where: { $0.id == newId }) {
                            Task { await model.selectCandidate(cand) }
                        }
                    }
                }
            }
            .frame(minWidth: 400)
            
            // Detail Inspector
            if let selected = currentSelectedCandidate {
                CandidateDetailView(
                    candidate: selected,
                    detail: model.selectedCandidateDetail,
                    isSimulating: model.isSimulating,
                    isProposing: model.isProposing,
                    onSimulate: {
                        Task { await model.simulateCleanup(candidateIds: [selected.id]) }
                    },
                    onPropose: {
                        Task { await model.proposeCleanup(candidateIds: [selected.id]) }
                    }
                )
                .background(Color(NSColor.controlBackgroundColor))
            } else {
                VStack(spacing: 12) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select a candidate to view evidence graph and reclaim analysis")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                }
                .frame(minWidth: 260, maxWidth: 320, maxHeight: .infinity)
                .padding(20)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("Cleanup Candidates")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.refreshCandidates() }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.isLoading)
            }
        }
        .sheet(item: $model.activeSimulation) { sim in
            CleanupSimulationSheet(simulation: sim)
        }
        .sheet(item: $model.activeProposal) { prop in
            CleanupProposalSheet(proposal: prop)
        }
    }
    
    private var currentSelectedCandidate: CandidateSummaryV1? {
        model.candidates.first(where: { $0.id == selectedCandidateId })
    }
    
    // MARK: - Filter Bar
    
    private var filterBar: some View {
        HStack(spacing: 12) {
            Picker("Risk", selection: $model.candidateRiskFilter) {
                Text("All Risks").tag(McpRiskFilter?.none)
                ForEach(McpRiskFilter.allCases, id: \.self) { risk in
                    Text(risk.displayName).tag(McpRiskFilter?.some(risk))
                }
            }
            .frame(width: 140)
            .onChange(of: model.candidateRiskFilter) { _, _ in
                Task { await model.refreshCandidates() }
            }
            
            Spacer()
            
            Text("\(model.candidates.count) items found")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
    }
    
    // MARK: - Empty State
    
    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.circle")
                .font(.system(size: 40))
                .foregroundStyle(.secondary)
            Text("No candidates matching current filter")
                .font(.headline)
            Text("Adjust risk filter or select another root directory in Settings.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
