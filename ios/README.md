# Horizon for iOS

An iOS shell around the Horizon engine: a Rust static library the Xcode app
links, a thin Swift app that starts it, and a GitHub Actions workflow that
builds an **unsigned** `.ipa` on a macOS runner. Install it with
SideStore/AltStore, which signs it on install with a free Apple ID (7-day
cert, same weekly chore as Madeira).

Status: shell + workflow only. Nothing here has run on a device yet — the
first iPhone run is the real test, exactly like the Android port it mirrors
(see `IOS_PORT_PLAN.md` in `~/workspace/horizon-ios-scope/`; that plan is the
integrator's map for the mobile layer in shared code, and this README points
at its sections).

## Layout

| Path | What |
|---|---|
| `ios/Cargo.toml`, `ios/src/lib.rs`, `ios/src/os_log.rs` | The `horizon-ios` crate: a `staticlib` shell (own workspace, `publish = false`). Entry point `horizon_ios_main(Documents path) -> exit code`. |
| `ios/build-rust.sh` | Builds the staticlib (`cargo build --release --target aarch64-apple-ios`); seeds `ios/Cargo.lock` from the desktop lock. Run by the Xcode pre-build phase and by CI. |
| `ios/build-ipa.sh` | One-command local build: staticlib → `xcodegen` → `xcodebuild` → unsigned `dist/horizon-ios-unsigned.ipa`. |
| `ios/app/main.swift` | The process entry point (no `@main` AppDelegate — see below). |
| `ios/app/LauncherAppDelegate.swift`, `ios/app/LauncherViewController.swift` | The launcher phase: resource status, last-run error, import buttons, share-the-log. Mirrors Android's `LauncherActivity`. |
| `ios/app/ResourceImporter.swift`, `ios/app/ZipExtractor.swift` | Import: document picker → staging → `resources/`; zip via zlib raw-deflate, folder via recursive copy. |
| `ios/app/Strings.swift` | Every user-facing string in the Swift shell, in one place (no hardcoded strings scattered in code). |
| `ios/app/Horizon-Bridging-Header.h` | The C surface Swift sees (`horizon_ios_main`, zlib). |
| `ios/app/Info.plist`, `ios/app/LaunchScreen.storyboard` | Bundle id `dev.horizon.ios` (placeholder), no scene manifest (deliberate), Files sharing on. |
| `ios/project.yml` | XcodeGen project: unsigned (`CODE_SIGNING_ALLOWED=NO`), iOS 17.0 deployment target, pre-build script builds the staticlib, `-lhorizon_ios`. |
| `.github/workflows/ios.yml` | CI: tree guard → `cargo check`/`clippy` for `aarch64-apple-ios` → `build-ipa.sh` → engine-only `.ipa` check → artifact (14 days) → `ios-latest` pre-release. |

## The Rust entry point

The shell calls **`horizon_client::game::run_with()`** — not `entry::main()`
and not a new `run_game()` addition. `run_with()` is public
(`crates/horizon-client/src/game.rs:22186`, `pub mod game` in `lib.rs:8`) and
is the direct successor of the fork's `game::run()`; the fork's `run_game()`
wrapper is unnecessary on current upstream, whose `lib.rs` already exposes
`game` unconditionally (`IOS_PORT_PLAN.md` §4.1). **No `lib.rs` change is
needed for the loop itself.**

Why not `entry::main()`: it never returns on error — it calls
`std::process::exit` — which would kill the process before the Swift launcher
could read `last-error.txt` and show it. The shell therefore does the setup
`entry::main()` would have done (game home, logging, crash handler, allocator)
and lets `run_with()` return its error, exactly as the Android shell does
with `run_game()`.

