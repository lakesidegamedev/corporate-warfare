import ImageIO
import MetalKit
import SwiftUI

// Clouds from a real satellite cloud map of the whole Earth ("clouds.webp", from Solar System
// Scope's 8K Earth clouds, CC BY 4.0, based on NASA Blue Marble imagery; see Settings). The map
// holds real weather: fronts, cyclones, tropical clusters, marine cloud sheets. It moves the way
// the atmosphere does, west on the trade winds, east on the westerlies, slowly west near the
// poles, using two flow layers so the shear between wind bands never stretches it far. It is
// drawn as a textured mesh through the map's projection, so it curves with the globe and tilts
// with the map, and its shadow falls on the ground below. SwiftUI's canvas has no textured mesh,
// so the clouds are their own Metal layer between the still map and the moving overlay. The
// mesh is rebuilt only when the camera or the sun moves; each frame only slides the texture.
// Clouds drift a fraction of a degree a second, so the map redraws them about eight times a
// second, and mesh cells over clear sky are left out using a max-pooled mask of each texture.

/// The prevailing wind in degrees of longitude a second: west in the tropics and near the poles,
/// east in between.
func windAt(_ latDegrees: Double) -> Double {
    let a = abs(latDegrees)
    func bump(_ centre: Double, _ width: Double) -> Double { exp(-((a - centre) / width) * ((a - centre) / width)) }
    return -0.05 * bump(14, 12) + 0.10 * bump(47, 13) - 0.02 * bump(75, 8)
}

/// Flow-map phases: each of two layers slides with the wind for one period then starts over, half
/// a period apart, and is invisible as it restarts.
enum CloudFlow {
    static let period = 90.0

    /// How far a layer has slid in seconds of wind, and its weight.
    static func phase(_ seconds: Double, layer: Int) -> (slide: Double, weight: Double) {
        let phase = fract(seconds / period + Double(layer) * 0.5)
        return (phase * period, 1 - abs(1 - 2 * phase))
    }
}

/// A scenario's close-up cloud texture and the area it covers, in degrees.
struct CloudRegion {
    let west: Double
    let south: Double
    let east: Double
    let north: Double

    /// Must match REGIONS in tools/cloud_tex.py and CloudRegions on Android.
    static let all: [String: CloudRegion] = [
        "great-lakes-2000": CloudRegion(west: -122, south: 18, east: -42, north: 66),
        "texas-1981": CloudRegion(west: -137, south: 8, east: -57, north: 56),
        "silicon-valley-1995": CloudRegion(west: -160, south: 13, east: -80, north: 61),
        "ruhr-rhine-1991": CloudRegion(west: -32, south: 28, east: 48, north: 76),
    ]
}

/// How opaque the clouds are at each setting: thin, light, heavy.
func cloudStrength(_ amount: Int) -> Double {
    switch amount {
    case 1: return 0.4
    case 2: return 0.72
    default: return 1
    }
}

/// Everything one frame of clouds needs. `amount` is the setting (0 off, 1 thin, 2 light, 3 heavy).
/// `origin` moves the drawing, as the menu moves its globe off centre.
struct CloudSky: Equatable {
    let camera: MapCamera
    let size: CGSize
    let tilt: Double
    var origin: CGPoint = .zero
    let sun: MapSun
    let seconds: Double
    let amount: Int
    let bounds: (west: Double, south: Double, east: Double, north: Double)
    let altitude: Double
    let zoom: Double
    let shadows: Bool
    var scenario: String?

    static func == (a: CloudSky, b: CloudSky) -> Bool {
        a.camera == b.camera && a.size == b.size && a.tilt == b.tilt && a.origin == b.origin && a.sun == b.sun
            && a.seconds == b.seconds && a.amount == b.amount && a.bounds == b.bounds && a.altitude == b.altitude
            && a.zoom == b.zoom && a.shadows == b.shadows && a.scenario == b.scenario
    }
}

/// The clouds as a transparent layer. Draws in step with SwiftUI, so they never lag the map.
struct CloudLayer: UIViewRepresentable {
    let sky: CloudSky

    func makeCoordinator() -> CloudPainter { CloudPainter() }

