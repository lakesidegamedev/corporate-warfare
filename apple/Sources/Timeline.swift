import SwiftUI

/// What the History card shows: the timeline and the weeks the paper has an issue for.
struct TimelineSnapshot: Equatable {
    var timeline: TimelineDto
    var issues: Set<UInt32>

    static func == (a: TimelineSnapshot, b: TimelineSnapshot) -> Bool {
        a.issues == b.issues && a.timeline.mode == b.timeline.mode
            && a.timeline.upcoming.map(\.id) == b.timeline.upcoming.map(\.id)
            && a.timeline.upcoming.map(\.weeksAway) == b.timeline.upcoming.map(\.weeksAway)
            && a.timeline.past.map(\.id) == b.timeline.past.map(\.id)
    }
}

func timelineAboutWeeks(_ weeks: UInt32) -> String {
    weeks == 1 ? "in about 1 week" : "in about \(weeks) weeks"
}

func timelineDate(year: Int32, weekOfYear: UInt32) -> String {
    "week \(weekOfYear), \(year)"
}

/// The History card of the economy panel: announced events still to come, then the ones that
/// happened, newest first. The engine is read when the card is on screen and the week changes,
/// and once only while time runs fast.
struct TimelineSection: View {
    @Environment(AppStore.self) private var store
    @State private var view: TimelineSnapshot?

    private var readKey: String {
        guard let overview = store.overview else { return "none" }
        return store.runSpeed > 1 ? "fast" : "tick-\(overview.tick)"
    }

    var body: some View {
        Group {
            if let view {
                TimelineCard(view: view) { week in
                    store.paperWeek = week
                    store.tutorial.requestedPage = .paper
                }
            }
        }
        .task(id: readKey) {
            let next = await store.read { TimelineSnapshot(timeline: $0.timeline(), issues: Set($0.paperArchive())) }
            if let next, next != view { view = next }
        }
    }
}

struct TimelineCard: View {
    let view: TimelineSnapshot
    let onOpenIssue: (UInt32) -> Void

    var body: some View {
        let t = view.timeline
        VStack(alignment: .leading, spacing: 6) {
            Eyebrow(text: "History")
            Rectangle().fill(Theme.primaryText.opacity(0.9)).frame(height: 3)
            if t.mode == "off" {
                Text("Scenario history is off for this career. No dated events will happen.")
                    .font(.caption).foregroundStyle(Theme.secondary)
            } else {
                let past = t.past.sorted { $0.week > $1.week }
                if t.upcoming.isEmpty && past.isEmpty {
                    Text("Nothing announced yet. Big events are flagged a few weeks ahead.")
                        .font(.caption).foregroundStyle(Theme.secondary)
                }
                ForEach(t.upcoming.sorted { $0.week < $1.week }, id: \.id) { e in
                    entry(e.title, e.summary,
                          "\(timelineDate(year: e.year, weekOfYear: e.weekOfYear)) · \(timelineAboutWeeks(e.weeksAway))",
                          Theme.gold, nil)
                }
                if !past.isEmpty && !t.upcoming.isEmpty {
                    Rectangle().fill(Theme.primaryText.opacity(0.3)).frame(height: 1)
                }
                ForEach(past, id: \.id) { e in
                    entry(e.title, e.summary, timelineDate(year: e.year, weekOfYear: e.weekOfYear), Theme.secondary,
                          view.issues.contains(e.week) ? e.week : nil)
                }
            }
        }
    }

    @ViewBuilder
    private func entry(_ title: String, _ summary: String, _ when: String, _ tint: Color, _ issue: UInt32?) -> some View {
        let body = VStack(alignment: .leading, spacing: 2) {
            Text(when.uppercased()).font(Pixel.font(10)).foregroundStyle(tint)
            Text(title).font(Pixel.font(14)).foregroundStyle(Theme.primaryText)
            Text(summary).font(.caption).foregroundStyle(Theme.secondary)
            if issue != nil {
                Text("Read the paper from that week").font(.caption2).foregroundStyle(Theme.gold)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Theme.ink, in: Pixel.small)
        if let issue {
            Button { onOpenIssue(issue) } label: { body.contentShape(Rectangle()) }.buttonStyle(.plain)
        } else {
            body
        }
    }
}