`run_with()` reads `std::env::args()` (`Options::read`); on iOS argv is just
the executable path, so options come out as defaults. The shell sets
`HORIZON_HOME` to the Documents folder, `HOME` to the app container,
`TMPDIR` to `Documents/tmp`, `RUST_BACKTRACE=1`, and the working directory to
Documents before calling it. It also reads `horizon-env.txt` (`NAME=value` per
line, `#` comments — same file as Android) and honours `--resources DIR` from
`horizon-args.txt` in its own preflight (the game itself only honours that
file once the mobile layer's `launch_args()` cfg covers iOS; see
"Integrator").

Exit codes from `horizon_ios_main`: `0` ended, `1` error (message in
`Documents/last-error.txt`), `2` resources missing (the launcher takes over).

## Two phases, one process (read this before changing `main.swift`)

Winit's iOS backend, vendored at
`~/.cargo/registry/src/*/winit-0.30.13/src/platform_impl/ios/`:

- `event_loop.rs`, `EventLoop::new()`: **must be created on the main thread**
  (`MainThreadMarker::new().expect("On iOS, \`EventLoop\` must be created on
  the main thread")`), and only one may ever exist (singleton assert).
- `event_loop.rs`, `EventLoop::run()` (which `run_app` delegates to):
  **calls `UIApplicationMain` itself** (principal and delegate `None`) and
  **asserts `sharedApplication` is still `nil`** — "`EventLoop` cannot be
  `run` after a call to `UIApplicationMain` on iOS".
