import UIKit

enum QuickAccessText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.quick_access." + key) }
}

/// Presentation-only customization shared by the two native vertical bars.
/// A drag keeps the real control in the stack while a lifted snapshot follows
/// the contact. Commands still use the control's existing production action.
final class QuickAccessBar: NSObject, UIGestureRecognizerDelegate {
    private let stack: UIStackView
    private let scroll: UIScrollView
    private let container: UIView
    private let preferenceKey: String
    private let defaults: [String]
    private var layout: QuickAccessLayout
    private(set) var buttons: [String: WorkspaceButton] = [:]
    private var catalog: [String] = []
    private var fixed: [UIView] = []
    var customizationEnabled = true {
        didSet {
            if !customizationEnabled { cancelDrag() }
            for (id, button) in buttons { configureHoldActions(id, button: button) }
        }
    }
    var onChange: (() -> Void)?
    var onBegin: (() -> Void)?
    var onLayout: (() -> Void)?
    private var held: String?
    private var beforeDrag: QuickAccessLayout?
    private var lifted: UIView?
    private var grabOffset = CGPoint.zero
    private var contact = CGPoint.zero
    private var displayLink: CADisplayLink?
    private var scrollWasEnabled = true
    private var lastTick: CFTimeInterval = 0
    private let removal = UILabel()

    init(stack: UIStackView, scroll: UIScrollView, container: UIView,
         preferenceKey: String, defaults: [String]) {
        self.stack = stack; self.scroll = scroll; self.container = container
        self.preferenceKey = preferenceKey; self.defaults = defaults
        layout = QuickAccessLayout(defaults: defaults, saved: UserDefaults.standard.data(forKey: preferenceKey))
        super.init()
        removal.text = QuickAccessText.text("remove")
        removal.font = .preferredFont(forTextStyle: .subheadline)
        removal.adjustsFontForContentSizeCategory = true
        removal.textAlignment = .center; removal.numberOfLines = 0
        removal.textColor = WorkspaceStyle.foreground
        removal.backgroundColor = WorkspaceStyle.surface
        removal.layer.cornerRadius = WorkspaceStyle.radius; removal.clipsToBounds = true
        removal.isUserInteractionEnabled = false
        removal.accessibilityIdentifier = preferenceKey + ".remove"
        // Resigning active also cancels an unfinished edit and releases the timer.
        NotificationCenter.default.addObserver(self, selector: #selector(cancelDrag),
            name: UIApplication.willResignActiveNotification, object: nil)
    }

    deinit { displayLink?.invalidate() }

