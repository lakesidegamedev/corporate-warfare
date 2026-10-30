import QuartzCore
import SwiftUI
import UIKit

// The new GPU map on iPhone, iPad and Mac (Catalyst), an opt-in preview. The engine draws the
// stylized terrain, roads and buildings with Metal straight into a CAMetalLayer owned by a UIView
// under the SwiftUI overlays; a dedicated thread draws, the main thread never waits on a frame.
// Names, badges and picking use the engine's projection of the camera of the frame on screen.
// Any failure (no adapter, missing files, a driver fault) leaves the classic map in place.

/// The camera the GPU map is drawn from, in the renderer's terms.
struct GpuCamera: Equatable {
    var lon: Double
    var lat: Double
    var widthKm: Double
    var tiltDeg: Double
    var headingDeg: Double

    /// The classic map's camera as the GPU camera: the same centre, the width on the ground,
    /// the turn and the tilt. `tilt` is the player's tilt setting in radians.
    init(_ cam: MapCamera, size: CGSize, tilt: Double) {
        let radius = max(cam.radius(in: size), 1)
        lon = cam.lon
        lat = cam.lat
        widthKm = Double(size.width) * 6371.0 / radius
        tiltDeg = tilt * min(max(cam.tilt, 0), 1) * 180 / .pi
        headingDeg = cam.bearing
    }

    var values: [Double] { [lon, lat, widthKm, tiltDeg, headingDeg, 28] }
}

/// One building of the map: where it stands and whose it is.
struct GpuBuilding {
    let lon: Double
    let lat: Double
    let owner: String
    let colour: UInt32
    var isPlayer: Bool { owner == "player" }
    var isScenery: Bool { owner == "scenery" }
}

/// A UIView whose layer is a CAMetalLayer: the engine draws into it and presents each frame.
final class GpuMetalView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
    /// Called on the main thread when the size, scale or window changes.
    var onChange: (() -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        isOpaque = true
        isUserInteractionEnabled = false
        backgroundColor = UIColor(red: 0.043, green: 0.086, blue: 0.133, alpha: 1)
        metalLayer.isOpaque = true
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    override func layoutSubviews() {
        super.layoutSubviews()
        onChange?()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        onChange?()
    }
}

/// One open GPU map for a scenario. Created on the main thread; the scene is built and drawn on
/// its own thread, so the classic map keeps drawing until `isReady`.
final class GpuMapSession: @unchecked Sendable {
    private struct Request {
        var camera: GpuCamera
        var size: CGSize
        var scale: CGFloat
    }

    let scenario: String
    let view = GpuMetalView(frame: .zero)
    /// Called on the main thread when readiness, a failure or a new frame changes.
    var onChange: (() -> Void)?

    private let cond = NSCondition()
    private let updates = DispatchQueue(label: "cw-gpu-updates")
    private let finished = DispatchSemaphore(value: 0)
    private var handle: UInt64 = 0
    private var request: Request?
    private var wantedGeneration = 0
    private var renderedGeneration = 0
    private var appActive = true
    private var closed = false
    private var pendingUpdate: String?
    private var notifyQueued = false
    private var viewInWindow = false
    private var rendering = false

    private var _ready = false
    private var _failure: String?
    private var _adapter = ""
    private var _software = false
    private var _renderMs = 0.0
    private var _shown: (camera: GpuCamera, size: CGSize)?
    private var _buildings: [GpuBuilding] = []
    private var frameTimes: [Double] = []
    private var _frames = 0
    private var observers: [NSObjectProtocol] = []