- `src/platform/ios.rs` (winit's own docs): "The simplest way to integrate
  your app into xcode environment is to build it as a static library. Wrap
  your main function and export it… Use start_winit_app inside your xcode's
  main function." And: "Winit guarantees that it will not register an
  application delegate, so you can set up a custom one in a nib file instead."
- Lifecycle reaches the game through `NSNotificationCenter` observers in
  `EventLoop::new()`: `applicationDidBecomeActive` → `Resumed`,
  `applicationWillResignActive` → `Suspended`, memory warning →
  `MemoryWarning`. Touch reaches it through the view (`view.rs`,
  `touchesBegan/Moved/Ended/Cancelled` → `WindowEvent::Touch`); finger-id
  stability and physical-pixel positions are still device-unverified.
- Winit creates its own `UIWindow` and view controller when the game creates
  its window in `resumed`. **The Swift side needs no view controller for the
  game.**

Consequences, all deliberate:

1. There is **no `@main` AppDelegate**. `main.swift` is the entry point and
   hands the main thread straight to `horizon_ios_main()`. A `@main`
   delegate would call `UIApplicationMain` first, and winit would then hit
   its assert and panic.
2. The document-picker import cannot run before the game in the same
   process: once `UIApplicationMain` has run, winit can never run. So each
   launch is one phase or the other. `main.swift` calls the Rust entry; if
   it returns `2` (resources missing) or another error, `main.swift` starts
   UIKit with the Swift launcher, which offers the import and shows
   `last-error.txt`. After a successful import the player **relaunches the
   app** (closes it, opens it again) — on that launch the resources are
   there and the game starts directly. The launcher's Start button only
   shows the "relaunch to play" note. This is the v1 limitation; the
   follow-up is presenting the picker from inside the game's own winit
   window (needs the mobile layer's touch/surface work first).
3. There is **no scene manifest** in `Info.plist`, so the launcher phase
   uses the pre-iOS-13 window path (`LauncherAppDelegate` owns the window).

## The resource-zip contract

Same as Android (`ANDROID.md` "Resources", `IOS_PORT_PLAN.md` §6): the
prepared folder is the one that holds **`horizon-resources.json`** at its top
level (output of `horizon-prepare` or the launcher's Prepare button, from the
player's own Minecraft Java Edition 26.3 install). The zip is a zip of that
folder's contents (`tar -a -c -f horizon-resources.zip -C <folder> .`), so
the manifest is at the zip root — or a few levels down, which the importer
searches for (depth ≤ 4 for zips, ≤ 3 for folders, matching Android).

The import unpacks to staging `Documents/resources.importing`, requires the
manifest, then replaces `Documents/resources/` and deletes staging — so a
failed import never leaves a half-copied `resources/` behind. The zip itself
is streamed from its security-scoped URL while access is held (not copied out
first — a 1–2 GB double-copy is wasteful); the folder import copies the
manifest's folder, not the whole picked tree. Zip entries that would escape
the game folder (absolute paths, `..`) are rejected. The extractor is a
minimal streaming reader (`ZipExtractor.swift`): stored/deflate only, zip64
end-of-central-directory records, no encryption, no multi-disk archives.

Layout the game expects under the game home (`Documents/`):

| Path | What |
|---|---|
| `resources/` | the prepared resources (`resources/horizon-resources.json`) |
| `.local/horizon-client/26.3` | **symlink → `../../resources`** (see below) |
| `saves/` | worlds |
| `options.txt`, `horizon-graphics.txt`, `horizon-env.txt`, `horizon-args.txt`, `horizon-touch.txt` | settings / switches, as on desktop |
| `logs/latest.log`, `crash-reports/` | run log and crash reports |
| `last-error.txt`, `running.marker` | what the launcher screen reads |

**The symlink bridge.** Upstream's `Options::parse` only looks for resources
beside the executable and at `.local/horizon-client/<version>`; the fork's
branch that defaults them to `resources/` is `cfg(target_os = "android")`
and does not cover iOS yet. Until the integrator extends it, both the Rust
entry and the Swift importer link `.local/horizon-client/26.3` to
`resources/`, so the unmodified game finds an import today. A real prepared
folder in `.local` wins over the link and is left alone. Remove the bridge
when the cfg becomes `android | ios`.

**Game home is `Documents/`**, not Application Support: the player can see
(and rescue) saves and logs in the Files app, and `UIFileSharingEnabled` /
`LSSupportsOpeningDocumentsInPlace` are on for the same reason. Trade-off
(`IOS_PORT_PLAN.md` §8.5): worlds are backed up with the app; that is
accepted for v1.

## Build

On a Mac (Xcode + `brew install xcodegen` + Rust with the
`aarch64-apple-ios` target):

```sh
ios/build-ipa.sh            # → dist/horizon-ios-unsigned.ipa
```

CI does the same thing (`.github/workflows/ios.yml`, branch `ios-port` +
`workflow_dispatch`): `cargo check` and `clippy -D warnings` for the iOS
target, `xcodegen generate`, `xcodebuild -sdk iphoneos
CODE_SIGNING_ALLOWED=NO`, `ditto` packaging, an engine-only content check,
artifact upload (14 days), and the `ios-latest` pre-release as the fixed
download link. No signing anywhere, no secrets, no Apple credentials.

Install: download the `.ipa` (artifact or `ios-latest`), install with
SideStore/AltStore (signs with a free Apple ID, 7-day cert). Then, in the
app, import the prepared-resources zip (prepared on the Deck — launcher
Prepare, then zip the prepared folder and send it to the phone), close the
app, reopen it.

## Integrator — what this shell does NOT do

Everything below lives outside `ios/` (this shell's scope) and must land
before, or with, the first device run. Pointers are to `IOS_PORT_PLAN.md`.

1. **Root `Cargo.toml`**: add `"ios"` to `[workspace] exclude` (this crate
   has its own workspace, like `android/native` and the launcher; the shell
   sits at `ios/`, not `ios/native/` as the plan sketched, so the entry is
   `"ios"`). Until then the directory is simply not a member of that
   workspace.
2. **Mobile layer** (`IOS_PORT_PLAN.md` §2–§3, §9): copy the fork's mobile
   modules (`touch.rs`, `game/touch.rs`, `mobile.rs`, `platform.rs`,
   `game/surface.rs`, `winit_host.rs`) into their new crate homes and extend
   every `android` cfg to `android | ios`. Without this, the game builds and
   runs but has **no touch controls, no surface suspend/resume, no phone
   defaults, and no `platform` bridge**. Most urgent for this shell:
   - `game.rs` `Options::parse`: the resources default branch
     (`PathBuf::from("resources")`) — without it, only the symlink bridge
     (§ above) makes an import findable.
   - `game.rs` `launch_args()`: read `horizon-args.txt` on iOS too.
   - `logging.rs` + `save.rs`: the lock-tolerance edits (iOS's sandbox
     supports file locks, so these may never trigger — keep for parity).
   - `crash.rs`: port `mark_main_thread()`; this shell does not call it
     because upstream has no such function, and on iOS the game thread *is*
     the main thread (winit requires it), so the hook's `thread name ==
     "main"` check already covers it.
   - `horizon-client-input`: gate `arboard` out for `android | ios` — it
     pulls `wayland-backend`, which does not compile for iOS. This blocks
     `cargo check` for the iOS target until done (alongside `ring`, below).
   - **No `run_game()` addition is needed** (see "The Rust entry point").
   - **No `mods` feature exists upstream** (§7.4): this crate deliberately
     has no `mods` feature (the fork's wasmtime world is gone; code mods
     are native now). Content-only mods (`.hmod`) should work through the
     content path; native code mods on iOS are undecided (§8.6) and not
     promised here.
3. **Log sink**: `IOS_PORT_PLAN.md` §5.2 assumes `platform::set_log_sink`
   exists; upstream it does not yet. This shell's `os_log.rs` writes the
   *shell's own* lifecycle lines to `os_log` (Console.app) via a direct
   `os_log_create` / `os_log_with_type` FFI — the game's lines go to
   `logs/latest.log` only. When `platform.rs` + the logging sink forwarding
   land, the shell should pass its os_log writer to
   `platform::set_log_sink` instead.
4. **`tools/check_ipa.py` + `tests/test_ipa.py`** (plan §5.6): the engine-only
   `.ipa` check currently lives inline in `ios.yml` (same forbidden list as
   `check_android_apk.py`, adapted to `.ipa` layout), because `tools/` is
   outside this shell's scope. If a checker of its own is written, the
   workflow step should call it instead.
5. **Swift feeders for `platform.rs`** (plan §5.5, once it exists):
   `ProcessInfo.thermalState` → thermal status, and an exit export on
   terminate → `platform::request_exit()`. Not wired in v1; the game has no
   thermal cap and no graceful-exit path from Swift yet.

## Not verified on this VM (Linux, no iOS SDK, no Swift)

- Nothing here compiled: `cargo check` for `aarch64-apple-ios` stops at
  `ring`'s build script (needs Apple's `xcrun`/iOS SDK — environmental, not
  a code problem; CI's macOS runner has it). The Rust files pass `rustfmt`
  only; the Swift files, `project.yml`, and the workflow have had no
  compiler or `xcodegen` run at all. The first CI run is the first real
  check — expect at least one round of fixes (linker frameworks in
  `project.yml`'s `OTHER_LDFLAGS` are the likeliest gap: the list names
  Metal/QuartzCore/CoreGraphics/IOSurface/CoreVideo/AudioToolbox/AVFoundation;
  the link error will name anything still missing).
- `mimalloc` as the global allocator (`#[global_allocator]` in `lib.rs`,
  mirroring `src/main.rs`): expected to build on the Mac runner; if it does
  not, `--features system-allocator` is the escape hatch (and note the game
  then runs on the system heap, which is also fine).
- `cpal` audio on iOS (CoreAudio backend): unverified; `--no-default-features`
  is the escape hatch if it fails to build or link.
- The `ZipExtractor` (zlib raw-deflate via the bridging header) has never
  run; the folder import is the fallback if it misbehaves on a real zip.
- No device run: Metal on A15, touch delivery, suspend/resume, safe areas
  (Dynamic Island / home indicator — the launcher and HUD do not avoid them
  yet), multiplayer/online sign-in (v1 scope is single-player + LAN;
  `IOS_PORT_PLAN.md` §8.11). The phone defaults (`mobile::seed`: render
  distance 6, battery mode, …) are not applied until the mobile layer
  lands, so a first launch runs desktop defaults — heavy on a phone; lower
  the render distance in settings if it struggles.
