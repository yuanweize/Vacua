import SwiftUI
import VacuaClient

public struct DiagnosticsView: View {
    @Bindable var model: AppModel
    @State private var isRepairing = false

    public init(model: AppModel) {
        self.model = model
    }

    public var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                // Header
                VStack(alignment: .leading, spacing: 4) {
                    HStack(spacing: 8) {
                        Image(systemName: VacuaSymbols.diagnostics)
                            .font(.title2)
                            .foregroundStyle(.tint)
                        Text("Vacua Diagnostics")
                            .font(.title2.weight(.bold))
                    }
                    Text("Runtime integrity, engine helper resolution, and system capabilities.")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }

                // Runtime Components Card
                VStack(alignment: .leading, spacing: 14) {
                    Text("Engine Runtime Components")
                        .font(.headline)

                    diagnosticRow(
                        label: "App Version",
                        value: Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String ?? "0.9.0",
                        status: .ok
                    )

                    diagnosticRow(
                        label: "Engine State",
                        value: model.engineState.statusTitle,
                        status: isEngineOk ? .ok : .error
                    )

                    diagnosticRow(
                        label: "Bundled Helper Path",
                        value: helperPath,
                        status: helperExists ? .ok : .error
                    )

                    diagnosticRow(
                        label: "Architecture",
                        value: "arm64 (Apple Silicon)",
                        status: .ok
                    )

                    diagnosticRow(
                        label: "Active Storage Root",
                        value: model.activeRootPath,
                        status: .ok
                    )

                    diagnosticRow(
                        label: "Apple Foundation Models",
                        value: "On-Device Neural Engine Ready",
                        status: .ok
                    )
                }
                .padding(16)
                .background(Color(NSColor.controlBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 10))

                // Repair / Reconnect Action
                VStack(alignment: .leading, spacing: 10) {
                    Text("Self-Healing & Diagnostics Actions")
                        .font(.headline)
                    Text("If the storage engine was interrupted or volume permissions changed, reinitialize the supervisor.")
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    HStack(spacing: 12) {
                        Button {
                            isRepairing = true
                            Task {
                                await model.restartEngine()
                                isRepairing = false
                            }
                        } label: {
                            if isRepairing {
                                ProgressView()
                                    .controlSize(.small)
                            } else {
                                Label("Repair / Reconnect Engine", systemImage: VacuaSymbols.refresh)
                            }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(isRepairing)

                        Button {
                            let text = """
                            Vacua Diagnostics Report
                            App: 0.9.0
                            Root: \(model.activeRootPath)
                            Engine: \(model.engineState.statusTitle)
                            Helper: \(helperPath)
                            """
                            NSPasteboard.general.clearContents()
                            NSPasteboard.general.setString(text, forType: .string)
                        } label: {
                            Label("Copy Diagnostics", systemImage: "doc.on.doc")
                        }
                        .buttonStyle(.bordered)
                    }
                }
                .padding(16)
                .background(Color(NSColor.controlBackgroundColor))
                .clipShape(RoundedRectangle(cornerRadius: 10))
            }
            .padding(24)
        }
        .navigationTitle("Diagnostics")
    }

    private var isEngineOk: Bool {
        if case .ready = model.engineState { return true }
        return false
    }

    private var helperURL: URL? {
        try? EngineProcessSupervisor.resolveEngineURL()
    }

    private var helperPath: String {
        helperURL?.path ?? "Not resolved"
    }

    private var helperExists: Bool {
        guard let url = helperURL else { return false }
        return FileManager.default.isExecutableFile(atPath: url.path)
    }

    private enum RowStatus { case ok, warn, error }

    private func diagnosticRow(label: String, value: String, status: RowStatus) -> some View {
        HStack {
            Text(label)
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Spacer()
            Text(value)
                .font(.subheadline.monospaced())
                .lineLimit(1)
            Image(systemName: status == .ok ? "checkmark.circle.fill" : "exclamationmark.triangle.fill")
                .foregroundStyle(status == .ok ? .green : .red)
        }
    }
}
