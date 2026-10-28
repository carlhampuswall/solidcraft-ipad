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
