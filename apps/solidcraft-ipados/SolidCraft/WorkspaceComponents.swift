import UIKit

enum BrowserText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.browser." + key) }
}

enum ViewText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.view." + key) }
}

enum HistoryText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.history." + key) }
}

enum InspectorText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.inspector." + key) }
}

/// Native presentation tokens stay inside the approved UIKit boundary. Geometry,
/// camera colours and highlights continue to come from the shared renderer.
enum WorkspaceStyle {
    static let spacing: CGFloat = 12
    static let inset: CGFloat = 16
    static let radius: CGFloat = 14
    static let touch: CGFloat = 44
    static let background = UIColor.systemBackground
    static let surface = UIColor.secondarySystemBackground
    static let separator = UIColor.separator
    static let fieldBackground = UIColor.tertiarySystemBackground
    static let selectionFill = UIColor.tertiarySystemFill
    static let scrim = UIColor.systemBackground.withAlphaComponent(0.45)
    static let foreground = UIColor.label
    static let secondary = UIColor.secondaryLabel
    static let accent = UIColor.systemBlue
    static let selectedForeground = UIColor.white
    static let error = UIColor.systemRed

    static func floatingSurface(_ view: UIView) {
        view.backgroundColor = surface.withAlphaComponent(0.96)
        view.layer.cornerRadius = radius
        view.layer.borderWidth = 0.5
        view.layer.borderColor = separator.cgColor
        view.clipsToBounds = true
    }

    static func text(_ key: String) -> String { NativeText.value("ui.ipados.workspace." + key) }
    static func label(_ style: UIFont.TextStyle = .body, secondary: Bool = false) -> UILabel {
        let label = UILabel()
        label.font = .preferredFont(forTextStyle: style)
        label.adjustsFontForContentSizeCategory = true
        label.textColor = secondary ? self.secondary : foreground
        label.numberOfLines = 0
        return label
    }
}

/// The native image cache consumes Xcode's vector assets generated from the same
/// SVG masters, semantic aliases and palette as egui. No native icon registry.
enum WorkspaceIcon {
    private static let cache = NSCache<NSString, UIImage>()
    private static let commands: [String: String] = {
        guard let url = Bundle.main.url(forResource: "SolidCraftCommandIcons", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let values = try? JSONDecoder().decode([String: String].self, from: data) else { return [:] }
        cache.countLimit = 256
        return values
    }()

    static func command(_ id: String, traits: UITraitCollection, size: CGFloat = 28) -> UIImage? {
        guard let icon = commands[id] else { return nil }
        return image(icon, traits: traits, size: size)
    }

    static func image(_ id: String, traits: UITraitCollection, size: CGFloat = 28) -> UIImage? {
        let key = "\(id):\(traits.userInterfaceStyle.rawValue):\(traits.displayScale):\(size)" as NSString
        if let cached = cache.object(forKey: key) { return cached }
        let name = (size * max(1, traits.displayScale) <= 24 ? "sc-small-" : "sc-") + id
        guard let source = UIImage(named: name, in: .main, compatibleWith: traits) else { return nil }
        let format = UIGraphicsImageRendererFormat()
        format.scale = max(1, traits.displayScale)
        let rendered = UIGraphicsImageRenderer(size: CGSize(width: size, height: size), format: format).image { _ in
            source.draw(in: CGRect(x: 0, y: 0, width: size, height: size))
        }.withRenderingMode(.alwaysOriginal)
        cache.setObject(rendered, forKey: key)
        return rendered
    }
}

/// A compact entry point expands into the engine's ordered feature history.
/// IDs and selected state always come from the current workspace projection.
private final class HistoryMarkerView: UIView {
    var onStep: ((Int) -> Void)?
    override func accessibilityIncrement() { onStep?(1) }
    override func accessibilityDecrement() { onStep?(-1) }
}

final class FeatureHistoryBar: UIView, UIGestureRecognizerDelegate {
    var onPreferredSizeChange: (() -> Void)?
    var onToggle: (() -> Void)?
    var onSelect: ((UInt64) -> Void)?
    var onShowActions: ((UInt64, UIView) -> Void)?
    var onMove: ((Int) -> Void)?
    var onScrubBegan: (() -> Void)?
    var onScrubMove: ((Int, @escaping (Bool) -> Void) -> Void)?
    private lazy var toggle = WorkspaceButton(HistoryText.text("title")) { [weak self] in self?.onToggle?() }
    private lazy var back = WorkspaceButton(HistoryText.text("back")) { [weak self] in
        guard let self, !self.scrub.active else { return }; self.onMove?(max(0, self.position - 1))
    }
    private lazy var forward = WorkspaceButton(HistoryText.text("forward")) { [weak self] in
        guard let self, !self.scrub.active else { return }; self.onMove?(self.position < self.features.count ? self.position + 1 : self.features.count)
    }
    private let transport = UIStackView()
    private let marker = HistoryMarkerView()
    private lazy var markerPan = UIPanGestureRecognizer(target: self, action: #selector(dragMarker(_:)))
    private let scroll = UIScrollView()
    private let items = UIStackView()
    private let empty = WorkspaceStyle.label(.footnote, secondary: true)
    private var features: [ModellingPresentation.Feature] = []
    private var icons: [UInt64: String] = [:]
    private var selected: Set<UInt64> = []
    private var buttons: [UInt64: WorkspaceButton] = [:]
    private var needsRebuild = true
    private var needsReveal = false
    private var enabled = false
    private var navigationEnabled = false
    private var position = 0
    private var revealMarker = false
    private var scrub = HistoryScrubQueue()
    private var scrubbingEnabled = false
    private var documentID: String?
    private var grabOffset: CGFloat = 0
    private var dragPoint: CGPoint?
    private var dragScrollTimer: Timer?
    private var lastLayoutSize: CGSize = .zero
    var preferredToggleWidth: CGFloat { max(52, ceil(toggle.intrinsicContentSize.width) + 8) }
    var scrubInFlight: Bool { scrub.inFlight != nil }
    var isScrubbing: Bool { scrub.active }
    private(set) var expanded = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        accessibilityIdentifier = "workspace-feature-history"
        toggle.artwork("rollback")
        toggle.accessibilityIdentifier = "workspace-feature-history-toggle"
        addSubview(toggle); addSubview(transport); addSubview(scroll)
        transport.axis = .horizontal; transport.spacing = 0; transport.distribution = .fillEqually
        for (control, id) in [(back, "back"), (forward, "forward")] {
            control.artwork("chevron_right", compact: true)
            control.accessibilityIdentifier = "workspace-history-" + id
            transport.addArrangedSubview(control)
        }
        back.imageView?.transform = CGAffineTransform(rotationAngle: .pi)
        back.menu = UIMenu(children: [UIAction(title: HistoryText.text("first")) { [weak self] _ in self?.onMove?(0) }])
        forward.menu = UIMenu(children: [UIAction(title: HistoryText.text("last")) { [weak self] _ in
            guard let self else { return }; self.onMove?(self.features.count)
        }])
        marker.backgroundColor = WorkspaceStyle.accent
        marker.isAccessibilityElement = true
        marker.accessibilityTraits = .adjustable
        marker.accessibilityLabel = HistoryText.text("position")
        marker.accessibilityHint = HistoryText.text("drag")
        marker.accessibilityIdentifier = "workspace-history-marker"
        marker.onStep = { [weak self] direction in
            guard let self, self.navigationEnabled, !self.scrub.active else { return }
            let next = direction > 0 ? (self.position < self.features.count ? self.position + 1 : self.position) : max(0, self.position - 1)
            if next != self.position { self.onMove?(next) }
        }
        scroll.addSubview(items)
        scroll.addSubview(marker)
        markerPan.allowedTouchTypes = [NSNumber(value: UITouch.TouchType.pencil.rawValue), NSNumber(value: UITouch.TouchType.direct.rawValue)]
        markerPan.maximumNumberOfTouches = 1
        markerPan.requiresExclusiveTouchType = true
        markerPan.delegate = self
        scroll.addGestureRecognizer(markerPan)
        scroll.panGestureRecognizer.require(toFail: markerPan)
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.showsHorizontalScrollIndicator = true
        scroll.accessibilityIdentifier = "workspace-feature-history-items"
        items.axis = .horizontal; items.spacing = 4; items.alignment = .fill
        items.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            items.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 8),
            items.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -8),
            items.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            items.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            items.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor)
        ])
        empty.text = HistoryText.text("empty")
        empty.numberOfLines = 1
        setExpanded(false)
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (bar: FeatureHistoryBar, _: UITraitCollection) in
            WorkspaceStyle.floatingSurface(bar)
        }
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (bar: FeatureHistoryBar, _: UITraitCollection) in
            bar.needsRebuild = true
            if bar.expanded { bar.rebuildIfNeeded(); bar.revealSelection() }
            bar.setNeedsLayout()
            bar.onPreferredSizeChange?()
        }
    }
    convenience init() { self.init(frame: .zero) }
    required init?(coder: NSCoder) { nil }
    deinit { dragScrollTimer?.invalidate() }

    func setExpanded(_ value: Bool) {
        if !value { cancelScrub() }
        expanded = value; scroll.isHidden = !value; transport.isHidden = !value
        accessibilityElements = value ? [transport, scroll, toggle] : [toggle]
        toggle.active(value)
        // Keep the original compact toggle's neutral selected surface; only its
        // label and measured width change, not the timeline's presentation.
        var toggleStyle = toggle.configuration
        toggleStyle?.baseBackgroundColor = value ? WorkspaceStyle.selectionFill : .clear
        toggleStyle?.baseForegroundColor = WorkspaceStyle.foreground
        toggleStyle?.background.backgroundColor = value ? WorkspaceStyle.selectionFill : .clear
        toggle.configuration = toggleStyle
        toggle.accessibilityHint = HistoryText.text(value ? "collapse" : "expand")
        toggle.toolTip = toggle.accessibilityHint
        WorkspaceStyle.floatingSurface(self)
        if value { rebuildIfNeeded(); revealSelection() }
        setNeedsLayout()
    }

    func update(features: [ModellingPresentation.Feature], icons: [UInt64: String], selected: Set<UInt64>, position: Int,
                enabled: Bool, navigationEnabled: Bool, scrubbingEnabled: Bool, documentID: String?) {
        let documentChanged = self.documentID != documentID
        if documentChanged || self.features.map(\.id) != features.map(\.id) || !scrubbingEnabled { cancelScrub() }
        self.documentID = documentID; self.scrubbingEnabled = scrubbingEnabled
        let position = min(features.count, max(0, position))
        if self.position != position && !scrub.active { revealMarker = true; revealSelection() }
        if documentChanged || self.features != features || self.icons != icons {
            self.features = features; self.icons = icons; needsRebuild = true
        }
        let changedSelection = self.selected != selected
        self.position = position; self.selected = selected; self.enabled = enabled; self.navigationEnabled = navigationEnabled
        scrub.acknowledge(position)
        marker.accessibilityValue = "\(scrub.target ?? position) / \(features.count)"
        if expanded { rebuildIfNeeded() }
        refreshControls()
        if expanded && changedSelection { revealSelection() }
        setNeedsLayout()
    }

    private func refreshControls() {
        back.availability(enabled: navigationEnabled && !scrub.active && position > 0, reason: HistoryText.text("back"))
        forward.availability(enabled: navigationEnabled && !scrub.active && position < features.count, reason: HistoryText.text("forward"))
        for feature in features {
            guard let button = buttons[feature.id] else { continue }
            button.active(selected.contains(feature.id))
            style(button, feature: feature)
        }
    }

    private func rebuildIfNeeded() {
        guard needsRebuild else { return }
        needsRebuild = false
        items.arrangedSubviews.forEach { items.removeArrangedSubview($0); $0.removeFromSuperview() }
        buttons.removeAll()
        if features.isEmpty { items.addArrangedSubview(empty) }
        for feature in features {
            let button = WorkspaceButton(feature.name, activationEvent: .primaryActionTriggered) { [weak self] in self?.onSelect?(feature.id) }
            // Reserve the hold for our anchored action sheet. The built-in
            // context-menu preview can hide this icon and raise the keyboard.
            button.showsLargeContentViewer = false
            for interaction in button.interactions where interaction is UILargeContentViewerInteraction {
                button.removeInteraction(interaction)
            }
            let hold = UILongPressGestureRecognizer(target: self, action: #selector(featureHeld(_:)))
            hold.cancelsTouchesInView = true
            button.addGestureRecognizer(hold)
            button.accessibilityCustomActions = [UIAccessibilityCustomAction(name: ModellingText.text("delete_feature")) { [weak self, weak button] _ in
                guard let self, let button else { return false }
                return self.showActions(for: feature.id, source: button)
            }]
            button.artwork(icons[feature.id] ?? "component", compact: true)
            button.widthAnchor.constraint(equalToConstant: 44).isActive = true
            button.accessibilityIdentifier = "workspace-history-feature-" + String(feature.id)
            button.active(selected.contains(feature.id))
            style(button, feature: feature)
            items.addArrangedSubview(button); buttons[feature.id] = button
        }
    }

    @objc private func featureHeld(_ gesture: UILongPressGestureRecognizer) {
        guard gesture.state == .began, let button = gesture.view as? WorkspaceButton,
              let id = buttons.first(where: { $0.value === button })?.key else { return }
        button.cancelTracking(with: nil)
        _ = showActions(for: id, source: button)
    }

    @discardableResult
    func showActions(for id: UInt64, source: UIView) -> Bool {
        guard enabled, !scrub.active, buttons[id] === source, source.window != nil,
              features.contains(where: { $0.id == id }), let onShowActions else { return false }
        onShowActions(id, source)
        return true
    }

    private func style(_ button: WorkspaceButton, feature: ModellingPresentation.Feature) {
        let future = feature.order >= (scrub.target ?? position)
        button.availability(enabled: enabled && !scrub.active && (!future || navigationEnabled), reason: enabled ? HistoryText.text("edit_hint") : WorkspaceStyle.text("editing_lock"))
        button.accessibilityValue = feature.error ?? (future ? HistoryText.text("future") : (feature.suppressed ? ModellingText.text("suppressed") : feature.warning))
        button.alpha = future ? 0.35 : (feature.suppressed ? 0.55 : 1)
    }

    private func revealSelection() {
        guard !scrub.active else { return }
        needsReveal = true; setNeedsLayout()
    }

    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        guard gestureRecognizer === markerPan else { return true }
        guard expanded, scrubbingEnabled, !scrub.active, !features.isEmpty else { return false }
        let point = touch.location(in: scroll)
        guard marker.frame.insetBy(dx: -22, dy: 0).contains(point) else { return false }
        grabOffset = point.x - marker.frame.midX
        return true
    }

    @objc private func dragMarker(_ gesture: UIPanGestureRecognizer) {
        switch gesture.state {
        case .began: beginDrag(at: gesture.location(in: self))
        case .changed: updateDrag(at: gesture.location(in: self))
        case .ended:
            updateDrag(at: gesture.location(in: self))
            finishDrag()
        case .cancelled, .failed: cancelScrub()
        default: break
        }
    }

    private func beginDrag(at point: CGPoint) {
        guard expanded, scrubbingEnabled, !scrub.active else { return }
        scrub.begin(at: position); needsReveal = false
        onScrubBegan?()
        let timer = Timer(timeInterval: 1.0 / 60.0, repeats: true) { [weak self] _ in self?.scrollDraggedMarker() }
        dragScrollTimer = timer; RunLoop.main.add(timer, forMode: .common)
        updateDrag(at: point)
    }

    private func finishDrag() { scrub.end(); stopDragScrolling(); drainScrub() }

    private func updateDrag(at point: CGPoint) {
        guard scrub.dragging else { return }
        dragPoint = point
        let x = scroll.convert(point, from: self).x - grabOffset
        let centres = features.compactMap { buttons[$0.id].map { $0.convert($0.bounds, to: scroll).midX } }
        guard centres.count == features.count, let target = HistoryScrubGeometry.position(at: x, centres: centres) else { return }
        scrub.move(to: target)
        refreshControls()
        layoutMarker(); drainScrub()
    }

    private func drainScrub() {
        scrub.acknowledge(position)
        guard let request = scrub.next() else { refreshControls(); layoutMarker(); return }
        guard let onScrubMove else { cancelScrub(); return }
        onScrubMove(request.position) { [weak self] succeeded in
            guard let self, self.scrub.complete(request, succeeded: succeeded) else { return }
            if !succeeded { self.stopDragScrolling() }
            self.drainScrub()
        }
    }

    private func stopDragScrolling() {
        dragScrollTimer?.invalidate(); dragScrollTimer = nil; dragPoint = nil
    }

    func cancelScrub() {
        guard scrub.active else { return }
        scrub.cancel(); stopDragScrolling()
        markerPan.isEnabled = false; markerPan.isEnabled = true
        refreshControls()
        setNeedsLayout()
    }

    private func scrollDraggedMarker() {
        guard scrub.dragging, expanded, window != nil, let dragPoint, scroll.bounds.width > 0 else { return }
        let point = scroll.convert(dragPoint, from: self)
        let edge = min(32, scroll.bounds.width / 4)
        let step: CGFloat
        if point.x < scroll.bounds.minX + edge { step = -min(8, (scroll.bounds.minX + edge - point.x) / 4) }
        else if point.x > scroll.bounds.maxX - edge { step = min(8, (point.x - scroll.bounds.maxX + edge) / 4) }
        else { return }
        let offset = min(max(0, scroll.contentOffset.x + step), max(0, scroll.contentSize.width - scroll.bounds.width))
        guard offset != scroll.contentOffset.x else { return }
        scroll.setContentOffset(CGPoint(x: offset, y: scroll.contentOffset.y), animated: false)
        updateDrag(at: dragPoint)
    }

    private func boundaryX(_ target: Int) -> CGFloat? {
        if target == 0, let first = features.first.flatMap({ buttons[$0.id] }) { return first.convert(first.bounds, to: scroll).minX - 4 }
        if let feature = features.prefix(target).last, let button = buttons[feature.id] { return button.convert(button.bounds, to: scroll).maxX + 4 }
        return nil
    }

    private func layoutMarker() {
        marker.isHidden = features.isEmpty
        let target = min(features.count, max(0, scrub.target ?? position))
        guard let x = boundaryX(target) else { return }
        marker.frame = CGRect(x: x - 1.5, y: 0, width: 3, height: scroll.bounds.height)
        marker.accessibilityValue = "\(target) / \(features.count)"
        marker.alpha = scrubbingEnabled ? 1 : 0.5
    }

