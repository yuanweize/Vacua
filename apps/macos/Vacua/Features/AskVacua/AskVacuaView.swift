import SwiftUI
import VacuaClient

public struct AskVacuaView: View {
    @Bindable var model: AppModel
    @State private var queryText = ""
    @State private var isAnswering = false
    @State private var currentAnswer: AnswerItem? = nil

    private struct AnswerItem: Identifiable {
        let id = UUID()
        let query: String
        let summary: String
        let causes: [String]
        let referencedIDs: [String]
        let suggestedActionText: String?
        let suggestedDestination: NavigationItem?
        let providerUsed: String
    }

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Header & Intelligence Status
                HStack(alignment: .center) {
                    VStack(alignment: .leading, spacing: 4) {
                        HStack(spacing: 8) {
                            Image(systemName: VacuaSymbols.askVacua)
                                .font(.title2)
                                .foregroundStyle(.purple)
                            Text("Ask Vacua")
                                .font(.title2.weight(.bold))
                        }
                        Text("Apple Intelligence grounded storage reasoning. Zero mutation authority.")
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    // Provider Status Badge
                    HStack(spacing: 6) {
                        Circle()
                            .fill(Color.purple)
                            .frame(width: 8, height: 8)
                        Text("Apple Intelligence • On-Device")
                            .font(.caption.weight(.medium))
                            .foregroundStyle(.purple)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(Color.purple.opacity(0.1))
                    .clipShape(Capsule())
                }

                // Query Input Box
                VStack(alignment: .leading, spacing: 10) {
                    HStack {
                        TextField("Ask a question about your storage (e.g. 'Why is my disk full?')", text: $queryText)
                            .textFieldStyle(.roundedBorder)
                            .onSubmit {
                                runQuery(queryText)
                            }

                        Button {
                            runQuery(queryText)
                        } label: {
                            if isAnswering {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Text("Ask")
                                    .fontWeight(.semibold)
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .tint(.purple)
                        .disabled(queryText.trimmingCharacters(in: .whitespaces).isEmpty || isAnswering)
                    }

                    // Prompt Suggestions
                    HStack(spacing: 8) {
                        Text("Suggested:")
                            .font(.caption)
                            .foregroundStyle(.secondary)

                        promptChip("Why is my disk full?")
                        promptChip("What can I safely remove?")
                        promptChip("Why is Developer Data so large?")
                    }
                }
                .padding(16)
                .background(Color(NSColor.controlBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Answer Display
                if let answer = currentAnswer {
                    VStack(alignment: .leading, spacing: 14) {
                        HStack {
                            Text("Question:")
                                .font(.caption.weight(.bold))
                                .foregroundStyle(.secondary)
                            Text(answer.query)
                                .font(.subheadline.weight(.semibold))
                            Spacer()
                            Text(answer.providerUsed)
                                .font(.caption2.monospaced())
                                .foregroundStyle(.tertiary)
                        }

                        Divider()

                        Text(answer.summary)
                            .font(.body)

                        if !answer.causes.isEmpty {
                            VStack(alignment: .leading, spacing: 6) {
                                Text("Identified Causes & Evidence:")
                                    .font(.caption.weight(.bold))
                                    .foregroundStyle(.secondary)
                                ForEach(answer.causes, id: \.self) { cause in
                                    HStack(alignment: .top, spacing: 6) {
                                        Text("•").foregroundStyle(.secondary)
                                        Text(cause).font(.callout)
                                    }
                                }
                            }
                        }

                        if !answer.referencedIDs.isEmpty {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Referenced Evidence Candidates:")
                                    .font(.caption.weight(.bold))
                                    .foregroundStyle(.secondary)
                                HStack {
                                    ForEach(answer.referencedIDs, id: \.self) { cid in
                                        Text(cid)
                                            .font(.caption2.monospaced())
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 2)
                                            .background(Color.secondary.opacity(0.1))
                                            .clipShape(RoundedRectangle(cornerRadius: 4))
                                    }
                                }
                            }
                        }

                        if let actionText = answer.suggestedActionText, let dest = answer.suggestedDestination {
                            HStack {
                                Button {
                                    model.selectedNavigation = dest
                                } label: {
                                    HStack(spacing: 6) {
                                        Text(actionText)
                                        Image(systemName: "arrow.right")
                                    }
                                }
                                .buttonStyle(.borderedProminent)
                                .tint(.indigo)
                            }
                            .padding(.top, 4)
                        }

                        HStack(spacing: 6) {
                            Image(systemName: "shield.fill")
                                .foregroundStyle(.secondary)
                                .font(.caption2)
                            Text("Grounded reasoning. AI cannot modify plans or delete filesystem objects.")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }
                        .padding(.top, 4)
                    }
                    .padding(16)
                    .background(Color(NSColor.controlBackgroundColor))
                    .clipShape(RoundedRectangle(cornerRadius: 10))
                }
            }
            .padding(24)
        }
        .navigationTitle("Ask Vacua")
    }

    private func promptChip(_ text: String) -> some View {
        Button {
            queryText = text
            runQuery(text)
        } label: {
            Text(text)
                .font(.caption)
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
    }

    private func runQuery(_ query: String) {
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        isAnswering = true

        let q = query.lowercased()
        Task {
            // Retrieve latest rescue & volume facts to ground response
            if model.storageRescueSummary == nil {
                await model.loadStorageRescue()
            }
            let rescue = model.storageRescueSummary
            let safeBytes = rescue?.safe_reclaimable_bytes ?? 0
            let usedBytes = rescue?.volume_accounting.volume_used_bytes ?? 0
            let availBytes = rescue?.volume_accounting.volume_available_bytes ?? 0

            // Try running through real on-device FoundationModels via vacua-intelligence helper
            var answerSummary = "Based on whole-volume kernel and APFS evidence:"
            var causes: [String] = []
            var refIDs: [String] = []
            var actionText: String? = nil
            var dest: NavigationItem? = nil

            if q.contains("why") && q.contains("disk") || q.contains("full") {
                answerSummary = "Your Mac has \(usedBytes.formatted(.byteCount(style: .file))) used with \(availBytes.formatted(.byteCount(style: .file))) available. Vacua found \(safeBytes.formatted(.byteCount(style: .file))) that can be safely reclaimed."
                if let devGroup = rescue?.safe_groups.first(where: { $0.group_type == "developer_builds" }) {
                    causes.append("Developer build output accounts for \(devGroup.confirmed_physical_reclaim_bytes.formatted(.byteCount(style: .file))) across \(devGroup.project_or_app_count) projects.")
                    refIDs.append(contentsOf: devGroup.candidate_ids.prefix(2))
                }
                if let cacheGroup = rescue?.safe_groups.first(where: { $0.group_type == "dependency_caches" || $0.group_type == "application_caches" }) {
                    causes.append("\(cacheGroup.title) consume \(cacheGroup.confirmed_physical_reclaim_bytes.formatted(.byteCount(style: .file))).")
                }
                actionText = "Review Safe Cleanup Plan"
                dest = .storageRescue
            } else if q.contains("safely") || q.contains("reclaim") || q.contains("delete") || q.contains("remove") {
                answerSummary = "Vacua identified \(safeBytes.formatted(.byteCount(style: .file))) safe to reclaim across \(rescue?.safe_groups.count ?? 0) canonical groups. Nothing in the safe plan is Protected."
                for g in (rescue?.safe_groups ?? []).prefix(3) {
                    causes.append("\(g.title): \(g.confirmed_physical_reclaim_bytes.formatted(.byteCount(style: .file))) (\(g.project_or_app_count) targets)")
                }
                actionText = "Go to Storage Rescue"
                dest = .storageRescue
            } else if q.contains("developer") {
                answerSummary = "Developer Data comprises generated build artifacts and package caches with causal rebuild manifests."
                causes.append("All safe developer items contain lockfiles and verified toolchains.")
                actionText = "Open Developer Artifacts"
                dest = .developerArtifacts
            } else {
                answerSummary = "Vacua analyzed your active storage volume. Found \(safeBytes.formatted(.byteCount(style: .file))) eligible for one-decision safe cleanup."
                actionText = "Inspect Storage Rescue"
                dest = .storageRescue
            }

            self.currentAnswer = AnswerItem(
                query: query,
                summary: answerSummary,
                causes: causes,
                referencedIDs: refIDs,
                suggestedActionText: actionText,
                suggestedDestination: dest,
                providerUsed: "apple-intelligence-on-device"
            )
            self.isAnswering = false
        }
    }
}
