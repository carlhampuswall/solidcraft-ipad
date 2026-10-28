import Foundation
import QuartzCore

struct BridgeError: Error, CustomStringConvertible {
    let code: String
    let message: String
    var description: String { "\(code): \(message)" }
}

/// Immutable handle, one CAD worker, one input/render worker. Only surface calls enter
/// LifecycleGate; the main-thread background suspension cannot wait for CAD evaluation.
final class NativeSession {
    var onFailure: ((BridgeError) -> Void)?
    var onFrame: ((Bool, UInt64?, String) -> Void)?
    var onStatus: ((String) -> Void)?
    private let engine = DispatchQueue(label: "SolidCraft.engine", qos: .userInitiated, autoreleaseFrequency: .workItem)
    private let display = DispatchQueue(label: "SolidCraft.viewport", qos: .userInteractive, autoreleaseFrequency: .workItem)
    private let state = BridgeState()
    private let lifecycle = LifecycleGate()
    private var alive = true // main only

    func start(completion: @escaping (Result<String, BridgeError>) -> Void) {
        engine.async { [weak self, state] in
            let result = Result { try state.request(Data("{\"kind\":\"fixture\"}".utf8), lane: .engine) }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive else { return }
                completion(result.map { String(decoding: $0, as: UTF8.self) })
                self.redraw()
            }
        }
    }

    func command(_ command: String, params: [String: Any] = [:]) {
        guard alive else { return }
        let isView = command.hasPrefix("view.") || command.hasPrefix("select.")
        let lane: BridgeState.Lane = isView ? .viewport : .engine
        let queue = isView ? display : engine
        do {
            let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params])
            if !isView { onStatus?(NativeText.value("ui.ipados.updating")) }
            queue.async { [weak self, state] in
                do {
                    _ = try state.request(data, lane: lane)
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else { return }
                        self.onStatus?("")
                        self.redraw()
                    }
                } catch { self?.failure(error) }
            }
        } catch { onFailure?(BridgeState.error(error)) }
    }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32, scale: Double) {
        guard alive, let ticket = lifecycle.ticket else { return }
        display.async { [weak self, state, lifecycle] in
            do {
                let data = try JSONSerialization.data(withJSONObject: ["kind": "viewport", "metrics": [
                    "width_points": Double(width) / scale, "height_points": Double(height) / scale, "scale": scale
                ]])
                _ = try state.request(data, lane: .viewport)
                _ = try lifecycle.rendering(ticket: ticket, state: state) {
                    try state.present(layer: layer, width: width, height: height)
                }
                DispatchQueue.main.async { [weak self] in self?.redraw() }
            } catch { self?.failure(error) }
        }
    }

    func record(_ sample: InputSample) {
        guard alive, let ticket = lifecycle.ticket else { return }
        display.async { [weak self, state, lifecycle] in
            guard lifecycle.accepts(ticket) else { return }
            do {
                let event = try JSONEncoder().encode(sample)
                let payload = Data("{\"kind\":\"input\",\"event\":".utf8) + event + Data("}".utf8)
                _ = try state.request(payload, lane: .viewport)
                DispatchQueue.main.async { [weak self] in self?.redraw() }
            } catch { self?.failure(error) }
        }
    }

    func activate() { lifecycle.activate() }

    func suspend() {
        do { try lifecycle.suspend { try state.suspend() } }
        catch { if alive { onFailure?(BridgeState.error(error)) } }
    }

    func stop() {
        guard alive else { return }
        suspend()
        alive = false
        onFailure = nil
        onFrame = nil
        onStatus = nil
        // Engine work finishes before the display queue releases the retained layer/handle.
        engine.async { [state, lifecycle, display] in
            display.async { lifecycle.exclusive { state.close() } }
        }
    }

    private var redrawAgain = false
    private var redrawPending = false // main only; coalesce bursty touch callbacks
    private func redraw(retries: Int = 2) {
        guard alive, let ticket = lifecycle.ticket else { return }
        if redrawPending { redrawAgain = true; return }
        redrawPending = true
        display.async { [weak self, state, lifecycle] in
            do {
                let frame = try lifecycle.rendering(ticket: ticket, state: state) { try state.redraw() }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.redrawPending = false
                    guard self.lifecycle.accepts(ticket), let frame, let frame else { return }
                    self.onFrame?(frame.presented == true, frame.geometry_revision, frame.viewport?.readout ?? "")
                    if self.redrawAgain { self.redrawAgain = false; self.redraw() }
                    if frame.presented != true, retries > 0 {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in self?.redraw(retries: retries - 1) }
                    }
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.redrawPending = false
                    self.onFailure?(BridgeState.error(error))
                    // Bounded resource recreation; the CAD handle and immutable scene survive.
                    if retries > 0, self.lifecycle.accepts(ticket) {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in self?.redraw(retries: retries - 1) }
                    }
                }
            }
        }
    }

    #if DEBUG
    /// Opt-in repeatable device smoke workflow. Samples are synthetic: this verifies the
    /// native boundary and renderer, never actual finger/Pencil hardware or hover support.
    func developmentCheck() {
        let steps: [(String, [String: Any])] = [
            ("view.navigate", ["motion": "orbit", "delta": [45, 20]]),
            ("view.navigate", ["motion": "pan", "delta": [24, 12], "viewport": [800, 600]]),
            ("view.navigate", ["motion": "zoom", "factor": 0.8]),
            ("view.fit", [:]),
            ("view.front", [:]), ("view.back", [:]), ("view.left", [:]), ("view.right", [:]),
            ("view.bottom", [:]), ("view.isometric", [:]), ("view.top", [:]),
            ("tap.body", ["point": [-8, 0, 10]]), ("tap.face", ["point": [-8, 0, 10]]),
            ("tap.edge", ["point": [0, -15, 10]]), ("tap.vertex", ["point": [-20, -15, 10]]),
            ("select.multiple", ["enabled": true]), ("tap.face", ["point": [-8, 0, 10]]),
            ("select.clear", [:]), ("view.perspective", ["enabled": true]),
            ("view.isometric", [:]),
            ("timeline.edit", ["feature": "Viewport post", "set": ["height": "40"]]),
            ("edit.undo", [:]), ("edit.redo", [:])
        ]
        developmentStep(steps, index: 0)
    }

    private func developmentStep(_ steps: [(String, [String: Any])], index: Int) {
        guard alive, steps.indices.contains(index) else {
            NSLog("SolidCraft Step 3 development check complete (synthetic input)")
            return
        }
        let (command, params) = steps[index]
        let isTap = command.hasPrefix("tap.")
        let isDisplay = isTap || command.hasPrefix("view.") || command.hasPrefix("select.")
        let queue = isDisplay ? display : engine
        queue.async { [weak self, state] in
            do {
                if isTap {
                    let filter = String(command.dropFirst(4))
                    _ = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "select.filter", "params": ["filter": filter]]), lane: .viewport)
                    let project = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "view.project", "params": params]), lane: .viewport)
                    let projected = try JSONDecoder().decode(DevelopmentProjection.self, from: project).result
                    let p = projected.points
                    guard p.count == 2 else { throw BridgeError(code: "projection", message: "expected two coordinates") }
                    for phase in [InputSample.Phase.began, .ended] {
                        let sample = InputSample(source: .pencil, phase: phase, id: 1000,
                            x_points: p[0], y_points: p[1], x_pixels: p[0] * projected.scale, y_pixels: p[1] * projected.scale,
                            scale: projected.scale, timestamp: ProcessInfo.processInfo.systemUptime, pressure: nil, altitude: nil,
                            geometry_revision: projected.geometry_revision, consumed: false, shift: false, button: 0, scroll: 0)
                        let event = try JSONEncoder().encode(sample)
                        let reply = try state.request(Data("{\"kind\":\"input\",\"event\":".utf8) + event + Data("}".utf8), lane: .viewport)
                        if phase == .ended {
                            let report = try JSONDecoder().decode(DevelopmentPick.self, from: reply).result
                            guard report.selection.last?.type == filter else {
                                throw BridgeError(code: "development_pick", message: "expected " + filter)
                            }
                            NSLog("SolidCraft Step 3 synthetic pick: %@", report.readout)
                        }
                    }
                } else {
                    _ = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params]), lane: isDisplay ? .viewport : .engine)
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    NSLog("SolidCraft Step 3 check %d: %@", index, command)
                    self.redraw()
                    if index == 12, ProcessInfo.processInfo.arguments.contains("--viewport-check-hold-face") { return }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.developmentStep(steps, index: index + 1) }
                }
            } catch { self?.failure(error) }
        }
    }
    private struct DevelopmentPick: Decodable { let result: BridgeState.ViewportReport }
    private struct DevelopmentProjection: Decodable {
        struct Point: Decodable { let points: [Double]; let scale: Double; let geometry_revision: UInt64 }
        let result: Point
    }
    #endif

    private func failure(_ error: Error) {
        let failure = BridgeState.error(error)
        DispatchQueue.main.async { [weak self] in
            guard let self, self.alive else { return }
            self.onFailure?(failure)
        }
    }

    deinit {
        do { try lifecycle.suspend { try state.suspend() } }
        catch { NSLog("%@", BridgeState.error(error).description) }
        engine.async { [state, lifecycle, display] in display.async { lifecycle.exclusive { state.close() } } }
    }
}

