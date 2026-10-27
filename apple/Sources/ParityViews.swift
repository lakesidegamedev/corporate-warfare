import Charts
import SwiftUI

/// Panels the desktop app shows in side columns and wide tables, laid out for a phone so every
/// desktop figure has a place on iPhone and iPad.

/// The company's four-weekly revenue and profit bars, as on the desktop CEO Office.
struct FourWeeklyResultsPanel: View {
    @EnvironmentObject private var store: AppStore

    var body: some View {
        let periods = ResultBuckets.make(.month, weeks: [], history: store.history)
        if periods.count > 1 {
            SectionCard(title: "Four-weekly results") {
                Chart {
                    ForEach(periods) { b in
                        BarMark(x: .value("Period", b.label), y: .value("Revenue", Double(b.revenue) / 100))
                            .foregroundStyle(Theme.gold.opacity(0.55))
                            .position(by: .value("Series", "Revenue"))
                        BarMark(x: .value("Period", b.label), y: .value("Net income", Double(b.netIncome) / 100))
                            .foregroundStyle(b.netIncome >= 0 ? Theme.positive : Theme.negative)
                            .position(by: .value("Series", "Net income"))
                    }
                }
                .chartXAxis {
                    AxisMarks(values: .automatic(desiredCount: 4)) { _ in
                        AxisValueLabel().font(.caption2)
                    }
                }
                .chartYAxis {
                    AxisMarks { value in
                        AxisGridLine().foregroundStyle(Theme.stroke)
                        AxisValueLabel {
                            if let dollars = value.as(Double.self) {
                                Text(Format.moneyCompact(Int64(dollars * 100))).font(.caption2)
                            }
                        }
                    }
                }
                .frame(height: 160)
            }
        }
    }
}

/// Shares held outside the companies you control, with stake, value and the quarter's move.
struct ShareholdingsPanel: View {
    @EnvironmentObject private var store: AppStore

    private var held: [StockDto] {
        store.stocks
            .filter { $0.yourShares > 0 }
            .sorted { $0.yourShares * $0.priceCents > $1.yourShares * $1.priceCents }
    }

    var body: some View {
        Panel(padding: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Eyebrow(text: "Your shareholdings · \(held.count)")
                if held.isEmpty {
                    Text("You hold no shares outside your companies. Buy on the Markets screen.")
                        .font(.caption)
                        .foregroundStyle(Theme.secondary)
                }
                ForEach(Array(held.enumerated()), id: \.element.firm) { index, stock in
                    if index > 0 { Divider().overlay(Theme.stroke) }
                    HStack(spacing: 10) {
                        SectorBadge(sector: stock.industry, size: 24)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(stock.name + (stock.controlledByYou ? " ★" : ""))
                                .font(.callout.weight(.medium))
                                .foregroundStyle(Theme.primaryText)
                                .lineLimit(1)
                            Text(detail(stock))
                                .font(.caption2)
                                .foregroundStyle(Theme.secondary)
                                .lineLimit(1)
                        }
                        Spacer(minLength: 6)
                        VStack(alignment: .trailing, spacing: 1) {
                            Text(Format.moneyCompact(stock.yourShares * stock.priceCents))
                                .font(.callout.weight(.semibold).monospacedDigit())
                                .foregroundStyle(Theme.primaryText)
                            Text(Change.text(stock.changeQuarterBps))
                                .font(.caption2.monospacedDigit())
                                .foregroundStyle(Change.tint(stock.changeQuarterBps))
                        }
                    }
                    .accessibilityElement(children: .combine)
                }
            }
        }
    }

    private func detail(_ stock: StockDto) -> String {
        let stake = stock.sharesIssued > 0
            ? String(format: "%.1f%% stake", Double(stock.yourShares) * 100 / Double(stock.sharesIssued))
            : ""
        return "\(stock.yourShares.formatted()) shares · \(stake) · \(Format.price(stock.priceCents))"
    }
}