    func makeUIView(context: Context) -> UIView {
        guard let renderer = CloudRenderer.shared else {
            let view = UIView()
            view.isUserInteractionEnabled = false
            return view
        }
        let view = CloudView(frame: .zero, device: renderer.device)
        view.isOpaque = false
        view.layer.isOpaque = false
        view.backgroundColor = .clear
        view.clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        view.colorPixelFormat = .bgra8Unorm
        // Readable drawables, so a window snapshot can capture the clouds.
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

    func updateUIView(_ view: UIView, context: Context) {
        context.coordinator.sky = sky
        context.coordinator.redraw()
    }
}

/// Draws the clouds into one Metal view. Keeps the meshes for the last camera.
final class CloudPainter: NSObject, MTKViewDelegate {
    weak var view: MTKView?
    var sky: CloudSky?
    /// Frames drawn with clouds in them, and draws that found no drawable, for tests.
    private(set) var drawn = 0
    private(set) var missed = 0
    private let regionCloud = CloudGrid()
    private let regionShadow = CloudGrid()
    private let worldCloud = CloudGrid()
    private let worldShadow = CloudGrid()
    private var observer: NSObjectProtocol?

    override init() {
        super.init()
        // A texture that finishes loading shows at once, even on a still map.
        observer = NotificationCenter.default.addObserver(forName: CloudRenderer.loaded, object: nil, queue: .main) { [weak self] _ in
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
        guard let renderer = CloudRenderer.shared, let drawable = view.currentDrawable,
              let pass = view.currentRenderPassDescriptor, let buffer = renderer.queue.makeCommandBuffer(),
              let encoder = buffer.makeRenderCommandEncoder(descriptor: pass) else {
            missed += 1
            return
        }
        encoder.setRenderPipelineState(renderer.pipeline)
        if let sky, sky.amount > 0 {
            encode(sky, renderer, encoder)
            drawn += 1
        }
        encoder.endEncoding()
        buffer.commit()
        buffer.waitUntilScheduled()
        drawable.present()
    }

    /// One frame of clouds drawn off screen at `scale` pixels per point, for tests and review.
    func snapshot(_ sky: CloudSky, scale: CGFloat = 2) -> CGImage? {
        guard let renderer = CloudRenderer.shared else { return nil }
        let width = Int(sky.size.width * scale)
        let height = Int(sky.size.height * scale)
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
        encode(sky, renderer, encoder)
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

    /// The clouds for one frame. On the flat map only the area round the scenario is meshed,
    /// finely enough that the tilt's perspective does not bend the texture; on the globe the
    /// whole world, coarsely.
    private func encode(_ sky: CloudSky, _ renderer: CloudRenderer, _ encoder: MTLRenderCommandEncoder) {
        let p = MapProjection(camera: sky.camera, size: sky.size, tilt: sky.tilt)
        var base = CloudUniforms()
        base.width = Float(sky.size.width)
        base.height = Float(sky.size.height)
        base.ox = Float(sky.origin.x)
        base.oy = Float(sky.origin.y)
        // Shadows double the cost, so only heavy clouds cast them.
        let passes = sky.shadows && sky.amount >= 3 && sky.altitude > 0 ? [true, false] : [false]
        let shift = CGSize(width: sky.altitude * 0.5, height: sky.altitude * 0.32)
        // Clouds clear as you zoom in, so a close look at the map is never through haze.
        let near = 1 - smoothstep(9, 22, sky.zoom)
        // Close up, the scenario's own full-resolution texture; it fades in as the map flattens.
        var closeUp = 0.0
        if let id = sky.scenario, let r = CloudRegion.all[id], let texture = renderer.texture("clouds-\(id)"),
           let mask = renderer.mask("clouds-\(id)") {
            closeUp = smoothstep(0.4, 0.8, p.t)
            if closeUp > 0 {
                // One texture slid east or west with the wind at the scenario's latitude. Its
                // edges mirror, so however far it drifts it never shows a seam.
                let wind = windAt((r.south + r.north) / 2)
                var u = base
                u.uOffset = Float(-r.west)
                u.uScale = Float(1 / (r.east - r.west))
                u.vOffset = Float(r.north)
                u.vScale = Float(1 / (r.north - r.south))
                u.slide = Float(sky.seconds)
                u.strength = Float(cloudStrength(sky.amount) * 0.95 * closeUp * near)
                for shadow in passes {
                    let grid = shadow ? regionShadow : regionCloud
                    grid.update(CloudGrid.Key(camera: sky.camera, size: sky.size, tilt: sky.tilt, sun: sky.sun,
                                              west: r.west, east: r.east, south: r.south, north: r.north, step: 1,
                                              up: shadow ? 0 : sky.altitude, shift: shadow ? shift : .zero, region: true),
                                device: renderer.device, projection: p,
                                // Fade out over the outer five degrees so the layer has no edge.
                                fade: { lon, lat in smoothstep(0, 5, min(lon - r.west, r.east - lon, lat - r.south, r.north - lat)) },
                                wind: { _ in wind })
                    u.shadow = shadow ? 1 : 0
                    grid.draw(encoder, uniforms: u, texture: texture, sampler: renderer.mirror, mask: mask, repeatX: false)
                }
            }
        }
        guard closeUp < 0.999, let world = renderer.texture("clouds"), let mask = renderer.mask("clouds") else { return }
        let strength = cloudStrength(sky.amount) * (1 - closeUp)
        let west, east, south, north, step: Double
        if p.t > 0.5 {
            let b = sky.bounds
            west = floor(b.west - 30)
            east = west + max(60, b.east - b.west + 60)
            south = max(-84, floor(b.south - 20))
            north = min(84, b.north + 20)
            step = 1
        } else {
            west = -180
            east = 180
            south = -84
            north = 84
            step = 4
        }
        for layer in 0..<2 {
            let flow = CloudFlow.phase(sky.seconds, layer: layer)
            if flow.weight < 0.01 { continue }
            var u = base
            // Each layer starts from its own place in the map so a restart never shows.
            u.uOffset = Float(180 + Double(layer) * 133)
            u.uScale = 1 / 360
            u.vOffset = 90
            u.vScale = 1 / 180
            u.slide = Float(flow.slide)
            u.strength = Float(strength * near * flow.weight)
            for shadow in passes {
                let grid = shadow ? worldShadow : worldCloud
                grid.update(CloudGrid.Key(camera: sky.camera, size: sky.size, tilt: sky.tilt, sun: sky.sun,
                                          west: west, east: east, south: south, north: north, step: step,
                                          up: shadow ? 0 : sky.altitude, shift: shadow ? shift : .zero, region: false),
                            device: renderer.device, projection: p,
                            // Thin out from far below the poles, where the map's rows crowd together,
                            // so the layer fades away there instead of ending in a ring.
                            fade: { _, lat in smoothstep(82, 56, abs(lat)) },
                            wind: windAt)
                u.shadow = shadow ? 1 : 0
                grid.draw(encoder, uniforms: u, texture: world, sampler: renderer.repeatX, mask: mask, repeatX: true)
            }
        }
    }
}

/// The Metal view, redrawn once it has a size and a window: SwiftUI may hand it its first sky
/// before either.
final class CloudView: MTKView {
    weak var painter: CloudPainter?

    override func layoutSubviews() {
        super.layoutSubviews()
        painter?.redraw()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        painter?.redraw()
    }
}

/// One mesh vertex: where it lands on screen, its place in degrees, the wind there, its fade and
/// its sunlight. Matches CloudVertex in the shader.
private struct CloudVertex {
    var x: Float = 0
    var y: Float = 0
    var lon: Float = 0
    var lat: Float = 0
    var wind: Float = 0
    var alpha: Float = 0
    var light: Float = 0
    var pad: Float = 0
}

/// One draw's view size, offset and texture slide. Matches CloudUniforms in the shader.
private struct CloudUniforms {
    var width: Float = 1
    var height: Float = 1
    var ox: Float = 0
    var oy: Float = 0
    var uOffset: Float = 0
    var uScale: Float = 1
    var vOffset: Float = 0
    var vScale: Float = 1
    var slide: Float = 0
    var strength: Float = 0
    var shadow: Float = 0
    var pad: Float = 0
}

/// A meshed patch of sky for one camera, sun and height, with the cells worth drawing.
private final class CloudGrid {
    struct Key: Equatable {
        let camera: MapCamera
        let size: CGSize
        let tilt: Double
        let sun: MapSun
        let west: Double
        let east: Double
        let south: Double
        let north: Double
        let step: Double
        let up: Double
        let shift: CGSize
        let region: Bool
    }

    /// The texture slide a set of cells was picked for.
    private struct Slide: Equatable {
        let uOffset: Float
        let slide: Float
    }

    private var key: Key?
    private var mesh: [CloudVertex] = []
    private var vertices: MTLBuffer?
    /// Cells that pass every test but the texture's, as the index of their first corner.
    private var candidates: [Int] = []
    private var columns = 0
    /// The cells worth drawing for the last two slides, one per flow layer.
    private var picked: [(slide: Slide, indices: MTLBuffer?, count: Int)] = []

    func update(_ key: Key, device: MTLDevice, projection p: MapProjection,
                fade: (Double, Double) -> Double, wind: (Double) -> Double) {
        guard key != self.key else { return }
        self.key = key
        let nx = Int((key.east - key.west) / key.step) + 1
        let ny = Int((key.north - key.south) / key.step) + 1
        var mesh = [CloudVertex](repeating: CloudVertex(), count: nx * ny)
        var visible = [Bool](repeating: false, count: nx * ny)
        for j in 0..<ny {
            let latDeg = key.north - Double(j) * key.step
            let w = Float(wind(latDeg))
            for i in 0..<nx {
                let k = j * nx + i
                let lonDeg = key.west + Double(i) * key.step
                let lon = lonDeg * .pi / 180
                let lat = latDeg * .pi / 180
                guard let g = p.ground(lon: lon, lat: lat) else { continue }
                var a = fade(lonDeg, latDeg)
                if p.t < 0.5 { a *= smoothstep(0, 0.25, p.toView(lon: lon, lat: lat).2) }
                let at = p.lift(g, key.up)
                // Clouds on the night side are lit only faintly; their shadows only fall by day.
                let light = 0.18 + 0.82 * smoothstep(-0.15, 0.1, key.sun.facing(lon: lon, lat: lat))
                mesh[k] = CloudVertex(x: Float(Double(at.x) + key.shift.width), y: Float(Double(at.y) + key.shift.height),
                                      lon: Float(lonDeg), lat: Float(latDeg), wind: w, alpha: Float(a), light: Float(light))
                visible[k] = true
            }
        }
        let jump = Float(p.r * 0.5)
        var cells: [Int] = []
        cells.reserveCapacity(max(nx - 1, 0) * max(ny - 1, 0))
        for j in 0..<max(ny - 1, 0) {
            for i in 0..<max(nx - 1, 0) {
                let k = j * nx + i
                let c1 = k + 1
                let c2 = k + nx
                let c3 = k + nx + 1
                guard visible[k], visible[c1], visible[c2], visible[c3] else { continue }
                if mesh[k].alpha <= 0 && mesh[c1].alpha <= 0 && mesh[c2].alpha <= 0 && mesh[c3].alpha <= 0 { continue }
                // Skip cells torn across the date line or the globe's edge.
                let x0 = mesh[k].x
                let y0 = mesh[k].y
                if abs(mesh[c3].x - x0) + abs(mesh[c3].y - y0) > jump { continue }
                if abs(mesh[c1].x - x0) + abs(mesh[c2].y - y0) > jump { continue }
                cells.append(k)
            }
        }
        self.mesh = mesh
        candidates = cells
        columns = nx
        picked = []
        vertices = cells.isEmpty ? nil : device.makeBuffer(bytes: mesh, length: MemoryLayout<CloudVertex>.stride * mesh.count)
    }

    /// The cells whose patch of texture holds some cloud at this slide. Clear sky costs nothing.
    private func cells(_ u: CloudUniforms, texture: MTLTexture, mask: CloudMask, repeatX: Bool, device: MTLDevice) -> (MTLBuffer?, Int) {
        let slide = Slide(uOffset: u.uOffset, slide: u.slide)
        if let hit = picked.first(where: { $0.slide == slide }) { return (hit.indices, hit.count) }
        let tw = Float(texture.width) * u.uScale
        let th = Float(texture.height) * u.vScale
        var out: [UInt32] = []
        out.reserveCapacity(candidates.count * 6)
        for k in candidates {
            let c1 = k + 1
            let c2 = k + columns
            let c3 = k + columns + 1
            var u0 = Float.greatestFiniteMagnitude
            var u1 = -Float.greatestFiniteMagnitude
            var v0 = Float.greatestFiniteMagnitude
            var v1 = -Float.greatestFiniteMagnitude
            for c in [k, c1, c2, c3] {
                let v = mesh[c]
                // As the shader places the texture, in texels.
                let x = (v.lon - v.wind * u.slide + u.uOffset) * tw
                let y = (u.vOffset - v.lat) * th
                u0 = min(u0, x)
                u1 = max(u1, x)
                v0 = min(v0, y)
                v1 = max(v1, y)
            }
            guard mask.anyCloud(u0, v0, u1, v1, repeatX: repeatX) else { continue }
            out.append(contentsOf: [UInt32(k), UInt32(c1), UInt32(c2), UInt32(c1), UInt32(c3), UInt32(c2)])
        }
        let indices = out.isEmpty ? nil : device.makeBuffer(bytes: out, length: MemoryLayout<UInt32>.stride * out.count)
        picked.append((slide, indices, out.count))
        if picked.count > 2 { picked.removeFirst() }
        return (indices, out.count)
    }

    func draw(_ encoder: MTLRenderCommandEncoder, uniforms: CloudUniforms, texture: MTLTexture, sampler: MTLSamplerState,
              mask: CloudMask, repeatX: Bool) {
        guard uniforms.strength > 0, let vertices else { return }
        let (indices, indexCount) = cells(uniforms, texture: texture, mask: mask, repeatX: repeatX, device: encoder.device)
        guard indexCount > 0, let indices else { return }
        var u = uniforms
        encoder.setVertexBuffer(vertices, offset: 0, index: 0)
        encoder.setVertexBytes(&u, length: MemoryLayout<CloudUniforms>.stride, index: 1)
        encoder.setFragmentTexture(texture, index: 0)
        encoder.setFragmentSamplerState(sampler, index: 0)
        encoder.drawIndexedPrimitives(type: .triangle, indexCount: indexCount, indexType: .uint32,
                                      indexBuffer: indices, indexBufferOffset: 0)
    }
}

/// The most cloud in each 16 by 16 block of a texture, read once, so mesh cells over clear sky can
/// be left out and never cost a pixel.
final class CloudMask {
    private static let block = 16
    private let w: Int
    private let h: Int
    private let width: Int
    private let height: Int
    private var most: [UInt8]

    init(_ alpha: UnsafePointer<UInt8>, width: Int, height: Int, stride: Int) {
        let block = Self.block
        let w = (width + block - 1) / block
        self.w = w
        h = (height + block - 1) / block
        self.width = width
        self.height = height
        var most = [UInt8](repeating: 0, count: w * h)
        most.withUnsafeMutableBufferPointer { most in
            for y in 0..<height {
                let row = alpha + y * stride
                let base = (y / block) * w
                for x in 0..<width where row[x] > most[base + x / block] {
                    most[base + x / block] = row[x]
                }
            }
        }
        self.most = most
    }

    /// Whether any texel in the texture rectangle holds cloud. `repeatX` wraps east to west;
    /// otherwise edges mirror.
    func anyCloud(_ u0: Float, _ v0: Float, _ u1: Float, _ v1: Float, repeatX: Bool) -> Bool {
        func place(_ u: Float, _ size: Int, _ repeats: Bool) -> Float {
            let size = Float(size)
            if repeats { return (u.truncatingRemainder(dividingBy: size) + size).truncatingRemainder(dividingBy: size) }
            let period = 2 * size
            let m = (u.truncatingRemainder(dividingBy: period) + period).truncatingRemainder(dividingBy: period)
            return m < size ? m : period - m
        }
        let a = place(u0, width, repeatX)
        let b = place(u1, width, repeatX)
        // A cell that crosses a wrap or a mirror edge is kept rather than worked out.
        if b < a || b - a > (u1 - u0) + 1 { return true }
        let c = place(v0, height, false)
        let d = place(v1, height, false)
        if d < c || d - c > (v1 - v0) + 1 { return true }
        let block = Float(Self.block)
        let bx0 = min(max(Int(a / block), 0), w - 1)
        let bx1 = min(max(Int(b / block), 0), w - 1)
        let by0 = min(max(Int(c / block), 0), h - 1)
        let by1 = min(max(Int(d / block), 0), h - 1)
        for y in by0...by1 {
            for x in bx0...bx1 where most[y * w + x] > 5 { return true }
        }
        return false
    }
}

/// The shared Metal device, pipeline and cloud textures. Textures load in the background the
/// first time they are asked for, as one alpha channel with mipmaps so the globe stays smooth.
final class CloudRenderer {
    static let shared = CloudRenderer()
    static let loaded = Notification.Name("CloudRenderer.loaded")

    let device: MTLDevice
    let queue: MTLCommandQueue
    let pipeline: MTLRenderPipelineState
    /// Wraps round the world east and west.
    let repeatX: MTLSamplerState
    /// Mirrors at every edge, for a drifting close-up texture.
    let mirror: MTLSamplerState
    private var textures: [String: MTLTexture] = [:]
    private var masks: [String: CloudMask] = [:]
    private var loading: Set<String> = []

    private init?() {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue(),
              let library = try? device.makeLibrary(source: Self.shader, options: nil),
              let vertex = library.makeFunction(name: "cloudVertex"),
              let fragment = library.makeFunction(name: "cloudFragment") else { return nil }
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
        func sampler(_ s: MTLSamplerAddressMode, _ t: MTLSamplerAddressMode) -> MTLSamplerState? {
            let d = MTLSamplerDescriptor()
            d.minFilter = .linear
            d.magFilter = .linear
            d.mipFilter = .linear
            d.sAddressMode = s
            d.tAddressMode = t
            return device.makeSamplerState(descriptor: d)
        }
        guard let repeatX = sampler(.repeat, .clampToEdge), let mirror = sampler(.mirrorRepeat, .mirrorRepeat) else { return nil }
        self.device = device
        self.queue = queue
        self.pipeline = pipeline
        self.repeatX = repeatX
        self.mirror = mirror
    }

    /// Whether every named texture has loaded, starting any that have not. Main thread only.
    func ready(_ names: [String]) -> Bool {
        names.map { texture($0) != nil }.allSatisfy { $0 }
    }

    /// A texture from the bundle, or nil until it has loaded. Main thread only.
    func texture(_ name: String) -> MTLTexture? {
        if let texture = textures[name] { return texture }
        guard !loading.contains(name) else { return nil }
        loading.insert(name)
        let device = device
        let queue = queue
        DispatchQueue.global(qos: .userInitiated).async {
            let loaded = Self.load(name, device: device, queue: queue)
            DispatchQueue.main.async {
                // A texture that fails to load stays in `loading`, so it is not tried every frame.
                guard let loaded else { return }
                self.textures[name] = loaded.0
                self.masks[name] = loaded.1
                self.loading.remove(name)
                NotificationCenter.default.post(name: Self.loaded, object: nil)
            }
        }
        return nil
    }

    /// A loaded texture's cloud mask, or nil until the texture has loaded. Main thread only.
    func mask(_ name: String) -> CloudMask? { masks[name] }

    /// Decodes a white-on-clear WebP into its alpha alone, reads its mask and uploads it with mipmaps.
    private static func load(_ name: String, device: MTLDevice, queue: MTLCommandQueue) -> (MTLTexture, CloudMask)? {
        guard let url = Bundle.main.url(forResource: name, withExtension: "webp"),
              let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { return nil }
        let width = image.width
        let height = image.height
        let stride = (width + 255) / 256 * 256
        guard let staging = device.makeBuffer(length: stride * height, options: .storageModeShared),
              let context = CGContext(data: staging.contents(), width: width, height: height, bitsPerComponent: 8,
                                      bytesPerRow: stride, space: CGColorSpaceCreateDeviceGray(),
                                      bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue) else { return nil }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let mask = CloudMask(staging.contents().assumingMemoryBound(to: UInt8.self), width: width, height: height, stride: stride)
        let descriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .r8Unorm, width: width, height: height, mipmapped: true)
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
        return buffer.status == .completed ? (texture, mask) : nil
    }

    /// White cloud, or black shadow, modulated by the texture's alpha; premultiplied out.
    private static let shader = """
    #include <metal_stdlib>
    using namespace metal;

    struct CloudVertex { float x, y, lon, lat, wind, alpha, light, pad; };
    struct CloudUniforms { float width, height, ox, oy, uOffset, uScale, vOffset, vScale, slide, strength, shadow, pad; };
    struct Shaded { float4 position [[position]]; float2 uv; float4 color; };

    vertex Shaded cloudVertex(uint id [[vertex_id]], const device CloudVertex *vertices [[buffer(0)]],
                              constant CloudUniforms &u [[buffer(1)]]) {
        CloudVertex v = vertices[id];
        Shaded out;
        out.position = float4((v.x + u.ox) / u.width * 2 - 1, 1 - (v.y + u.oy) / u.height * 2, 0, 1);
        out.uv = float2((v.lon - v.wind * u.slide + u.uOffset) * u.uScale, (u.vOffset - v.lat) * u.vScale);
        float a = u.strength * v.alpha;
        out.color = u.shadow > 0.5 ? float4(0, 0, 0, clamp(0.14 * a * v.light, 0.0, 1.0))
                                   : float4(v.light, v.light, min(v.light * 1.03, 1.0), clamp(a, 0.0, 1.0));
        return out;
    }

    fragment float4 cloudFragment(Shaded in [[stage_in]], texture2d<float> sky [[texture(0)]], sampler s [[sampler(0)]]) {
        float a = sky.sample(s, in.uv).r * in.color.a;
        return float4(in.color.rgb * a, a);
    }
    """
}
