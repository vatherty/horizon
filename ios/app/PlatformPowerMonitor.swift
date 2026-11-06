// This monitor belongs to the process, including the game phase where winit
// owns UIApplicationMain. LauncherAppDelegate only runs for the importer.
import Foundation
import UIKit

final class PlatformPowerMonitor {
    static let shared = PlatformPowerMonitor()
    private var observers: [NSObjectProtocol] = []

    private init() {}

    func start() {
        guard observers.isEmpty else { return }
        sample()
        let notifications: [Notification.Name] = [
            ProcessInfo.thermalStateDidChangeNotification,
            .NSProcessInfoPowerStateDidChange,
            UIApplication.didBecomeActiveNotification,
        ]
        for name in notifications {
            observers.append(NotificationCenter.default.addObserver(
                forName: name, object: nil, queue: .main
            ) { [weak self] _ in
                self?.sample()
            })
        }
    }

    private func sample() {
        // A stable bridge contract, not the Android thermal-status numbering
        // and not a dependence on Apple's enum raw values.
        let state: Int32
        switch ProcessInfo.processInfo.thermalState {
        case .nominal: state = 0
        case .fair: state = 1
        case .serious: state = 2
        case .critical: state = 3
        @unknown default: state = 3
        }
        horizon_ios_set_thermal_state(state)
        horizon_ios_set_low_power_mode(ProcessInfo.processInfo.isLowPowerModeEnabled ? 1 : 0)
    }
}