#if DEBUG
    /// Production coordinate/queue/command route on a disposable device fixture.
    /// This does not synthesize or certify hardware Pencil input.
    func developmentScrub(_ targets: [Int]) -> Bool {
        guard expanded, scrubbingEnabled, !scrub.active,
              markerPan.allowedTouchTypes.contains(NSNumber(value: UITouch.TouchType.pencil.rawValue)),
              targets.allSatisfy({ (0...features.count).contains($0) }) else { return false }
        layoutIfNeeded(); scroll.layoutIfNeeded(); grabOffset = 0
        beginDrag(at: marker.convert(CGPoint(x: marker.bounds.midX, y: marker.bounds.midY), to: self))
        for target in targets {
            guard let x = boundaryX(target) else { cancelScrub(); return false }
            updateDrag(at: scroll.convert(CGPoint(x: x, y: scroll.bounds.midY), to: self))
        }
        finishDrag()
        return true
    }
#endif

    override func didMoveToWindow() {
        super.didMoveToWindow()
        if window == nil { cancelScrub() }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        if bounds.size != lastLayoutSize { cancelScrub(); lastLayoutSize = bounds.size }
        let inset = min(4, bounds.width / 8, bounds.height / 8)
        let toggleWidth = min(preferredToggleWidth - 2 * inset, max(0, bounds.width - 2 * inset))
        toggle.frame = CGRect(x: max(inset, bounds.width - inset - toggleWidth), y: inset,
            width: toggleWidth, height: max(0, bounds.height - 2 * inset))
        transport.frame = CGRect(x: inset, y: inset, width: 88, height: max(0, bounds.height - 2 * inset))
        scroll.frame = CGRect(x: min(bounds.width, transport.frame.maxX + inset), y: inset,
            width: max(0, toggle.frame.minX - transport.frame.maxX - 2 * inset), height: max(0, bounds.height - 2 * inset))
        scroll.layoutIfNeeded(); layoutMarker()
        if needsReveal && expanded && scroll.bounds.width > 0 {
            needsReveal = false
            scroll.layoutIfNeeded()
            if revealMarker, marker.superview != nil {
                revealMarker = false
                scroll.scrollRectToVisible(marker.convert(marker.bounds, to: scroll).insetBy(dx: -44, dy: 0), animated: false)
            } else if let id = features.first(where: { selected.contains($0.id) })?.id, let button = buttons[id] {
                scroll.scrollRectToVisible(button.convert(button.bounds, to: scroll).insetBy(dx: -4, dy: 0), animated: false)
            }
        }
    }
}

final class WorkspaceStatusLabel: UILabel {
    override init(frame: CGRect) {
        super.init(frame: frame)
        font = .preferredFont(forTextStyle: .caption1)
        adjustsFontForContentSizeCategory = true
        textColor = WorkspaceStyle.secondary
        numberOfLines = 2
    }
    convenience init() { self.init(frame: .zero) }
    required init?(coder: NSCoder) { nil }
    override func drawText(in rect: CGRect) { super.drawText(in: rect.insetBy(dx: 10, dy: 4)) }
    override func sizeThatFits(_ size: CGSize) -> CGSize {
        let fitted = super.sizeThatFits(CGSize(width: max(0, size.width - 20), height: max(0, size.height - 8)))
        return CGSize(width: fitted.width + 20, height: fitted.height + 8)
    }
}

/// Icon-first chrome retains full labels for VoiceOver, pointer tooltips and
/// menus. Forms continue to use text. Missing artwork keeps a readable title.
final class WorkspaceButton: UIButton {
    private var fullTitle: String
    private let isViewportPoint: Bool
    private var iconID: String?
    private var commandID: String?
    private var systemIcon: String?
    private var compactIcon = false
    private var choiceCheck: UIImageView?
    init(_ title: String, prominent: Bool = false, viewportPoint: Bool = false,
         activationEvent: UIControl.Event = .touchUpInside, action: @escaping () -> Void) {
        fullTitle = title
        isViewportPoint = viewportPoint
        super.init(frame: .zero)
        var config = prominent ? UIButton.Configuration.filled() : .plain()
        config.title = viewportPoint ? nil : title
        config.cornerStyle = .medium
        config.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 10, bottom: 10, trailing: 10)
        configuration = config
        titleLabel?.adjustsFontForContentSizeCategory = true
        titleLabel?.numberOfLines = 2
        heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        widthAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        accessibilityLabel = title
        toolTip = title
        isPointerInteractionEnabled = true
        showsLargeContentViewer = true
        largeContentTitle = title
        addInteraction(UILargeContentViewerInteraction())
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitDisplayScale.self]) { (button: WorkspaceButton, _: UITraitCollection) in
            button.refreshIcon()
        }
        addAction(UIAction { _ in action() }, for: activationEvent)
    }
    required init?(coder: NSCoder) { nil }

    func updateTitle(_ title: String) {
        fullTitle = title
        refreshIcon()
        accessibilityLabel = title
        largeContentTitle = title; toolTip = title
    }

    func artwork(_ id: String, compact: Bool = false) {
        iconID = id; commandID = nil; systemIcon = nil; compactIcon = compact; refreshIcon()
    }
    func commandArtwork(_ id: String, compact: Bool = false) {
        commandID = id; iconID = nil; systemIcon = nil; compactIcon = compact; refreshIcon()
    }
    func textOnly() {
        iconID = nil; commandID = nil; systemIcon = nil; compactIcon = false; refreshIcon()
    }
    func systemArtwork(_ name: String, compact: Bool = false) {
        iconID = nil; commandID = nil; systemIcon = name; compactIcon = compact; refreshIcon()
    }
    private func refreshIcon() {
        // The Rust guide is the visible dot. Keep only its accessible touch target.
        guard !isViewportPoint else { return }
        let image = commandID.flatMap { WorkspaceIcon.command($0, traits: traitCollection) }
            ?? iconID.flatMap { WorkspaceIcon.image($0, traits: traitCollection) }
            ?? systemIcon.flatMap { UIImage(systemName: $0) }
        var config = configuration
        config?.image = image
        config?.imagePadding = compactIcon ? 0 : 8
        config?.title = compactIcon && image != nil ? nil : fullTitle
        config?.contentInsets = NSDirectionalEdgeInsets(top: compactIcon ? 8 : 10, leading: 8, bottom: compactIcon ? 8 : 10, trailing: 8)
        configuration = config
        accessibilityLabel = fullTitle
        largeContentImage = image
        if choiceCheck != nil { styleChoice() }
    }

    func choiceArtwork(_ id: String, selected: Bool) {
        if choiceCheck == nil {
            let check = UIImageView(image: UIImage(systemName: "checkmark"))
            check.translatesAutoresizingMaskIntoConstraints = false
            check.isAccessibilityElement = false
            check.isUserInteractionEnabled = false
            check.tintColor = WorkspaceStyle.accent
            check.contentMode = .scaleAspectFit
            addSubview(check)
            NSLayoutConstraint.activate([
                check.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -8),
                check.centerYAnchor.constraint(equalTo: centerYAnchor),
                check.widthAnchor.constraint(equalToConstant: 14),
                check.heightAnchor.constraint(equalToConstant: 14)
            ])
            choiceCheck = check
        }
        artwork(id)
        active(selected)
    }

    private func styleChoice() {
        var config = configuration
        config?.baseForegroundColor = WorkspaceStyle.foreground
        config?.background.backgroundColor = isSelected ? WorkspaceStyle.accent.withAlphaComponent(0.18) : WorkspaceStyle.selectionFill
        config?.background.strokeColor = isSelected ? WorkspaceStyle.accent : WorkspaceStyle.separator
        config?.background.strokeWidth = isSelected ? 1.5 : 0.5
        config?.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 8, bottom: 10, trailing: 26)
        config?.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { incoming in
            var attributes = incoming
            attributes.font = UIFont.preferredFont(forTextStyle: .subheadline)
            return attributes
        }
        configuration = config
        choiceCheck?.isHidden = !isSelected
    }

    func availability(enabled: Bool, reason: String? = nil) {
        isEnabled = enabled
        accessibilityHint = reason
        accessibilityValue = reason
        toolTip = reason.map { fullTitle + " · " + $0 } ?? fullTitle
    }
    func active(_ value: Bool) {
        isSelected = value
        var config = configuration
        let fill = isViewportPoint ? UIColor.clear : (compactIcon ? WorkspaceStyle.selectionFill : WorkspaceStyle.accent)
        config?.baseBackgroundColor = value ? fill : .clear
        config?.baseForegroundColor = value && !compactIcon ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        if value { config?.background.backgroundColor = fill }
        else { config?.background.backgroundColor = .clear }
        configuration = config
        if value { accessibilityTraits.insert(.selected) }
        else { accessibilityTraits.remove(.selected) }
        if choiceCheck != nil { styleChoice() }
    }
}

