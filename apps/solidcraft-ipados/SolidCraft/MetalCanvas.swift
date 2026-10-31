import Metal
import QuartzCore
import UIKit

final class MetalCanvas: UIView, UIGestureRecognizerDelegate, UIPencilInteractionDelegate {
    var connectedEdgeSelectionEnabled = true
    var displayingPreview = false
    var geometryRevision: UInt64?
    var viewportEpoch: UInt64?
    var cameraEpoch: UInt64?
    var documentID: String?
    var onCancel: (() -> Void)?
    var onPreviousTool: (() -> Void)?
    var onUndo: (() -> Void)?
    var onRedo: (() -> Void)?
    var historyGesturesEnabled = true {
        didSet { resetHistoryGestures() }
    }

    // The canvas owns these shortcuts; text fields retain UIKit's editing gestures.
    override var editingInteractionConfiguration: UIEditingInteractionConfiguration {
        historyGesturesEnabled ? .none : .default
    }

    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer? { layer as? CAMetalLayer }
    var onResize: (() -> Void)?
    var onAppearanceChange: (() -> Void)?
    var onInput: ((InputSample) -> Void)?
    var dimensionAt: ((CGPoint) -> String?)?
    var onDimensionTap: ((String) -> Void)?
    private var dimensionTaps = DimensionTapTracker()
    private var buttons: [ObjectIdentifier: UInt8] = [:]
    private var identities: [ObjectIdentifier: UInt64] = [:]
    private var nextIdentity: UInt64 = 1
    private var sequence: UInt64 = 0
    private var previousSize: CGSize = .zero
    private var previousScale: CGFloat = 0
    private var fingerOrigins: [ObjectIdentifier: CGPoint] = [:]
    private lazy var undoTap = historyTap(fingers: 2, action: #selector(undoTapped(_:)))
    private lazy var redoTap = historyTap(fingers: 3, action: #selector(redoTapped(_:)))

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
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (canvas: MetalCanvas, _: UITraitCollection) in
            canvas.onAppearanceChange?()
        }
        let hover = UIHoverGestureRecognizer(target: self, action: #selector(hovered(_:)))
        hover.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.indirectPointer.rawValue)]
        hover.cancelsTouchesInView = false
        addGestureRecognizer(hover)
        let pencilHover = UIHoverGestureRecognizer(target: self, action: #selector(pencilHovered(_:)))
        pencilHover.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.pencil.rawValue)]
        pencilHover.cancelsTouchesInView = false
        addGestureRecognizer(pencilHover)
        let pencil = UIPencilInteraction()
        pencil.delegate = self
        addInteraction(pencil)
        let scroll = UIPanGestureRecognizer(target: self, action: #selector(scrolled(_:)))
        scroll.allowedTouchTypes = []
        scroll.allowedScrollTypesMask = [.continuous, .discrete]
        scroll.cancelsTouchesInView = false
        scroll.delegate = self
        addGestureRecognizer(scroll)
        addGestureRecognizer(undoTap)
        addGestureRecognizer(redoTap)
        undoTap.require(toFail: redoTap)
    }

    required init?(coder: NSCoder) { nil }

    private func historyTap(fingers: Int, action: Selector) -> UITapGestureRecognizer {
        let tap = UITapGestureRecognizer(target: self, action: action)
        tap.numberOfTapsRequired = 2
        tap.numberOfTouchesRequired = fingers
        tap.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.direct.rawValue)]
        // Observe the original contacts without delaying navigation or synthesising input.
        tap.cancelsTouchesInView = false
        tap.delaysTouchesBegan = false
        tap.delaysTouchesEnded = false
        return tap
    }

    private func resetHistoryGestures() {
        for tap in [undoTap, redoTap] {
            tap.isEnabled = false
            tap.isEnabled = historyGesturesEnabled
        }
    }

    @objc private func undoTapped(_ tap: UITapGestureRecognizer) {
        if tap.state == .ended { onUndo?() }
    }

    @objc private func redoTapped(_ tap: UITapGestureRecognizer) {
        if tap.state == .ended { onRedo?() }
    }

    func updateDrawableSize() {
        guard let metalLayer else { return }
        let scale = window?.screen.scale ?? traitCollection.displayScale
        guard scale.isFinite, scale > 0 else { return }
        contentScaleFactor = scale
        metalLayer.contentsScale = scale
        let size = CGSize(width: (bounds.width * scale).rounded(), height: (bounds.height * scale).rounded())
        guard size.width.isFinite, size.height.isFinite, size != previousSize || scale != previousScale else { return }
        cancelInput()
        previousSize = size
        previousScale = scale
        metalLayer.drawableSize = size
        onResize?()
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .began, event: event) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .moved, event: event) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .ended, event: event) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { forward(touches, phase: .cancelled, event: event) }

    /// Native references are used only synchronously in this callback. Coalesced
    /// Pencil samples include the delivered touch, so it is not appended twice.
    private func forward(_ touches: Set<UITouch>, phase: InputSample.Phase, event: UIEvent?) {
        guard viewportEpoch != nil, cameraEpoch != nil, geometryRevision != nil, documentID != nil else { return }
        var samples: [InputSample] = []
        for touch in touches {
            let key = ObjectIdentifier(touch)
            if touch.type == .direct {
                let point = touch.preciseLocation(in: self)
                if phase == .began { fingerOrigins[key] = point }
                if let origin = fingerOrigins[key], hypot(point.x - origin.x, point.y - origin.y) > 6 {
                    // Rust starts finger navigation beyond six points. A short
                    // drag must not subsequently count as a history tap.
                    resetHistoryGestures()
                }
                if phase == .ended || phase == .cancelled { fingerOrigins.removeValue(forKey: key) }
            }
            if phase == .began, identities[key] == nil {
                identities[key] = nextIdentity
                nextIdentity &+= 1
                let button: UInt8 = event?.buttonMask.contains(.secondary) == true ? 1 : ((event?.buttonMask.rawValue ?? 0) & 4 != 0 ? 2 : 0)
                buttons[key] = button
            }
            guard let id = identities[key] else { continue }
            let source: InputSample.Source
            switch touch.type {
            case .pencil: source = .pencil
            case .direct: source = .finger
            case .indirect, .indirectPointer: source = .indirect
            @unknown default: source = .unknown
            }
            // End/cancel are always preserved. No predicted or estimated-property
            // callback can become an authoritative geometry sample.
            let actual = phase == .moved && source == .pencil ? (event?.coalescedTouches(for: touch) ?? [touch]) : [touch]
            for value in actual.suffix(32) {
                let point = value.preciseLocation(in: self)
                let scale = contentScaleFactor
                let pressure = value.maximumPossibleForce > 0 ? Double(value.force / value.maximumPossibleForce) : nil
                var sample = InputSample(source: source, phase: phase, id: id,
                    x_points: Double(point.x), y_points: Double(point.y),
                    x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
                    scale: Double(scale), timestamp: value.timestamp,
                    pressure: pressure, altitude: source == .pencil ? Double(value.altitudeAngle) : nil,
                    geometry_revision: geometryRevision, consumed: false,
                    shift: event?.modifierFlags.contains(.shift) == true, button: buttons[key] ?? 0, scroll: 0)
                sample.tap_count = connectedEdgeSelectionEnabled ? UInt32(clamping: value.tapCount) : 0
                sample.azimuth = source == .pencil ? Double(value.azimuthAngle(in: self)) : nil
                samples.append(sample)
            }
            if phase == .ended || phase == .cancelled { identities.removeValue(forKey: key); buttons.removeValue(forKey: key) }
        }
        samples.sort { $0.timestamp == $1.timestamp ? $0.id < $1.id : $0.timestamp < $1.timestamp }
        for sample in samples { deliver(sample) }
    }

    private func deliver(_ value: InputSample) {
        var sample = value
        sequence &+= 1
        sample.sequence = sequence
        guard viewportEpoch != nil, cameraEpoch != nil else { return }
        sample.viewport_epoch = viewportEpoch
        sample.camera_epoch = cameraEpoch
        sample.document_id = documentID
        sample.display_preview = displayingPreview
        var dimension: String?
        if let documentID, let geometryRevision, let viewportEpoch, let cameraEpoch {
            let context = DimensionTapTracker.Context(document: documentID, geometry: geometryRevision, viewport: viewportEpoch, camera: cameraEpoch)
            let point = CGPoint(x: sample.x_points, y: sample.y_points)
            switch sample.phase {
            case .began:
                let target = sample.button == 0 && !sample.shift ? dimensionAt?(point) : nil
                sample.sketch_annotation = target
                dimensionTaps.begin(id: sample.id, point: point, target: target, context: context)
            case .moved: dimensionTaps.move(id: sample.id, point: point)
            case .ended, .cancelled:
                dimension = dimensionTaps.end(id: sample.id, point: point, context: context, cancelled: sample.phase == .cancelled)
            default: break
            }
        }
        onInput?(sample)
        if let dimension { onDimensionTap?(dimension) }
    }

    func cancelInput() {
        dimensionTaps.cancel()
        resetHistoryGestures()
        fingerOrigins.removeAll()
        identities.removeAll(); buttons.removeAll()
        viewportEpoch = nil
        onCancel?()
    }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { cancelInput() }
        else { onAppearanceChange?() }
    }

    @objc private func hovered(_ recognizer: UIHoverGestureRecognizer) {
        let phase: InputSample.Phase = recognizer.state == .ended || recognizer.state == .cancelled ? .cancelled : .hover
        indirect(recognizer.location(in: self), phase: phase, scroll: 0)
    }

    @objc private func pencilHovered(_ recognizer: UIHoverGestureRecognizer) {
        if #available(iOS 17.5, *), !UIPencilInteraction.prefersHoverToolPreview { return }
        let phase: InputSample.Phase = recognizer.state == .ended || recognizer.state == .cancelled ? .cancelled : .hover
        let point = recognizer.location(in: self)
        let scale = contentScaleFactor
        var sample = InputSample(source: .pencil, phase: phase, id: 0,
            x_points: Double(point.x), y_points: Double(point.y),
            x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
            scale: Double(scale), timestamp: ProcessInfo.processInfo.systemUptime,
            pressure: nil, altitude: Double(recognizer.altitudeAngle), geometry_revision: geometryRevision,
            consumed: false, shift: false, button: 0, scroll: 0)
        sample.azimuth = Double(recognizer.azimuthAngle(in: self))
        deliver(sample)
    }

    // UIKit dispatches these only for capable Pencil hardware. New and legacy tap
    // delegates are mutually exclusive on iPadOS 17.5+, per the SDK contract.
    func pencilInteractionDidTap(_ interaction: UIPencilInteraction) {
        if UIPencilInteraction.preferredTapAction == .switchPrevious { onPreviousTool?() }
    }
    @available(iOS 17.5, *)
    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveTap tap: UIPencilInteraction.Tap) {
        if UIPencilInteraction.preferredTapAction == .switchPrevious { onPreviousTool?() }
    }
    @available(iOS 17.5, *)
    func pencilInteraction(_ interaction: UIPencilInteraction, didReceiveSqueeze squeeze: UIPencilInteraction.Squeeze) {
        if squeeze.phase == .ended, UIPencilInteraction.preferredSqueezeAction == .switchPrevious { onPreviousTool?() }
    }

    @objc private func scrolled(_ recognizer: UIPanGestureRecognizer) {
        let delta = recognizer.translation(in: self).y
        recognizer.setTranslation(.zero, in: self)
        if delta != 0 { indirect(recognizer.location(in: self), phase: .scroll, scroll: Double(delta)) }
    }

    private func indirect(_ point: CGPoint, phase: InputSample.Phase, scroll: Double) {
        let scale = contentScaleFactor
        deliver(InputSample(source: .indirect, phase: phase, id: 0,
            x_points: Double(point.x), y_points: Double(point.y),
            x_pixels: Double(point.x * scale), y_pixels: Double(point.y * scale),
            scale: Double(scale), timestamp: ProcessInfo.processInfo.systemUptime,
            pressure: nil, altitude: nil, geometry_revision: geometryRevision,
            consumed: false, shift: false, button: 0, scroll: scroll))
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer) -> Bool { true }

}
