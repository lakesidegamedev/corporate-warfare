import ImageIO
import MetalKit
import SwiftUI

// The real Earth under the map: NASA's Blue Marble (public domain), baked by tools/earth_tex.py
// into a world image for the globe and a sharp crop per scenario for close up, drawn as a textured
// mesh through the map's projection like the clouds, as its own Metal layer under the map. The
// seasons follow the game's calendar: a snow line that creeps south in winter (drawn through a
// land mask, so never on the sea), amber in the autumn and a browner winter in the mid-latitudes,
// all half a year apart in the south. The meshes are rebuilt only when the camera or the week
// changes. The crops decode to 3200 by 1920, so they load in the background the first time they
// are needed and only the current scenario's are kept.

enum Earth {
    /// Week of the year 1..52 from a label such as "2000 W10"; midsummer when there is none.
    static func week(of label: String?) -> Int {
        guard let label, let range = label.range(of: #"W(\d+)"#, options: .regularExpression),
              let week = Int(label[range].dropFirst()) else { return 28 }
        return week
    }

    /// How deep in winter a latitude is in `week`: 1 at midwinter, 0 from spring to autumn.
    static func winter(_ week: Int, _ latDeg: Double) -> Double {
        let shifted = latDeg >= 0 ? week : (week + 26) % 52
        return max(0, cos(2 * .pi * Double(shifted - 3) / 52))
    }

    /// How much autumn colour a latitude shows in `week`, 0 to 1.
    static func autumn(_ week: Int, _ latDeg: Double) -> Double {
        let shifted = latDeg >= 0 ? week : (week + 26) % 52
        let d = Double(shifted - 42)
        let band = smoothstep(28, 36, abs(latDeg)) * (1 - smoothstep(58, 66, abs(latDeg)))
        return exp(-(d / 5) * (d / 5)) * band
    }

    /// Snow cover 0 to 1: the line sits near 66 degrees in summer and reaches 40 in midwinter.
    static func snow(_ week: Int, _ latDeg: Double) -> Double {
        let line = 68 - 28 * winter(week, latDeg)
        return smoothstep(line - 3, line + 5, abs(latDeg)) * 0.6
    }

    /// The prop variant for the season: 0 summer, 1 autumn, 2 winter.
    static func season(_ week: Int, _ lat: Double) -> Int {
        let shifted = lat >= 0 ? week : (week + 26) % 52
        let winterNow = shifted <= 10 || shifted >= 49 || (shifted >= 46 && abs(lat) > 45)
        if winterNow && abs(lat) > 38 { return 2 }
        if shifted >= 38 && shifted <= 47 && abs(lat) > 30 { return 1 }
        return 0
    }
}

/// Everything one frame of the Earth needs.
struct EarthView: Equatable {
    let camera: MapCamera
    let size: CGSize
    let tilt: Double
    let week: Int
    let bounds: (west: Double, south: Double, east: Double, north: Double)
    let scenario: String?

    static func == (a: EarthView, b: EarthView) -> Bool {
        a.camera == b.camera && a.size == b.size && a.tilt == b.tilt && a.week == b.week && a.bounds == b.bounds
            && a.scenario == b.scenario
    }
}

/// The Earth as a transparent layer under the map. Draws in step with SwiftUI.
struct EarthLayer: UIViewRepresentable {
    let view: EarthView

    func makeCoordinator() -> EarthPainter { EarthPainter() }

