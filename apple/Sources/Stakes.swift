import SwiftUI

/// Holdings of 5 percent or more in a company, as filed. A rival building toward a bid shows its
/// stake over time as a small line, from the week the holding became public.
struct DisclosedStakes: View {
    let stakes: [StakeDto]
    var onOpenFirm: (UInt64) -> Void = { _ in }

    var body: some View {
        if !stakes.isEmpty {
            Eyebrow(text: "Disclosed stakes")
            ForEach(Array(stakes.enumerated()), id: \.offset) { _, stake in
                VStack(alignment: .leading, spacing: 3) {
                    HStack(spacing: 8) {
                        FirmLogo(name: stake.holderName, size: 20)
                        Text(stake.holderName).font(.caption).foregroundStyle(Theme.primaryText).lineLimit(1)
                        Spacer()
                        Text(Format.percentBps(stake.bps)).font(.caption.weight(.semibold))
                            .foregroundStyle(stake.bps >= 2_500 ? Theme.warning : Theme.gold)
                    }
                    if let since = stake.since {
                        let from = stake.history.first.map { ", from \(Format.percentBps($0.bps))" } ?? ""
                        Text("Public since \(ResultBuckets.weekLabel(since))\(from)")
                            .font(.caption2).foregroundStyle(Theme.secondary)
                    }
                    if stake.history.count >= 2 {
                        Sparkline(values: stake.history.map { Int64($0.bps) }, tint: stake.campaign ? Theme.warning : Theme.secondary)
                            .frame(height: 34)
                    }
                }
                .contentShape(Rectangle())
                .onTapGesture { onOpenFirm(stake.holder) }
            }
        }
    }
}

/// One line on what a rival is likely to do next, from its filings and the public record only.
struct LikelyNextMove: View {
    let text: String?

    var body: some View {
        HStack(alignment: .top, spacing: 6) {
            PixelIcon("target", size: 12).foregroundStyle(Theme.gold)
            Text("Likely next move: " + (text ?? "nothing signalled. It has filed no stake and made no open bid."))
                .font(.caption).foregroundStyle(text != nil ? Theme.primaryText : Theme.secondary)
        }
    }
}
