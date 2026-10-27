import Foundation
import QuartzCore

struct BridgeError: Error, CustomStringConvertible {
    let code: String
    let message: String
    var description: String { "\(code): \(message)" }
}

/// CAD work executes in order off the window thread. A separate render-only lock also allows
/// synchronous main-thread suspension before backgrounding. UIKit owns layer geometry, retained
/// until the Rust renderer has released the corresponding surface.
final class NativeSession {
    var onFailure: ((BridgeError) -> Void)?
    var onFrame: ((Bool) -> Void)?
    private let queue = DispatchQueue(label: "SolidCraft.engine", qos: .userInitiated, autoreleaseFrequency: .workItem)
    private let state = BridgeState()
    private let lifecycle = LifecycleGate()
    private var alive = true // main thread only; filters callbacks after scene disconnection

    func start(completion: @escaping (Result<String, BridgeError>) -> Void) {
        queue.async { [weak self, state] in
            let result = Result { try state.start() }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard self?.alive == true else { return }
                completion(result)
            }
        }
    }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32) {
        guard let ticket = lifecycle.ticket else { return }
        draw(ticket: ticket, retries: 3) { state in
            try state.present(layer: layer, width: width, height: height)
        }
    }

    func activate() {
        lifecycle.activate()
    }

    func record(_ sample: InputSample) {
        // No navigation or CAD mutation occurs here; Step 3 consumes the typed Rust event.
        performInput { state in
            let event = try JSONEncoder().encode(sample)
            let payload = Data("{\"kind\":\"input\",\"event\":".utf8) + event + Data("}".utf8)
            _ = try state.request(payload)
        }
    }

    func suspend() {
        // Called from sceneWillResignActive on main. The same lock surrounds every GPU
        // operation, closing the check/submit race before UIKit can background the scene.
        // Before attach there is no renderer: this never enters Rust while proof evaluates.
        do { try lifecycle.suspend { try state.suspend() } }
        catch { if alive { onFailure?(BridgeState.error(error)) } }
    }

    func stop() {
        guard alive else { return }
        suspend()
        alive = false
        onFailure = nil
        onFrame = nil
        queue.async { [state, lifecycle] in lifecycle.exclusive { state.close() } }
    }

    private func performInput(_ operation: @escaping (BridgeState) throws -> Void) {
        guard alive else { return }
        queue.async { [weak self, state, lifecycle] in
            // Only the bounded identity/coordinate transport uses this path. CAD evaluation
            // must remain outside this lock. Serializing input also protects response len/copy
            // against the synchronous suspend operation replacing the session's response.
            do { try lifecycle.input { try operation(state) } }
            catch {
                let failure = BridgeState.error(error)
                DispatchQueue.main.async { [weak self] in
                    guard self?.alive == true else { return }
                    self?.onFailure?(failure)
                }
            }
        }
    }

    private func draw(ticket: UInt64, retries: Int, operation: @escaping (BridgeState) throws -> Bool?) {
        guard alive else { return }
        queue.async { [weak self, state, lifecycle] in
            do {
                guard let frame = try lifecycle.rendering(ticket: ticket, state: state, operation),
                      let presented = frame else { return }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, self.lifecycle.accepts(ticket) else { return }
                    self.onFrame?(presented)
                    if !presented, retries > 0 {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                            guard let self, self.alive, self.lifecycle.accepts(ticket) else { return }
                            self.draw(ticket: ticket, retries: retries - 1) { state in
                                try state.redraw()
                            }
                        }
                    }
                }
            } catch {
                let failure = BridgeState.error(error)
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, self.lifecycle.accepts(ticket) else { return }
                    self.onFailure?(failure)
                }
            }
        }
    }

    deinit {
        do { try lifecycle.suspend { try state.suspend() } }
        catch { NSLog("%@", BridgeState.error(error).description) }
        queue.async { [state, lifecycle] in lifecycle.exclusive { state.close() } }
    }
}

/// The render-only lock is independent of startup CAD work, and is never held for commands.
/// All attach/resize/draw/suspend/destroy operations use it. Scene resign-active closes it
/// synchronously, so a queued render cannot pass a stale lifecycle check and submit afterward.
/// Step 3 must keep expensive CAD commands independent of the Rust renderer's lock as well.
private final class LifecycleGate {
    private let lock = NSLock()
    private var active = false
    private var generation: UInt64 = 0
    private var hasRenderer = false

    var ticket: UInt64? {
        lock.lock()
        defer { lock.unlock() }
        return active ? generation : nil
    }

    func activate() {
        lock.lock()
        active = true
        generation &+= 1
        lock.unlock()
    }

