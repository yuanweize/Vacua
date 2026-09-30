import SwiftUI
import AppKit
import VacuaClient

public struct SettingsView: View {
    @Bindable var model: AppModel
    
    public init(model: AppModel) {
        self.model = model
    }
    
    public var body: some View {
        Form {
            Section("Inspection Root Folder") {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Vacua only inspects filesystem trees within allowed roots.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    
                    HStack {
                        Text(model.supervisor.currentRootPath)
                            .font(.system(.body, design: .monospaced))
                            .lineLimit(1)
                            .truncationMode(.middle)
                            .textSelection(.enabled)
                        
                        Spacer()
                        
                        Button("Choose Folder…") {
                            chooseFolder()
                        }
                    }
                    .padding(8)
                    .background(Color.secondary.opacity(0.08))
                    .clipShape(RoundedRectangle(cornerRadius: 6))
                }
            }
            
            Section("Engine Status & Safety Boundaries") {
                VStack(alignment: .leading, spacing: 10) {
                    statusRow(label: "Engine State", value: model.engineState.statusTitle)
                    
                    if case .ready(let caps) = model.engineState {
                        statusRow(label: "Server Version", value: caps.server_version)
                        statusRow(label: "Protocol Generation", value: caps.mcp_protocol_generation)
                        statusRow(label: "Mutation Authority", value: caps.mutation_authority ? "ENABLED (VIOLATION)" : "DISABLED (Verified Safe)")
                        statusRow(label: "Executor Linked", value: caps.executor_linked ? "YES (VIOLATION)" : "NO (Verified Decoupled)")
                        statusRow(label: "Path Disclosure Mode", value: caps.path_disclosure_mode)
                    }
                    
                    Button("Restart Engine Subprocess") {
                        Task {
                            await model.restartEngine()
                        }
                    }
                    .padding(.top, 4)
                }
            }
            
            Section("Build Identity & Provenance") {
                VStack(alignment: .leading, spacing: 10) {
                    statusRow(label: "App Version", value: appVersionWithCommit)
                    statusRow(label: "Source Commit", value: appGitCommit)
                }
            }

            Section("macOS Storage Access Truth") {
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: "lock.shield")
                            .foregroundStyle(.blue)
                        Text("Permission & Transparency Reality")
                            .font(.subheadline.weight(.semibold))
                    }
                    Text("Vacua runs with standard user privileges. Access to protected areas (e.g. ~/Library/Mail, Safari data) requires system Full Disk Access granted in macOS System Settings. Vacua will never spoof or claim Full Disk Access without actual OS verification.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    
                    Button("Open Privacy & Security Settings…") {
                        if let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") {
                            NSWorkspace.shared.open(url)
                        }
                    }
                    .font(.caption)
                    .padding(.top, 4)
                }
            }
        }
        .formStyle(.grouped)
        .navigationTitle("Settings")
    }
    
    private func statusRow(label: String, value: String) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.system(.subheadline, design: .monospaced))
        }
    }
    
    private func chooseFolder() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "Select Root"
        
        if panel.runModal() == .OK, let selectedURL = panel.url {
            Task {
                await model.updateRoot(path: selectedURL.path)
            }
        }
    }

    private var appVersion: String {
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.7.0"
    }

    private var appGitCommit: String {
        Bundle.main.infoDictionary?["VacuaGitCommit"] as? String ?? "development"
    }

    private var appVersionWithCommit: String {
        let shortCommit = appGitCommit.count >= 7 ? String(appGitCommit.prefix(7)) : appGitCommit
        return "\(appVersion) (\(shortCommit))"
    }
}
