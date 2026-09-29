import Testing
import Foundation
@testable import VacuaClient

@Suite("Transport & Lifecycle Tests")
struct TransportTests {
    @Test("Engine not found error produces descriptive localized string")
    func testEngineNotFoundError() {
        let err = VacuaClientError.engineNotFound(path: "/non/existent/vacua-mcp")
        #expect(err.errorDescription?.contains("/non/existent/vacua-mcp") == true)
        #expect(err.recoverySuggestion != nil)
    }

    @Test("Server error code preservation")
    func testServerErrorCodePreservation() {
        let err = VacuaClientError.serverError(code: "VACUA_POLICY_DENIED", message: "Path is not allowed")
        #expect(err.errorDescription?.contains("VACUA_POLICY_DENIED") == true)
        #expect(err.recoverySuggestion?.contains("authorized root") == true)
    }

    @Test("Unsupported schema error message")
    func testUnsupportedSchemaError() {
        let err = VacuaClientError.unsupportedSchema(expected: "vacua.mcp.storage-summary.v1", actual: "unknown.v9")
        #expect(err.errorDescription?.contains("vacua.mcp.storage-summary.v1") == true)
        #expect(err.errorDescription?.contains("unknown.v9") == true)
    }

    @Test("Byte count formatter helpers")
    func testFormatters() {
        let formatted = Formatters.formatBytes(1024 * 1024 * 1024)
        #expect(formatted.contains("GB") || formatted.contains("1"))

        let deltaPos = Formatters.formatBytesSigned(1048576)
        #expect(deltaPos.hasPrefix("+"))

        let deltaNeg = Formatters.formatBytesSigned(-1048576)
        #expect(deltaNeg.hasPrefix("-"))
    }
}
