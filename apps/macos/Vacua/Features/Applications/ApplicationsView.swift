import SwiftUI
import VacuaClient

public struct ApplicationsView: View {
    @Bindable var model: AppModel
    @State private var selectedAppId: String?
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        HSplitView {
            VStack(spacing: 0) {
                // Filter picker (triggers server-side query with MCP filter)
                HStack {
                    Picker("Filter", selection: $model.appFilter) {
                        ForEach(ApplicationFilterMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(maxWidth: 320)
                    .onChange(of: model.appFilter) { _, _ in
                        selectedAppId = nil
                        model.selectedApplication = nil
                        model.selectedAppDetail = nil
                        Task { await model.loadApplications(force: true) }
                    }
                    
                    Spacer()
                    
                    if model.applicationsState.isLoading && !model.applications.isEmpty {
                        ProgressView()
                            .controlSize(.small)
                    }
                    
                    Text("\(model.applications.count) applications loaded\(model.hasMoreApplications ? " (more available)" : "")")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                
                Divider()
                
                if model.applicationsState.isLoading && model.applications.isEmpty {
                    VStack(spacing: 12) {
                        ProgressView()
                        Text("Gathering application metadata…")
                            .font(.headline)
                        Text("Querying installed applications and potential residual packages.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else if model.applications.isEmpty {
                    VacuaEmptyState(
                        symbol: VacuaSymbols.applications,
                        title: "No Applications Found",
                        message: "No matching application bundles or potential residual metadata found in scope.",
                        actionTitle: "Refresh Applications",
                        action: {
                            Task { await model.loadApplications(force: true) }
                        }
                    )
                } else {
                    VStack(spacing: 0) {
                        List(model.applications, selection: $selectedAppId) { app in
                            HStack(spacing: 12) {
                                Image(systemName: app.installed ? "app.fill" : "app.dashed")
                                    .font(.title2)
                                    .foregroundStyle(app.installed ? .blue : .orange)
                                
                                VStack(alignment: .leading, spacing: 3) {
                                    HStack {
                                        Text(app.app_name)
                                            .font(.headline)
                                        if !app.installed {
                                            Text("Potential Residual")
                                                .font(.caption2.weight(.medium))
                                                .padding(.horizontal, 6)
                                                .padding(.vertical, 2)
                                                .background(Color.orange.opacity(0.15))
                                                .foregroundStyle(.orange)
                                                .clipShape(Capsule())
                                        }
                                    }
                                    
                                    if let displayPath = app.display_path {
                                        Text(displayPath)
                                            .font(.system(.caption, design: .monospaced))
                                            .foregroundStyle(.secondary)
                                    }
                                }
                                
                                Spacer()
                            }
                            .padding(.vertical, 4)
                            .tag(app.id)
                        }
                        .listStyle(.inset(alternatesRowBackgrounds: true))
                        .onChange(of: selectedAppId) { _, newId in
                            if let app = model.applications.first(where: { $0.id == newId }) {
                                Task { await model.selectApplication(app) }
                            }
                        }
                        .onChange(of: model.applications) { _, newApps in
                            if let selId = selectedAppId, !newApps.contains(where: { $0.id == selId }) {
                                selectedAppId = nil
                                model.selectedApplication = nil
                                model.selectedAppDetail = nil
                            }
                        }
                        
                        // Pagination Footer
                        if model.hasMoreApplications {
                            Divider()
                            HStack {
                                Spacer()
                                Button {
                                    Task { await model.loadMoreApplications() }
                                } label: {
                                    if model.isLoadingNextApplicationsPage {
                                        ProgressView()
                                            .controlSize(.small)
                                    } else {
                                        Text("Load More Applications…")
                                            .font(.caption)
                                    }
                                }
                                .buttonStyle(.link)
                                .disabled(model.isLoadingNextApplicationsPage)
                                Spacer()
                            }
                            .padding(.vertical, 8)
                            .background(Color(NSColor.controlBackgroundColor))
                        }
                    }
                }
            }
            .frame(minWidth: 320)
            
            // Detail Inspector
            if let app = currentSelectedApp {
                appInspector(app: app, detail: model.selectedAppDetail)
                    .frame(minWidth: 340)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select an application to inspect related containers and caches")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("Applications & Residue")
        .task {
            // Lazy load when user enters Applications view
            await model.loadApplicationsIfNeeded()
        }
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button {
                    Task { await model.loadApplications(force: true) }
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                .disabled(model.applicationsState.isLoading)
            }
        }
    }
    
    private var currentSelectedApp: ApplicationSummaryV1? {
        model.applications.first(where: { $0.id == selectedAppId })
    }
    
    @ViewBuilder
    private func appInspector(app: ApplicationSummaryV1, detail: ApplicationDetailV1?) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(app.app_name)
                        .font(.title2.weight(.bold))
                    Spacer()
                    if app.installed {
                        Text("Installed")
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color.blue.opacity(0.12))
                            .foregroundStyle(.blue)
                            .clipShape(Capsule())
                    } else {
                        Text("Potential Residual")
                            .font(.caption.weight(.semibold))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(Color.orange.opacity(0.15))
                            .foregroundStyle(.orange)
                            .clipShape(Capsule())
                    }
                }
                
                if let displayPath = app.display_path {
                    Text(displayPath)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)
                }
            }
            
            Divider()
            
            if let detail = detail {
                // Storage footprint card
                VStack(alignment: .leading, spacing: 6) {
                    Text("Associated Storage Footprint")
                        .font(.subheadline.weight(.semibold))
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("Estimated Reclaim")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text(detail.estimated_reclaim_bytes.formatted(.byteCount(style: .file)))
                                .font(.title3.weight(.bold))
                                .foregroundStyle(.purple)
                        }
                        Spacer()
                        VStack(alignment: .trailing, spacing: 2) {
                            Text("Associated Artifacts")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                            Text("\(detail.artifacts.count) files/folders")
                                .font(.title3.weight(.medium))
                        }
                    }
                }
                .padding(12)
                .background(Color.purple.opacity(0.06))
                .clipShape(RoundedRectangle(cornerRadius: 8))
                
                // Associated Artifacts List
                VStack(alignment: .leading, spacing: 8) {
                    Text("Related Storage Locations (\(detail.artifacts.count))")
                        .font(.subheadline.weight(.medium))
                    
                    List(detail.artifacts) { artifact in
                        VStack(alignment: .leading, spacing: 3) {
                            HStack {
                                Text(artifact.kind.uppercased())
                                    .font(.caption2.weight(.bold))
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text(artifact.allocated_bytes.formatted(.byteCount(style: .file)))
                                    .font(.caption)
                                    .foregroundStyle(.purple)
                            }
                            Text(artifact.display_path)
                                .font(.system(.caption2, design: .monospaced))
                                .textSelection(.enabled)
                        }
                        .padding(.vertical, 2)
                    }
                    .listStyle(.bordered(alternatesRowBackgrounds: true))
                }
            } else {
                ProgressView("Loading application details…")
                    .controlSize(.small)
            }
            
            Spacer()
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
    }
}
