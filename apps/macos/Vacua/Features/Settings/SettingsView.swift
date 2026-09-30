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
            Section("Storage Root") {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Vacua only inspects filesystem trees within configured allowed roots.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    
                    HStack {
                        Image(systemName: VacuaSymbols.activeRoot)
                            .foregroundStyle(.tint)
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
                    .clipShape(RoundedRectangle(cornerRadius: VacuaMetrics.cornerRadiusSmall))
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
            
            Section("Privacy & Data Boundary") {
                VStack(alignment: .leading, spacing: 6) {
                    HStack {
                        Image(systemName: "lock.shield.fill")
                            .foregroundStyle(.green)
                        Text("100% Local Processing & Privacy First")
                            .font(.subheadline.weight(.semibold))
                    }
                    Text("• No cloud uploads, telemetry, or remote analytics of any kind.\n• Storage Map analysis operates strictly on local metadata.\n• Duplicate inspection uses staged local hashes; raw file bytes never leave your device.\n• Standard user permissions; system Full Disk Access is respected if granted.")
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

            Section("About Vacua") {
                HStack(spacing: 16) {
                    if let appIcon = NSImage(named: NSImage.applicationIconName) {
                        Image(nsImage: appIcon)
                            .resizable()
                            .frame(width: 56, height: 56)
                    }
                    
                    VStack(alignment: .leading, spacing: 3) {
                        Text("Vacua")
                            .font(.title3.weight(.bold))
                        Text("Version \(appVersionWithCommit)")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        Text("Licensed under Apache-2.0")
                            .font(.caption2)
                            .foregroundStyle(.tertiary)
                    }
                    
                    Spacer()
                    
                    Link("GitHub", destination: URL(string: "https://github.com/yuanweize/vacua")!)
                        .font(.caption)
                }
                .padding(.vertical, 4)
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
        Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.7.1"
    }

    private var appGitCommit: String {
        Bundle.main.infoDictionary?["VacuaGitCommit"] as? String ?? "development"
    }

    private var appVersionWithCommit: String {
        let shortCommit = appGitCommit.count >= 7 ? String(appGitCommit.prefix(7)) : appGitCommit
        return "\(appVersion) (\(shortCommit))"
    }
}
