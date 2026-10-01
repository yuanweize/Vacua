import SwiftUI
import Charts
import VacuaClient

public struct OverviewView: View {
    @Bindable var model: AppModel
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Active Root & Safety Header
                VStack(alignment: .leading, spacing: 12) {
                    HStack(alignment: .center) {
                        Image(systemName: VacuaSymbols.activeRoot)
                            .font(.title3)
                            .foregroundStyle(.tint)
                        VStack(alignment: .leading, spacing: 1) {
                            Text("Active Storage Root")
                                .font(.caption.weight(.medium))
                                .foregroundStyle(.secondary)
                            Text(model.activeRootPath)
                                .font(.subheadline.weight(.semibold))
                                .lineLimit(1)
                        }
                        Spacer()
                    }
                    .padding(12)
                    .background(Color(NSColor.controlBackgroundColor))
                    .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusMedium))

                    SafetyGuaranteeBanner()
                }

                if let summary = model.storageSummary {
                    storagePressureBanner(summary: summary)

                    // Storage Breakdown Card
                    storageHeroCard(summary: summary)
                    
                    // Reclaim Potential Grid
                    reclaimSummaryGrid(summary: summary)
                } else if model.isLoading {
                    ProgressView("Analyzing storage…")
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.vertical, 40)
                } else if let error = model.errorMessage {
                    VacuaErrorState(
                        title: "Storage Analysis Unavailable",
                        message: error,
                        retryAction: {
                            Task { await model.loadOverview(force: true) }
                        }
                    )
                }
                
                // Navigation Quick Cards
                quickJumpSection()
            }
            .padding(24)
        }
        .navigationTitle("Storage Overview")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.loadOverview(force: true) }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.overviewState.isLoading)
            }
        }
    }
    
    // MARK: - Storage Hero Card
    
    @ViewBuilder
    private func storageHeroCard(summary: StorageSummaryV1) -> some View {
        let purgeable = summary.purgeable_space_bytes ?? 0
        let used = summary.usedSpaceBytes
        
        VStack(alignment: .leading, spacing: 16) {
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Volume Capacity & Usage")
                        .font(.headline)
                    Text("\(summary.filesystem_type) Physical Allocation")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text("Total: \(summary.total_space_bytes.formatted(.byteCount(style: .file)))")
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(.secondary)
            }
            
            // Usage Progress Bar
            GeometryReader { geo in
                let total = max(Double(summary.total_space_bytes), 1.0)
                let usedRatio = min(Double(used) / total, 1.0)
                let purgeRatio = min(Double(purgeable) / total, 1.0 - usedRatio)
                
                ZStack(alignment: .leading) {
                    // Total Background
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.secondary.opacity(0.15))
                        .frame(height: 16)
                    
                    // Purgeable Overlay
                    HStack(spacing: 0) {
                        Rectangle()
                            .fill(Color.blue)
                            .frame(width: geo.size.width * usedRatio, height: 16)
                        
                        if purgeRatio > 0 {
                            Rectangle()
                                .fill(Color.cyan.opacity(0.6))
                                .frame(width: geo.size.width * purgeRatio, height: 16)
                        }
                    }
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }
            }
            .frame(height: 16)
            
            // Legend
            HStack(spacing: 20) {
                legendItem(color: .blue, title: "Used", bytes: used)
                if purgeable > 0 {
                    legendItem(color: .cyan.opacity(0.8), title: "Purgeable", bytes: purgeable)
                }
                legendItem(color: .secondary.opacity(0.3), title: "Available", bytes: summary.available_space_bytes)
            }
        }
        .padding(18)
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .shadow(color: .black.opacity(0.04), radius: 3, y: 1)
    }
    
    private func legendItem(color: Color, title: String, bytes: UInt64) -> some View {
        HStack(spacing: 6) {
            Circle()
                .fill(color)
                .frame(width: 8, height: 8)
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
            Text(bytes.formatted(.byteCount(style: .file)))
                .font(.caption.weight(.semibold))
        }
    }
    
    // MARK: - Reclaim Summary Grid
    
    @ViewBuilder
    private func reclaimSummaryGrid(summary: StorageSummaryV1) -> some View {
        HStack(spacing: 16) {
            // Confirmed Lower Bound (Solid Truth)
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                    Text("Confirmed Lower Bound")
                        .font(.subheadline.weight(.semibold))
                }
                Text(summary.candidate_confirmed_reclaim_bytes.formatted(.byteCount(style: .file)))
                    .font(.system(size: 26, weight: .bold, design: .rounded))
                    .foregroundStyle(.green)
                Text("Guaranteed reclaimable physical space (safe and cautiously isolated unreferenced allocations).")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color(NSColor.controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 12))
            
            // Estimated Upper Bound
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "chart.line.uptrend.xyaxis")
                        .foregroundStyle(.blue)
                    Text("Estimated Reclaim Upper Bound")
                        .font(.subheadline.weight(.semibold))
                }
                Text(summary.candidate_estimated_reclaim_bytes.formatted(.byteCount(style: .file)))
                    .font(.system(size: 26, weight: .bold, design: .rounded))
                    .foregroundStyle(.blue)
                Text("Potential reclaim including shared clones and pruneable developer caches.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color(NSColor.controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
    }
    
    // MARK: - Quick Navigation
    
    @ViewBuilder
    private func quickJumpSection() -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Storage Intelligence Domains")
                .font(.headline)
            
            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
                jumpCard(
                    title: "Candidates",
                    count: model.candidates.count,
                    icon: VacuaSymbols.candidates,
                    destination: .candidates
                )
                jumpCard(
                    title: "Duplicates",
                    count: model.duplicateGroups.count,
                    icon: VacuaSymbols.duplicates,
                    destination: .duplicates
                )
                jumpCard(
                    title: "Applications",
                    count: model.applications.count,
                    icon: VacuaSymbols.applications,
                    destination: .applications
                )
            }
        }
    }
    
    private func jumpCard(title: String, count: Int, icon: String, destination: NavigationItem) -> some View {
        Button {
            model.selectedNavigation = destination
        } label: {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: icon)
                        .font(.title2)
                        .foregroundStyle(.tint)
                    Spacer()
                    Image(systemName: "chevron.right")
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
                Text("\(count)")
                    .font(.title2.weight(.bold))
                Text(title)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(NSColor.controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private func storagePressureBanner(summary: StorageSummaryV1) -> some View {
        let isPressure = summary.pressure_level.lowercased() != "healthy" && summary.pressure_level.lowercased() != "normal"
        if isPressure || summary.available_space_bytes < (15 * 1024 * 1024 * 1024) {
            HStack(alignment: .center, spacing: 14) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.title2)
                    .foregroundStyle(.orange)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Storage pressure detected")
                        .font(.headline)
                    Text("Your Mac has only \(summary.available_space_bytes.formatted(.byteCount(style: .file))) available. Vacua can analyze your storage and prepare a safe reclaim plan.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Button {
                    model.selectedNavigation = .storageRescue
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: VacuaSymbols.storageRescue)
                        Text("Find Safe Space")
                            .fontWeight(.semibold)
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                }
                .buttonStyle(.borderedProminent)
                .tint(.orange)
            }
            .padding(16)
            .background(Color.orange.opacity(0.1))
            .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusMedium))
        }
    }
}
