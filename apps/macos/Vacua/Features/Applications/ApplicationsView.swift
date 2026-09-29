import SwiftUI
import VacuaClient

public struct ApplicationsView: View {
    @Bindable var model: AppModel
    @State private var selectedAppId: String?
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var filteredApplications: [ApplicationSummaryV1] {
        switch model.appFilter {
        case .all:
            return model.applications
        case .installed:
            return model.applications.filter { $0.installed }
        case .potentialResidual:
            return model.applications.filter { !$0.installed }
        }
    }
    
    public var body: some View {
        HSplitView {
            VStack(spacing: 0) {
                // Filter picker
                HStack {
                    Picker("Filter", selection: $model.appFilter) {
                        ForEach(ApplicationFilterMode.allCases) { mode in
                            Text(mode.rawValue).tag(mode)
                        }
                    }
                    .pickerStyle(.segmented)
                    .frame(maxWidth: 320)
                    
                    Spacer()
                    
                    Text("\(filteredApplications.count) applications")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                
                Divider()
                
                if filteredApplications.isEmpty && !model.isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "app.badge")
                            .font(.largeTitle)
                            .foregroundStyle(.tertiary)
                        Text("No Applications Found")
                            .font(.headline)
                        Text("No matching applications or residual metadata in scope.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                } else {
                    List(filteredApplications, selection: $selectedAppId) { app in
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
                }
            }
            .frame(minWidth: 360)
            
            // App Artifacts Inspector
            if let app = currentSelectedApp {
                appInspector(app: app, detail: model.selectedAppDetail)
                    .frame(minWidth: 320)
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "sidebar.right")
                        .font(.largeTitle)
                        .foregroundStyle(.tertiary)
                    Text("Select an application to view on-disk artifacts and storage impact")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(Color(NSColor.controlBackgroundColor))
            }
        }
        .navigationTitle("Applications & Residue")
    }
    
    private var currentSelectedApp: ApplicationSummaryV1? {
        model.applications.first(where: { $0.id == selectedAppId })
    }
    
    @ViewBuilder
    private func appInspector(app: ApplicationSummaryV1, detail: ApplicationDetailV1?) -> some View {
        VStack(alignment: .leading, spacing: 16) {
            // Header
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(app.app_name)
                        .font(.title2.weight(.bold))
                    Spacer()
                    if let detail = detail {
                        Text(detail.estimated_reclaim_bytes.formatted(.byteCount(style: .file)))
                            .font(.title3.weight(.bold))
                            .foregroundStyle(.blue)
                    }
                }
                if let displayPath = app.display_path {
                    Text(displayPath)
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
            
            Divider()
            
            // Artifacts
            if let detail = detail {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Associated Artifacts (\(detail.artifacts.count))")
                        .font(.subheadline.weight(.medium))
                    
                    List(detail.artifacts) { artifact in
                        VStack(alignment: .leading, spacing: 2) {
                            HStack {
                                Text(artifact.kind.capitalized)
                                    .font(.caption2.weight(.semibold))
                                    .foregroundStyle(.secondary)
                                Spacer()
                                Text(artifact.allocated_bytes.formatted(.byteCount(style: .file)))
                                    .font(.caption.weight(.medium))
                            }
                            Text(artifact.display_path)
                                .font(.system(.caption2, design: .monospaced))
                                .lineLimit(2)
                                .textSelection(.enabled)
                        }
                        .padding(.vertical, 2)
                    }
                    .listStyle(.bordered(alternatesRowBackgrounds: true))
                }
            }
            
            Spacer()
        }
        .padding(16)
        .background(Color(NSColor.controlBackgroundColor))
    }
}