    func register(_ id: String, button: WorkspaceButton) {
        guard buttons[id] == nil else { return }
        buttons[id] = button; catalog.append(id)
        let hold = UILongPressGestureRecognizer(target: self, action: #selector(drag(_:)))
        hold.minimumPressDuration = 0.4; hold.delegate = self
        hold.cancelsTouchesInView = true
        // Primary-action menus can open on touch down. Let a hold claim the
        // contact first; short taps are delivered when the hold fails.
        hold.delaysTouchesBegan = true
        button.addGestureRecognizer(hold)
        configureHoldActions(id, button: button)
    }

    private func configureHoldActions(_ id: String, button: WorkspaceButton) {
        for gesture in button.gestureRecognizers ?? [] where gesture.delegate === self {
            gesture.isEnabled = customizationEnabled
        }
        guard customizationEnabled else {
            button.accessibilityCustomActions = nil
            button.showsLargeContentViewer = true
            if !button.interactions.contains(where: { $0 is UILargeContentViewerInteraction }) {
                button.addInteraction(UILargeContentViewerInteraction())
            }
            return
        }
        // The toolbar's hold owns this contact, including menu buttons.
        button.showsLargeContentViewer = false
        for interaction in button.interactions where interaction is UILargeContentViewerInteraction {
            button.removeInteraction(interaction)
        }
        button.accessibilityCustomActions = [
            UIAccessibilityCustomAction(name: QuickAccessText.text("move_up")) { [weak self] _ in self?.step(id, down: false) ?? false },
            UIAccessibilityCustomAction(name: QuickAccessText.text("move_down")) { [weak self] _ in self?.step(id, down: true) ?? false },
            UIAccessibilityCustomAction(name: QuickAccessText.text("remove")) { [weak self] _ in
                self?.remove(id); return self != nil
            }
        ]
    }

    func contains(_ id: String) -> Bool { customizationEnabled && layout.contains(id) }

    func arrange(fixed: [UIView]) {
        self.fixed = fixed
        guard customizationEnabled else { return }
        if let id = held, buttons[id]?.isHidden != false { cancelDrag() }
        applyOrder()
    }

    private func applyOrder() {
        let desired: [UIView] = layout.order.compactMap { buttons[$0] } + fixed
        guard stack.arrangedSubviews != desired else { return }
        for view in stack.arrangedSubviews where !desired.contains(view) {
            stack.removeArrangedSubview(view); view.removeFromSuperview()
        }
        for (index, view) in desired.enumerated() where stack.arrangedSubviews.firstIndex(of: view) != index {
            stack.removeArrangedSubview(view)
            stack.insertArrangedSubview(view, at: index)
        }
    }

    /// Only contextually visible tools participate. Disabled tools stay visible
    /// with the same reason and remain customizable.
    private var visible: [String] { layout.order.filter { buttons[$0]?.isHidden == false } }

    func overflowMenu() -> UIMenu {
        let ids = layout.overflow(from: catalog.filter { buttons[$0]?.isHidden == false })
        let actions: [UIMenuElement] = ids.compactMap { id in
            guard let button = buttons[id] else { return nil }
            if let menu = button.menu, button.showsMenuAsPrimaryAction {
                return UIMenu(title: button.accessibilityLabel ?? "", image: button.configuration?.image,
                              children: menu.children)
            }
            return UIAction(title: button.accessibilityLabel ?? "", image: button.configuration?.image,
                            attributes: button.isEnabled ? [] : .disabled) { [weak button] _ in
                guard let button, button.isEnabled else { return }
                button.sendActions(for: .touchUpInside)
                button.sendActions(for: .primaryActionTriggered)
            }
        }
        let add: [UIMenuElement] = ids.compactMap { id in
            guard let button = buttons[id] else { return nil }
            return UIAction(title: button.accessibilityLabel ?? "", image: button.configuration?.image) { [weak self] _ in
                self?.add(id)
            }
        }
        let addition: [UIMenuElement] = add.isEmpty ? [] : [UIMenu(title: QuickAccessText.text("add"), children: add)]
        return UIMenu(options: .displayInline, children: actions + addition + [
            UIAction(title: QuickAccessText.text("reset")) { [weak self] _ in self?.reset() }
        ])
    }

    func addMenu(ids: [String]) -> UIMenu? {
        guard customizationEnabled else { return nil }
        let actions = layout.overflow(from: ids).compactMap { id -> UIAction? in
            guard let button = buttons[id], !button.isHidden else { return nil }
            return UIAction(title: button.accessibilityLabel ?? "", image: button.configuration?.image) { [weak self] _ in self?.add(id) }
        }
        return actions.isEmpty ? nil : UIMenu(title: QuickAccessText.text("add"), children: actions)
    }

    func add(_ id: String) {
        guard customizationEnabled, held == nil, buttons[id] != nil else { return }
        layout.add(id); changed()
    }

    private func remove(_ id: String) {
        guard customizationEnabled, held == nil else { return }
        layout.remove(id); changed()
    }

    private func reset() {
        guard held == nil else { return }
        layout = QuickAccessLayout(defaults: defaults); changed()
    }

    private func changed() {
        UserDefaults.standard.set(layout.saved, forKey: preferenceKey)
        animate { self.applyOrder(); self.onLayout?(); self.container.layoutIfNeeded() }
        onChange?()
        UIAccessibility.post(notification: .layoutChanged, argument: scroll)
    }

    private func step(_ id: String, down: Bool) -> Bool {
        let ids = visible
        guard customizationEnabled, held == nil, let index = ids.firstIndex(of: id),
              ids.indices.contains(index + (down ? 1 : -1)) else { return false }
        layout.move(id, beside: ids[index + (down ? 1 : -1)], after: down)
        changed(); return true
    }

    func gestureRecognizerShouldBegin(_ gestureRecognizer: UIGestureRecognizer) -> Bool {
        customizationEnabled && held == nil && gestureRecognizer.view?.superview === stack && !scroll.isDecelerating
    }

    // Menu buttons have UIKit's own preview hold; our earlier hold cancels it.
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer,
                           shouldBeRequiredToFailBy otherGestureRecognizer: UIGestureRecognizer) -> Bool {
        otherGestureRecognizer is UILongPressGestureRecognizer
    }

    @objc private func drag(_ gesture: UILongPressGestureRecognizer) {
        let point = gesture.location(in: container)
        switch gesture.state {
        case .began:
            guard let button = gesture.view as? WorkspaceButton else { return }
            beginDrag(button, at: point)
        case .changed:
            contact = point; updateDrag()
        case .ended:
            contact = point; finishDrag(cancelled: false)
        case .cancelled, .failed:
            finishDrag(cancelled: true)
        default: break
        }
    }