    init(scenario: String) {
        self.scenario = scenario
        appActive = UIApplication.shared.applicationState != .background
        let center = NotificationCenter.default
        // The graphics card is not ours in the background: stop drawing before the app leaves the
        // foreground and start again when it returns.
        for (name, active) in [(UIApplication.willResignActiveNotification, false),
                               (UIApplication.didBecomeActiveNotification, true)] {
            observers.append(center.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
                self?.setAppActive(active)
            })
        }
        view.onChange = { [weak self] in self?.viewChanged() }
        Self.lastOpened = self
        let layer = Unmanaged.passUnretained(view.metalLayer).toOpaque()
        let thread = Thread { [self] in run(layer: layer) }
        thread.name = "cw-gpu-map"
        thread.stackSize = 8 << 20
        thread.qualityOfService = .userInteractive
        thread.start()
    }

    // MARK: State, readable from the main thread

    var isReady: Bool { locked { _ready && _failure == nil } }
    var failure: String? { locked { _failure } }
    var adapter: String { locked { _adapter } }
    var isSoftware: Bool { locked { _software } }
    var renderMs: Double { locked { _renderMs } }
    /// The camera and view size of the frame last presented. Names and badges use this, never the
    /// live gesture camera, so nothing slides against the picture while the view moves.
    var shown: (camera: GpuCamera, size: CGSize)? { locked { _shown } }
    var buildings: [GpuBuilding] { locked { _buildings } }
    /// Frames presented since the map opened.
    var framesPresented: Int { locked { _frames } }
    /// The most recently opened session, for tests.
    static weak var lastOpened: GpuMapSession?
    var framesPerSecond: Double {
        locked {
            guard let first = frameTimes.first, let last = frameTimes.last, frameTimes.count > 1, last > first else { return 0 }
            return Double(frameTimes.count - 1) / (last - first)
        }
    }

    /// The frame counter line for the map.
    var line: String {
        if failure != nil { return "map: classic, no GPU" }
        guard isReady else { return "map: classic, GPU map loading" }
        let soft = isSoftware ? " (software)" : ""
        return "map: GPU \(adapter)\(soft), \(String(format: "%.1f", renderMs)) ms"
    }

    private func locked<T>(_ body: () -> T) -> T {
        cond.lock()
        defer { cond.unlock() }
        return body()
    }

    // MARK: Requests from the main thread

    /// Asks for a frame of the view. Only the newest request is drawn.
    func request(camera: GpuCamera, size: CGSize) {
        guard size.width > 1, size.height > 1 else { return }
        cond.lock()
        let scale = min(max(view.traitCollection.displayScale, 1), 2)
        let new = Request(camera: camera, size: size, scale: scale)
        if request == nil || request!.camera != camera || request!.size != size || request!.scale != scale {
            request = new
            wantedGeneration += 1
            cond.broadcast()
        }
        cond.unlock()
    }

    /// Sends game state as JSON; applied between frames.
    func update(_ json: String) {
        cond.lock()
        if handle == 0 {
            pendingUpdate = json
            cond.unlock()
            return
        }
        let id = handle
        cond.unlock()
        updates.async { [self] in
            guard !isClosed else { return }
            guard cw_gpu_update(id, json) >= 0 else { return }
            let list = Self.readBuildings(id)
            cond.lock()
            _buildings = list
            wantedGeneration += 1
            cond.broadcast()
            cond.unlock()
            queueNotify()
        }
    }

    private var isClosed: Bool { locked { closed } }

    /// Screen points (in the points of `size`) of (lon, lat) pairs; NaN behind the camera.
    func project(camera: GpuCamera, size: CGSize, lonLat: [Double]) -> [Float] {
        let id = liveHandle
        guard id != 0, !lonLat.isEmpty else { return [Float](repeating: .nan, count: lonLat.count) }
        var out = [Float](repeating: .nan, count: lonLat.count)
        let cam = camera.values
        _ = cw_gpu_project(id, cam, Float(size.width), Float(size.height), lonLat, lonLat.count / 2, &out)
        return out
    }

    /// The screen rectangle (left, top, right, bottom) each building fills for the camera, NaN
    /// when it is behind the camera. Four floats a building, in the order of `buildings`.
    func buildingRects(camera: GpuCamera, size: CGSize) -> [Float] {
        let id = liveHandle
        let count = buildings.count
        var out = [Float](repeating: .nan, count: count * 4)
        guard id != 0, count > 0 else { return out }
        let cam = camera.values
        _ = cw_gpu_building_rects(id, cam, Float(size.width), Float(size.height), &out, count)
        return out
    }

    /// The region code under a point of the view, from the geometry the frame was drawn from.
    func pick(camera: GpuCamera, size: CGSize, at point: CGPoint) -> String? {
        let id = liveHandle
        guard id != 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: 64)
        let cam = camera.values
        let n = cw_gpu_pick(id, cam, Float(size.width), Float(size.height), Float(point.x), Float(point.y), &buffer, buffer.count)
        return n > 0 ? String(cString: buffer) : nil
    }

    private var liveHandle: UInt64 { locked { closed ? 0 : handle } }

    /// Stops drawing and frees the map. The layer stays alive until the engine has let go of it.
    func close() {
        cond.lock()
        guard !closed else { cond.unlock(); return }
        closed = true
        cond.broadcast()
        cond.unlock()
        for token in observers { NotificationCenter.default.removeObserver(token) }
        observers = []
        view.onChange = nil
        onChange = nil
        let view = self.view
        DispatchQueue.global(qos: .utility).async { [self] in
            finished.wait()
            updates.sync {}
            let id = locked { let h = handle; handle = 0; return h }
            if id != 0 { cw_gpu_close(id) }
            _ = view
        }
    }

    // MARK: Visibility

    /// Drawn only while the app is in the foreground and the view is in a window. Leaving the
    /// foreground waits until a frame in flight has been handed over, so no graphics work is
    /// submitted from the background.
    private func setAppActive(_ active: Bool) {
        cond.lock()
        appActive = active
        cond.broadcast()
        while !active && rendering { cond.wait() }
        cond.unlock()
    }

    private func viewChanged() {
        cond.lock()
        viewInWindow = view.window != nil
        cond.broadcast()
        cond.unlock()
    }

    // MARK: The render thread

    private func run(layer: UnsafeMutableRawPointer) {
        defer { finished.signal() }
        guard let assets = Bundle.main.resourcePath else { return fail("the map files are missing") }
        let id = scenario.withCString { s in assets.withCString { a in cw_gpu_open_layer(a, s, layer) } }
        guard id != 0 else { return fail(Self.lastError() ?? "no graphics adapter") }
        let info = Self.text { cw_gpu_info(id, $0, $1) }?.split(separator: "|", omittingEmptySubsequences: false).map(String.init) ?? []
        cond.lock()
        if closed { handle = id; cond.unlock(); return }
        handle = id
        _adapter = info.first ?? "unknown"
        _software = info.count > 2 && info[2] == "1"
        let first = pendingUpdate
        pendingUpdate = nil
        cond.unlock()
        if let first, cw_gpu_update(id, first) >= 0 {
            let list = Self.readBuildings(id)
            locked { _buildings = list }
        }
        locked { _ready = true; wantedGeneration += 1 }
        queueNotify()

        var timing = [Double](repeating: 0, count: 6)
        while true {
            cond.lock()
            while !closed && !(request != nil && renderedGeneration != wantedGeneration && appActive && viewInWindow) {
                cond.wait()
            }
            if closed { cond.unlock(); return }
            let job = request!
            let generation = wantedGeneration
            rendering = true
            cond.unlock()

            let width = UInt32(max((job.size.width * job.scale).rounded(), 1))
            let height = UInt32(max((job.size.height * job.scale).rounded(), 1))
            let cam = job.camera.values
            let code = cw_gpu_render_layer(id, cam, width, height, &timing)
            locked { rendering = false; cond.broadcast() }
            if code < 0 { return fail(Self.lastError() ?? "the graphics card stopped drawing") }
            if code == 1 {
                // The display could not take a frame (hidden, busy): try again shortly.
                Thread.sleep(forTimeInterval: 0.04)
                continue
            }
            let now = Date.timeIntervalSinceReferenceDate
            cond.lock()
            renderedGeneration = generation
            _renderMs = timing[0]
            _shown = (job.camera, job.size)
            _frames += 1
            frameTimes.append(now)
            frameTimes.removeAll { now - $0 > 1 }
            cond.unlock()
            queueNotify()
        }
    }

    private func fail(_ reason: String) {
        locked { _failure = String(reason.prefix(80)) }
        queueNotify()
    }

    /// Tells the main thread something changed; one notification is queued at a time.
    private func queueNotify() {
        cond.lock()
        if notifyQueued { cond.unlock(); return }
        notifyQueued = true
        cond.unlock()
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            locked { self.notifyQueued = false }
            onChange?()
        }
    }

    // MARK: C helpers

    private static func text(_ call: (UnsafeMutablePointer<CChar>, Int) -> Int32) -> String? {
        var buffer = [CChar](repeating: 0, count: 256)
        var n = call(&buffer, buffer.count)
        if n < 0, Int(-n) > buffer.count {
            buffer = [CChar](repeating: 0, count: Int(-n))
            n = call(&buffer, buffer.count)
        }
        return n >= 0 ? String(cString: buffer) : nil
    }

    private static func lastError() -> String? {
        text { cw_gpu_last_error($0, $1) }
    }

    private static func readBuildings(_ id: UInt64) -> [GpuBuilding] {
        guard let json = text({ cw_gpu_buildings(id, $0, $1) }),
              let data = json.data(using: .utf8),
              let items = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else { return [] }
        return items.map { item in
            GpuBuilding(
                lon: (item["lon"] as? Double) ?? 0, lat: (item["lat"] as? Double) ?? 0,
                owner: (item["owner"] as? String) ?? "scenery",
                colour: UInt32((item["colour"] as? String) ?? "", radix: 16) ?? 0x808080
            )
        }
    }
}
