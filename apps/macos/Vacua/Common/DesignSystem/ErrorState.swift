import SwiftUI

/// Privacy-safe, actionable error presentation view.
/// Avoids leaking raw system paths while providing clear diagnostics and recovery options.
public struct VacuaErrorState: View {
    public let title: String
    public let message: String
    public let diagnosticCode: String?
    public let retryAction: (() -> Void)?

    public init(
        title: String = "Operation Failed",
        message: String,
        diagnosticCode: String? = nil,
        retryAction: (() -> Void)? = nil
    ) {
        self.title = title
        self.message = message
        self.diagnosticCode = diagnosticCode
        self.retryAction = retryAction
    }

    public var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 40))
                .foregroundStyle(.orange)

            VStack(spacing: 4) {
                Text(title)
                    .font(.headline)
                    .foregroundStyle(.primary)

                Text(message)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 420)

                if let diagnosticCode {
                    Text("Code: \(diagnosticCode)")
                        .font(.caption.monospaced())
                        .foregroundStyle(.tertiary)
                        .padding(.top, 2)
                }
            }

            if let retryAction {
                Button(action: retryAction) {
                    Label("Try Again", systemImage: VacuaSymbols.refresh)
                        .padding(.horizontal, 6)
                }
                .buttonStyle(.bordered)
                .controlSize(.regular)
                .padding(.top, 4)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .padding(VacuaMetrics.pagePadding)
    }
}