/// Shared chrome for docked panels and compact contextual sheets.
/// Retain UIKit's standard clear action and accessibility with a full touch target.
final class BrowserSearchField: UITextField {
    override func clearButtonRect(forBounds bounds: CGRect) -> CGRect {
        CGRect(x: max(0, bounds.maxX - 44), y: bounds.midY - 22, width: 44, height: 44)
    }
}

final class WorkspacePanel: UIView {
    let content = UIView()
    private let body = UIStackView()
    private let footer = UIStackView()
    private let heading = WorkspaceStyle.label(.headline)
    private let close: WorkspaceButton
    private let headerActions = UIStackView()
    init(title: String, onClose: @escaping () -> Void) {
        close = WorkspaceButton(WorkspaceStyle.text("close_short"), action: onClose)
        super.init(frame: .zero)
        backgroundColor = WorkspaceStyle.surface
        close.artwork("close", compact: true)
        layer.cornerRadius = WorkspaceStyle.radius
        layer.borderWidth = 0.5
        layer.borderColor = WorkspaceStyle.separator.cgColor
        clipsToBounds = true
        heading.text = title
        heading.accessibilityTraits = .header
        heading.numberOfLines = 1
        heading.lineBreakMode = .byTruncatingTail
        heading.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        close.accessibilityLabel = WorkspaceStyle.text("close")
        close.titleLabel?.numberOfLines = 1
        close.configuration?.titleLineBreakMode = .byClipping
        close.setContentCompressionResistancePriority(.required, for: .horizontal)
        close.setContentHuggingPriority(.required, for: .horizontal)
        body.axis = .vertical
        footer.axis = .vertical
        footer.isLayoutMarginsRelativeArrangement = true
        footer.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16)
        footer.isHidden = true
        footer.setContentCompressionResistancePriority(.required, for: .vertical)
        body.addArrangedSubview(content); body.addArrangedSubview(footer)
        headerActions.axis = .horizontal
        headerActions.spacing = 0
        for child in [heading, headerActions, close, body] { child.translatesAutoresizingMaskIntoConstraints = false; addSubview(child) }
        NSLayoutConstraint.activate([
            heading.leadingAnchor.constraint(equalTo: leadingAnchor, constant: WorkspaceStyle.inset),
            heading.topAnchor.constraint(equalTo: topAnchor, constant: 14),
            heading.trailingAnchor.constraint(lessThanOrEqualTo: headerActions.leadingAnchor, constant: -4),
            headerActions.trailingAnchor.constraint(equalTo: close.leadingAnchor),
            headerActions.topAnchor.constraint(equalTo: close.topAnchor),
            headerActions.heightAnchor.constraint(equalToConstant: 44),
            close.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            body.topAnchor.constraint(greaterThanOrEqualTo: close.bottomAnchor, constant: 6),
            body.topAnchor.constraint(greaterThanOrEqualTo: heading.bottomAnchor, constant: 6),
            body.leadingAnchor.constraint(equalTo: leadingAnchor),
            body.trailingAnchor.constraint(equalTo: trailingAnchor),
            body.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
        let contentTop = body.topAnchor.constraint(equalTo: close.bottomAnchor, constant: 6)
        contentTop.priority = .defaultHigh
        contentTop.isActive = true
        close.accessibilityIdentifier = "workspace-close-" + title
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (panel: WorkspacePanel, _: UITraitCollection) in
            panel.layer.borderColor = WorkspaceStyle.separator.cgColor
        }
    }
    func setHeaderAction(_ action: WorkspaceButton) {
        headerActions.addArrangedSubview(action)
        action.widthAnchor.constraint(equalToConstant: 44).isActive = true
    }
    func setTitle(_ title: String) { heading.text = title }
    func setFooter(_ view: UIView?) {
        footer.arrangedSubviews.forEach { footer.removeArrangedSubview($0); $0.removeFromSuperview() }
        if let view { footer.addArrangedSubview(view) }
        footer.isHidden = view == nil
    }
    required init?(coder: NSCoder) { nil }
}

final class WorkspacePropertyRow: UIStackView {
    init(label: String, value: String) {
        super.init(frame: .zero)
        axis = .vertical; spacing = 3
        let name = WorkspaceStyle.label(.caption1, secondary: true)
        name.text = label
        let detail = WorkspaceStyle.label(.body)
        detail.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 16, weight: .medium))
        detail.text = value
        addArrangedSubview(name); addArrangedSubview(detail)
        isAccessibilityElement = true
        accessibilityLabel = label; accessibilityValue = value
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
}

/// Inline validation/progress is used by both panels and numeric transactions.
final class WorkspaceMessage: UIStackView {
    private let spinner = UIActivityIndicatorView(style: .medium)
    private let label = WorkspaceStyle.label(.footnote, secondary: true)
    init() {
        super.init(frame: .zero)
        axis = .horizontal; alignment = .center; spacing = 8
        addArrangedSubview(spinner); addArrangedSubview(label)
        spinner.hidesWhenStopped = true
        isHidden = true
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    func show(_ message: String?, error: Bool = false, busy: Bool = false) {
        label.text = message
        label.textColor = error ? WorkspaceStyle.error : WorkspaceStyle.secondary
        if busy { spinner.startAnimating() } else { spinner.stopAnimating() }
        isHidden = message?.isEmpty != false && !busy
        accessibilityLabel = message
    }
}

/// Editing text is deliberately provisional. Only explicit Preview/Apply invoke
/// the upstream expression evaluator; cancellation never rewrites the document.
final class NumericExpressionEditor: UIStackView, UITextFieldDelegate {
    let field = UITextField()
    let message = WorkspaceMessage()
    let actionFooter: NumericEditorActions
    let supplementalOptions = UIStackView()
    private let evaluation = WorkspaceStyle.label(.footnote, secondary: true)
    private(set) var busy = false
    private(set) var draft = NumericDraftValidation()
    private var committing = false
    private let showsActions: Bool
    var onPreview: ((String) -> Void)?
    var onApply: ((String) -> Void)?
    var onCancel: (() -> Void)?
    var onFocus: (() -> Void)?
    var onChange: (() -> Void)?
    var onLayoutChange: (() -> Void)?
    private var usesNumericKeyboard = false
    private let panelInput: Bool
    private let padArea = UIStackView()
    private let padUnits = UIStackView()
    private var keypad: ExpressionKeypad?
    private var closeButton: WorkspaceButton?
    private let suppressedInput = UIInputView(frame: .zero, inputViewStyle: .default)
    var onRequestEntry: (() -> Bool)?
    var panelKeypadVisible: Bool { panelInput && !padArea.isHidden }

    init(expression: String, units: String, showsActions: Bool = true, numericKeyboard: Bool = false,
         label: String? = nil, compact: Bool = false, primary: Bool = false, context: String? = nil) {
        actionFooter = NumericEditorActions()
        self.showsActions = showsActions
        panelInput = numericKeyboard
        super.init(frame: .zero)
        axis = .vertical; spacing = numericKeyboard ? 8 : (compact ? 6 : WorkspaceStyle.spacing)
        let heading = WorkspaceStyle.label(numericKeyboard ? .title2 : (compact ? .caption1 : .headline), secondary: compact)
        if numericKeyboard { heading.font = UIFontMetrics(forTextStyle: .title2).scaledFont(for: .systemFont(ofSize: 26, weight: .bold)) }
        heading.text = (label ?? WorkspaceStyle.text("expression")) + (compact && !units.isEmpty ? " · " + units : "")
        heading.accessibilityTraits = .header
        if numericKeyboard {
            let close = WorkspaceButton(WorkspaceStyle.text("close")) { [weak self] in
                guard let self, !self.committing else { return }; self.onCancel?()
            }
            closeButton = close
            close.artwork("close", compact: true)
            close.accessibilityIdentifier = "workspace-numeric-close"
            heading.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
            close.setContentHuggingPriority(.required, for: .horizontal)
            let title = UIStackView(arrangedSubviews: [heading, close]); title.alignment = .center; title.spacing = 8
            addArrangedSubview(title)
            if let context, !context.isEmpty {
                let subtitle = WorkspaceStyle.label(.subheadline, secondary: true)
                subtitle.text = context; addArrangedSubview(subtitle)
            }
        } else { addArrangedSubview(heading) }
        if !numericKeyboard && !compact && !units.isEmpty {
            let unitLabel = WorkspaceStyle.label(.caption1, secondary: true)
            unitLabel.text = WorkspaceStyle.text("expression") + " · " + units
            addArrangedSubview(unitLabel)
        }
        field.text = expression
        field.borderStyle = .none
        field.backgroundColor = WorkspaceStyle.fieldBackground
        field.textColor = WorkspaceStyle.foreground
        field.tintColor = WorkspaceStyle.accent
        field.layer.cornerRadius = 8
        field.leftView = UIView(frame: CGRect(x: 0, y: 0, width: 12, height: 1))
        field.rightView = UIView(frame: CGRect(x: 0, y: 0, width: 12, height: 1))
        field.leftViewMode = .always; field.rightViewMode = .always
        field.font = UIFontMetrics(forTextStyle: primary || numericKeyboard ? .title2 : .body).scaledFont(for: .monospacedSystemFont(ofSize: primary || numericKeyboard ? 24 : 18, weight: .medium))
        field.adjustsFontForContentSizeCategory = true
        field.keyboardType = .asciiCapable
        field.returnKeyType = .done
        field.autocorrectionType = .no
        field.autocapitalizationType = .none
        field.smartDashesType = .no; field.smartQuotesType = .no
        field.delegate = self
        usesNumericKeyboard = numericKeyboard
        if numericKeyboard {
            // The reference's pad lives in this panel. Keep UIKit text editing
            // and hardware input without opening a separate software keyboard.
            suppressedInput.allowsSelfSizing = true
            suppressedInput.heightAnchor.constraint(equalToConstant: 0).isActive = true
            field.inputView = suppressedInput
            field.inputAssistantItem.leadingBarButtonGroups = []
            field.inputAssistantItem.trailingBarButtonGroups = []
        }
        field.accessibilityLabel = [heading.text, units.isEmpty ? nil : units].compactMap { $0 }.joined(separator: " · ")
        field.accessibilityIdentifier = "workspace-numeric-expression"
        field.heightAnchor.constraint(greaterThanOrEqualToConstant: primary || numericKeyboard ? 56 : 48).isActive = true
        field.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.invalidatePreview()
            self.onChange?()
        }, for: .editingChanged)
        if !numericKeyboard { field.inputAccessoryView = makeAccessory() }
        addArrangedSubview(field)
        evaluation.isHidden = true
        evaluation.accessibilityIdentifier = "workspace-numeric-evaluation"
        addArrangedSubview(message); addArrangedSubview(evaluation)
        if numericKeyboard { buildPanelKeypad(units: units) }
        if !compact {
            let help = InspectorDisclosure(title: NumericText.text("help"), framed: numericKeyboard, onToggle: { [weak self] _ in self?.onLayoutChange?() })
            help.accessibilityIdentifier = "workspace-expression-help"
            let hint = WorkspaceStyle.label(.footnote, secondary: true)
            hint.text = WorkspaceStyle.text("decimal_help")
            help.content.addArrangedSubview(hint)
            supplementalOptions.axis = .vertical; supplementalOptions.spacing = 8
            help.content.addArrangedSubview(supplementalOptions)
            addArrangedSubview(help)
        }
        addArrangedSubview(actionFooter); actionFooter.isHidden = !showsActions
        actionFooter.preview.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy else { return }
            self.onPreview?(self.field.text ?? "")
        }, for: .touchUpInside)
        actionFooter.apply.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy, self.draft.isValidated else { return }
            self.field.resignFirstResponder(); self.onApply?(self.field.text ?? "")
        }, for: .touchUpInside)
        actionFooter.cancel.addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .touchUpInside)
        updateAvailability()
        updateInputAppearance()
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (editor: NumericExpressionEditor, _: UITraitCollection) in editor.updateInputAppearance() }
        if showsActions { message.show(previewRequired) }
    }
    required init(coder: NSCoder) {
        actionFooter = NumericEditorActions(); showsActions = false; panelInput = false
        super.init(coder: coder)
    }
    private var previewRequired: String { panelInput ? NumericText.text("preview_required") : ModellingText.text("preview_required") }
    private func makeAccessory() -> ExpressionAccessory {
        ExpressionAccessory(field: field, numeric: usesNumericKeyboard) { [weak self] in
            guard let self else { return false }
            self.setNumericInput(!self.usesNumericKeyboard)
            return self.usesNumericKeyboard
        }
    }
    private func buildPanelKeypad(units: String) {
        padArea.axis = .vertical; padArea.spacing = 10
        padUnits.axis = .horizontal; padUnits.spacing = 4; padUnits.distribution = .fillEqually
        let insertions = units.isEmpty ? ["(", ")", "−", "*", "/"] : (units == "deg" ? ["deg", "(", ")", "*", "/"] : ["mm", "cm", "in", "(", ")"])
        for insertion in insertions {
            let key = NumericPadButton(insertion) { [weak field] in field?.insertText(insertion == "−" ? "-" : insertion) }
            key.accessibilityIdentifier = "workspace-expression-insert-" + insertion
            if insertion == units {
                key.configuration?.background.backgroundColor = WorkspaceStyle.accent.withAlphaComponent(0.18)
                key.configuration?.background.strokeColor = WorkspaceStyle.accent
                key.accessibilityTraits.insert(.selected)
            }
            padUnits.addArrangedSubview(key)
        }
        let mode = NumericPadButton(NumericText.text("full_keyboard")) { [weak self] in self?.setNumericInput(false) }
        mode.accessibilityIdentifier = "workspace-expression-keyboard"
        mode.accessibilityLabel = NumericText.text("full_keyboard_label")
        padUnits.addArrangedSubview(mode)
        let pad = ExpressionKeypad(field: field) { [weak self] in self?.dismissInput() }
        keypad = pad
        padArea.addArrangedSubview(padUnits); padArea.addArrangedSubview(pad)
        addArrangedSubview(padArea)
    }
    private func setNumericInput(_ numeric: Bool) {
        usesNumericKeyboard = numeric
        if panelInput {
            padArea.isHidden = !numeric
            field.inputView = numeric ? suppressedInput : nil
            field.inputAccessoryView = numeric ? nil : makeAccessory()
        } else {
            // Non-popup fields retain full UIKit expression entry.
            field.inputView = nil
        }
        updateInputAppearance()
        if field.isFirstResponder { field.reloadInputViews() }
        else { field.becomeFirstResponder() }
        onLayoutChange?()
    }
    func dismissInput() {
        field.resignFirstResponder()
        if panelInput { padArea.isHidden = true }
        updateFieldOutline(); onLayoutChange?()
    }
    private func updateFieldOutline() {
        guard panelInput else { return }
        field.layer.borderWidth = field.isFirstResponder ? 1.5 : 0.5
        field.layer.borderColor = (field.isFirstResponder ? WorkspaceStyle.accent : WorkspaceStyle.separator).resolvedColor(with: traitCollection).cgColor
    }
    func invalidatePreview() {
        draft.changed(); evaluation.isHidden = true
        message.show(showsActions ? previewRequired : nil)
        updateAvailability(); onLayoutChange?()
    }
    /// A late result may describe a retained scene, but cannot validate newer text.
    func completePreview(revision: UInt64, valid: Bool = true, value: Double? = nil, units: String = "", feedback: String? = nil, error: String? = nil) {
        setBusy(false)
        guard draft.complete(revision: revision, valid: valid && error == nil) else {
            message.show(previewRequired); return
        }
        if valid && error == nil, let value { setEvaluation(value: value, units: units) }
        else { evaluation.isHidden = true }
        message.show(error ?? (valid ? feedback ?? WorkspaceStyle.text("preview_ready") : previewRequired), error: error != nil)
        updateAvailability(); onLayoutChange?()
    }
    func setEvaluation(value: Double, units: String) {
        guard value.isFinite else { evaluation.isHidden = true; return }
        let formatter = NumberFormatter()
        formatter.locale = .current
        formatter.numberStyle = .decimal
        formatter.usesSignificantDigits = true
        formatter.minimumSignificantDigits = 1
        formatter.maximumSignificantDigits = 12
        guard let formatted = formatter.string(from: NSNumber(value: value)) else { evaluation.isHidden = true; return }
        let resolved = formatted + (units.isEmpty ? "" : " " + units)
        evaluation.text = WorkspaceStyle.text("numeric_resolved") + (panelInput ? " · " : "\n") + resolved
        evaluation.accessibilityLabel = WorkspaceStyle.text("numeric_resolved")
        evaluation.accessibilityValue = resolved
        evaluation.isHidden = false
    }
    func setBusy(_ value: Bool, committing: Bool = false) {
        busy = value; self.committing = value && committing
        // Preview leaves the input and first responder intact. Only acceptance
        // locks editing; asynchronous results never rewrite the raw expression.
        field.isEnabled = !self.committing
        padArea.isUserInteractionEnabled = !self.committing
        updateAvailability()
        if value { message.show(NativeText.value("ui.ipados.updating"), busy: true) }
        else { message.show(showsActions ? previewRequired : nil) }
    }
    private func updateAvailability() {
        actionFooter.preview.availability(enabled: !busy)
        actionFooter.apply.availability(enabled: !busy && draft.isValidated,
            reason: draft.isValidated ? nil : previewRequired)
        actionFooter.cancel.availability(enabled: !committing)
        closeButton?.availability(enabled: !committing)
    }
    private func updateInputAppearance() {
        let style = traitCollection.userInterfaceStyle
        field.keyboardAppearance = style == .dark ? .dark : (style == .light ? .light : .default)
        field.inputView?.overrideUserInterfaceStyle = style
        field.inputAccessoryView?.overrideUserInterfaceStyle = style
        updateFieldOutline()
    }
    func textFieldShouldBeginEditing(_ textField: UITextField) -> Bool {
        if !panelInput, onRequestEntry?() == true { return false }
        if panelInput && usesNumericKeyboard { padArea.isHidden = false }
        return !committing
    }
    func textFieldDidBeginEditing(_ textField: UITextField) { updateFieldOutline(); onFocus?() }
    func textFieldDidEndEditing(_ textField: UITextField) { updateFieldOutline() }
    func textFieldShouldReturn(_ textField: UITextField) -> Bool { dismissInput(); return true }
}

