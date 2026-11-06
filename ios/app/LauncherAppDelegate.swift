// LauncherAppDelegate.swift
//
// The launcher phase's app delegate. This class is NOT `@main`: `main.swift`
// starts UIKit with it only on a launch where the game did not start, and
// passes its name to `UIApplicationMain` there.
//
// There is deliberately no scene manifest in Info.plist, so the pre-iOS-13
// window path below is the one that runs: the delegate owns the window, and
// winit (which creates its own window when the game runs) is not involved on
// this launch at all.
//
// The launcher sits in a navigation controller so its log screen
// (LogViewController) can be pushed on top; the bar is dark to match the
// launcher's own background.

import UIKit

final class LauncherAppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        let window = UIWindow(frame: UIScreen.main.bounds)
        let navigation = UINavigationController(rootViewController: LauncherViewController())
        navigation.navigationBar.barStyle = .black
        window.rootViewController = navigation
        window.makeKeyAndVisible()
        self.window = window
        return true
    }
}
