import Foundation

/// Structured domain and transport errors for Vacua client operations.
public enum VacuaClientError: Error, LocalizedError, Sendable, Equatable {
    case engineNotFound(path: String)
    case engineLaunchFailed(reason: String)
    case protocolViolation(message: String)
    case requestTimedOut(method: String, timeoutSeconds: Double)
    case engineTerminated(exitCode: Int32, stderr: String)
    case incompatibleVersion(expected: String, actual: String)
    case unsupportedSchema(expected: String, actual: String)
    case serverError(code: String, message: String)
    case decodingFailure(details: String)
    case responseTooLarge(bytes: Int, maxBytes: Int)
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .engineNotFound(let path):
            return "Vacua storage engine executable not found at '\(path)'."
        case .engineLaunchFailed(let reason):
            return "Failed to launch Vacua storage engine: \(reason)"
        case .protocolViolation(let message):
            return "MCP protocol violation: \(message)"
        case .requestTimedOut(let method, let timeoutSeconds):
            return "Engine request '\(method)' timed out after \(timeoutSeconds) seconds."
        case .engineTerminated(let exitCode, let stderr):
            let errSuffix = stderr.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                ? "" : " Details: \(stderr)"
            return "Storage engine terminated unexpectedly with exit code \(exitCode).\(errSuffix)"
        case .incompatibleVersion(let expected, let actual):
            return "Incompatible storage engine version: expected '\(expected)', found '\(actual)'."
        case .unsupportedSchema(let expected, let actual):
            return "Unsupported response schema: expected '\(expected)', got '\(actual)'."
        case .serverError(let code, let message):
            return "[\(code)] \(message)"
        case .decodingFailure(let details):
            return "Failed to parse engine data: \(details)"
        case .responseTooLarge(let bytes, let maxBytes):
            return "Engine response size (\(bytes) bytes) exceeded limit of \(maxBytes) bytes."
        case .cancelled:
            return "Operation cancelled."
        }
    }

    public var recoverySuggestion: String? {
        switch self {
        case .engineNotFound, .engineLaunchFailed:
            return "Ensure Vacua is properly installed with bundled engine helpers, or review Settings."
        case .engineTerminated:
            return "You can restart the storage engine from the Settings tab or toolbar."
        case .serverError(let code, _):
            if code == "VACUA_POLICY_DENIED" {
                return "The requested path is outside the authorized root folder. Choose another root folder in Settings."
            } else if code == "VACUA_STALE_STATE" {
                return "The persistent index schema is outdated. Run 'vacua index refresh' once to upgrade."
            } else if code == "VACUA_PROTECTED" {
                return "This item is protected by system safety invariants and cannot be proposed for cleanup."
            }
            return nil
        default:
            return nil
        }
    }
}