enum NumericText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.numeric_entry." + key) }
}

/// The compact editor can pin this footer outside its scrolling form.
final class NumericEditorActions: UIStackView {
    let preview = WorkspaceButton(WorkspaceStyle.text("preview"), prominent: true, action: {})
    let apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
    let cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
    private let row = UIStackView()
    init() {
        super.init(frame: .zero)
        axis = .vertical; spacing = 12
        let divider = UIView(); divider.backgroundColor = WorkspaceStyle.separator
        divider.heightAnchor.constraint(equalToConstant: 0.5).isActive = true
        addArrangedSubview(divider)
        row.spacing = 8; row.distribution = .fillEqually
        for button in [cancel, preview, apply] { button.textOnly(); row.addArrangedSubview(button) }
        cancel.configuration?.background.backgroundColor = WorkspaceStyle.fieldBackground
        cancel.configuration?.baseForegroundColor = WorkspaceStyle.foreground
        addArrangedSubview(row)
        preview.accessibilityIdentifier = "workspace-numeric-preview"
        apply.accessibilityIdentifier = "workspace-numeric-apply"
        cancel.accessibilityIdentifier = "workspace-numeric-cancel"
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    override func layoutSubviews() {
        let needed = [cancel, preview, apply].reduce(CGFloat(16)) { $0 + max(44, $1.intrinsicContentSize.width) }
        let direction: NSLayoutConstraint.Axis = bounds.width < needed ? .vertical : .horizontal
        if row.axis != direction { row.axis = direction }
        super.layoutSubviews()
    }
}

/// Numeric keypad buttons are scoped to this editor; no shared menu/browser
/// appearance changes. Semantic fills and borders follow the effective theme.
private final class NumericPadButton: UIButton {
    init(_ title: String, digit: Bool = false, prominent: Bool = false, action: @escaping () -> Void) {
        super.init(frame: .zero)
        configuration = prominent ? .filled() : .plain()
        configuration?.title = title
        configuration?.cornerStyle = .medium
        heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        widthAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        accessibilityLabel = title
        addAction(UIAction { _ in action() }, for: .touchUpInside)
        configuration?.baseForegroundColor = prominent ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        if !prominent { configuration?.background.backgroundColor = WorkspaceStyle.fieldBackground }
        configuration?.background.strokeColor = WorkspaceStyle.separator
        configuration?.background.strokeWidth = 0.5
        configuration?.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 4, bottom: 8, trailing: 4)
        configuration?.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { [weak self, digit] attributes in
            var attributes = attributes
            attributes.font = UIFontMetrics(forTextStyle: digit ? .title2 : .body).scaledFont(
                for: .systemFont(ofSize: digit ? 24 : 17, weight: .medium), maximumPointSize: digit ? 32 : 24,
                compatibleWith: self?.traitCollection)
            return attributes
        }
        titleLabel?.numberOfLines = 1
        titleLabel?.adjustsFontSizeToFitWidth = true
        titleLabel?.minimumScaleFactor = 0.7
    }
    required init?(coder: NSCoder) { nil }
}

/// A scrollable insertion strip retains every expression character. Keyboard
/// switching and Done remain reachable without scrolling the strip to its end.
private final class ExpressionAccessory: UIInputView {
    private var height: NSLayoutConstraint?
    init(field: UITextField, numeric: Bool, toggle: @escaping () -> Bool) {
        super.init(frame: CGRect(x: 0, y: 0, width: 320, height: 56), inputViewStyle: .keyboard)
        allowsSelfSizing = true
        backgroundColor = WorkspaceStyle.surface
        let scroll = UIScrollView()
        let keys = UIStackView(); keys.axis = .horizontal; keys.spacing = 4
        let mode = WorkspaceButton(numeric ? NumericText.text("full_keyboard") : NumericText.text("numeric_keyboard"), action: {})
        mode.accessibilityLabel = NumericText.text(numeric ? "full_keyboard_label" : "numeric_keyboard_label")
        mode.addAction(UIAction { [weak mode] _ in
            let numeric = toggle()
            mode?.updateTitle(NumericText.text(numeric ? "full_keyboard" : "numeric_keyboard"))
            mode?.accessibilityLabel = NumericText.text(numeric ? "full_keyboard_label" : "numeric_keyboard_label")
        }, for: .touchUpInside)
        mode.accessibilityIdentifier = "workspace-expression-keyboard"
        let done = WorkspaceButton(WorkspaceStyle.text("keyboard_done")) { [weak field] in field?.resignFirstResponder() }
        done.accessibilityIdentifier = "workspace-expression-done"
        for text in ["mm", "cm", "in", "deg", "(", ")", "−", ".", ",", "+", "*", "/", "^"] {
            keys.addArrangedSubview(WorkspaceButton(text) { [weak field] in field?.insertText(text == "−" ? "-" : text) })
        }
        let row = UIStackView(arrangedSubviews: [mode, scroll, done]); row.spacing = 4
        row.translatesAutoresizingMaskIntoConstraints = false; keys.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row); scroll.addSubview(keys)
        mode.setContentCompressionResistancePriority(.required, for: .horizontal)
        done.setContentCompressionResistancePriority(.required, for: .horizontal)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 4),
            row.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -4),
            row.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            row.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
            keys.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            keys.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            keys.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            keys.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            keys.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor)
        ])
        height = heightAnchor.constraint(equalToConstant: 56); height?.isActive = true
        updateHeight()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (view: ExpressionAccessory, _: UITraitCollection) in view.updateHeight() }
    }
    required init?(coder: NSCoder) { nil }
    private func updateHeight() {
        height?.constant = max(WorkspaceStyle.touch, UIFont.preferredFont(forTextStyle: .body, compatibleWith: traitCollection).lineHeight + 20) + 8
    }
}

enum HandleText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.handles." + key) }
}

/// This inserts expression characters through UIKit. Parsing and units stay in Rust.
private final class ExpressionKeypad: UIView {
    init(field: UITextField, done: @escaping () -> Void) {
        super.init(frame: .zero)
        accessibilityIdentifier = "workspace-embedded-keypad"
        var signEdit: (original: String, wrapped: String)?
        let rows = UIStackView(); rows.axis = .vertical; rows.spacing = 8; rows.distribution = .fillEqually
        rows.translatesAutoresizingMaskIntoConstraints = false
        for values in [["7", "8", "9", "⌫"], ["4", "5", "6", "−"], ["1", "2", "3", "+"], ["±", "0", ".", "Done"]] {
            let row = UIStackView(); row.axis = .horizontal; row.spacing = 8; row.distribution = .fillEqually
            for value in values {
                let key = NumericPadButton(value == "Done" ? WorkspaceStyle.text("done") : value,
                    digit: value != "Done", prominent: value == "Done") { [weak field] in
                    if value == "Done" { done() }
                    else if value == "⌫" { field?.deleteBackward() }
                    else if value == "±", let field {
                        // A text edit, not a numeric parser: negate the complete
                        // raw expression and allow the same key to undo that edit.
                        let raw = field.text ?? ""
                        if let previous = signEdit, raw == previous.wrapped {
                            field.selectAll(nil); field.insertText(previous.original); signEdit = nil
                        } else if raw.isEmpty { field.insertText("-") }
                        else {
                            let wrapped = "-(" + raw + ")"
                            field.selectAll(nil); field.insertText(wrapped)
                            signEdit = (raw, wrapped)
                        }
                    } else { field?.insertText(value == "−" ? "-" : value) }
                }
                if value == "⌫" { key.accessibilityLabel = HandleText.text("delete") }
                if value == "±" { key.accessibilityLabel = NumericText.text("change_sign") }
                key.accessibilityIdentifier = value == "Done" ? "workspace-expression-done" : "workspace-keypad-" + value
                key.heightAnchor.constraint(greaterThanOrEqualToConstant: 52).isActive = true
                row.addArrangedSubview(key)
            }
            rows.addArrangedSubview(row)
        }
        addSubview(rows)
        NSLayoutConstraint.activate([
            rows.topAnchor.constraint(equalTo: topAnchor), rows.bottomAnchor.constraint(equalTo: bottomAnchor),
            rows.leadingAnchor.constraint(equalTo: leadingAnchor), rows.trailingAnchor.constraint(equalTo: trailingAnchor)
        ])
    }
    required init?(coder: NSCoder) { nil }
}