    func makeUIView(context: Context) -> UIView {
        guard let renderer = EarthRenderer.shared else {
            let view = UIView()
            view.isUserInteractionEnabled = false
            return view
        }
        let view = EarthMetalView(frame: .zero, device: renderer.device)
        view.isOpaque = false
        view.layer.isOpaque = false
        view.backgroundColor = .clear
        view.clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        view.colorPixelFormat = .bgra8Unorm
        view.framebufferOnly = false
        view.isPaused = true
        view.enableSetNeedsDisplay = false
        view.presentsWithTransaction = true
        view.isUserInteractionEnabled = false
        view.delegate = context.coordinator
        view.painter = context.coordinator
        context.coordinator.view = view
        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        guard context.coordinator.earth != view else { return }
        context.coordinator.earth = view
        context.coordinator.redraw()
    }
}

/// The Metal view, redrawn once it has a size and a window.
final class EarthMetalView: MTKView {
    weak var painter: EarthPainter?

    override func layoutSubviews() {
        super.layoutSubviews()
        painter?.redraw()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        painter?.redraw()
    }
}

/// One mesh vertex: screen point, texture point, colour and the perspective's depth at the point.
/// Matches EarthVertex in the shader.
private struct EarthVertex {
    var x: Float = 0
    var y: Float = 0
    /// One over the tilt's perspective scale, so the image is interpolated in perspective across a
    /// cell rather than flat on the screen: otherwise the photo slides against the roads and borders
    /// drawn over it by up to a few dozen points as the camera moves.
    var w: Float = 1
    var u: Float = 0
    var v: Float = 0
    var r: Float = 1
    var g: Float = 1
    var b: Float = 1
    var a: Float = 0
}

/// A built mesh for one pass, kept until its key changes.
private final class EarthMesh {
    private var key: [AnyHashable]?
    private(set) var vertices: MTLBuffer?
    private(set) var indices: MTLBuffer?
    private(set) var count = 0

    /// Rebuilds for a new key: a grid from west to east and north to south at `step` degrees,
    /// each vertex placed by `place` (texture 0..1) and coloured by `color`.
    func update(_ key: [AnyHashable], device: MTLDevice, projection p: MapProjection,
                west: Double, east: Double, south: Double, north: Double, step: Double,
                place: (Double, Double) -> (Double, Double), color: (Double, Double) -> (Double, Double, Double, Double)) {
        guard key != self.key else { return }
        self.key = key
        let nx = Int((east - west) / step) + 1
        let ny = Int((north - south) / step) + 1
        var mesh = [EarthVertex](repeating: EarthVertex(), count: nx * ny)
        var visible = [Bool](repeating: false, count: nx * ny)
        for j in 0..<ny {
            let latDeg = north - Double(j) * step
            for i in 0..<nx {
                let k = j * nx + i
                let lonDeg = west + Double(i) * step
                guard let g = p.ground(lon: lonDeg * .pi / 180, lat: latDeg * .pi / 180) else { continue }
                let at = p.lift(g)
                let uv = place(lonDeg, latDeg)
                let c = color(lonDeg, latDeg)
                mesh[k] = EarthVertex(x: Float(at.x), y: Float(at.y), w: Float(1 / p.scale(atFlatY: g.y)), u: Float(uv.0), v: Float(uv.1),
                                      r: Float(c.0), g: Float(c.1), b: Float(c.2), a: Float(min(max(c.3, 0), 1)))
                visible[k] = true
            }
        }
        let jump = Float(p.r * 0.5)
        var out: [UInt32] = []
        out.reserveCapacity(max(nx - 1, 0) * max(ny - 1, 0) * 6)
        for j in 0..<max(ny - 1, 0) {
            for i in 0..<max(nx - 1, 0) {
                let k = j * nx + i
                let c1 = k + 1
                let c2 = k + nx
                let c3 = k + nx + 1
                guard visible[k], visible[c1], visible[c2], visible[c3] else { continue }
                if mesh[k].a <= 0 && mesh[c1].a <= 0 && mesh[c2].a <= 0 && mesh[c3].a <= 0 { continue }
                // Skip cells torn across the date line or the globe's edge.
                let x0 = mesh[k].x
                let y0 = mesh[k].y
                if abs(mesh[c3].x - x0) + abs(mesh[c3].y - y0) > jump { continue }
                if abs(mesh[c1].x - x0) + abs(mesh[c2].y - y0) > jump { continue }
                out.append(contentsOf: [UInt32(k), UInt32(c1), UInt32(c2), UInt32(c1), UInt32(c3), UInt32(c2)])
            }
        }
        count = out.count
        vertices = out.isEmpty ? nil : device.makeBuffer(bytes: mesh, length: MemoryLayout<EarthVertex>.stride * mesh.count)
        indices = out.isEmpty ? nil : device.makeBuffer(bytes: out, length: MemoryLayout<UInt32>.stride * out.count)
    }

