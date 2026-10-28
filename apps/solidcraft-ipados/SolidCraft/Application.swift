import UIKit

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        configurationForConnecting connectingSceneSession: UISceneSession,
        options: UIScene.ConnectionOptions
    ) -> UISceneConfiguration {
        UISceneConfiguration(name: "Default", sessionRole: connectingSceneSession.role)
    }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    private var controller: FoundationController?

    func scene(
        _ scene: UIScene,
        willConnectTo session: UISceneSession,
        options connectionOptions: UIScene.ConnectionOptions
    ) {
        guard let windowScene = scene as? UIWindowScene else { return }
        let controller = FoundationController()
        let window = UIWindow(windowScene: windowScene)
        window.frame = windowScene.coordinateSpace.bounds
        window.rootViewController = controller
        self.controller = controller
        self.window = window
        window.makeKeyAndVisible()
    }

    func windowScene(_ windowScene: UIWindowScene, didUpdate previousCoordinateSpace: UICoordinateSpace,
                     interfaceOrientation previousInterfaceOrientation: UIInterfaceOrientation,
                     traitCollection previousTraitCollection: UITraitCollection) {
        // Resizable iPad scenes can change their coordinate space independently of the
        // screen. Keep the client window in that space before recalculating the canvas.
        window?.frame = windowScene.coordinateSpace.bounds
        controller?.view.setNeedsLayout()
    }

    func sceneDidBecomeActive(_ scene: UIScene) {
        controller?.activate()
    }

    func sceneWillResignActive(_ scene: UIScene) {
        controller?.suspend()
    }

    func sceneDidEnterBackground(_ scene: UIScene) {
        controller?.suspend()
    }

    func sceneDidDisconnect(_ scene: UIScene) {
        controller?.shutdown()
        controller = nil
        window = nil
    }
}