/// Small dimension text with a full native touch target around it. Disabling
/// interaction lets the canvas retain annotation dragging in the Move tool.
final class SketchDimensionControl: UIControl {
    private let label = WorkspaceStyle.label(.caption1)
    var onEdit: (() -> Void)?
    private var driven = false
    var editable = false {
        didSet {
            let key = driven ? "reference_dimension" : (editable ? "edit_dimension" : "driving_dimension")
            accessibilityLabel = NativeText.value("ui.ipados.sketch." + key)
            accessibilityTraits = editable ? .button : .staticText
        }
    }
    init() {
        super.init(frame: .zero)
        label.numberOfLines = 1
        label.layer.cornerRadius = 4; label.clipsToBounds = true
        addSubview(label)
        isAccessibilityElement = true
        isUserInteractionEnabled = false // MetalCanvas retains taps and drags.
        accessibilityTraits = .button
        addAction(UIAction { [weak self] _ in self?.onEdit?() }, for: .touchUpInside)
    }
    required init?(coder: NSCoder) { nil }
    func update(text: String, selected: Bool, driven: Bool, editable: Bool) {
        label.text = " " + text + " "
        label.textColor = selected ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        label.backgroundColor = selected ? WorkspaceStyle.accent : WorkspaceStyle.surface.withAlphaComponent(0.92)
        self.driven = driven
        accessibilityValue = text
        self.editable = editable
        invalidateIntrinsicContentSize(); setNeedsLayout()
    }
    override var intrinsicContentSize: CGSize {
        let size = label.intrinsicContentSize
        return CGSize(width: max(44, size.width), height: max(44, size.height))
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        label.sizeToFit(); label.center = CGPoint(x: bounds.midX, y: bounds.midY)
    }
    override func accessibilityActivate() -> Bool {
        guard editable else { return false }
        onEdit?(); return true
    }
}

/// The compact entry sheet shares the existing numeric or modelling transaction.
final class NumericValueEditorController: UIViewController {
    let parameter: String?
    let editor: NumericExpressionEditor
    private let inlineChoices = UIStackView()
    private let scroll = UIScrollView()
    private let content = UIStackView()
    private var focusedInitially = false
    func setInlineChoices(_ field: ModellingState.Field?, selected: PresentationJSON?, enabled: Bool, action: @escaping (PresentationJSON) -> Void) {
        inlineChoices.arrangedSubviews.forEach { inlineChoices.removeArrangedSubview($0); $0.removeFromSuperview() }
        inlineChoices.isHidden = field == nil
        guard let field else { return }
        let disclosure = InspectorDisclosure(title: ModellingText.label(field.label))
        let choices = ModellingInlineChoices(field: field, selected: selected, action: action)
        choices.isUserInteractionEnabled = enabled
        disclosure.content.addArrangedSubview(choices)
        inlineChoices.addArrangedSubview(disclosure)
    }
    init(expression: String, units: String, name: String? = nil, context: String? = nil, parameter: String? = nil) {
        self.parameter = parameter
        editor = NumericExpressionEditor(expression: expression, units: units, numericKeyboard: true, label: name, context: context)
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .popover
        preferredContentSize = CGSize(width: 400, height: 640)
        popoverPresentationController?.adaptiveSheetPresentationController.detents = [.large()]
    }
    required init?(coder: NSCoder) { nil }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        editor.field.resignFirstResponder()
    }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.surface
        view.isOpaque = true
        popoverPresentationController?.backgroundColor = WorkspaceStyle.surface
        inlineChoices.axis = .vertical; inlineChoices.spacing = 8
        content.axis = .vertical; content.spacing = WorkspaceStyle.spacing
        // Put the field and its feedback first; long optional choices scroll
        // below it while the transaction actions stay above the keyboard.
        editor.removeArrangedSubview(editor.actionFooter); editor.actionFooter.removeFromSuperview()
        content.addArrangedSubview(editor); editor.supplementalOptions.addArrangedSubview(inlineChoices)
        inlineChoices.isHidden = inlineChoices.arrangedSubviews.isEmpty
        let footer = editor.actionFooter
        for child in [scroll, footer] { child.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(child) }
        content.translatesAutoresizingMaskIntoConstraints = false; scroll.addSubview(content)
        scroll.keyboardDismissMode = .interactive
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: footer.topAnchor, constant: -8),
            scroll.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor),
            footer.leadingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.leadingAnchor, constant: WorkspaceStyle.inset),
            footer.trailingAnchor.constraint(equalTo: view.safeAreaLayoutGuide.trailingAnchor, constant: -WorkspaceStyle.inset),
            footer.bottomAnchor.constraint(equalTo: view.keyboardLayoutGuide.topAnchor, constant: -8),
            content.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: WorkspaceStyle.inset),
            content.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -WorkspaceStyle.inset),
            content.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: WorkspaceStyle.inset),
            content.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -WorkspaceStyle.inset),
            content.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -2 * WorkspaceStyle.inset)
        ])
        editor.onLayoutChange = { [weak self] in self?.revealInput() }
        editor.onFocus = { [weak self] in self?.revealInput() }
    }
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); revealInput() }
    private func revealInput() {
        let feedback: UIView = editor.message.isHidden ? editor.field : editor.message
        let rect = editor.field.convert(editor.field.bounds, to: scroll).union(feedback.convert(feedback.bounds, to: scroll))
        scroll.scrollRectToVisible(rect.insetBy(dx: 0, dy: -8), animated: false)
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !focusedInitially else { return }
        focusedInitially = true
        editor.field.becomeFirstResponder(); editor.field.selectAll(nil)
    }
}

enum ModellingText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.modelling." + key) }
    static func label(_ value: String) -> String { PresentationLabelKey.label(value, lookup: NativeText.value) }
}

enum AssemblyText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.assembly." + key) }
}

/// Always-visible choices over the engine's opaque option values. Shared by
/// the inspector and the on-model distance editor; no CAD policy lives here.
private final class InspectorDisclosure: UIStackView {
    let content = UIStackView()
    private let button: WorkspaceButton
    init(title: String, expanded: Bool = false, framed: Bool = false, identifier: String? = nil, onToggle: ((Bool) -> Void)? = nil) {
        button = WorkspaceButton(title, action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = 8
        content.axis = .vertical; content.spacing = 6; content.isHidden = !expanded
        button.contentHorizontalAlignment = .leading
        button.configuration?.baseForegroundColor = WorkspaceStyle.foreground
        button.accessibilityIdentifier = identifier
        if framed {
            button.configuration?.baseForegroundColor = WorkspaceStyle.foreground
            button.configuration?.background.backgroundColor = WorkspaceStyle.fieldBackground
            button.configuration?.background.strokeColor = WorkspaceStyle.separator
            button.configuration?.background.strokeWidth = 0.5
        }
        reflect()
        button.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.content.isHidden.toggle(); self.reflect(); onToggle?(!self.content.isHidden)
        }, for: .touchUpInside)
        addArrangedSubview(button); addArrangedSubview(content)
    }
    required init(coder: NSCoder) { button = WorkspaceButton("", action: {}); super.init(coder: coder) }
    private func reflect() {
        button.systemArtwork(content.isHidden ? "chevron.right" : "chevron.down")
        button.configuration?.imagePlacement = .trailing
        button.configuration?.imagePadding = 8
        button.accessibilityValue = InspectorText.text(content.isHidden ? "collapsed" : "expanded")
    }
}

private final class InspectorActionRow: UIStackView {
    init(buttons: [WorkspaceButton]) {
        super.init(frame: .zero)
        buttons.forEach(addArrangedSubview)
        spacing = 8; distribution = .fillEqually
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (row: InspectorActionRow, _: UITraitCollection) in
            row.setNeedsLayout()
        }
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    override func layoutSubviews() {
        let needed = arrangedSubviews.reduce(CGFloat(0)) { $0 + max(44, $1.intrinsicContentSize.width) }
            + CGFloat(max(0, arrangedSubviews.count - 1)) * spacing
        let direction: NSLayoutConstraint.Axis = traitCollection.preferredContentSizeCategory.isAccessibilityCategory || needed > bounds.width ? .vertical : .horizontal
        if axis != direction { axis = direction; invalidateIntrinsicContentSize() }
        super.layoutSubviews()
    }
}

private final class InspectorReferenceHeading: UIStackView {
    init(title: UILabel, count: UILabel) {
        super.init(frame: .zero)
        addArrangedSubview(title); addArrangedSubview(count)
        updateLayout()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (heading: InspectorReferenceHeading, _: UITraitCollection) in
            heading.updateLayout()
        }
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    private func updateLayout() {
        let large = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        axis = large ? .vertical : .horizontal
        alignment = large ? .leading : .firstBaseline
        spacing = large ? 4 : 8
    }
}

final class ModellingInlineChoices: UIStackView {
    init(field: ModellingState.Field, selected: PresentationJSON?, illustrated: Bool = false, action: @escaping (PresentationJSON) -> Void) {
        super.init(frame: .zero)
        axis = .vertical; spacing = 8
        let options = field.options ?? []
        for start in stride(from: 0, to: options.count, by: 2) {
            let row = UIStackView(); row.axis = .horizontal; row.spacing = 8; row.distribution = .fillEqually
            for index in start..<min(start + 2, options.count) {
                let option = options[index]
                let button = WorkspaceButton(ModellingText.label(option.label)) { action(option.value) }
                button.active(option.value == selected)
                // These are presentation cues over opaque engine values. The
                // original canonical masters remain the only artwork authority.
                if illustrated, let icon = InspectorPresentation.choiceIcon(field: field.key, value: option.value) {
                    button.choiceArtwork(icon, selected: option.value == selected)
                }
                button.accessibilityIdentifier = "workspace-model-inline-" + field.key + "-" + String(index)
                row.addArrangedSubview(button)
            }
            addArrangedSubview(row)
        }
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
}

/// A form over Rust-authored fields and opaque JSON. It has no operation map,
/// topology rules or numeric parser. Only a matching validated preview enables Apply.
final class ModellingInspector: UIStackView {
    let footer = UIStackView()
    let title: String
    let compact: Bool
    private(set) var params: [String: PresentationJSON]
    private(set) var fields: [ModellingState.Field]
    private var validated: [String: PresentationJSON]?
    private var locallyEdited = false
    private var editors: [NumericExpressionEditor] = []
    private var textFields: [UITextField] = []
    private var fieldViews: [UIView] = []
    private var fieldGeneration: UInt64 = 0
    private var discreteFieldViews: [ModellingFormGroup] = []
    private var busy = false
    private var inputWhileBusy = false
    private var committing = false
    private var sliderActive = false
    private var contextRefreshing = false
    private var sliders: [UISlider] = []
    let message = WorkspaceMessage()
    private let feedback = WorkspaceMessage()
    private let fieldsStack = UIStackView()
    private let additionalFields = UIStackView()
    private var optionsExpanded = false
    private let preview: WorkspaceButton
    private let apply: WorkspaceButton
    private let cancel: WorkspaceButton
    private let select: WorkspaceButton
    var onPreview: (([String: PresentationJSON]) -> Void)?
    var onLivePreview: (([String: PresentationJSON]) -> Void)?
    var onContextChange: (([String: PresentationJSON]) -> Void)?
    var onInlineChoice: (([String: PresentationJSON]) -> Void)?
    var onApply: (([String: PresentationJSON]) -> Void)?
    var onCancel: (() -> Void)?
    var onFocus: (() -> Void)?
    var onChange: (() -> Void)?
    var onSelect: ((String?) -> Void)?
    var onEditValue: ((ModellingState.Field, String, UIView) -> Void)?
    var focusedField: UITextField? { editors.first(where: { $0.field.isFirstResponder })?.field ?? textFields.first(where: { $0.isFirstResponder }) }
    var textEntryActive: Bool { focusedField != nil }
    var handleEntryEnabled: Bool { !busy && !contextRefreshing && !committing }
    var hasLocalChanges: Bool { locallyEdited }
    var numericContext: String {
        guard let source = fields.first(where: { $0.kind == "reference" && $0.key == "profile" }) else { return title }
        let selected = parameter(source.key)
        let summary = source.options?.first(where: { $0.value == selected }).map { ModellingText.label($0.label) } ?? referenceSummary(selected)
        return title + " · " + summary
    }

    func selectInlineChoice(_ key: String, value: PresentationJSON) {
        guard handleEntryEnabled else { return }
        if key == "operation" { params["_auto_subtract"] = .bool(false) }
        setParameter(key, value: value); changed(); rebuildFields()
        onInlineChoice?(params)
    }

    func editExpression(_ path: String, expression: String) {
        setParameter(path, value: .string(expression)); changed()
        if !textEntryActive { rebuildFields() }
    }

