import Metal
import QuartzCore
import UIKit

/// Step 2 forwards identity and coordinates only; it intentionally applies no gestures.
struct InputSample: Encodable {
    enum Source: String, Encodable { case pencil, finger, indirect, unknown }
    enum Phase: String, Encodable { case began, moved, ended, cancelled }
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
}

final class MetalCanvas: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer? { layer as? CAMetalLayer }
    var onResize: (() -> Void)?
    var onInput: ((InputSample) -> Void)?
    private var identities: [ObjectIdentifier: UInt64] = [:]
    private var nextIdentity: UInt64 = 1
    private var previousSize: CGSize = .zero

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
    }

    required init?(coder: NSCoder) { nil }

    func updateDrawableSize() {
        guard let metalLayer else { return }
        let scale = window?.screen.scale ?? traitCollection.displayScale
        guard scale.isFinite, scale > 0 else { return }
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        let size = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        guard size.width.isFinite, size.height.isFinite, size != previousSize else { return }
        previousSize = size
        metalLayer.drawableSize = size
        onResize?()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .began) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .moved) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .ended) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .cancelled) }

    private func forward(_ touches: Set<UITouch>, phase: InputSample.Phase) {
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
            let point = touch.location(in: self)
            let scale = contentScaleFactor
            let pressure = touch.maximumPossibleForce > 0 ? Double(touch.force / touch.maximumPossibleForce) : nil
            onInput?(InputSample(
                source: source, phase: phase, id: id,
                x_points: Double(point.x), y_points: Double(point.y),
                x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
                scale: Double(scale), timestamp: touch.timestamp,
                pressure: pressure, altitude: source == .pencil ? Double(touch.altitudeAngle) : nil
            ))
            if phase == .ended || phase == .cancelled { identities.removeValue(forKey: key) }
        }
    }
}