final class FoundationController: UIViewController {
    private let canvas = MetalCanvas()
    private let status = UILabel()
    private let session = NativeSession()
    private var active = false
    private var generation: UInt64 = 0
    private var failure: String?
    private var pending = ""
    private var tallPost = false
    private var developmentCheckStarted = false
    private var reportedCanvasSize: CGSize = .zero

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        status.textColor = .label
        status.font = .preferredFont(forTextStyle: .caption1)
        status.adjustsFontForContentSizeCategory = true
        status.numberOfLines = 2
        status.text = NativeText.value("ui.ipados.foundation_loading")
        status.accessibilityIdentifier = "viewport-status"
        canvas.accessibilityIdentifier = "native-cad-surface"
        let controls = toolbar()
        for child in [controls, status, canvas] {
            child.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(child)
        }
        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            controls.topAnchor.constraint(equalTo: safe.topAnchor, constant: 4),
            controls.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 8),
            controls.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -8),
            status.topAnchor.constraint(equalTo: controls.bottomAnchor, constant: 4),
            status.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 12),
            status.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -12),
            status.heightAnchor.constraint(equalToConstant: 42),
            canvas.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 4),
            canvas.leadingAnchor.constraint(equalTo: safe.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
            canvas.bottomAnchor.constraint(equalTo: safe.bottomAnchor)
        ])
        canvas.onResize = { [weak self] in self?.updateSurface() }
        canvas.onInput = { [weak self] sample in self?.session.record(sample) }
        session.onFailure = { [weak self] error in
            self?.failure = error.description
            self?.status.text = NativeText.value("ui.ipados.foundation_error") + "\n" + error.description
        }
        session.onStatus = { [weak self] message in
            self?.pending = message
            self?.failure = nil
            if !message.isEmpty { self?.status.text = message }
        }
        session.onFrame = { [weak self] presented, revision, diagnostic in
            guard let self, self.active else { return }
            // The stamp names the geometry actually presented, not the latest worker result.
            if presented { self.canvas.geometryRevision = revision }
            #if DEBUG
            if presented, !self.developmentCheckStarted, ProcessInfo.processInfo.arguments.contains("--viewport-check") {
                self.developmentCheckStarted = true
                self.session.developmentCheck()
            }
            #endif
            guard self.failure == nil else { return }
            self.status.text = (self.pending.isEmpty ? NativeText.value("ui.ipados.viewport_help") : self.pending) + "\n" + diagnostic
        }
        let expected = generation
        session.start { [weak self] result in
            guard let self, self.generation == expected else { return }
            switch result {
            case .success: self.failure = nil; self.updateSurface()
            case .failure(let error): self.failure = error.description; self.status.text = error.description
            }
        }
    }

    private func button(_ key: String, action: @escaping () -> Void) -> UIButton {
        let button = UIButton(type: .system)
        button.setTitle(NativeText.value("ui.ipados." + key), for: .normal)
        button.addAction(UIAction { _ in action() }, for: .touchUpInside)
        button.accessibilityIdentifier = "viewport-" + key
        return button
    }

    /// Controls occupy their own native hierarchy above the canvas. UIKit consumes those
    /// touches before MetalCanvas receives them; no toolbar coordinate is forwarded to CAD.
    private func toolbar() -> UIStackView {
        let fit = button("fit") { [weak self] in self?.session.command("view.fit") }
        let views = button("views") {}
        views.showsMenuAsPrimaryAction = true
        views.menu = UIMenu(children: ["front", "back", "left", "right", "top", "bottom", "isometric"].map { name in
            UIAction(title: NativeText.value("ui.ipados." + name)) { [weak self] _ in self?.session.command("view." + name) }
        })
        let perspective = UISwitch()
        perspective.accessibilityLabel = NativeText.value("ui.ipados.perspective")
        perspective.addAction(UIAction { [weak self, weak perspective] _ in
            self?.session.command("view.perspective", params: ["enabled": perspective?.isOn == true])
        }, for: .valueChanged)
        let projectionLabel = UILabel()
        projectionLabel.text = NativeText.value("ui.ipados.perspective")
        let filters = UISegmentedControl(items: ["auto", "body", "face", "edge", "vertex"].map { NativeText.value("ui.ipados." + $0) })
        filters.selectedSegmentIndex = 2
        filters.accessibilityIdentifier = "viewport-selection-filter"
        filters.addAction(UIAction { [weak self, weak filters] _ in
            let values = ["auto", "body", "face", "edge", "vertex"]
            let index = filters?.selectedSegmentIndex ?? 2
            if values.indices.contains(index) { self?.session.command("select.filter", params: ["filter": values[index]]) }
        }, for: .valueChanged)
        let multiple = UISwitch()
        multiple.accessibilityLabel = NativeText.value("ui.ipados.multiple")
        multiple.addAction(UIAction { [weak self, weak multiple] _ in
            self?.session.command("select.multiple", params: ["enabled": multiple?.isOn == true])
        }, for: .valueChanged)
        let multipleLabel = UILabel()
        multipleLabel.text = NativeText.value("ui.ipados.multiple")
        let clear = button("clear") { [weak self] in self?.session.command("select.clear") }
        let change = button("change") { [weak self] in
            guard let self else { return }
            self.tallPost.toggle()
            self.session.command("timeline.edit", params: ["feature": "Viewport post", "set": ["height": self.tallPost ? "40" : "25"]])
        }
        let undo = button("undo") { [weak self] in self?.session.command("edit.undo") }
        let redo = button("redo") { [weak self] in self?.session.command("edit.redo") }
        let stack = UIStackView(arrangedSubviews: [
            scrollingRow([fit, views, projectionLabel, perspective, change, undo, redo]),
            scrollingRow([filters, multipleLabel, multiple, clear])
        ])
        stack.axis = .vertical
        stack.spacing = 4
        return stack
    }

    private func scrollingRow(_ children: [UIView]) -> UIView {
        let row = UIStackView(arrangedSubviews: children)
        row.axis = .horizontal; row.spacing = 12; row.alignment = .center
        let scroll = UIScrollView()
        scroll.showsHorizontalScrollIndicator = false
        scroll.addSubview(row)
        row.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            row.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            row.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            row.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            row.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor),
            scroll.heightAnchor.constraint(equalToConstant: 36)
        ])
        return scroll
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        canvas.updateDrawableSize()
        #if DEBUG
        if ProcessInfo.processInfo.arguments.contains("--viewport-check"), reportedCanvasSize != canvas.bounds.size {
            reportedCanvasSize = canvas.bounds.size
            NSLog("SolidCraft viewport layout: canvas=%@ window=%@ scene=%@", String(describing: canvas.bounds.size),
                  String(describing: view.window?.bounds ?? .zero), String(describing: view.window?.windowScene?.coordinateSpace.bounds ?? .zero))
        }
        #endif
    }
    func activate() { loadViewIfNeeded(); active = true; session.activate(); updateSurface() }
    func suspend() { guard active else { return }; active = false; session.suspend() }
    func shutdown() { generation &+= 1; active = false; session.stop() }
    private func updateSurface() {
        guard active, let layer = canvas.metalLayer, let size = canvas.pixelSize else { return }
        session.present(layer: layer, width: size.width, height: size.height, scale: Double(canvas.contentScaleFactor))
    }
}
