import Foundation

/// Represents the loading lifecycle of an asynchronous, lazily-fetched domain feature.
public enum LoadState<Value: Sendable>: Sendable {
    case idle
    case loading(previous: Value?)
    case loaded(Value)
    case failed(message: String, previous: Value?)

    /// Returns the current value if available (either freshly loaded or preserved from a previous load).
    public var value: Value? {
        switch self {
        case .idle: return nil
        case .loading(let prev): return prev
        case .loaded(let val): return val
        case .failed(_, let prev): return prev
        }
    }

    /// Indicates whether a network or engine fetch is currently in flight.
    public var isLoading: Bool {
        if case .loading = self { return true }
        return false
    }

    /// Indicates whether the feature has never been requested.
    public var isIdle: Bool {
        if case .idle = self { return true }
        return false
    }

    /// Returns the error message if the last fetch failed.
    public var errorMessage: String? {
        if case .failed(let msg, _) = self { return msg }
        return nil
    }
}

extension LoadState: Equatable where Value: Equatable {}
