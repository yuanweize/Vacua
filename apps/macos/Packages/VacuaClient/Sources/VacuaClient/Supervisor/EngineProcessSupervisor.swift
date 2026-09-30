import Foundation
import os

/// High-level engine supervisor managing child process lifecycle, restarts, and root transitions.
@MainActor
public final class EngineProcessSupervisor: ObservableObject {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "EngineSupervisor")

    @Published public private(set) var state: EngineState = .starting
    @Published public private(set) var currentRootPath: String

    private var transport: MCPStdioTransport?
    private var client: MCPVacuaEngineClient?
    private let helperOverrideURL: URL?

    public init(initialRootPath: String = NSHomeDirectory(), helperOverrideURL: URL? = nil) {
        self.currentRootPath = initialRootPath
        self.helperOverrideURL = helperOverrideURL
    }

    /// Resolves the trusted engine executable URL.
    /// In Production, only bundled Contents/Helpers/vacua-mcp is trusted.
    public static func resolveEngineURL(override: URL? = nil) throws -> URL {
        if let override = override {
            guard FileManager.default.isExecutableFile(atPath: override.path) else {
                throw VacuaClientError.engineNotFound(path: override.path)
            }
            return override
        }

        // 1. Primary: Look in App Bundle Contents/Helpers/vacua-mcp
        if let bundleHelpersURL = Bundle.main.resourceURL?.deletingLastPathComponent().appendingPathComponent("Helpers") {
            let bundledMCP = bundleHelpersURL.appendingPathComponent("vacua-mcp")
            if FileManager.default.isExecutableFile(atPath: bundledMCP.path) {
                return bundledMCP
            }
        }

        let fallbackURL = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/vacua-mcp")
        if FileManager.default.isExecutableFile(atPath: fallbackURL.path) {
            return fallbackURL
        }

        #if DEBUG
        // 2. In Debug/development, check environment variable
        if let envPath = ProcessInfo.processInfo.environment["VACUA_MCP_PATH"], !envPath.isEmpty {
            let envURL = URL(fileURLWithPath: envPath)
            if FileManager.default.isExecutableFile(atPath: envURL.path) {
                return envURL
            }
        }

        // 3. In Debug/development, fall back to locally installed Homebrew binary
        let devCandidates = [
            "/opt/homebrew/bin/vacua-mcp",
            "/usr/local/bin/vacua-mcp"
        ]
        for candidate in devCandidates {
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return URL(fileURLWithPath: candidate)
            }
        }
        #endif

        throw VacuaClientError.engineNotFound(path: fallbackURL.path)
    }

    /// Starts or restarts the engine for the current root folder.
    public func start() async {
        state = .starting

        // Clean up previous instance
        if let oldTransport = transport {
            await oldTransport.shutdown()
            transport = nil
            client = nil
        }

        let engineURL: URL
        do {
            engineURL = try Self.resolveEngineURL(override: helperOverrideURL)
        } catch let err as VacuaClientError {
            state = .unavailable(error: err)
            return
        } catch {
            state = .unavailable(error: .engineLaunchFailed(reason: error.localizedDescription))
            return
        }

        let args = [
            "--allow-root", currentRootPath,
            "--path-disclosure", "home-relative"
        ]

        let newTransport = MCPStdioTransport(executableURL: engineURL, arguments: args)
        do {
            try await newTransport.start()
        } catch let err as VacuaClientError {
            state = .unavailable(error: err)
            return
        } catch {
            state = .unavailable(error: .engineLaunchFailed(reason: error.localizedDescription))
            return
        }

        let newClient = MCPVacuaEngineClient(transport: newTransport)
        self.transport = newTransport
        self.client = newClient

        do {
            let caps = try await newClient.initialize()
            state = .ready(capabilities: caps)
            logger.info("Vacua engine ready for root: \(self.currentRootPath, privacy: .private)")
        } catch let err as VacuaClientError {
            state = .unavailable(error: err)
        } catch {
            state = .unavailable(error: .protocolViolation(message: error.localizedDescription))
        }
    }

    /// Changes the active root folder, restarting the engine with updated permissions.
    public func updateRoot(newPath: String) async {
        guard newPath != currentRootPath else { return }
        currentRootPath = newPath
        await start()
    }

    /// Returns the active typed engine client, or throws if unavailable.
    public func getClient() throws -> VacuaEngineClient {
        guard case .ready = state, let client = client else {
            throw VacuaClientError.engineLaunchFailed(reason: "Storage engine is not currently running.")
        }
        return client
    }
}

/// Represents the high-level operational state of the storage engine.
public enum EngineState: Sendable, Equatable {
    case starting
    case ready(capabilities: ServerCapabilitiesV1)
    case unavailable(error: VacuaClientError)
    case incompatible(reason: String)

    public var isReady: Bool {
        if case .ready = self { return true }
        return false
    }

    public var statusTitle: String {
        switch self {
        case .starting: return "Starting Engine…"
        case .ready: return "Engine Ready"
        case .unavailable: return "Engine Unavailable"
        case .incompatible: return "Engine Incompatible"
        }
    }
}