    init(state: ModellingState, compact: Bool = true) {
        self.compact = compact
        if let id = state.feature, let feature = state.workspace.modelling?.timeline.first(where: { $0.id == id }) {
            title = ModellingText.text("editing") + ": " + feature.name
        } else if let operation = state.operation {
            title = ((state.workspace.modelling?.tools ?? []) + (state.workspace.assembly?.tools ?? [])).first(where: { $0.id == operation }).map { ModellingText.label($0.label) }
                ?? ModellingText.label("cmd." + operation)
        } else { title = ModellingText.text("tools") }
        params = state.params ?? [:]
        fields = state.fields ?? []
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        select = WorkspaceButton(ModellingText.text("select_in_viewport"), action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = compact ? 12 : 16
        preview.artwork("eye"); apply.artwork("finish"); cancel.artwork("close"); select.artwork("select")
        let heading = WorkspaceStyle.label(.headline)
        heading.text = title
        heading.accessibilityTraits = .header
        if !compact { addArrangedSubview(heading) }
        let hint = WorkspaceStyle.label(.footnote, secondary: true)
        hint.text = ModellingText.text("draft_hint")
        if !compact { addArrangedSubview(hint) }
        fieldsStack.axis = .vertical; fieldsStack.spacing = compact ? 6 : 16
        additionalFields.axis = .vertical; additionalFields.spacing = 6
        addArrangedSubview(fieldsStack)
        if compact {
            let help = InspectorDisclosure(title: InspectorText.text("expression_help"))
            let syntax = WorkspaceStyle.label(.footnote, secondary: true)
            syntax.text = WorkspaceStyle.text("decimal_help") + "\n" + ModellingText.text("draft_hint")
            help.content.addArrangedSubview(syntax)
            addArrangedSubview(help)
            addArrangedSubview(feedback)
        } else { addArrangedSubview(select) }
        footer.axis = .vertical; footer.spacing = 8
        if compact {
            let divider = UIView(); divider.backgroundColor = WorkspaceStyle.separator
            divider.heightAnchor.constraint(equalToConstant: 0.5).isActive = true
            footer.addArrangedSubview(divider)
        }
        footer.addArrangedSubview(message)
        let actions: UIStackView = compact ? InspectorActionRow(buttons: [cancel, preview, apply]) : UIStackView(arrangedSubviews: [preview, apply])
        actions.spacing = 8; actions.distribution = .fillEqually
        if compact {
            // Text keeps three actions readable at narrow widths. Their
            // canonical tool artwork remains on the operation choices.
            for button in [cancel, preview, apply] { button.textOnly() }
            footer.addArrangedSubview(actions)
        } else { footer.addArrangedSubview(actions); footer.addArrangedSubview(cancel); addArrangedSubview(footer) }
        preview.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy else { return }; self.endEditing(true); self.onPreview?(self.params)
        }, for: .touchUpInside)
        apply.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy, self.validated == self.params else { return }; self.endEditing(true); self.onApply?(self.params)
        }, for: .touchUpInside)
        cancel.addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .touchUpInside)
        select.addAction(UIAction { [weak self] _ in self?.endEditing(true); self?.onSelect?(nil) }, for: .touchUpInside)
        preview.accessibilityIdentifier = "workspace-model-preview"
        apply.accessibilityIdentifier = "workspace-model-apply"
        cancel.accessibilityIdentifier = "workspace-model-cancel"
        rebuildFields(); updateAvailability()
    }
    required init(coder: NSCoder) {
        title = ModellingText.text("tools"); compact = false
        params = [:]; fields = []
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        select = WorkspaceButton(ModellingText.text("select_in_viewport"), action: {})
        super.init(coder: coder)
    }

    func evaluated(_ state: ModellingState) {
        busy = false; committing = false; inputWhileBusy = false
        if state.params == params, let fields = state.fields { self.fields = fields }
        validated = state.valid == true && state.params == params ? params : nil
        if validated != nil { locallyEdited = false }
        if state.params == params && !sliderActive && !textEntryActive { rebuildFields() }
        updateAvailability()
        showFeedback(state)
    }
    func inspected(_ state: ModellingState) {
        // Inspection refreshes available native references. Current draft text
        // remains owned by its fields; inspection never silently replaces an input.
        if let fields = state.fields { self.fields = fields }
        if !locallyEdited, let params = state.params { self.params = params }
        validated = state.valid == true && state.params == params ? params : nil
        if !textEntryActive { rebuildFields() }
        updateAvailability()
        showFeedback(state)
    }
    private func showFeedback(_ state: ModellingState) {
        let status = validated == nil ? ModellingText.text("preview_required") : WorkspaceStyle.text("preview_ready")
        let details = (state.feedback ?? []).map(ModellingText.label)
        message.show(compact ? status : ([status] + details).joined(separator: "\n"))
        feedback.show(compact ? details.joined(separator: "\n") : nil)
    }
    func beginContextRefresh() {
        contextRefreshing = true; validated = nil; busy = false; inputWhileBusy = false
        editors.forEach { $0.setBusy(false) }
        updateAvailability()
        message.show(NativeText.value("ui.ipados.updating"), busy: true)
    }
    func contextRefreshed(_ state: ModellingState) {
        if let params = state.params { self.params = params }
        if let fields = state.fields { self.fields = fields }
        contextRefreshing = false; busy = false; inputWhileBusy = false
        locallyEdited = false; validated = nil
        if !textEntryActive { rebuildFields() }
        updateAvailability(); message.show(ModellingText.text("preview_required"))
    }
    func contextRefreshDiscarded() {
        contextRefreshing = false; validated = nil; busy = false; inputWhileBusy = false
        editors.forEach { $0.setBusy(false) }
        updateAvailability()
        message.show(ModellingText.text("preview_required"))
    }
    func failed(_ error: BridgeError) {
        busy = false; committing = false; inputWhileBusy = false; contextRefreshing = false; validated = nil
        editors.forEach { $0.setBusy(false) }
        updateAvailability(); message.show(error.message, error: true)
    }
    func setBusy(committing: Bool, allowInput: Bool = false) {
        busy = true; self.committing = committing; inputWhileBusy = allowInput && !committing
        editors.forEach { $0.setBusy(!inputWhileBusy, committing: committing) }
        updateAvailability(); message.show(NativeText.value("ui.ipados.updating"), busy: true)
    }
    func invalidatePreview() {
        validated = nil; updateAvailability()
        if !busy { message.show(ModellingText.text("preview_required")) }
    }
    private func updateAvailability() {
        preview.availability(enabled: !busy && !contextRefreshing)
        apply.availability(enabled: !busy && !contextRefreshing && validated == params, reason: validated == params ? nil : ModellingText.text("preview_required"))
        cancel.availability(enabled: !committing)
        select.availability(enabled: !busy && !contextRefreshing)
        fieldViews.forEach { $0.isUserInteractionEnabled = !contextRefreshing && (!busy || inputWhileBusy) }
        discreteFieldViews.forEach { $0.setEditingEnabled(handleEntryEnabled) }
        sliders.forEach { $0.isUserInteractionEnabled = !contextRefreshing && (!busy || inputWhileBusy) }
    }
    private func changed() {
        locallyEdited = true
        invalidatePreview(); onChange?()
    }
    private func parameter(_ path: String) -> PresentationJSON? { PresentationJSON.parameter(path, in: params) }
    private func setParameter(_ path: String, value: PresentationJSON) { params = PresentationJSON.setting(path, value: value, in: params) }
    private func display(_ value: PresentationJSON) -> String {
        if case .string(let text) = value { return text }
        guard JSONSerialization.isValidJSONObject([value.value]),
              let data = try? JSONSerialization.data(withJSONObject: [value.value], options: [.sortedKeys]),
              let result = String(data: data, encoding: .utf8), result.count >= 2 else { return "—" }
        return String(result.dropFirst().dropLast())
    }
    private func rebuildFields() {
        fieldsStack.arrangedSubviews.forEach { fieldsStack.removeArrangedSubview($0); $0.removeFromSuperview() }
        editors.removeAll(); textFields.removeAll(); fieldViews.removeAll(); sliders.removeAll()
        additionalFields.arrangedSubviews.forEach { additionalFields.removeArrangedSubview($0); $0.removeFromSuperview() }
        discreteFieldViews.removeAll(); fieldGeneration &+= 1
        let generation = fieldGeneration
        var choiceGroup: ModellingFormGroup?
        var choiceStack: UIStackView?
        let compactExtrude = compact && fields.contains { $0.key == "operation" && $0.kind == "inline_choice" }
        let orderedFields = InspectorPresentation.orderedFields(fields, compact: compact)
        for field in orderedFields {
            let stack = compactExtrude && !InspectorPresentation.isPrimary(field) ? additionalFields : fieldsStack
            if field.kind == "choice" {
                let group: ModellingFormGroup
                if let existing = choiceGroup, choiceStack === stack { group = existing }
                else {
                    group = ModellingFormGroup(); choiceGroup = group; choiceStack = stack
                    stack.addArrangedSubview(group); fieldViews.append(group); discreteFieldViews.append(group)
                }
                buildChoice(field, in: group, generation: generation)
                continue
            }
            choiceGroup = nil; choiceStack = nil
            let heading = WorkspaceStyle.label(.body)
            heading.text = ModellingText.label(field.label)
            if field.kind != "references" && (!compact || field.kind != "expression" && field.kind != "slider") {
                heading.font = .preferredFont(forTextStyle: compact ? .caption1 : .body)
                stack.addArrangedSubview(heading)
            }
            if field.kind == "inline_choice" {
                let choices = ModellingInlineChoices(field: field, selected: parameter(field.key), illustrated: compact) { [weak self] value in
                    self?.selectInlineChoice(field.key, value: value)
                }
                stack.addArrangedSubview(choices); fieldViews.append(choices)
            } else if field.kind == "text" {
                let input = UITextField()
                input.text = parameter(field.key).map(display) ?? ""
                input.borderStyle = .roundedRect; input.autocorrectionType = .no; input.autocapitalizationType = .none
                input.font = .preferredFont(forTextStyle: .body); input.adjustsFontForContentSizeCategory = true
                input.heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
                input.accessibilityLabel = ModellingText.label(field.label)
                input.accessibilityIdentifier = "workspace-model-field-" + field.key
                input.addAction(UIAction { [weak self, weak input] _ in self?.setParameter(field.key, value: .string(input?.text ?? "")); self?.changed() }, for: .editingChanged)
                input.addAction(UIAction { [weak self] _ in self?.onFocus?() }, for: .editingDidBegin)
                textFields.append(input); fieldViews.append(input); stack.addArrangedSubview(input)
            } else if field.kind == "expression" || field.kind == "slider" {
                let expression = parameter(field.key).map(display) ?? ""
                let editor = NumericExpressionEditor(expression: expression, units: field.units ?? "", showsActions: false,
                                                     label: compact ? ModellingText.label(field.label) : nil, compact: compact,
                                                     primary: compactExtrude && InspectorPresentation.isDistance(field.key))
                editor.field.accessibilityIdentifier = "workspace-model-field-" + field.key
                editor.onChange = { [weak self, weak editor] in
                    guard let self, let editor else { return }
                    self.setParameter(field.key, value: .string(editor.field.text ?? "")); self.changed()
                }
                editor.onFocus = { [weak self] in self?.onFocus?() }
                editor.onRequestEntry = { [weak self, weak editor] in
                    guard let self, let editor, self.handleEntryEnabled, let open = self.onEditValue else { return false }
                    open(field, editor.field.text ?? "", editor.field)
                    return true
                }
                editors.append(editor); fieldViews.append(editor); stack.addArrangedSubview(editor)
                if field.kind == "slider" { buildSlider(field, editor: editor, in: stack) }
            } else if field.kind == "references" {
                buildReferences(field, in: stack)
            } else if compact && field.kind == "reference" {
                buildSource(field, in: stack)
            } else {
                let selected = parameter(field.key)
                let title = field.options?.first(where: { $0.value == selected }).map { ModellingText.label($0.label) }
                    ?? selected.map(display) ?? ModellingText.text("choose")
                let button = choiceButton(title, field: field) { [weak self] value in
                    guard let self else { return }
                    self.setParameter(field.key, value: value); self.changed(); self.rebuildFields()
                    self.onContextChange?(self.params)
                }
                stack.addArrangedSubview(button); fieldViews.append(button)
                if field.options?.isEmpty != false {
                    let hint = WorkspaceStyle.label(.footnote, secondary: true); hint.text = ModellingText.text("no_inputs"); stack.addArrangedSubview(hint)
                }
            }
            if !compact && field.key == "profile" {
                let pick = WorkspaceButton(ModellingText.text("select_in_viewport")) { [weak self] in
                    self?.endEditing(true); self?.onSelect?(field.key)
                }
                pick.accessibilityIdentifier = "workspace-model-select-" + field.key
                stack.addArrangedSubview(pick); fieldViews.append(pick)
            }
            if stack === fieldsStack { fieldsStack.setCustomSpacing(compact ? 12 : 16, after: stack.arrangedSubviews.last ?? heading) }
        }
        if !additionalFields.arrangedSubviews.isEmpty {
            let options = InspectorDisclosure(title: InspectorText.text("more_options"), expanded: optionsExpanded,
                                              identifier: "workspace-model-more-options") { [weak self] expanded in
                self?.optionsExpanded = expanded
            }
            options.content.addArrangedSubview(additionalFields)
            fieldsStack.addArrangedSubview(options)
        }
        if compact && !fields.contains(where: { $0.key == "profile" && $0.kind == "reference" }) {
            fieldsStack.addArrangedSubview(select)
        }
        updateAvailability()
    }
    private func buildSource(_ field: ModellingState.Field, in stack: UIStackView) {
        let selected = parameter(field.key)
        let summary = WorkspaceStyle.label(.subheadline)
        summary.text = field.options?.first(where: { $0.value == selected }).map { ModellingText.label($0.label) }
            ?? referenceSummary(selected)
        summary.numberOfLines = 2
        summary.lineBreakMode = .byTruncatingTail
        summary.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        summary.accessibilityLabel = ModellingText.label(field.label)
        summary.accessibilityValue = summary.text
        summary.accessibilityIdentifier = "workspace-model-summary-" + field.key
        let change = choiceButton(InspectorText.text("change"), field: field) { [weak self] value in
            guard let self else { return }
            self.setParameter(field.key, value: value); self.changed(); self.rebuildFields()
            self.onContextChange?(self.params)
        }
        change.isEnabled = true
        change.accessibilityHint = nil
        change.accessibilityValue = nil
        let options = change.menu?.children ?? []
        change.menu = UIMenu(children: options + [UIAction(title: ModellingText.text("select_in_viewport")) { [weak self] _ in
            self?.endEditing(true); self?.onSelect?(field.key)
        }])
        change.setContentCompressionResistancePriority(.required, for: .horizontal)
        change.setContentHuggingPriority(.required, for: .horizontal)
        let row = UIStackView(arrangedSubviews: [summary, change])
        row.axis = .horizontal; row.spacing = 8; row.alignment = .center
        row.isLayoutMarginsRelativeArrangement = true
        row.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 4, leading: 10, bottom: 4, trailing: 2)
        row.backgroundColor = WorkspaceStyle.selectionFill
        row.layer.cornerRadius = 8
        stack.addArrangedSubview(row); fieldViews.append(row)
    }
    private func referenceSummary(_ value: PresentationJSON?) -> String {
        guard let value, value != .null else { return ModellingText.text("no_selection") }
        if case .string(let name) = value { return name }
        // Body names are supplied by Rust; never invent a face index from a
        // point or expose the JSON command representation as a user label.
        if case .object(let source) = value, case .string(let name) = source["face_body"] { return name }
        return InspectorText.text("selected_input")
    }
    private func canEditFields(_ generation: UInt64) -> Bool {
        generation == fieldGeneration && handleEntryEnabled
    }
    private func buildChoice(_ field: ModellingState.Field, in group: ModellingFormGroup, generation: UInt64) {
        let selected = parameter(field.key)
        let label = ModellingText.label(field.label)
        let options = field.options ?? []
        let control: UIControl
        if let value = ModellingFormPresentation.booleanValue(kind: field.kind, options: options.map(\.value), selected: selected) {
            let toggle = ModellingFormSwitch()
            toggle.isOn = value
            toggle.accessibilityLabel = label
            toggle.accessibilityIdentifier = "workspace-model-field-" + field.key
            toggle.addAction(UIAction { [weak self, weak toggle] _ in
                guard let self, let toggle else { return }
                guard self.canEditFields(generation) else {
                    if case .bool(let value) = self.parameter(field.key) { toggle.setOn(value, animated: false) }
                    return
                }
                self.setParameter(field.key, value: .bool(toggle.isOn)); self.changed(); self.rebuildFields()
                self.onContextChange?(self.params)
            }, for: .valueChanged)
            control = toggle
        } else {
            let title = options.first(where: { $0.value == selected }).map { ModellingFormText.choice($0.label) }
                ?? selected.map(display) ?? ModellingText.text("choose")
            let button = ModellingFormButton(title: title, symbol: "chevron.up.chevron.down", label: label,
                                             identifier: "workspace-model-field-" + field.key)
            button.accessibilityValue = title
            button.menu = UIMenu(children: options.map { option in
                UIAction(title: ModellingFormText.choice(option.label), state: option.value == selected ? .on : .off) { [weak self] _ in
                    guard let self, self.canEditFields(generation) else { return }
                    self.setParameter(field.key, value: option.value); self.changed(); self.rebuildFields()
                    self.onContextChange?(self.params)
                }
            })
            button.showsMenuAsPrimaryAction = true; button.isEnabled = !options.isEmpty
            if options.isEmpty { button.accessibilityHint = ModellingText.text("no_inputs") }
            control = button
        }
        group.append(ModellingFormRow(title: label, control: control))
    }
    private func choiceButton(_ title: String, field: ModellingState.Field, action: @escaping (PresentationJSON) -> Void) -> WorkspaceButton {
        let button = WorkspaceButton(title, action: {})
        button.menu = UIMenu(children: (field.options ?? []).map { option in
            UIAction(title: ModellingText.label(option.label)) { _ in action(option.value) }
        })
        button.showsMenuAsPrimaryAction = true
        button.accessibilityIdentifier = "workspace-model-field-" + field.key
        if field.options?.isEmpty != false { button.availability(enabled: false, reason: ModellingText.text("no_inputs")) }
        return button
    }
    private func buildSlider(_ field: ModellingState.Field, editor: NumericExpressionEditor, in stack: UIStackView) {
        guard let minimum = field.min, let maximum = field.max,
              minimum.isFinite, maximum.isFinite, minimum < maximum,
              abs(minimum) < Double(Float.greatestFiniteMagnitude), abs(maximum) < Double(Float.greatestFiniteMagnitude) else { return }
        let resolved: Double?
        switch parameter(field.key) {
        case .number(let value): resolved = value
        case .integer(let value): resolved = Double(value)
        case .unsignedInteger(let value): resolved = Double(value)
        default: resolved = field.value
        }
        guard let resolved, resolved.isFinite, abs(resolved) < Double(Float.greatestFiniteMagnitude) else { return }
        let slider = UISlider()
        slider.minimumValue = Float(minimum); slider.maximumValue = Float(maximum)
        slider.value = Float(resolved); slider.isContinuous = true
        slider.accessibilityLabel = ModellingText.label(field.label)
        slider.accessibilityIdentifier = "workspace-model-slider-" + field.key
        slider.heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        var start = parameter(field.key)
        slider.addAction(UIAction { [weak self] _ in
            guard let self else { return }; start = self.parameter(field.key); self.sliderActive = true
        }, for: .touchDown)
        slider.addAction(UIAction { [weak self, weak slider, weak editor] _ in
            guard let self, let slider else { return }
            let units = field.units ?? ""
            let value = min(maximum, max(minimum, DragResolution.snapped(Double(slider.value), units: units, step: DragResolution.current)))
            slider.value = Float(value)
            let number = String(value)
            let expression = number + (units.isEmpty ? "" : " " + units)
            self.setParameter(field.key, value: units.isEmpty ? .number(value) : .string(expression))
            editor?.field.text = expression; self.changed(); self.onLivePreview?(self.params)
            slider.accessibilityValue = expression
        }, for: .valueChanged)
        slider.addAction(UIAction { [weak self] _ in
            guard let self else { return }; self.sliderActive = false; self.onLivePreview?(self.params)
        }, for: [.touchUpInside, .touchUpOutside])
        slider.addAction(UIAction { [weak self, weak editor] _ in
            guard let self else { return }; self.sliderActive = false
            self.setParameter(field.key, value: start ?? .null)
            editor?.field.text = start.map(self.display) ?? ""
            self.changed(); self.onLivePreview?(self.params)
        }, for: .touchCancel)
        stack.addArrangedSubview(slider); fieldViews.append(slider); sliders.append(slider)
        let range = WorkspaceStyle.label(.footnote, secondary: true)
        range.text = String(minimum) + " … " + String(maximum) + " " + (field.units ?? "")
        stack.addArrangedSubview(range)
    }
    private func buildReferences(_ field: ModellingState.Field, in stack: UIStackView) {
        let values: [PresentationJSON]
        if case .array(let current) = parameter(field.key) { values = current } else { values = [] }
        let generation = fieldGeneration
        let body = field.key == "targets"
        let title = WorkspaceStyle.label(.subheadline)
        title.text = body ? ModellingFormText.text("target_bodies") : ModellingText.label(field.label)
        title.accessibilityTraits = .header
        title.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let count = WorkspaceStyle.label(.caption1, secondary: true)
        count.text = ModellingFormText.text("selection_count").replacingOccurrences(of: "{count}", with: String(values.count))
        count.setContentHuggingPriority(.required, for: .horizontal)
        let heading = InspectorReferenceHeading(title: title, count: count)
        heading.accessibilityIdentifier = "workspace-model-reference-" + field.key + "-heading"
        stack.addArrangedSubview(heading); stack.setCustomSpacing(8, after: heading)
        let group = ModellingFormGroup()
        stack.addArrangedSubview(group); fieldViews.append(group); discreteFieldViews.append(group)
        if values.isEmpty {
            let empty = WorkspaceStyle.label(.footnote, secondary: true)
            empty.text = ModellingText.text("no_selection")
            let row = UIStackView(arrangedSubviews: [empty]); row.isLayoutMarginsRelativeArrangement = true
            row.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 14, leading: 14, bottom: 14, trailing: 14)
            group.append(row)
        }
        for (index, value) in values.enumerated() {
            let name = field.options?.first(where: { $0.value == value }).map { ModellingText.label($0.label) } ?? display(value)
            var moves: [UIAction] = []
            if index > 0 {
                moves.append(UIAction(title: ModellingText.text("move_up"), image: UIImage(systemName: "arrow.up")) { [weak self] _ in
                    self?.editReferences(field.key, generation: generation) { ModellingFormPresentation.moving(from: index, to: index - 1, in: $0) }
                })
            }
            if index + 1 < values.count {
                moves.append(UIAction(title: ModellingText.text("move_down"), image: UIImage(systemName: "arrow.down")) { [weak self] _ in
                    self?.editReferences(field.key, generation: generation) { ModellingFormPresentation.moving(from: index, to: index + 1, in: $0) }
                })
            }
            let row = ModellingReferenceRow(title: name, index: index, count: values.count, body: body,
                                           identifier: "workspace-model-reference-" + field.key + "-" + String(index), moveActions: moves) { [weak self] in
                self?.editReferences(field.key, generation: generation) { ModellingFormPresentation.removing(at: index, from: $0) }
            }
            group.append(row)
        }
        let options = (field.options ?? []).filter { !values.contains($0.value) }
        let addTitle = ModellingFormText.text(body ? "select_bodies" : "select_inputs")
        let add = ModellingFormButton(title: addTitle, symbol: "plus", label: addTitle,
                                      identifier: "workspace-model-field-" + field.key, accent: true)
        let listActions = options.map { option in
            UIAction(title: ModellingText.label(option.label)) { [weak self] _ in
                self?.editReferences(field.key, generation: generation) { current in
                    current.contains(option.value) ? nil : current + [option.value]
                }
            }
        }
        var actions: [UIMenuElement] = listActions
        if body {
            actions = listActions.isEmpty ? [] : [UIMenu(title: ModellingFormText.text("choose_from_list"), children: listActions)]
            actions.append(UIAction(title: ModellingFormText.text("select_in_model"), image: UIImage(systemName: "viewfinder")) { [weak self] _ in
                guard let self, self.canEditFields(generation) else { return }
                self.endEditing(true); self.onSelect?(field.key)
            })
        }
        add.menu = UIMenu(children: actions)
        add.showsMenuAsPrimaryAction = true; add.isEnabled = !actions.isEmpty
        if actions.isEmpty { add.accessibilityHint = ModellingText.text("no_inputs") }
        group.append(add)
    }
    private func editReferences(_ key: String, generation: UInt64, edit: ([PresentationJSON]) -> [PresentationJSON]?) {
        guard canEditFields(generation) else { return }
        let current: [PresentationJSON]
        if case .array(let values) = parameter(key) { current = values } else { current = [] }
        guard let next = edit(current), next != current else { return }
        setParameter(key, value: .array(next)); changed(); rebuildFields()
    }
}