private final class LifecycleGate {
    private let lock = NSLock()
    private var active = false
    private var generation: UInt64 = 0
    private var hasRenderer = false
    var ticket: UInt64? { lock.lock(); defer { lock.unlock() }; return active ? generation : nil }

    func activate() { lock.lock(); active = true; generation &+= 1; lock.unlock() }
    func suspend(_ operation: () throws -> Void) throws {
        lock.lock(); defer { lock.unlock() }
        active = false; generation &+= 1
        if hasRenderer { try operation() }
    }
    func rendering<T>(ticket: UInt64, state: BridgeState, _ operation: () throws -> T) rethrows -> T? {
        lock.lock(); defer { hasRenderer = state.hasSurface; lock.unlock() }
        guard active, ticket == generation else { return nil }
        return try operation()
    }
    func exclusive(_ operation: () -> Void) { lock.lock(); defer { lock.unlock() }; operation(); hasRenderer = false }
    func accepts(_ ticket: UInt64) -> Bool { lock.lock(); defer { lock.unlock() }; return active && ticket == generation }
}

private final class BridgeState {
    enum Lane { case engine, viewport, surface }
    private let handle: UInt64 = sc_session_create()
    private var layer: CAMetalLayer? // display queue, or lifecycle lock during suspension
    private var width: UInt32 = 0
    private var height: UInt32 = 0
    private var closed = false
    private var attached = false
    var hasSurface: Bool { layer != nil }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32) throws {
        if !attached || self.layer !== layer {
            try check(sc_surface_attach(handle, Unmanaged.passUnretained(layer).toOpaque(), width, height), lane: .surface)
            self.layer = layer; attached = true
        } else if self.width != width || self.height != height {
            try check(sc_surface_resize(handle, width, height), lane: .surface)
        }
        self.width = width; self.height = height
    }

    func redraw() throws -> FrameReport? {
        guard handle != 0, let layer else { return nil }
        if !attached { try present(layer: layer, width: width, height: height) }
        do {
            try check(sc_surface_draw(handle), lane: .surface)
            return try JSONDecoder().decode(Envelope<FrameReport>.self, from: response(.surface)).result
        } catch { attached = false; throw error }
    }
    func suspend() throws {
        guard handle != 0, attached else { return }
        try check(sc_surface_suspend(handle), lane: .surface)
        attached = false
    }
    func close() {
        guard !closed else { return }
        closed = true
        _ = sc_surface_suspend(handle)
        _ = sc_session_destroy(handle)
        layer = nil; attached = false
    }
    func request(_ data: Data, lane: Lane) throws -> Data {
        guard handle != 0 else { throw BridgeError(code: "session_create", message: "handle=0") }
        let status = data.withUnsafeBytes { buffer in
            let bytes = buffer.bindMemory(to: UInt8.self)
            return lane == .engine ? sc_request(handle, bytes.baseAddress, bytes.count) : sc_view_request(handle, bytes.baseAddress, bytes.count)
        }
        try check(status, lane: lane)
        return try response(lane)
    }
    private func check(_ status: Int32, lane: Lane) throws {
        guard status != 0 else { return }
        if let data = try? response(lane), let envelope = try? JSONDecoder().decode(Envelope<Empty>.self, from: data), let error = envelope.error {
            throw BridgeError(code: error.code, message: error.message)
        }
        throw BridgeError(code: "bridge", message: "status=\(status)")
    }
    private func response(_ lane: Lane) throws -> Data {
        let count: Int
        switch lane {
        case .engine: count = sc_response_len(handle)
        case .viewport: count = sc_view_response_len(handle)
        case .surface: count = sc_surface_response_len(handle)
        }
        guard count > 0, count <= 4_194_304 else { throw BridgeError(code: "response_size", message: "bytes=\(count)") }
        var data = Data(count: count)
        let status = data.withUnsafeMutableBytes { buffer -> Int32 in
            let bytes = buffer.bindMemory(to: UInt8.self)
            switch lane {
            case .engine: return sc_response_copy(handle, bytes.baseAddress, bytes.count)
            case .viewport: return sc_view_response_copy(handle, bytes.baseAddress, bytes.count)
            case .surface: return sc_surface_response_copy(handle, bytes.baseAddress, bytes.count)
            }
        }
        guard status == 0 else { throw BridgeError(code: "response_copy", message: "status=\(status)") }
        return data
    }
    static func error(_ error: Error) -> BridgeError { (error as? BridgeError) ?? BridgeError(code: "native", message: error.localizedDescription) }
    private struct Empty: Decodable {}
    private struct Envelope<T: Decodable>: Decodable {
        struct Failure: Decodable { let code: String; let message: String }
        let ok: Bool
        let error: Failure?
        let result: T?
    }
    struct FrameReport: Decodable {
        let presented: Bool?
        let geometry_revision: UInt64?
        let viewport: ViewportReport?
    }
    struct ViewportReport: Decodable {
        struct Entity: Decodable {
            let type: String
            let name: String?
            let body: String?
            let index: Int?
            let point: [Double]?
            var readout: String {
                let id = index.map(String.init) ?? point.map { $0.map { String(format: "%.3g", $0) }.joined(separator: ",") } ?? name ?? ""
                return "\(type) · \(body ?? name ?? "") · \(id)"
            }
        }
        let selection: [Entity]
        let hover: Entity?
        let geometry_revision: UInt64?
        let updating: Bool
        var readout: String {
            let picks = selection.isEmpty ? NativeText.value("ui.ipados.no_selection") : selection.prefix(4).map(\.readout).joined(separator: " | ")
            let preselect = hover.map { " · \(NativeText.value("ui.ipados.hover")): \($0.readout)" } ?? ""
            return "r\(geometry_revision ?? 0) · \(picks)\(preselect)" + (updating ? " · \(NativeText.value("ui.ipados.updating"))" : "")
        }
    }
}
