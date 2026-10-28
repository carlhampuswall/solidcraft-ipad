import Metal
import QuartzCore
import UIKit

/// UIKit captures source identity and local coordinates. Rust owns gesture policy and camera math.
struct InputSample: Encodable {
    enum Source: String, Encodable { case pencil, finger, indirect, unknown }
    enum Phase: String, Encodable { case began, moved, ended, cancelled, hover, scroll }
    let source: Source
    let phase: Phase
    let id: UInt64
    let x_points: Double
    let y_points: Double
    let x_pixels: Double
    let y_pixels: Double
    let scale: Double
    let timestamp: Double
    let pressure: Double?
    let altitude: Double?
    let geometry_revision: UInt64?
    let consumed: Bool
    let shift: Bool
    let button: UInt8
    let scroll: Double
}

final class MetalCanvas: UIView, UIGestureRecognizerDelegate {
    var geometryRevision: UInt64?
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer? { layer as? CAMetalLayer }
    var onResize: (() -> Void)?
    var onInput: ((InputSample) -> Void)?
    private var buttons: [ObjectIdentifier: UInt8] = [:]
    private var identities: [ObjectIdentifier: UInt64] = [:]
    private var nextIdentity: UInt64 = 1
    private var previousSize: CGSize = .zero
    private var previousScale: CGFloat = 0

    var pixelSize: (width: UInt32, height: UInt32)? {
        guard let layer = metalLayer else { return nil }
        let size = layer.drawableSize
        guard size.width.isFinite, size.height.isFinite,
              size.width >= 0, size.height >= 0,
              size.width <= 8192, size.height <= 8192
        else { return nil }
        return (UInt32(size.width), UInt32(size.height))
    }

    init() {
        super.init(frame: .zero)
        isMultipleTouchEnabled = true
        isOpaque = true
        backgroundColor = .secondarySystemBackground
        metalLayer?.pixelFormat = .bgra8Unorm
        metalLayer?.framebufferOnly = true
        metalLayer?.isOpaque = true
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(hovered(_:)))
        hover.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.indirectPointer.rawValue)]
        addGestureRecognizer(hover)
        let scroll = UIPanGestureRecognizer(target: self, action: #selector(scrolled(_:)))
        scroll.allowedTouchTypes = []
        scroll.allowedScrollTypesMask = [.continuous, .discrete]
        scroll.cancelsTouchesInView = false
        scroll.delegate = self
        addGestureRecognizer(scroll)
    }

    required init?(coder: NSCoder) { nil }

    func updateDrawableSize() {
        guard let metalLayer else { return }
        let scale = window?.screen.scale ?? traitCollection.displayScale
        guard scale.isFinite, scale > 0 else { return }
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        let size = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        guard size.width.isFinite, size.height.isFinite, size != previousSize || scale != previousScale else { return }
        previousSize = size
        previousScale = scale
        metalLayer.drawableSize = size
        onResize?()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .began, event: event) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .moved, event: event) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .ended, event: event) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .cancelled, event: event) }

    private func forward(_ touches: Set<UITouch>, phase: InputSample.Phase, event: UIEvent?) {
        for touch in touches {
            let key = ObjectIdentifier(touch)
            let id: UInt64
            if let existing = identities[key] { id = existing }
            else {
                id = nextIdentity
                nextIdentity &+= 1
                identities[key] = id
            }
            let source: InputSample.Source
            switch touch.type {
            case .pencil: source = .pencil
            case .direct: source = .finger
            case .indirect, .indirectPointer: source = .indirect
            @unknown default: source = .unknown
            }
            let currentButton: UInt8 = event?.buttonMask.contains(.secondary) == true ? 1 : ((event?.buttonMask.rawValue ?? 0) & 4 != 0 ? 2 : 0)
            if phase == .began { buttons[key] = currentButton }
            let point = touch.location(in: self)
            let scale = contentScaleFactor
            let pressure = touch.maximumPossibleForce > 0 ? Double(touch.force / touch.maximumPossibleForce) : nil
            onInput?(InputSample(
                source: source, phase: phase, id: id,
                x_points: Double(point.x), y_points: Double(point.y),
                x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
                scale: Double(scale), timestamp: touch.timestamp,
                pressure: pressure, altitude: source == .pencil ? Double(touch.altitudeAngle) : nil,
                geometry_revision: geometryRevision, consumed: false,
                shift: event?.modifierFlags.contains(.shift) == true,
                button: buttons[key] ?? currentButton,
                scroll: 0
            ))
            if phase == .ended || phase == .cancelled { identities.removeValue(forKey: key); buttons.removeValue(forKey: key) }
        }
    }
    @objc private func hovered(_ recognizer: UIHoverGestureRecognizer) {
        let phase: InputSample.Phase = recognizer.state == .ended || recognizer.state == .cancelled ? .cancelled : .hover
        indirect(recognizer.location(in: self), phase: phase, scroll: 0)
    }

    @objc private func scrolled(_ recognizer: UIPanGestureRecognizer) {
        let delta = recognizer.translation(in: self).y
        recognizer.setTranslation(.zero, in: self)
        if delta != 0 { indirect(recognizer.location(in: self), phase: .scroll, scroll: Double(delta)) }
    }

    private func indirect(_ point: CGPoint, phase: InputSample.Phase, scroll: Double) {
        let scale = contentScaleFactor
        onInput?(InputSample(source: .indirect, phase: phase, id: 0,
            x_points: Double(point.x), y_points: Double(point.y),
            x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
            scale: Double(scale), timestamp: ProcessInfo.processInfo.systemUptime,
            pressure: nil, altitude: nil, geometry_revision: geometryRevision,
            consumed: false, shift: false, button: 0, scroll: scroll))
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool { true }

}