/// View-only controls: the tool menus and model browser keep their own chrome.
/// This popover contains no text input and dispatches existing viewport commands.
private final class ViewOptionButton: UIButton {
    private let label: String
    init(_ title: String, id: String, action: @escaping () -> Void) {
        label = title
        super.init(frame: .zero)
        accessibilityIdentifier = "workspace-view-" + id
        accessibilityLabel = title
        titleLabel?.numberOfLines = 0
        titleLabel?.adjustsFontForContentSizeCategory = true
        heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
        isPointerInteractionEnabled = true
        render(selected: false, enabled: true)
    }
    required init?(coder: NSCoder) { nil }
    func render(selected: Bool, enabled: Bool) {
        isEnabled = enabled; isSelected = selected
        var style = UIButton.Configuration.plain()
        style.title = label
        style.titleAlignment = .leading
        style.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 12, bottom: 10, trailing: 12)
        style.baseForegroundColor = WorkspaceStyle.foreground
        style.background.backgroundColor = selected ? WorkspaceStyle.accent.withAlphaComponent(0.16) : WorkspaceStyle.fieldBackground
        style.background.cornerRadius = 8
        style.background.strokeWidth = selected ? 1 : 0.5
        style.background.strokeColor = selected ? WorkspaceStyle.accent : WorkspaceStyle.separator
        style.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { original in
            var attributes = original; attributes.font = UIFont.preferredFont(forTextStyle: .body); return attributes
        }
        style.image = selected ? UIImage(systemName: "checkmark") : nil
        style.imagePlacement = .trailing; style.imagePadding = 8
        style.imageColorTransformer = UIConfigurationColorTransformer { _ in WorkspaceStyle.accent }
        configuration = style
        contentHorizontalAlignment = .fill
        if selected { accessibilityTraits.insert(.selected) } else { accessibilityTraits.remove(.selected) }
    }
}