    func suspend(_ operation: () throws -> Void) throws {
        lock.lock()
        defer { lock.unlock() }
        active = false
        generation &+= 1
        if hasRenderer { try operation() }
    }

    func rendering<T>(ticket: UInt64, state: BridgeState, _ operation: (BridgeState) throws -> T) rethrows -> T? {
        lock.lock()
        defer {
            hasRenderer = state.hasSurface
            lock.unlock()
        }
        guard active, ticket == generation else { return nil }
        return try operation(state)
    }

    func exclusive(_ operation: () -> Void) {
        lock.lock()
        defer { lock.unlock() }
        operation()
        hasRenderer = false
    }

    func input(_ operation: () throws -> Void) rethrows {
        lock.lock()
        defer { lock.unlock() }
        try operation()
    }

    func accepts(_ ticket: UInt64) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return active && ticket == generation
    }
}

/// CAD methods run on NativeSession.queue; renderer methods require LifecycleGate's lock.
/// No Rust pointers or borrowed render buffers escape.
private final class BridgeState {
    private var handle: UInt64 = 0
    private var layer: CAMetalLayer?
    private var width: UInt32 = 0
    private var height: UInt32 = 0
    private var attached = false
    private var closed = false
    var hasSurface: Bool { layer != nil }

    func start() throws -> String {
        guard !closed else { throw BridgeError(code: "closed", message: "session") }
        handle = sc_session_create()
        guard handle != 0 else { throw BridgeError(code: "session_create", message: "handle=0") }
        return try request(Data("{\"kind\":\"proof\"}".utf8))
    }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32) throws -> Bool? {
        guard handle != 0, !closed else { return nil }
        if !attached || self.layer !== layer {
            try check(sc_surface_attach(handle, Unmanaged.passUnretained(layer).toOpaque(), width, height))
            self.layer = layer
            attached = true
        } else if self.width != width || self.height != height {
            try check(sc_surface_resize(handle, width, height))
        } else { return nil }
        self.width = width
        self.height = height
        return try redraw()
    }

    func redraw() throws -> Bool? {
        guard attached, handle != 0, !closed else { return nil }
        try check(sc_surface_draw(handle))
        let envelope = try JSONDecoder().decode(Envelope.self, from: response())
        return envelope.result?.presented == true
    }

    func suspend() throws {
        guard handle != 0, !closed, attached else { return }
        try check(sc_surface_suspend(handle))
        attached = false
        // The Rust surface still owns a reference to the layer; retain through destroy/reattach.
    }

    func close() {
        guard !closed else { return }
        closed = true
        if handle != 0 {
            _ = sc_surface_suspend(handle)
            _ = sc_session_destroy(handle)
            handle = 0
        }
        layer = nil
        attached = false
    }

    func request(_ payload: Data) throws -> String {
        guard handle != 0, !closed else { throw BridgeError(code: "closed", message: "session") }
        let status: Int32 = payload.withUnsafeBytes { buffer in
            sc_request(handle, buffer.bindMemory(to: UInt8.self).baseAddress, buffer.count)
        }
        try check(status)
        let data = try response()
        let envelope = try JSONDecoder().decode(Envelope.self, from: data)
        if !envelope.ok {
            throw BridgeError(code: envelope.error?.code ?? "engine", message: envelope.error?.message ?? "response")
        }
        return String(decoding: data, as: UTF8.self)
    }

    private func check(_ status: Int32) throws {
        guard status != 0 else { return }
        if let data = try? response(), let envelope = try? JSONDecoder().decode(Envelope.self, from: data), let error = envelope.error {
            throw BridgeError(code: error.code, message: error.message)
        }
        throw BridgeError(code: "bridge", message: "status=\(status)")
    }

    private func response() throws -> Data {
        let count = sc_response_len(handle)
        guard count > 0, count <= 1_048_576 else { throw BridgeError(code: "response_size", message: "bytes=\(count)") }
        var data = Data(count: count)
        let status = data.withUnsafeMutableBytes { buffer in
            sc_response_copy(handle, buffer.bindMemory(to: UInt8.self).baseAddress, buffer.count)
        }
        guard status == 0 else { throw BridgeError(code: "response_copy", message: "status=\(status)") }
        return data
    }

    static func error(_ error: Error) -> BridgeError {
        (error as? BridgeError) ?? BridgeError(code: "native", message: error.localizedDescription)
    }

    private struct Envelope: Decodable {
        struct Failure: Decodable { let code: String; let message: String }
        struct Frame: Decodable { let presented: Bool? }
        let ok: Bool
        let error: Failure?
        let result: Frame?
    }
}
