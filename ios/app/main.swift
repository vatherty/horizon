// main.swift
//
// The app's entry point. This file is the `main` — there is deliberately no
// `@main` AppDelegate.
//
// Why: winit's iOS backend calls `UIApplicationMain` itself when the game's
// event loop runs, and refuses to run after it has already been called
// (winit-0.30.13 `src/platform_impl/ios/event_loop.rs`, `EventLoop::run`:
// "`EventLoop` cannot be `run` after a call to `UIApplicationMain`"). So the
// game phase must be the one that starts UIKit, and this file hands the main
// thread straight to Rust. The Swift launcher phase below only runs on a launch
// where the game did not start — the resources are missing, or the game stopped
// with an error — and on that launch winit never runs, so starting UIKit here
// is safe. One process, one `UIApplicationMain`, whichever phase owns it.
// (ios/README.md, "Two phases, one process".)

import Foundation
import UIKit

// The game folder: the app's Documents directory. The Rust side takes it as a
// C string rather than asking the OS itself, so there is exactly one place
// that decides where the game lives.
let documentsURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!

let exitCode = documentsURL.path.withCString { horizon_ios_main($0) }

if exitCode != 0 {
    // 2: the prepared resources are not on the phone — the launcher offers the
    // import. Anything else: the game stopped with an error, and the launcher
    // shows Documents/last-error.txt. Either way the game did not start, so
    // this launch belongs to UIKit now.
    _ = UIApplicationMain(
        CommandLine.argc,
        CommandLine.unsafeArgv,
        nil,
        NSStringFromClass(LauncherAppDelegate.self)
    )
}
// exitCode == 0: the game ended. Nothing more to do; the process ends here.
// (On iOS the event loop never returns in practice, so this is near-unreachable.)