    func draw(_ encoder: MTLRenderCommandEncoder, size: CGSize, texture: MTLTexture, sampler: MTLSamplerState) {
        guard count > 0, let vertices, let indices else { return }
        var viewSize = SIMD2<Float>(Float(size.width), Float(size.height))
        encoder.setVertexBuffer(vertices, offset: 0, index: 0)
        encoder.setVertexBytes(&viewSize, length: MemoryLayout<SIMD2<Float>>.stride, index: 1)
        encoder.setFragmentTexture(texture, index: 0)
        encoder.setFragmentSamplerState(sampler, index: 0)
        encoder.drawIndexedPrimitives(type: .triangle, indexCount: count, indexType: .uint32, indexBuffer: indices, indexBufferOffset: 0)
    }
}

/// Draws the Earth into one Metal view, keeping the meshes for the last camera and week.
final class EarthPainter: NSObject, MTKViewDelegate {
    weak var view: MTKView?
    var earth: EarthView?
    private let world = EarthMesh()
    private let crop = EarthMesh()
    private let snow = EarthMesh()
    private var observer: NSObjectProtocol?

    override init() {
        super.init()
        // An image that finishes loading shows at once, even on a still map.
        observer = NotificationCenter.default.addObserver(forName: EarthRenderer.loaded, object: nil, queue: .main) { [weak self] _ in
            self?.redraw()
        }
    }

    deinit {
        if let observer { NotificationCenter.default.removeObserver(observer) }
    }

    func redraw() {
        guard let view, view.window != nil, view.bounds.width >= 1, view.bounds.height >= 1 else { return }
        view.draw()
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {
        DispatchQueue.main.async { [weak self] in self?.redraw() }
    }

    func draw(in view: MTKView) {
        guard let renderer = EarthRenderer.shared, let drawable = view.currentDrawable,
              let pass = view.currentRenderPassDescriptor, let buffer = renderer.queue.makeCommandBuffer(),
              let encoder = buffer.makeRenderCommandEncoder(descriptor: pass) else { return }
        encoder.setRenderPipelineState(renderer.pipeline)
        if let earth { encode(earth, renderer, encoder) }
        encoder.endEncoding()
        buffer.commit()
        buffer.waitUntilScheduled()
        drawable.present()
    }

    /// One frame drawn off screen at `scale` pixels per point, for tests and review. Waits for the
    /// images to load first.
    func snapshot(_ earth: EarthView, scale: CGFloat = 2) -> CGImage? {
        guard let renderer = EarthRenderer.shared else { return nil }
        renderer.loadNow(EarthRenderer.names(for: earth.scenario))
        let width = Int(earth.size.width * scale)
        let height = Int(earth.size.height * scale)
        let stride = (width * 4 + 255) / 256 * 256
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .bgra8Unorm, width: width, height: height, mipmapped: false)
        descriptor.usage = .renderTarget
        descriptor.storageMode = .private
        guard width > 0, height > 0, let target = renderer.device.makeTexture(descriptor: descriptor),
              let readback = renderer.device.makeBuffer(length: stride * height, options: .storageModeShared),
              let buffer = renderer.queue.makeCommandBuffer() else { return nil }
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = target
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].storeAction = .store
        pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        guard let encoder = buffer.makeRenderCommandEncoder(descriptor: pass) else { return nil }
        encoder.setRenderPipelineState(renderer.pipeline)
        encode(earth, renderer, encoder)
        encoder.endEncoding()
        guard let blit = buffer.makeBlitCommandEncoder() else { return nil }
        blit.copy(from: target, sourceSlice: 0, sourceLevel: 0, sourceOrigin: MTLOrigin(x: 0, y: 0, z: 0),
                  sourceSize: MTLSize(width: width, height: height, depth: 1), to: readback, destinationOffset: 0,
                  destinationBytesPerRow: stride, destinationBytesPerImage: stride * height)
        blit.endEncoding()
        buffer.commit()
        buffer.waitUntilCompleted()
        let info = CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedFirst.rawValue
        return CGContext(data: readback.contents(), width: width, height: height, bitsPerComponent: 8, bytesPerRow: stride,
                         space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: info)?.makeImage()
    }