private final class ViewSwitchRow: UIButton {
    let toggle = UISwitch()
    private let title = WorkspaceStyle.label(.body)
    private let subtitle = WorkspaceStyle.label(.caption1, secondary: true)
    private let mixedMark = WorkspaceStyle.label(.caption1)
    private let detail: String?
    init(_ label: String, subtitle detail: String? = nil, id: String, action: @escaping () -> Void) {
        self.detail = detail
        super.init(frame: .zero)
        isAccessibilityElement = false
        isPointerInteractionEnabled = true
        addAction(UIAction { _ in action() }, for: .primaryActionTriggered)
        backgroundColor = WorkspaceStyle.fieldBackground
        layer.cornerRadius = 8; layer.borderWidth = 0.5
        title.text = label; subtitle.text = detail
        title.isAccessibilityElement = false; subtitle.isAccessibilityElement = false
        let labels = UIStackView(arrangedSubviews: [title, subtitle]); labels.axis = .vertical; labels.spacing = 2
        subtitle.isHidden = detail == nil
        toggle.accessibilityIdentifier = "workspace-view-" + id
        toggle.accessibilityLabel = label
        toggle.addAction(UIAction { _ in action() }, for: .valueChanged)
        mixedMark.text = "−"; mixedMark.textColor = WorkspaceStyle.selectedForeground; mixedMark.isAccessibilityElement = false
        mixedMark.textAlignment = .center; mixedMark.isUserInteractionEnabled = false
        [labels, toggle, mixedMark].forEach { $0.translatesAutoresizingMaskIntoConstraints = false }
        addSubview(labels); addSubview(toggle); toggle.addSubview(mixedMark)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(greaterThanOrEqualToConstant: 48),
            labels.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            labels.topAnchor.constraint(equalTo: topAnchor, constant: 9),
            labels.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -9),
            labels.trailingAnchor.constraint(lessThanOrEqualTo: toggle.leadingAnchor, constant: -12),
            toggle.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            toggle.centerYAnchor.constraint(equalTo: centerYAnchor),
            mixedMark.leadingAnchor.constraint(equalTo: toggle.leadingAnchor, constant: 4),
            mixedMark.centerYAnchor.constraint(equalTo: toggle.centerYAnchor),
            mixedMark.widthAnchor.constraint(equalToConstant: 20)
        ])
        toggle.setContentHuggingPriority(.required, for: .horizontal)
        toggle.setContentCompressionResistancePriority(.required, for: .horizontal)
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (row: ViewSwitchRow, _: UITraitCollection) in
            row.layer.borderColor = WorkspaceStyle.separator.cgColor
        }
        render(on: false, enabled: false)
    }
    required init?(coder: NSCoder) { nil }
    func render(on: Bool, mixed: Bool = false, enabled: Bool) {
        isEnabled = enabled
        toggle.setOn(on || mixed, animated: false)
        toggle.isEnabled = enabled
        toggle.onTintColor = mixed ? .systemGray : WorkspaceStyle.accent
        mixedMark.isHidden = !mixed
        subtitle.text = mixed ? [detail, ViewText.text("some_visible")].compactMap { $0 }.joined(separator: " · ") : detail
        subtitle.isHidden = subtitle.text?.isEmpty != false
        toggle.accessibilityValue = mixed ? ViewText.text("some_visible") : nil
        toggle.accessibilityHint = detail
        layer.borderColor = WorkspaceStyle.separator.cgColor
        title.alpha = enabled ? 1 : 0.5; subtitle.alpha = title.alpha
    }
}

final class WorkspaceViewPopover: UIViewController {
    var onCommand: ((String) -> Void)?
    var onPerspective: (() -> Void)?
    var onGrid: (() -> Void)?
    var onIntegratedAxes: (() -> Void)?
    var onOrigin: (() -> Void)?
    var onReference: ((String?, String?, Bool?) -> Void)?
    var onOriginInModel: (() -> Void)?
    var onLegacyPlanes: (() -> Void)?
    private let scroll = UIScrollView()
    private let root = UIStackView()
    private let main = UIStackView()
    private let individuals = UIStackView()
    private let views = UIStackView()
    private let heading = WorkspaceStyle.label(.title2)
    private let separator = UIView()
    private var viewButtons: [String: ViewOptionButton] = [:]
    private lazy var fit = ViewOptionButton(WorkspaceStyle.text("fit"), id: "fit") { [weak self] in self?.onCommand?("view.fit") }
    private lazy var exitPlanar = ViewOptionButton(NativeText.value("ui.ipados.exit_planar"), id: "exit-planar") { [weak self] in self?.onCommand?("view.exit_planar") }
    private lazy var perspective = ViewSwitchRow(NativeText.value("ui.ipados.perspective"), id: "perspective") { [weak self] in self?.onPerspective?() }
    private lazy var grid = ViewSwitchRow(ViewText.text("grid"), id: "grid") { [weak self] in self?.onGrid?() }
    private lazy var integratedAxes = ViewSwitchRow(ViewText.text("integrated_axes"), subtitle: ViewText.text("integrated_axes_help"), id: "integrated-axes") { [weak self] in self?.onIntegratedAxes?() }
    private lazy var origin = ViewSwitchRow(SketchText.text("origin_references"), subtitle: ViewText.text("origin_subtitle"), id: "origin") { [weak self] in self?.onOrigin?() }
    private lazy var individualButton = ViewOptionButton(ViewText.text("individual_references"), id: "individual-references") { [weak self] in self?.showIndividuals(true) }
    private lazy var automatic = ViewOptionButton(SketchText.text("automatic_origin"), id: "automatic") { [weak self] in self?.onReference?(nil, nil, nil) }
    private lazy var originInModel = ViewSwitchRow(WorkspaceStyle.text("origin_in_model"), id: "origin-in-model") { [weak self] in self?.onOriginInModel?() }
    private lazy var legacyPlanes = ViewSwitchRow(ViewText.text("legacy_planes"), subtitle: ViewText.text("legacy_planes_help"), id: "legacy-planes") { [weak self] in self?.onLegacyPlanes?() }
    private var references: [String: ViewSwitchRow] = [:]
    private var groups: [String: ViewSwitchRow] = [:]
    private var allActions: [ViewOptionButton] = []
    private var current: ViewControlsPresentation?

    init() {
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .popover
        traitOverrides.userInterfaceLevel = .base
        preferredContentSize = CGSize(width: 340, height: 590)
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.surface
        scroll.contentInsetAdjustmentBehavior = .never
        root.axis = .vertical; root.spacing = 0
        for stack in [main, individuals, views] { stack.axis = .vertical; stack.spacing = 8 }
        root.addArrangedSubview(main); root.addArrangedSubview(individuals)
        scroll.translatesAutoresizingMaskIntoConstraints = false; root.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll); scroll.addSubview(root)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: view.topAnchor), scroll.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            root.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16),
            root.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16),
            root.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            root.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -16),
            root.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32)
        ])
        heading.text = WorkspaceStyle.text("view"); heading.accessibilityTraits.insert(.header)
        main.addArrangedSubview(heading); main.setCustomSpacing(16, after: heading)
        main.addArrangedSubview(fit)
        let standard = WorkspaceStyle.label(.subheadline, secondary: true); standard.text = ViewText.text("standard_views")
        views.addArrangedSubview(standard)
        for names in [["front", "back"], ["left", "right"], ["top", "bottom"], ["isometric"]] {
            let row = UIStackView(); row.axis = .horizontal; row.spacing = 8; row.distribution = .fillEqually
            for name in names {
                let button = ViewOptionButton(NativeText.value("ui.ipados." + name), id: name) { [weak self] in self?.onCommand?("view." + name) }
                viewButtons[name] = button; row.addArrangedSubview(button)
            }
            views.addArrangedSubview(row)
        }
        main.setCustomSpacing(16, after: fit); main.addArrangedSubview(views)
        main.addArrangedSubview(exitPlanar)
        separator.backgroundColor = WorkspaceStyle.separator
        separator.heightAnchor.constraint(equalToConstant: 0.5).isActive = true
        main.addArrangedSubview(separator); main.setCustomSpacing(12, after: separator)
        [perspective, grid, integratedAxes, origin, individualButton].forEach(main.addArrangedSubview)
        let back = ViewOptionButton(WorkspaceStyle.text("view"), id: "back") { [weak self] in self?.showIndividuals(false) }
        back.configuration?.image = WorkspaceIcon.image("chevron_right", traits: traitCollection, size: 18)
        back.configuration?.imagePlacement = .leading; back.imageView?.transform = CGAffineTransform(rotationAngle: .pi)
        individuals.addArrangedSubview(back)
        let individualTitle = WorkspaceStyle.label(.headline); individualTitle.text = ViewText.text("individual_references")
        individualTitle.accessibilityTraits.insert(.header); individuals.addArrangedSubview(individualTitle)
        func reference(_ item: String) {
            let row = ViewSwitchRow(item == "O" ? SketchText.text("origin_point") : item, id: "reference-" + item) { [weak self] in
                guard let self else { return }; self.onReference?(item, nil, !(self.current?.originVisible(item) ?? false))
            }
            references[item] = row; individuals.addArrangedSubview(row)
        }
        reference("O")
        for (group, items) in [("axes", ["X", "Y", "Z"]), ("planes", ["XY", "XZ", "YZ"])] {
            let row = ViewSwitchRow(WorkspaceStyle.text("all_" + group), id: "group-" + group) { [weak self] in
                guard let self else { return }
                self.onReference?(nil, group, !items.allSatisfy { self.current?.originVisible($0) == true })
            }
            groups[group] = row; individuals.addArrangedSubview(row)
            items.forEach(reference)
        }
        let allRow = UIStackView(); allRow.spacing = 8; allRow.distribution = .fillEqually
        for visible in [true, false] {
            let button = ViewOptionButton(SketchText.text(visible ? "show_origin" : "hide_origin"), id: visible ? "show-all" : "hide-all") { [weak self] in self?.onReference?(nil, nil, visible) }
            allActions.append(button); allRow.addArrangedSubview(button)
        }
        individuals.addArrangedSubview(legacyPlanes)
        individuals.addArrangedSubview(allRow); individuals.addArrangedSubview(automatic); individuals.addArrangedSubview(originInModel)
        showIndividuals(false)
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (panel: WorkspaceViewPopover, _: UITraitCollection) in panel.view.setNeedsLayout() }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        popoverPresentationController?.backgroundColor = WorkspaceStyle.surface.resolvedColor(with: traitCollection)
        let width = min(340, max(240, (presentingViewController?.view.bounds.width ?? 372) - 32))
        let size = root.systemLayoutSizeFitting(CGSize(width: width - 32, height: 0),
            withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel)
        let desired = CGSize(width: width, height: ceil(size.height) + 32)
        if preferredContentSize != desired { preferredContentSize = desired }
    }
    private func showIndividuals(_ visible: Bool) {
        for page in [main, individuals] { root.removeArrangedSubview(page); page.removeFromSuperview() }
        root.addArrangedSubview(visible ? individuals : main)
        scroll.setContentOffset(.zero, animated: false); view.setNeedsLayout()
        if view.window != nil { UIAccessibility.post(notification: .layoutChanged, argument: visible ? individuals : main) }
    }
    func render(_ state: ViewControlsPresentation?, cameraEnabled: Bool, referencesEnabled: Bool,
                inSketch: Bool, automatic: Bool, originInModel: Bool, legacyPlanes: Bool) {
        loadViewIfNeeded(); current = state
        fit.render(selected: false, enabled: cameraEnabled)
        fit.contentHorizontalAlignment = .leading
        var arranged: [UIView] = [heading, fit]
        if !inSketch { arranged.append(views) }
        if !inSketch && state?.planar == true { arranged.append(exitPlanar) }
        arranged.append(separator)
        if !inSketch { arranged.append(perspective) }
        arranged += [grid, integratedAxes, origin, individualButton]
        if main.arrangedSubviews.count != arranged.count || !zip(main.arrangedSubviews, arranged).allSatisfy({ $0 === $1 }) {
            for child in main.arrangedSubviews { main.removeArrangedSubview(child); child.removeFromSuperview() }
            arranged.forEach(main.addArrangedSubview)
            main.setCustomSpacing(16, after: heading); main.setCustomSpacing(16, after: fit); main.setCustomSpacing(12, after: separator)
        }
        exitPlanar.render(selected: false, enabled: cameraEnabled)
        for (name, button) in viewButtons { button.render(selected: state?.camera.standardView == name, enabled: cameraEnabled && state?.planar != true) }
        perspective.render(on: (state?.camera.fov ?? 0) > 0, enabled: cameraEnabled)
        grid.render(on: state?.grid.visible == true, enabled: referencesEnabled)
        integratedAxes.render(on: state?.grid.integrated_axes == true, enabled: referencesEnabled)
        origin.render(on: state?.visibility == .all, mixed: state?.visibility == .some, enabled: referencesEnabled)
        individualButton.isEnabled = true
        individualButton.configuration?.image = WorkspaceIcon.image("chevron_right", traits: traitCollection, size: 18)
        individualButton.configuration?.imagePlacement = .trailing
        for (name, row) in references { row.render(on: state?.originVisible(name) == true, enabled: referencesEnabled) }
        for (name, row) in groups {
            let items = name == "axes" ? ["X", "Y", "Z"] : ["XY", "XZ", "YZ"]
            let status = ViewControlsPresentation.visibility(items.map { state?.originVisible($0) == true })
            row.render(on: status == .all, mixed: status == .some, enabled: referencesEnabled)
        }
        self.automatic.render(selected: automatic, enabled: referencesEnabled)
        self.originInModel.render(on: originInModel, enabled: true)
        self.legacyPlanes.render(on: legacyPlanes, enabled: referencesEnabled)
        allActions.forEach { $0.render(selected: false, enabled: referencesEnabled) }
        view.setNeedsLayout()
    }
    override var keyCommands: [UIKeyCommand]? {
        [UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(close))]
    }
    @objc private func close() { dismiss(animated: true) }
}
