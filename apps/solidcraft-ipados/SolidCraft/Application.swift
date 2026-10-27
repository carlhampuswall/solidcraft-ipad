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
        window.rootViewController = controller
        self.controller = controller
        self.window = window
        window.makeKeyAndVisible()
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
    private var diagnostic = ""

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .systemBackground
        status.textColor = .label
        status.font = .preferredFont(forTextStyle: .caption1)
        status.adjustsFontForContentSizeCategory = true
        status.numberOfLines = 5
        status.text = NativeText.value("ui.ipados.foundation_loading")
        status.accessibilityIdentifier = "foundation-status"
        canvas.accessibilityIdentifier = "native-cad-surface"
        for child in [status, canvas] {
            child.translatesAutoresizingMaskIntoConstraints = false
            view.addSubview(child)
        }
        let safe = view.safeAreaLayoutGuide
        NSLayoutConstraint.activate([
            status.topAnchor.constraint(equalTo: safe.topAnchor, constant: 8),
            status.leadingAnchor.constraint(equalTo: safe.leadingAnchor, constant: 12),
            status.trailingAnchor.constraint(equalTo: safe.trailingAnchor, constant: -12),
            canvas.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 8),
            canvas.leadingAnchor.constraint(equalTo: safe.leadingAnchor),
            canvas.trailingAnchor.constraint(equalTo: safe.trailingAnchor),
            canvas.bottomAnchor.constraint(equalTo: safe.bottomAnchor)
        ])
        canvas.onResize = { [weak self] in self?.updateSurface() }
        canvas.onInput = { [weak self] sample in self?.session.record(sample) }
        session.onFailure = { [weak self] error in self?.show(error) }
        session.onFrame = { [weak self] presented in
            guard let self, self.active else { return }
            let key = presented ? "ui.ipados.foundation_ready" : "ui.ipados.foundation_loading"
            self.status.text = NativeText.value(key) + "\n" + self.diagnostic
        }
        let expected = generation
        session.start { [weak self] result in
            guard let self, self.generation == expected else { return }
            switch result {
            case .success(let diagnostic):
                self.diagnostic = diagnostic
            case .failure(let error): self.show(error)
            }
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        canvas.updateDrawableSize()
    }

    func activate() {
        loadViewIfNeeded()
        active = true
        session.activate()
        updateSurface()
    }

    func suspend() {
        guard active else { return }
        active = false
        // Returns only after the render-only critical section has drained and closed.
        // It never waits for the startup CAD evaluation.
        session.suspend()
    }

    func shutdown() {
        generation &+= 1
        active = false
        session.stop()
    }

    private func updateSurface() {
        guard active, let layer = canvas.metalLayer, let size = canvas.pixelSize else { return }
        session.present(layer: layer, width: size.width, height: size.height)
    }

    private func show(_ error: BridgeError) {
        status.text = NativeText.value("ui.ipados.foundation_error") + "\n" + error.description
    }

}