    /// The world, coarse, for the globe and the unrolling; the scenario's crop fading in as the
    /// map flattens; then snow through the land mask, close up only over the view's own patch.
    private func encode(_ e: EarthView, _ renderer: EarthRenderer, _ encoder: MTLRenderCommandEncoder) {
        let p = MapProjection(camera: e.camera, size: e.size, tilt: e.tilt)
        renderer.keep(e.scenario)
        let week = e.week
        let base: [AnyHashable] = [e.camera.lon, e.camera.lat, e.camera.zoom, e.camera.tilt, e.camera.bearing,
                                   e.size.width, e.size.height, e.tilt, week]
        func keyed(_ extra: AnyHashable...) -> [AnyHashable] { base + extra }
        let region = e.scenario.flatMap { CloudRegion.all[$0] }
        let cropTexture = e.scenario.flatMap { renderer.texture("earth-\($0)", ext: "jpg") }
        let close = cropTexture != nil && region != nil ? smoothstep(0.4, 0.8, p.t) : 0
        // Autumn warms the leaves, winter browns the mid-latitudes; a crop's edge fades so it
        // melts into the world image.
        func tint(_ lon: Double, _ lat: Double, west: Double, east: Double, south: Double, north: Double,
                  alpha: Double) -> (Double, Double, Double, Double) {
            let a = Earth.autumn(week, lat)
            let w = Earth.winter(week, lat) * smoothstep(30, 45, abs(lat))
            let edge = alpha < 1 || east - west < 359
                ? smoothstep(0, 4, min(min(lon - west, east - lon), min(lat - south, north - lat))) : 1
            return (min(max(1 - 0.06 * w, 0), 1), min(max(1 - 0.12 * a - 0.12 * w, 0), 1),
                    min(max(1 - 0.32 * a - 0.10 * w, 0), 1), alpha * edge)
        }
        if close < 0.999, let texture = renderer.texture("earth", ext: "jpg") {
            let step = p.t > 0.5 ? 2.0 : 4.0
            world.update(keyed("world"), device: renderer.device, projection: p, west: -180, east: 180, south: -88, north: 88,
                         step: step, place: { lon, lat in ((lon + 180) / 360, (90 - lat) / 180) },
                         color: { lon, lat in tint(lon, lat, west: -180, east: 180, south: -88, north: 88, alpha: 1) })
            world.draw(encoder, size: e.size, texture: texture, sampler: renderer.repeatX)
        }
        if close > 0.001, let r = region, let texture = cropTexture {
            crop.update(keyed("crop", close), device: renderer.device, projection: p, west: r.west, east: r.east,
                        south: r.south, north: r.north, step: 1,
                        place: { lon, lat in ((lon - r.west) / (r.east - r.west), (r.north - lat) / (r.north - r.south)) },
                        color: { lon, lat in tint(lon, lat, west: r.west, east: r.east, south: r.south, north: r.north, alpha: close) })
            crop.draw(encoder, size: e.size, texture: texture, sampler: renderer.clamp)
        }
        // Snow on land only: the land mask, tinted white by how far into winter each place is.
        let near = close > 0.5 && region != nil
        let sharp = near ? e.scenario.flatMap { renderer.texture("earth-land-\($0)", ext: "png") } : nil
        guard let land = sharp ?? renderer.texture("earth-land", ext: "png") else { return }
        let b = e.bounds
        // Close up only the view's own patch, finely enough that coasts stay crisp.
        let west = near ? b.west - 6 : -180
        let east = near ? b.east + 6 : 180
        let south = near ? b.south - 6 : -88
        let north = near ? b.north + 6 : 88
        snow.update(keyed("snow", sharp != nil, near), device: renderer.device, projection: p, west: west, east: east,
                    south: south, north: north, step: near ? 0.25 : 4,
                    place: { lon, lat in
                        if sharp != nil, let r = region {
                            return ((lon - r.west) / (r.east - r.west), (r.north - lat) / (r.north - r.south))
                        }
                        return ((lon + 180) / 360, (90 - lat) / 180)
                    },
                    color: { _, lat in (1, 1, 1, Earth.snow(week, lat)) })
        snow.draw(encoder, size: e.size, texture: land, sampler: sharp == nil ? renderer.repeatX : renderer.clamp)
    }
}

