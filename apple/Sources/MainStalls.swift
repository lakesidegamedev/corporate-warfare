import Foundation

/// Times named pieces of main-thread work, so the max-speed test can say what held the main
/// thread instead of only how long. Off unless a test turns it on; then it costs two clock reads.
/// Not actor-bound, so canvas renderers can use it; a lock guards the list.
enum MainStalls {
    struct Span {
        let label: String
        let start: Double
        let seconds: Double
    }

    static var enabled = false
    private static var spans: [Span] = []
    private static let lock = NSLock()

    static func time<T>(_ label: String, _ body: () throws -> T) rethrows -> T {
        guard enabled else { return try body() }
        let start = Date.timeIntervalSinceReferenceDate
        defer {
            let seconds = Date.timeIntervalSinceReferenceDate - start
            if seconds >= 0.005 {
                lock.lock()
                spans.append(Span(label: label, start: start, seconds: seconds))
                lock.unlock()
            }
        }
        return try body()
    }

    static func reset() {
        lock.lock()
        spans.removeAll()
        lock.unlock()
    }

    private static func all() -> [Span] {
        lock.lock()
        defer { lock.unlock() }
        return spans
    }

    /// The named work that ran inside `from ... to`, longest first.
    static func inside(from: Double, to: Double) -> [Span] {
        all().filter { $0.start >= from - 0.001 && $0.start <= to }.sorted { $0.seconds > $1.seconds }
    }

    /// The worst and total time per label.
    static func summary() -> String {
        var worst: [String: Double] = [:]
        var total: [String: Double] = [:]
        var count: [String: Int] = [:]
        for span in all() {
            worst[span.label] = max(worst[span.label] ?? 0, span.seconds)
            total[span.label, default: 0] += span.seconds
            count[span.label, default: 0] += 1
        }
        return worst.keys.sorted().map { label in
            "\(label) n=\(count[label] ?? 0) worst=\(Int((worst[label] ?? 0) * 1000))ms total=\(Int((total[label] ?? 0) * 1000))ms"
        }.joined(separator: "; ")
    }
}
