import Foundation

/// Utilities for parsing RFC3339 / ISO8601 date strings and presenting human-readable localized dates.
public enum VacuaDateFormatter: Sendable {
    public static func parse(_ dateString: String) -> Date? {
        let withFraction = ISO8601DateFormatter()
        withFraction.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = withFraction.date(from: dateString) {
            return date
        }

        let standard = ISO8601DateFormatter()
        standard.formatOptions = [.withInternetDateTime]
        return standard.date(from: dateString)
    }

    public static func formatDisplay(_ dateString: String) -> String {
        guard let date = parse(dateString) else {
            return dateString
        }
        return date.formatted(date: .abbreviated, time: .shortened)
    }
}