/// The shared pipeline and Earth images. Images load in the background the first time they are
/// asked for, as colour with mipmaps; only the world and the current scenario's are kept.
final class EarthRenderer {
    static let shared = EarthRenderer()
    static let loaded = Notification.Name("EarthRenderer.loaded")

    let device: MTLDevice
    let queue: MTLCommandQueue
    let pipeline: MTLRenderPipelineState
    let repeatX: MTLSamplerState
    let clamp: MTLSamplerState
    private var textures: [String: MTLTexture] = [:]
    private var loading: Set<String> = []
    private var scenario: String?

    /// Every image a scenario's Earth uses.
    static func names(for scenario: String?) -> [(String, String)] {
        var out = [("earth", "jpg"), ("earth-land", "png")]
        if let scenario { out += [("earth-\(scenario)", "jpg"), ("earth-land-\(scenario)", "png")] }
        return out
    }

    private init?() {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue(),
              let library = try? device.makeLibrary(source: Self.shader, options: nil),
              let vertex = library.makeFunction(name: "earthVertex"),
              let fragment = library.makeFunction(name: "earthFragment") else { return nil }
        let descriptor = MTLRenderPipelineDescriptor()
        descriptor.vertexFunction = vertex
        descriptor.fragmentFunction = fragment
        let colour = descriptor.colorAttachments[0]!
        colour.pixelFormat = .bgra8Unorm
        colour.isBlendingEnabled = true
        colour.rgbBlendOperation = .add
        colour.alphaBlendOperation = .add
        colour.sourceRGBBlendFactor = .one
        colour.sourceAlphaBlendFactor = .one
        colour.destinationRGBBlendFactor = .oneMinusSourceAlpha
        colour.destinationAlphaBlendFactor = .oneMinusSourceAlpha
        guard let pipeline = try? device.makeRenderPipelineState(descriptor: descriptor) else { return nil }
        func sampler(_ s: MTLSamplerAddressMode) -> MTLSamplerState? {
            let d = MTLSamplerDescriptor()
            d.minFilter = .linear
            d.magFilter = .linear
            d.mipFilter = .linear
            d.sAddressMode = s
            d.tAddressMode = .clampToEdge
            return device.makeSamplerState(descriptor: d)
        }
        guard let repeatX = sampler(.repeat), let clamp = sampler(.clampToEdge) else { return nil }
        self.device = device
        self.queue = queue
        self.pipeline = pipeline
        self.repeatX = repeatX
        self.clamp = clamp
    }

