import Foundation
import SwiftUI

enum Theme {
    static let background = Color(red: 0.961, green: 0.949, blue: 0.918)
    static let surface = Color.white
    static let text = Color(red: 0.090, green: 0.125, blue: 0.165)
    static let secondary = Color(red: 0.322, green: 0.376, blue: 0.439)
    static let navy = Color(red: 0.078, green: 0.145, blue: 0.227)
    static let positive = Color(red: 0.118, green: 0.459, blue: 0.357)
    static let negative = Color(red: 0.631, green: 0.204, blue: 0.204)
    static let warning = Color(red: 0.502, green: 0.345, blue: 0.0)
    static let copper = Color(red: 0.702, green: 0.435, blue: 0.243)

    static func background(for scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.051, green: 0.090, blue: 0.141) : background
    }

    static func surface(for scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.090, green: 0.149, blue: 0.220) : surface
    }

    static func text(for scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.929, green: 0.941, blue: 0.957) : text
    }

    static func secondaryText(for scheme: ColorScheme) -> Color {
        scheme == .dark ? Color(red: 0.686, green: 0.741, blue: 0.804) : secondary
    }
}

enum Format {
    private static let currency: NumberFormatter = {
        let formatter = NumberFormatter()
        formatter.numberStyle = .currency
        formatter.currencyCode = "USD"
        formatter.maximumFractionDigits = 0
        return formatter
    }()

    static func money(_ cents: Int64) -> String {
        let dollars = Double(cents) / 100.0
        return currency.string(from: NSNumber(value: dollars)) ?? "$0"
    }

    static func moneyCompact(_ cents: Int64) -> String {
        let dollars = Double(cents) / 100.0
        let magnitude = abs(dollars)
        if magnitude >= 1_000_000_000 {
            return String(format: "$%.1fB", dollars / 1_000_000_000)
        }
        if magnitude >= 1_000_000 {
            return String(format: "$%.1fM", dollars / 1_000_000)
        }
        if magnitude >= 10_000 {
            return String(format: "$%.0fK", dollars / 1_000)
        }
        return money(cents)
    }

    static func percentBps(_ bps: Int32) -> String {
        String(format: "%.1f%%", Double(bps) / 100.0)
    }

    static func lots(_ lots: Int64) -> String {
        "\(lots.formatted()) lots"
    }

    static func signedMoney(_ cents: Int64) -> String {
        (cents >= 0 ? "+" : "") + money(cents)
    }
}

struct MetricTile: View {
    let title: String
    let value: String
    var note: String? = nil
    var tone: Int = 0 // -1 negative, 0 neutral, 1 positive
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(title)
                .font(.caption)
                .foregroundStyle(Theme.secondaryText(for: scheme))
            Text(value)
                .font(.headline.monospacedDigit())
                .foregroundStyle(toneColor)
                .accessibilityLabel("\(title): \(value)")
            if let note {
                Text(note)
                    .font(.caption2)
                    .foregroundStyle(Theme.secondaryText(for: scheme))
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(Theme.surface(for: scheme))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private var toneColor: Color {
        switch tone {
        case 1: return Theme.positive
        case -1: return Theme.negative
        default: return Theme.text(for: scheme)
        }
    }
}

struct SectionCard<Content: View>: View {
    let title: String
    @ViewBuilder var content: Content
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.headline)
                .foregroundStyle(Theme.text(for: scheme))
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(16)
        .background(Theme.surface(for: scheme))
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

struct EmptyNote: View {
    let text: String
    var body: some View {
        Text(text)
            .font(.body)
            .foregroundStyle(.secondary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}
