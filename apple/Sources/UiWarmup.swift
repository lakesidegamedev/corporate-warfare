import Foundation

/// Reads the office's art and builds its bitmaps on a background queue after launch, so the first
/// visit to the CEO Office does not decode the room, the cast and every portrait on the main thread.
/// The bitmaps go into the same cache the views draw from.
enum UiWarmup {
    private static let lock = NSLock()
    private static var started = false

    /// Starts once. Safe to call from any thread.
    static func start() {
        lock.lock()
        let first = !started
        started = true
        lock.unlock()
        guard first else { return }
        DispatchQueue.global(qos: .utility).async { run() }
    }

    /// Decodes the art files and fills the bitmap cache. Public for tests.
    static func run() {
        _ = PixelIcons.all
        _ = CastPortraits.all
        if let art = OfficeRoomArt.file {
            _ = PixelIcons.bitmap(key: "office|room", rows: art.room.rows, palette: art.room.palette)
            _ = PixelIcons.bitmap(key: "office|front", rows: art.front.rows, palette: art.front.palette)
            for (key, sprite) in art.sprites {
                _ = PixelIcons.bitmap(key: "office|\(key)", rows: sprite.rows, palette: sprite.palette)
            }
        }
        for (person, portrait) in CastPortraits.office {
            _ = PixelIcons.bitmap(key: "portrait|\(person)|office", rows: portrait.rows, palette: portrait.palette)
        }
    }
}