    /// Drops another scenario's images when the scenario changes. Main thread only.
    func keep(_ scenario: String?) {
        guard scenario != self.scenario else { return }
        self.scenario = scenario
        let wanted = Set(Self.names(for: scenario).map(\.0))
        textures = textures.filter { wanted.contains($0.key) }
    }

    /// An image from the bundle, or nil until it has loaded. Main thread only.
    func texture(_ name: String, ext: String) -> MTLTexture? {
        if let texture = textures[name] { return texture }
        guard !loading.contains(name) else { return nil }
        loading.insert(name)
        let device = device
        let queue = queue
        DispatchQueue.global(qos: .userInitiated).async {
            let loaded = Self.load(name, ext: ext, device: device, queue: queue)
            DispatchQueue.main.async {
                // An image that fails to load stays in `loading`, so it is not tried every frame.
                guard let loaded else { return }
                self.loading.remove(name)
                // A scenario left while its image loaded is not kept.
                guard Self.names(for: self.scenario).contains(where: { $0.0 == name }) else { return }
                self.textures[name] = loaded
                NotificationCenter.default.post(name: Self.loaded, object: nil)
            }
        }
        return nil
    }

    /// Loads every named image at once, for tests. Main thread only.
    func loadNow(_ names: [(String, String)]) {
        for (name, ext) in names where textures[name] == nil {
            if let texture = Self.load(name, ext: ext, device: device, queue: queue) { textures[name] = texture }
        }
    }

    /// Decodes an image into RGBA and uploads it with mipmaps.
    private static func load(_ name: String, ext: String, device: MTLDevice, queue: MTLCommandQueue) -> MTLTexture? {
        guard let url = Bundle.main.url(forResource: name, withExtension: ext),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        let width = image.width
        let height = image.height
        let stride = (width * 4 + 255) / 256 * 256
        guard let staging = device.makeBuffer(length: stride * height, options: .storageModeShared),
              let context = CGContext(data: staging.contents(), width: width, height: height, bitsPerComponent: 8,
                                      bytesPerRow: stride, space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: width, height: height, mipmapped: true)
        descriptor.usage = .shaderRead
        descriptor.storageMode = .private
        guard let texture = device.makeTexture(descriptor: descriptor), let buffer = queue.makeCommandBuffer(),
              let blit = buffer.makeBlitCommandEncoder() else { return nil }
        blit.copy(from: staging, sourceOffset: 0, sourceBytesPerRow: stride, sourceBytesPerImage: stride * height,
                  sourceSize: MTLSize(width: width, height: height, depth: 1), to: texture, destinationSlice: 0,
                  destinationLevel: 0, destinationOrigin: MTLOrigin(x: 0, y: 0, z: 0))
        blit.generateMipmaps(for: texture)
        blit.endEncoding()
        buffer.commit()
        buffer.waitUntilCompleted()
        return buffer.status == .completed ? texture : nil
    }

    /// The image modulated by the vertex colour; premultiplied out.
    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct EarthVertex { float x, y, w, u, v, r, g, b, a; };
    struct Shaded { float4 position [[position]]; float2 uv; float4 color; };

    vertex Shaded earthVertex(uint id [[vertex_id]], const device EarthVertex *vertices [[buffer(0)]],
                              constant float2 &size [[buffer(1)]]) {
        EarthVertex v = vertices[id];
        Shaded out;
        // Clip space scaled by w: the same point on screen, interpolated in perspective.
        out.position = float4((v.x / size.x * 2 - 1) * v.w, (1 - v.y / size.y * 2) * v.w, 0, v.w);
        out.uv = float2(v.u, v.v);
        out.color = float4(v.r, v.g, v.b, v.a);
        return out;
    }

    fragment float4 earthFragment(Shaded in [[stage_in]], texture2d<float> image [[texture(0)]], sampler s [[sampler(0)]]) {
        float4 t = image.sample(s, in.uv);
        return float4(t.rgb * in.color.rgb * in.color.a, t.a * in.color.a);
    }
    """
}