    private func beginDrag(_ button: WorkspaceButton, at point: CGPoint) {
        guard held == nil, let id = buttons.first(where: { $0.value === button })?.key,
              layout.contains(id), !button.bounds.isEmpty else { return }
        onBegin?(); container.layoutIfNeeded()
        // A render-server snapshot can lose UIButton configuration artwork in
        // hierarchy captures. A local image keeps the lifted icon visible too.
        let preview = UIGraphicsImageRenderer(bounds: button.bounds).image { context in
            button.layer.render(in: context.cgContext)
        }
        let snapshot = UIImageView(image: preview)
        held = id; beforeDrag = layout
        snapshot.frame = button.convert(button.bounds, to: container)
        grabOffset = CGPoint(x: point.x - snapshot.center.x, y: point.y - snapshot.center.y)
        contact = point; lifted = snapshot
        snapshot.isUserInteractionEnabled = false
        snapshot.backgroundColor = WorkspaceStyle.surface
        snapshot.layer.cornerRadius = WorkspaceStyle.radius
        snapshot.layer.shadowColor = UIColor.label.cgColor
        snapshot.layer.shadowOpacity = 0.2; snapshot.layer.shadowRadius = 8
        container.addSubview(snapshot); button.alpha = 0.18
        button.isHighlighted = false
        scrollWasEnabled = scroll.isScrollEnabled; scroll.isScrollEnabled = false
        showRemovalTarget()
        animate { snapshot.transform = CGAffineTransform(scaleX: 1.12, y: 1.12) }
        UIImpactFeedbackGenerator(style: .light).impactOccurred()
        lastTick = 0
        let link = CADisplayLink(target: self, selector: #selector(tick(_:)))
        link.add(to: .main, forMode: .common); displayLink = link
    }

    private func showRemovalTarget() {
        let bar = scroll.convert(scroll.bounds, to: container)
        let width = min(230, max(120, container.bounds.width - bar.width - 36))
        let x = bar.midX < container.bounds.midX ? bar.maxX + 12 : bar.minX - width - 12
        let height = max(60, removal.sizeThatFits(CGSize(width: width - 16, height: .greatestFiniteMagnitude)).height + 20)
        removal.frame = CGRect(x: max(8, min(x, container.bounds.width - width - 8)),
                               y: max(8, min(contact.y - height / 2, container.bounds.height - height - 8)),
                               width: width, height: height)
        removal.alpha = 0; container.addSubview(removal)
        animate { self.removal.alpha = 1 }
    }

    private func updateDrag() {
        guard let id = held, let snapshot = lifted, let button = buttons[id] else { return }
        snapshot.center = CGPoint(x: contact.x - grabOffset.x, y: contact.y - grabOffset.y)
        let removing = removal.frame.contains(contact)
        removal.backgroundColor = removing ? WorkspaceStyle.error : WorkspaceStyle.surface
        removal.textColor = removing ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        guard !removing else { return }
        let bar = scroll.convert(scroll.bounds, to: container).insetBy(dx: -24, dy: -30)
        guard bar.contains(contact) else { return }
        let y = container.convert(snapshot.center, to: stack).y
        let others = visible.filter { $0 != id }
        guard let neighbor = others.min(by: {
            abs((buttons[$0]?.center.y ?? 0) - y) < abs((buttons[$1]?.center.y ?? 0) - y)
        }), let target = buttons[neighbor] else { return }
        let after = y > target.frame.midY
        // Cross a neighbor's midpoint before moving the placeholder.
        guard after && button.frame.midY < target.frame.midY || !after && button.frame.midY > target.frame.midY else { return }
        layout.move(id, beside: neighbor, after: after)
        animate { self.applyOrder(); self.stack.layoutIfNeeded() }
        UISelectionFeedbackGenerator().selectionChanged()
    }

    @objc private func tick(_ link: CADisplayLink) {
        guard held != nil else { return }
        let dt = lastTick == 0 ? link.duration : min(0.05, link.timestamp - lastTick)
        lastTick = link.timestamp
        let bar = scroll.convert(scroll.bounds, to: container)
        guard abs(contact.x - bar.midX) < bar.width / 2 + 24, !removal.frame.contains(contact) else { return }
        let edge: CGFloat = min(44, bar.height / 3)
        let speed: CGFloat = contact.y < bar.minY + edge ? -180 : (contact.y > bar.maxY - edge ? 180 : 0)
        let maximum = max(0, scroll.contentSize.height - scroll.bounds.height)
        let y = max(0, min(maximum, scroll.contentOffset.y + speed * dt))
        if y != scroll.contentOffset.y { scroll.contentOffset.y = y; updateDrag() }
    }

    @objc func cancelDrag() { finishDrag(cancelled: true) }

    private func finishDrag(cancelled: Bool) {
        displayLink?.invalidate(); displayLink = nil
        guard let id = held else { return }
        if cancelled, let original = beforeDrag { layout = original }
        else if removal.frame.contains(contact) { layout.remove(id) }
        let snapshot = lifted
        let removed = !layout.contains(id)
        held = nil; beforeDrag = nil; lifted = nil
        scroll.isScrollEnabled = scrollWasEnabled
        if !cancelled { UserDefaults.standard.set(layout.saved, forKey: preferenceKey) }
        animate {
            self.applyOrder(); self.onLayout?(); self.container.layoutIfNeeded()
            self.removal.alpha = 0
            if let button = self.buttons[id], !removed {
                snapshot?.transform = .identity
                snapshot?.frame = button.convert(button.bounds, to: self.container)
                button.alpha = 1
            } else { snapshot?.alpha = 0; snapshot?.transform = CGAffineTransform(scaleX: 0.5, y: 0.5) }
        } completion: {
            snapshot?.removeFromSuperview(); self.removal.removeFromSuperview()
            self.buttons[id]?.alpha = 1
        }
        onChange?()
    }

    private func animate(_ changes: @escaping () -> Void, completion: (() -> Void)? = nil) {
        UIView.animate(withDuration: UIAccessibility.isReduceMotionEnabled ? 0 : 0.22,
                       delay: 0, options: [.beginFromCurrentState, .allowUserInteraction], animations: changes) { _ in completion?() }
    }

#if DEBUG
    /// Synthetic production-path checks; gesture delivery still needs fingers.
    func developmentCheck(capture: @escaping (String) -> Void, completion: @escaping ([String]) -> Void) {
        let original = layout
        let saved = UserDefaults.standard.data(forKey: preferenceKey)
        var issues: [String] = []
        guard customizationEnabled, visible.count >= 2,
              let first = visible.first, let button = buttons[first], let neighbor = buttons[visible[1]] else {
            completion(["Need two visible quick tools"]); return
        }
        func pause(_ next: @escaping () -> Void) { DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: next) }
        func check(_ condition: Bool, _ message: String) { if !condition { issues.append(message) } }
        func menuContains(_ menu: UIMenu, title: String) -> Bool {
            menu.children.contains { child in
                if let submenu = child as? UIMenu { return menuContains(submenu, title: title) }
                return child.title == title
            }
        }
        container.layoutIfNeeded(); capture("before")
        beginDrag(button, at: button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: container))
        check(held == first && !scroll.isScrollEnabled && button.alpha < 1, "Hold did not lift/cancel the control")
        pause {
            capture("lifted")
            self.contact = neighbor.convert(CGPoint(x: neighbor.bounds.midX, y: neighbor.bounds.midY + 4), to: self.container)
            self.updateDrag()
            pause {
                check(self.visible.first != first, "Dragging down did not reorder")
                capture("reordered")
                self.finishDrag(cancelled: false)
                pause {
                    check(self.scroll.isScrollEnabled == self.scrollWasEnabled && button.alpha == 1, "Drop left the bar locked or dimmed")
                    self.beginDrag(button, at: button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: self.container))
                    self.contact = CGPoint(x: self.removal.frame.midX, y: self.removal.frame.midY)
                    self.updateDrag()
                    pause {
                        capture("removing")
                        self.finishDrag(cancelled: false)
                        pause {
                            check(!self.layout.contains(first) && button.superview == nil, "Removal did not leave quick access")
                            check(menuContains(self.overflowMenu(), title: button.accessibilityLabel ?? ""), "Removed tool missing from overflow")
                            capture("removed")
                            self.add(first)
                            pause {
                                check(self.layout.contains(first) && button.superview === self.stack, "Adding did not restore quick access")
                                check(!menuContains(self.overflowMenu(), title: button.accessibilityLabel ?? ""), "Quick tool duplicated in overflow")
                                capture("added")
                                let beforeCancel = self.layout
                                self.beginDrag(button, at: button.convert(CGPoint(x: button.bounds.midX, y: button.bounds.midY), to: self.container))
                                self.cancelDrag()
                                pause {
                                    check(self.layout == beforeCancel && self.lifted == nil && button.alpha == 1, "Cancellation lost placement or cleanup")
                                    self.layout = original
                                    UserDefaults.standard.set(saved, forKey: self.preferenceKey)
                                    self.applyOrder(); self.onLayout?(); self.onChange?()
                                    completion(issues)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
#endif
}
