import SwiftUI
import VacuaClient

public struct DeveloperArtifactsView: View {
    @Bindable var model: DeveloperArtifactsModel
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityDifferentiateWithoutColor) private var differentiateWithoutColor

    public init(model: DeveloperArtifactsModel) {
        self.model = model
    }

    public var body: some View {
        VStack(spacing: 0) {
            switch model.analysisState {
            case .idle:
                idleView
            case .loading:
                loadingView
            case .failed(let message, _):
                errorView(message: message)
            case .loaded(let analysis):
                if analysis.projects.isEmpty {
                    emptyView
                } else {
                    loadedContentView(analysis: analysis)
                }
            }
        }
        .navigationTitle("Developer Artifacts")
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.analyzeDeveloperArtifacts(forceRefresh: true) }
                } label: {
                    Label("Refresh", systemImage: VacuaSymbols.refresh)
                }
                .disabled(model.isAnalyzing)
                .help("Re-analyze developer artifacts in active root")
            }
        }
    }

    // MARK: - Idle State

    private var idleView: some View {
        VStack(spacing: 20) {
            Spacer()

            Image(systemName: VacuaSymbols.developerArtifacts)
                .font(.system(size: 48))
                .foregroundStyle(.tint)
                .accessibilityHidden(true)

            Text("Developer Artifacts")
                .font(.title2.weight(.bold))

            Text("Find build outputs, dependency state, caches, and generated development data with evidence explaining how they can be reconstructed.")
                .font(.body)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 480)

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.shield")
                        .foregroundStyle(.secondary)
                    Text("Bounded metadata inspection: Reads manifests and lockfiles up to 1 MiB; dataless cloud files are never hydrated.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                HStack(spacing: 8) {
                    Image(systemName: "hand.raised")
                        .foregroundStyle(.secondary)
                    Text("Zero toolchain execution: Never executes cargo, npm, pip, gradle, cmake, or arbitrary shell commands.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                HStack(spacing: 8) {
                    Image(systemName: "info.circle")
                        .foregroundStyle(.secondary)
                    Text("Analyze-only: Artifact analysis is descriptive and provides no deletion authority.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
            }
            .padding(14)
            .background(Color.secondary.opacity(0.08))
            .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusMedium))
            .frame(maxWidth: 520)

            Button {
                Task { await model.analyzeDeveloperArtifacts(forceRefresh: false) }
            } label: {
                Label("Analyze Developer Artifacts", systemImage: VacuaSymbols.developerArtifacts)
                    .font(.headline)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .padding(.top, 8)

            Spacer()
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Loading State

    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .controlSize(.large)
            Text("Analyzing Developer Artifacts…")
                .font(.headline)
            Text("Inspecting project structures and lockfile evidence without executing commands.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Error State

    private func errorView(message: String) -> some View {
        VacuaErrorState(
            title: "Analysis Failed",
            message: message,
            retryAction: {
                Task { await model.analyzeDeveloperArtifacts(forceRefresh: true) }
            }
        )
    }

    // MARK: - Empty State

    private var emptyView: some View {
        VStack(spacing: 16) {
            Spacer()
            Image(systemName: "folder.badge.questionmark")
                .font(.system(size: 40))
                .foregroundStyle(.secondary)
                .accessibilityHidden(true)

            Text("No Supported Developer Artifacts")
                .font(.headline)

            Text("No supported developer artifacts were found in the selected root.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 400)

            Button("Re-Scan") {
                Task { await model.analyzeDeveloperArtifacts(forceRefresh: true) }
            }
            .buttonStyle(.bordered)
            Spacer()
        }
        .padding(32)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Loaded Content View

    private func loadedContentView(analysis: DeveloperArtifactAnalysisV1) -> some View {
        VStack(spacing: 0) {
            // Summary Header
            summaryHeader(analysis: analysis)

            Divider()

            // Ecosystem Filter Bar
            ecosystemFilterBar

            Divider()

            // Main Master-Detail Split
            HSplitView {
                projectsAndArtifactsList
                    .frame(minWidth: 320, idealWidth: 380, maxWidth: 500)

                artifactInspector
                    .frame(minWidth: 300, idealWidth: 360, maxWidth: .infinity)
            }
        }
    }

    // MARK: - Summary Header

    private func summaryHeader(analysis: DeveloperArtifactAnalysisV1) -> some View {
        HStack(spacing: 20) {
            summaryMetricCard(
                title: "Observed Allocation",
                value: Int64(analysis.total_allocated_bytes).formatted(.byteCount(style: .file)),
                subtitle: "\(analysis.total_logical_bytes.formatted(.byteCount(style: .file))) logical",
                icon: "internaldrive.fill"
            )

            summaryMetricCard(
                title: "Projects",
                value: "\(analysis.total_projects)",
                subtitle: "Identified roots",
                icon: "folder.fill"
            )

            summaryMetricCard(
                title: "Artifacts",
                value: "\(analysis.total_artifacts)",
                subtitle: "Generated items",
                icon: "cube.fill"
            )

            summaryMetricCard(
                title: "Coverage",
                value: "\(analysis.coverage.supported_ecosystems.count) Ecosystems",
                subtitle: "\(analysis.coverage.unclassified_candidate_directories) unclassified",
                icon: "shield.checkerboard"
            )

            Spacer()
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .background(Color(nsColor: .windowBackgroundColor))
    }

    private func summaryMetricCard(title: String, value: String, subtitle: String, icon: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: icon)
                .font(.title3)
                .foregroundStyle(.tint)
                .frame(width: 24)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(.secondary)
                Text(value)
                    .font(.headline.weight(.semibold))
                    .lineLimit(1)
                Text(subtitle)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
    }

    // MARK: - Ecosystem Filter Bar

    private var ecosystemFilterBar: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(model.availableEcosystems, id: \.self) { eco in
                    let isSelected = (model.selectedEcosystem == nil && eco == "All") || (model.selectedEcosystem == eco)
                    Button {
                        if eco == "All" {
                            model.selectedEcosystem = nil
                        } else {
                            model.selectedEcosystem = eco
                        }
                    } label: {
                        Text(eco)
                            .font(.caption.weight(isSelected ? .semibold : .regular))
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                            .background(isSelected ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.08))
                            .foregroundStyle(isSelected ? Color.accentColor : Color.primary)
                            .clipShape(Capsule())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(Text("Filter by \(eco)"))
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .background(Color(nsColor: .controlBackgroundColor))
    }

    // MARK: - Projects & Artifacts List

    private var projectsAndArtifactsList: some View {
        List(selection: Binding(
            get: { model.selectedProjectId },
            set: { newId in
                if let newId = newId {
                    Task { await model.selectProject(projectId: newId) }
                }
            }
        )) {
            ForEach(model.filteredProjects) { project in
                Section {
                    projectHeaderRow(project: project)

                    if model.selectedProjectId == project.project_id {
                        if let artifacts = model.projectArtifacts[project.project_id] {
                            ForEach(artifacts) { art in
                                artifactRow(artifact: art)
                            }
                        } else if model.loadingProjectIds.contains(project.project_id) {
                            HStack {
                                Spacer()
                                ProgressView()
                                    .controlSize(.small)
                                Text("Loading artifacts…")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                Spacer()
                            }
                            .padding(.vertical, 6)
                        }
                    }
                }
            }
        }
        .listStyle(.sidebar)
    }

    private func projectHeaderRow(project: DeveloperProjectSummaryV1) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(project.display_name)
                    .font(.subheadline.weight(.semibold))
                    .lineLimit(1)

                Spacer()

                ByteCountText(bytes: Int64(project.total_allocated_bytes), font: .caption, weight: .semibold)
            }

            HStack(spacing: 6) {
                CategoryBadge(category: project.primary_ecosystem)
                RebuildConfidenceBadge(confidence: project.rebuild_confidence)

                Spacer()

                Text("\(project.artifacts_count) artifacts")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.vertical, 4)
        .tag(project.project_id)
    }

    private func artifactRow(artifact: DeveloperArtifactSummaryV1) -> some View {
        Button {
            Task { await model.selectArtifact(artifactId: artifact.artifact_id) }
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "cube")
                    .font(.caption)
                    .foregroundStyle(.tint)
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 2) {
                    Text(artifact.display_name)
                        .font(.caption.weight(.medium))
                        .lineLimit(1)
                    Text(artifact.artifact_kind)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                VStack(alignment: .trailing, spacing: 2) {
                    ByteCountText(bytes: Int64(artifact.allocated_bytes), font: .caption2, weight: .medium)
                    RebuildConfidenceBadge(confidence: artifact.rebuild_confidence)
                }
            }
            .padding(.vertical, 4)
            .padding(.horizontal, 8)
            .background(model.selectedArtifactId == artifact.artifact_id ? Color.accentColor.opacity(0.12) : Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: 4))
        }
        .buttonStyle(.plain)
    }

    // MARK: - Artifact Inspector

    private var artifactInspector: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let detail = model.selectedArtifactDetail {
                    artifactDetailView(detail: detail)
                } else if model.isLoadingDetail {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text("Loading artifact evidence…")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, minHeight: 200)
                } else {
                    VStack(spacing: 12) {
                        Image(systemName: "sidebar.right")
                            .font(.system(size: 32))
                            .foregroundStyle(.tertiary)
                            .accessibilityHidden(true)
                        Text("Select an artifact")
                            .font(.headline)
                            .foregroundStyle(.secondary)
                        Text("Choose an artifact from the list to inspect rebuild evidence, storage attribution, and reconstruction parameters.")
                            .font(.caption)
                            .foregroundStyle(.tertiary)
                            .multilineTextAlignment(.center)
                            .frame(maxWidth: 240)
                    }
                    .frame(maxWidth: .infinity, minHeight: 280)
                }
            }
            .padding(16)
        }
        .background(Color(nsColor: .controlBackgroundColor))
    }

    private func artifactDetailView(detail: DeveloperArtifactDetailV1) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            VStack(alignment: .leading, spacing: 4) {
                Text(detail.display_name)
                    .font(.title3.weight(.bold))
                    .lineLimit(1)

                Text(detail.display_path)
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                    .textSelection(.enabled)

                HStack(spacing: 6) {
                    CategoryBadge(category: detail.ecosystem)
                    CategoryBadge(category: detail.artifact_kind)
                    RebuildConfidenceBadge(confidence: detail.rebuild_evidence.reconstruction_confidence)
                }
                .padding(.top, 4)
            }

            Divider()

            // Storage Breakdown
            VStack(alignment: .leading, spacing: 8) {
                Text("Storage Truth")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.secondary)

                inspectorRow(label: "Allocated", value: Int64(detail.allocated_bytes).formatted(.byteCount(style: .file)))
                inspectorRow(label: "Logical Size", value: Int64(detail.logical_bytes).formatted(.byteCount(style: .file)))
                inspectorRow(label: "Confirmed Reclaim Lower Bound", value: Int64(detail.confirmed_reclaim_lower_bound).formatted(.byteCount(style: .file)))

                if detail.physical_sharing_uncertainty {
                    HStack(spacing: 4) {
                        Image(systemName: "info.circle")
                            .font(.caption2)
                            .foregroundStyle(.orange)
                        Text("APFS clone extent or hardlink sharing may be present.")
                            .font(.caption2)
                            .foregroundStyle(.orange)
                    }
                    .padding(.top, 2)
                }
            }

            Divider()

            // Rebuild Evidence
            VStack(alignment: .leading, spacing: 8) {
                Text("Rebuild Evidence")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.secondary)

                inspectorRow(
                    label: "Manifest Present",
                    value: detail.rebuild_evidence.manifest_present ? "Yes" : "No"
                )
                if let mpath = detail.rebuild_evidence.manifest_path {
                    Text(mpath)
                        .font(.caption2.monospaced())
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }

                inspectorRow(
                    label: "Lockfile Present",
                    value: detail.rebuild_evidence.lockfile_present ? "Yes" : "No"
                )
                if let lpath = detail.rebuild_evidence.lockfile_path {
                    Text(lpath)
                        .font(.caption2.monospaced())
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }

                inspectorRow(
                    label: "Convention Recognized",
                    value: detail.rebuild_evidence.known_artifact_convention ? "Yes" : "No"
                )

                if let toolchain = detail.rebuild_evidence.toolchain_identified {
                    inspectorRow(label: "Toolchain Identified", value: toolchain)
                }

                inspectorRow(label: "Active State", value: detail.rebuild_evidence.active_project_state)

                if !detail.rebuild_evidence.reasons.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Evidence Reasons:")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                        ForEach(detail.rebuild_evidence.reasons, id: \.self) { reason in
                            HStack(alignment: .top, spacing: 4) {
                                Text("•")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                Text(reason)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                        }
                    }
                    .padding(.top, 4)
                }
            }

            if let template = detail.rebuild_evidence.rebuild_command_template {
                Divider()

                VStack(alignment: .leading, spacing: 6) {
                    Text("Rebuild Command Template")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)

                    Text(template)
                        .font(.caption.monospaced())
                        .padding(8)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color.secondary.opacity(0.08))
                        .clipShape(RoundedRectangle(cornerRadius: 6))
                        .textSelection(.enabled)

                    Text("Informational template only. Vacua never executes project commands or shell scripts.")
                        .font(.caption2)
                        .foregroundStyle(.tertiary)
                }
            }

            if let candidateId = detail.candidate_id {
                Divider()

                VStack(alignment: .leading, spacing: 6) {
                    Text("Candidate Association")
                        .font(.caption.weight(.bold))
                        .foregroundStyle(.secondary)

                    Button {
                        model.navigateToCandidate(candidateId: candidateId)
                    } label: {
                        Label("Review in Candidates", systemImage: VacuaSymbols.candidates)
                            .font(.caption)
                    }
                    .buttonStyle(.bordered)
                }
            }

            Spacer(minLength: 16)
        }
    }

    private func inspectorRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.caption)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.caption.weight(.medium))
        }
    }
}
