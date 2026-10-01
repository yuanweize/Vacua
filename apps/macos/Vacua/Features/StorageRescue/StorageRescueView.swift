import SwiftUI
import Charts
import VacuaClient

public struct StorageRescueView: View {
    @Bindable var model: AppModel
    @State private var showingConfirmationModal = false
    @State private var expandedGroupId: String? = nil

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 24) {
                // Header & Storage Pressure Banner
                rescueHeader

                if let rescue = model.storageRescueSummary {
                    // Whole-Volume Accounting Hero
                    volumeAccountingHero(rescue: rescue)

                    // Reclaim Opportunity Stats (Three Levels)
                    reclaimOpportunityStats(rescue: rescue)

                    // One-Decision Action Banner
                    oneDecisionActionBanner(rescue: rescue)

                    // Canonical Safe Groups (Primary cleanup target)
                    safeGroupsSection(rescue: rescue)

                    // Review Required Section (Safely segregated)
                    reviewGroupsSection(rescue: rescue)

                    // Protected Summary Guard
                    protectedGuardSection(summary: rescue.protected_summary)
                } else if model.rescueState.isLoading {
                    ProgressView("Analyzing whole-volume storage and preparing rescue plan…")
                        .frame(maxWidth: .infinity, alignment: .center)
                        .padding(.vertical, 60)
                } else if case .failed(let msg, _) = model.rescueState {
                    VacuaErrorState(
                        title: "Storage Rescue Unavailable",
                        message: msg,
                        retryAction: {
                            Task { await model.loadStorageRescue() }
                        }
                    )
                }
            }
            .padding(24)
        }
        .navigationTitle("Storage Rescue")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.loadStorageRescue() }
                } label: {
                    Label("Refresh", systemImage: VacuaSymbols.refresh)
                }
                .disabled(model.rescueState.isLoading || model.isExecutingGroupPlan)
            }
        }
        .task {
            if model.storageRescueSummary == nil {
                await model.loadStorageRescue()
            }
        }
        .sheet(isPresented: $showingConfirmationModal) {
            if let rescue = model.storageRescueSummary {
                confirmationSheet(rescue: rescue)
            }
        }
    }

    // MARK: - Header
    private var rescueHeader: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Image(systemName: VacuaSymbols.storageRescue)
                    .font(.title2)
                    .foregroundStyle(.tint)
                Text("Storage Rescue")
                    .font(.title2.weight(.bold))
                Spacer()
                if let rescue = model.storageRescueSummary {
                    Text("Pressure: \(rescue.volume_accounting.pressure_level.uppercased())")
                        .font(.caption.weight(.bold))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 4)
                        .background(pressureBadgeColor(rescue.volume_accounting.pressure_level).opacity(0.15))
                        .foregroundStyle(pressureBadgeColor(rescue.volume_accounting.pressure_level))
                        .clipShape(Capsule())
                }
            }
            Text("Evidence-backed whole-volume accounting and one-decision safe cleanup.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }

    // MARK: - Volume Accounting Hero
    private func volumeAccountingHero(rescue: StorageRescueSummaryV1) -> some View {
        let accounting = rescue.volume_accounting
        let total = max(Double(accounting.total_capacity_bytes), 1.0)
        let used = Double(accounting.volume_used_bytes)
        let avail = Double(accounting.volume_available_bytes)

        return VStack(alignment: .leading, spacing: 14) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Volume Capacity & Reconciliation")
                        .font(.headline)
                    Text("Observed via kernel statfs with physical block attribution")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Spacer()
                Text("\(accounting.volume_used_bytes.formatted(.byteCount(style: .file))) of \(accounting.total_capacity_bytes.formatted(.byteCount(style: .file))) used")
                    .font(.subheadline.weight(.semibold))
            }

            // Progress bar
            GeometryReader { geo in
                let w = geo.size.width
                let usedW = min(w * CGFloat(used / total), w)
                let availW = min(w * CGFloat(avail / total), w - usedW)

                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 6)
                        .fill(Color.secondary.opacity(0.15))

                    HStack(spacing: 0) {
                        RoundedRectangle(cornerRadius: 6)
                            .fill(LinearGradient(colors: [.indigo, .purple], startPoint: .leading, endPoint: .trailing))
                            .frame(width: usedW)

                        RoundedRectangle(cornerRadius: 6)
                            .fill(Color.green.opacity(0.6))
                            .frame(width: availW)
                    }
                }
            }
            .frame(height: 12)

            HStack {
                Label("Attributed: \(accounting.attributed_bytes.formatted(.byteCount(style: .file)))", systemImage: "circle.fill")
                    .font(.caption)
                    .foregroundStyle(.indigo)
                Spacer()
                Label("System-Managed: \(accounting.unattributed_system_managed_bytes.formatted(.byteCount(style: .file)))", systemImage: "circle.fill")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Spacer()
                Label("Available: \(accounting.volume_available_bytes.formatted(.byteCount(style: .file)))", systemImage: "circle.fill")
                    .font(.caption)
                    .foregroundStyle(.green)
            }

            if accounting.is_material_discrepancy {
                HStack(spacing: 8) {
                    Image(systemName: "info.circle")
                        .foregroundStyle(.orange)
                    Text("Some storage is not attributable from user-space evidence (APFS snapshots, purgeable blocks, or virtual swap).")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(8)
                .background(Color.orange.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    // MARK: - Reclaim Opportunities (Three Levels of Evidence)
    private func reclaimOpportunityStats(rescue: StorageRescueSummaryV1) -> some View {
        HStack(spacing: 16) {
            // SAFE
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: VacuaSymbols.riskSafe)
                        .foregroundStyle(.green)
                    Text("SAFE TO RECLAIM")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)
                }
                Text(rescue.safe_reclaimable_bytes.formatted(.byteCount(style: .file)))
                    .font(.title.weight(.bold))
                    .foregroundStyle(.green)
                Text("\(rescue.safe_groups.count) canonical safe groups")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color.green.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 10))

            // REVIEW
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: VacuaSymbols.riskReview)
                        .foregroundStyle(.orange)
                    Text("REVIEW REQUIRED")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)
                }
                Text(rescue.review_recommended_bytes.formatted(.byteCount(style: .file)))
                    .font(.title.weight(.bold))
                    .foregroundStyle(.orange)
                Text("Needs manual inspection")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color.orange.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 10))

            // PROTECTED
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Image(systemName: VacuaSymbols.riskProtected)
                        .foregroundStyle(.blue)
                    Text("PROTECTED")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)
                }
                Text("\(rescue.protected_summary.protected_locations_count) locations")
                    .font(.title.weight(.bold))
                    .foregroundStyle(.primary)
                Text("System & credentials guarded")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(16)
            .background(Color.blue.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: 10))
        }
    }

    // MARK: - One Decision Action Banner
    private func oneDecisionActionBanner(rescue: StorageRescueSummaryV1) -> some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 4) {
                Text("One-Decision Safe Cleanup")
                    .font(.headline)
                if rescue.safe_reclaimable_bytes > 0 {
                    Text("Vacua prepared a verified plan to reclaim \(rescue.safe_reclaimable_bytes.formatted(.byteCount(style: .file))) across \(rescue.safe_groups.count) safe groups.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                } else {
                    Text("No candidate storage currently meets automatic safety criteria. Inspect review areas below.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
            }
            Spacer()
            if rescue.safe_reclaimable_bytes > 0 {
                Button {
                    showingConfirmationModal = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "trash")
                        Text("Review Safe Cleanup (\(rescue.safe_reclaimable_bytes.formatted(.byteCount(style: .file))))")
                            .fontWeight(.semibold)
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 10)
                }
                .buttonStyle(.borderedProminent)
                .tint(.green)
                .disabled(model.isExecutingGroupPlan)
            }
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 10))
    }

    // MARK: - Canonical Safe Groups
    private func safeGroupsSection(rescue: StorageRescueSummaryV1) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Safe Reclaim Plan")
                .font(.headline)

            if rescue.safe_groups.isEmpty || rescue.safe_reclaimable_bytes == 0 {
                Text("Vacua could not identify storage that meets its automatic safety criteria.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .padding()
            } else {
                ForEach(rescue.safe_groups) { group in
                    groupCard(group: group, isSafe: true)
                }
            }
        }
    }

    // MARK: - Review Groups
    private func reviewGroupsSection(rescue: StorageRescueSummaryV1) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Review Recommended (Excluded from One-Decision Plan)")
                .font(.headline)

            if rescue.review_groups.isEmpty {
                Text("No items currently pending review.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            } else {
                ForEach(rescue.review_groups) { group in
                    groupCard(group: group, isSafe: false)
                }
            }
        }
    }

    // MARK: - Group Card
    private func groupCard(group: CandidateGroupSummaryV1, isSafe: Bool) -> some View {
        let isExpanded = expandedGroupId == group.group_id

        return VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .center) {
                Image(systemName: isSafe ? VacuaSymbols.riskSafe : VacuaSymbols.riskReview)
                    .foregroundStyle(isSafe ? .green : .orange)
                    .font(.title3)

                VStack(alignment: .leading, spacing: 2) {
                    Text(group.title)
                        .font(.subheadline.weight(.semibold))
                    Text("\(group.project_or_app_count) targets • \(group.item_count) items • \(group.description)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Text(group.confirmed_physical_reclaim_bytes.formatted(.byteCount(style: .file)))
                    .font(.subheadline.weight(.bold))
                    .foregroundStyle(isSafe ? .green : .primary)

                Button {
                    withAnimation {
                        if expandedGroupId == group.group_id {
                            expandedGroupId = nil
                        } else {
                            expandedGroupId = group.group_id
                        }
                    }
                } label: {
                    Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                        .foregroundStyle(.secondary)
                }
                .buttonStyle(.plain)
            }

            if isExpanded {
                Divider()
                VStack(alignment: .leading, spacing: 6) {
                    Text("Reconstruction & Safety Evidence:")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)
                    ForEach(group.evidence_reasons, id: \.self) { reason in
                        HStack(alignment: .top, spacing: 6) {
                            Text("•").foregroundStyle(.secondary)
                            Text(reason).font(.caption).foregroundStyle(.secondary)
                        }
                    }
                    if group.active_guard_deferred {
                        HStack(spacing: 6) {
                            Image(systemName: "clock.badge.exclamationmark")
                                .foregroundStyle(.orange)
                            Text("Active Process Guard: Deferred because active compiler or build tool detected.")
                                .font(.caption.weight(.medium))
                                .foregroundStyle(.orange)
                        }
                    }
                }
                .padding(.top, 4)
            }
        }
        .padding(14)
        .background(Color(NSColor.controlBackgroundColor))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Protected Guard Section
    private func protectedGuardSection(summary: ProtectedSummaryV1) -> some View {
        HStack(spacing: 12) {
            Image(systemName: VacuaSymbols.riskProtected)
                .font(.title3)
                .foregroundStyle(.blue)
            VStack(alignment: .leading, spacing: 2) {
                Text("Deterministic Protection Active")
                    .font(.subheadline.weight(.semibold))
                Text("\(summary.protected_locations_count) locations protected. \(summary.description)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(12)
        .background(Color.blue.opacity(0.06))
        .clipShape(RoundedRectangle(cornerRadius: 8))
    }

    // MARK: - Confirmation Sheet (The Single Decision)
    private func confirmationSheet(rescue: StorageRescueSummaryV1) -> some View {
        VStack(spacing: 20) {
            Image(systemName: VacuaSymbols.safetyShield)
                .font(.system(size: 44))
                .foregroundStyle(.green)

            VStack(spacing: 6) {
                Text("Ready to reclaim \(rescue.safe_reclaimable_bytes.formatted(.byteCount(style: .file)))")
                    .font(.title2.weight(.bold))
                Text("Vacua will move items to Trash using reversible macOS Trash APIs.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }

            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                    Text("Nothing in this plan is Protected.")
                        .font(.caption.weight(.medium))
                }
                HStack {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                    Text("All targets passed rebuildability and preservation preflights.")
                        .font(.caption.weight(.medium))
                }
                HStack {
                    Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
                    Text("Items can be restored from macOS Trash.")
                        .font(.caption.weight(.medium))
                }
            }
            .padding()
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color(NSColor.controlBackgroundColor))
            .clipShape(RoundedRectangle(cornerRadius: 8))

            if let report = model.executionReport {
                VStack(spacing: 4) {
                    Text("Reclaim Complete!")
                        .font(.headline)
                        .foregroundStyle(.green)
                    Text("\(report.actual_bytes_moved_to_trash.formatted(.byteCount(style: .file))) moved to Trash.")
                        .font(.subheadline)
                }
            }

            if let err = model.executionError {
                Text("Error: \(err)")
                    .font(.caption)
                    .foregroundStyle(.red)
            }

            HStack(spacing: 16) {
                Button("Cancel") {
                    showingConfirmationModal = false
                }
                .keyboardShortcut(.cancelAction)

                Button {
                    let groupIds = rescue.safe_groups.map { $0.group_id }
                    Task {
                        let success = await model.proposeAndExecuteSafePlan(groupIds: groupIds)
                        if success {
                            showingConfirmationModal = false
                        }
                    }
                } label: {
                    if model.isExecutingGroupPlan {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Text("Reclaim \(rescue.safe_reclaimable_bytes.formatted(.byteCount(style: .file)))")
                            .fontWeight(.semibold)
                    }
                }
                .buttonStyle(.borderedProminent)
                .tint(.green)
                .disabled(model.isExecutingGroupPlan)
            }
        }
        .padding(30)
        .frame(width: 480)
    }

    private func pressureBadgeColor(_ level: String) -> Color {
        switch level.lowercased() {
        case "critical": return .red
        case "low": return .orange
        case "elevated": return .yellow
        default: return .green
        }
    }
}
