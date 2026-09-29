import Foundation
import os

/// Low-level JSON-RPC 2.0 stdio transport managing a child process.
public actor MCPStdioTransport {
    private let logger = Logger(subsystem: "io.github.yuanweize.vacua", category: "MCPTransport")
    private let executableURL: URL
    private let arguments: [String]
    private let environment: [String: String]
    private let maxResponseBytes: Int = 16 * 1024 * 1024 // 16 MiB bound

    private var process: Process?
    private var stdinPipe: Pipe?
    private var stdoutPipe: Pipe?
    private var stderrPipe: Pipe?

    private var nextRequestId: Int64 = 1
    private var pendingRequests: [Int64: CheckedContinuation<Data, Error>] = [:]
    private var stdoutBuffer: Data = Data()
    private var stderrBuffer: String = ""
    private var isTerminating: Bool = false

    public init(executableURL: URL, arguments: [String] = [], environment: [String: String] = [:]) {
        self.executableURL = executableURL
        self.arguments = arguments
        self.environment = environment
    }

    deinit {
        if let proc = process, proc.isRunning {
            proc.terminate()
        }
    }

    /// Launches the child process and establishes stdio pipes.
    public func start() throws {
        guard process == nil else { return }

        guard FileManager.default.isExecutableFile(atPath: executableURL.path) else {
            throw VacuaClientError.engineNotFound(path: executableURL.path)
        }

        let proc = Process()
        proc.executableURL = executableURL
        proc.arguments = arguments

        var mergedEnv = ProcessInfo.processInfo.environment
        for (k, v) in environment {
            mergedEnv[k] = v
        }
        proc.environment = mergedEnv

        let inPipe = Pipe()
        let outPipe = Pipe()
        let errPipe = Pipe()

        proc.standardInput = inPipe
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        self.process = proc
        self.stdinPipe = inPipe
        self.stdoutPipe = outPipe
        self.stderrPipe = errPipe

        // Set non-blocking readability handlers BEFORE running
        outPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            if data.isEmpty { return }
            Task { [weak self] in
                await self?.processStdoutChunk(data)
            }
        }

        errPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            if data.isEmpty { return }
            Task { [weak self] in
                await self?.processStderrChunk(data)
            }
        }

        // Termination monitor
        proc.terminationHandler = { [weak self] p in
            Task { [weak self] in
                await self?.handleTermination(exitCode: p.terminationStatus)
            }
        }

        do {
            try proc.run()
        } catch {
            throw VacuaClientError.engineLaunchFailed(reason: error.localizedDescription)
        }
    }

    /// Sends a JSON-RPC 2.0 request and waits for the matching response data.
    public func sendRequest(method: String, params: [String: Any]? = nil, timeoutSeconds: Double = 30.0) async throws -> Data {
        guard let proc = process, proc.isRunning, let inHandle = stdinPipe?.fileHandleForWriting else {
            throw VacuaClientError.engineTerminated(exitCode: -1, stderr: stderrBuffer)
        }

        let requestId = nextRequestId
        nextRequestId += 1

        var rpc: [String: Any] = [
            "jsonrpc": "2.0",
            "id": requestId,
            "method": method
        ]
        if let params = params {
            rpc["params"] = params
        }

        let data: Data
        do {
            data = try JSONSerialization.data(withJSONObject: rpc, options: [])
        } catch {
            throw VacuaClientError.decodingFailure(details: "Failed to serialize request: \(error)")
        }

        var payload = data
        payload.append(0x0A) // newline delimiter

        do {
            try inHandle.write(contentsOf: payload)
        } catch {
            throw VacuaClientError.engineTerminated(exitCode: -1, stderr: "Failed to write to stdin: \(error)")
        }

        return try await withCheckedThrowingContinuation { continuation in
            pendingRequests[requestId] = continuation

            Task { [weak self] in
                try? await Task.sleep(nanoseconds: UInt64(timeoutSeconds * 1_000_000_000))
                await self?.handleRequestTimeout(for: requestId, method: method, timeoutSeconds: timeoutSeconds)
            }
        }
    }

    private func handleRequestTimeout(for id: Int64, method: String, timeoutSeconds: Double) {
        if let cont = pendingRequests.removeValue(forKey: id) {
            cont.resume(throwing: VacuaClientError.requestTimedOut(method: method, timeoutSeconds: timeoutSeconds))
        }
    }

    /// Sends a JSON-RPC 2.0 notification (fire-and-forget, no response expected).
    public func sendNotification(method: String, params: [String: Any]? = nil) throws {
        guard let proc = process, proc.isRunning, let inHandle = stdinPipe?.fileHandleForWriting else {
            throw VacuaClientError.engineTerminated(exitCode: -1, stderr: stderrBuffer)
        }

        var rpc: [String: Any] = [
            "jsonrpc": "2.0",
            "method": method
        ]
        if let params = params {
            rpc["params"] = params
        }

        let data = try JSONSerialization.data(withJSONObject: rpc, options: [])
        var payload = data
        payload.append(0x0A)
        try inHandle.write(contentsOf: payload)
    }

    /// Cleanly shuts down the child process.
    public func shutdown() async {
        isTerminating = true

        if let outPipe = stdoutPipe {
            outPipe.fileHandleForReading.readabilityHandler = nil
            try? outPipe.fileHandleForReading.close()
        }
        if let errPipe = stderrPipe {
            errPipe.fileHandleForReading.readabilityHandler = nil
            try? errPipe.fileHandleForReading.close()
        }
        if let inPipe = stdinPipe {
            try? inPipe.fileHandleForWriting.close()
        }

        if let proc = process, proc.isRunning {
            proc.terminate()
            for _ in 0..<10 {
                if !proc.isRunning { break }
                try? await Task.sleep(nanoseconds: 20_000_000)
            }
        }
        process = nil

        let err = VacuaClientError.engineTerminated(exitCode: -1, stderr: "Process shut down")
        for (_, cont) in pendingRequests {
            cont.resume(throwing: err)
        }
        pendingRequests.removeAll()
    }

    // MARK: - Private Actor Methods

    private func processStdoutChunk(_ chunk: Data) {
        if stdoutBuffer.count + chunk.count > maxResponseBytes {
            logger.error("Response exceeded max size limit")
            let err = VacuaClientError.responseTooLarge(bytes: stdoutBuffer.count + chunk.count, maxBytes: maxResponseBytes)
            for (_, cont) in pendingRequests {
                cont.resume(throwing: err)
            }
            pendingRequests.removeAll()
            return
        }
        stdoutBuffer.append(chunk)

        while let newlineIndex = stdoutBuffer.firstIndex(of: 0x0A) {
            let lineData = stdoutBuffer.subdata(in: 0..<newlineIndex)
            stdoutBuffer.removeSubrange(0...newlineIndex)

            if !lineData.isEmpty {
                handleIncomingLine(lineData)
            }
        }
    }

    private func processStderrChunk(_ chunk: Data) {
        if let str = String(data: chunk, encoding: .utf8) {
            stderrBuffer.append(str)
            if stderrBuffer.count > 10000 {
                stderrBuffer = String(stderrBuffer.suffix(8000))
            }
            logger.debug("engine stderr: \(str, privacy: .private)")
        }
    }

    private func handleIncomingLine(_ data: Data) {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            logger.error("Non-JSON stdout line received")
            return
        }

        let rawId = (json["id"] as? NSNumber)?.int64Value ?? (json["id"] as? Int64)
        if let id = rawId {
            if let cont = pendingRequests.removeValue(forKey: id) {
                if let errorObj = json["error"] as? [String: Any] {
                    let codeVal = (errorObj["data"] as? [String: Any])?["code"] as? String
                        ?? errorObj["code"].map { String(describing: $0) }
                        ?? "UNKNOWN"
                    let msg = errorObj["message"] as? String ?? "Unknown error"
                    cont.resume(throwing: VacuaClientError.serverError(code: codeVal, message: msg))
                } else {
                    cont.resume(returning: data)
                }
            }
        }
    }

    private func handleTermination(exitCode: Int32) {
        guard !isTerminating else { return }
        logger.warning("Child process terminated with code \(exitCode, privacy: .public)")
        let err = VacuaClientError.engineTerminated(exitCode: exitCode, stderr: stderrBuffer)
        for (_, cont) in pendingRequests {
            cont.resume(throwing: err)
        }
        pendingRequests.removeAll()
    }
}
