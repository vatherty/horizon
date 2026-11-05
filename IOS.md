# Horizon on iOS

Horizon runs natively on iOS (ARM64, Metal), aimed at the iPhone 13 Pro: the same
game loop as the desktop client, with touch controls and lighter defaults for a
phone. Install it with SideStore or AltStore, which signs the unsigned `.ipa` on
install with a free Apple ID (a 7-day certificate, refreshed weekly).

> **Read this first: it has not been run on a device.**
> The Rust and Swift build: GitHub Actions compiles the game for `aarch64-apple-ios`, assembles the
> unsigned `.ipa`, and checks it holds only the engine (see [Build the .ipa on GitHub](#build-the-ipa-on-github-no-local-setup)). Nothing past that has been seen working: no Metal
> device, no touch input, no lifecycle. Expect runtime surprises on the phone. [IOS_TESTING.md](IOS_TESTING.md) is a checklist ordered from "the app launches" to
> "walk around a world", so you can report exactly which step fails. The desktop builds do not change (see [What changed in shared code](#what-changed-in-shared-code)).

Horizon contains no Minecraft files. They are prepared on a computer from your own Minecraft Java
Edition with `horizon-prepare` (or the Horizon launcher's Prepare button), and imported on the phone; see [Resources](#resources).

## Quick start (no cables, no command line)

You need a computer once, to prepare the game files from your own Minecraft. The phone cannot do that step.
(The Steam Deck works: prepare with the launcher there, or `horizon-prepare --prepare`.)

1. **Install the app.** Download the `.ipa` from the repository's *Releases* page, the "Horizon for iOS (latest build)"
   pre-release (`horizon-ios-unsigned.ipa`), or from the *Actions* tab, the newest green run, *Artifacts*,
   `horizon-ios-unsigned-<sha>` (a zip to unzip). Install it with SideStore or AltStore, which signs it with a
   free Apple ID. (SideStore itself is installed with SideInstaller, from sideinstaller.net only.)
2. **Prepare on the computer.** Launch Minecraft 26.3 once from your launcher. Open the Horizon launcher,
   select the version and press **Prepare**. It needs a JDK 25 (Eclipse Temurin) and may ask for the
   26.3 server jar from minecraft.net. Wait for "Prepared for Minecraft 26.3"; a red message under the buttons says what is missing.
3. **Pack it.** Zip the *contents* of the prepared folder — the folder that holds `horizon-resources.json` —
   so the manifest is at the zip's root (on the Deck: `cd <prepared folder> && zip -r ~/horizon-resources.zip .`).
4. **Send it to the phone**: AirDrop, the Files app, a messaging app, whatever you like.
5. **Import.** Open Horizon on the phone. The first launch runs the game, which finds no resources and hands
   over to the launcher screen; tap **Import resources from a zip** and pick `horizon-resources.zip`. (Or
   **Import resources from a folder** if you sent the unzipped folder.) When it says "Imported", **close the
   app and open it again** — on that launch the resources are there and the game starts directly.

## Contents

- [Quick start](#quick-start-no-cables-no-command-line)
- [How it is built](#how-it-is-built) and why
- [Build](#build)
- [Device setup](#device-setup)
- [Resources](#resources)
- [Running](#running)
- [Controls](#controls)
- [Defaults for a phone](#defaults-for-a-phone)
- [Lifecycle: background, rotate](#lifecycle-background-rotate)
- [Logs and debugging](#logs-and-debugging)
- [Known issues and limits](#known-issues-and-limits)
- [What changed in shared code](#what-changed-in-shared-code)
- [Decisions](#decisions)
- [What next](#what-next)

## How it is built

```
ios/
  build-ipa.sh             the one build script -> dist/horizon-ios-unsigned.ipa
  build-rust.sh            cargo build --target aarch64-apple-ios (the static library)
  project.yml              the XcodeGen project (unsigned, iOS 17.0)
  Cargo.toml, src/lib.rs   crate horizon-ios (staticlib): horizon_ios_main, os_log sink
  src/os_log.rs            the shell's own log lines to os_log
  app/main.swift           the process entry point (no @main AppDelegate — winit owns UIApplicationMain)
  app/LauncherAppDelegate.swift, app/LauncherViewController.swift
                           the launcher phase: resource status, last-run error, import buttons, share-the-log
  app/ResourceImporter.swift, app/ZipExtractor.swift
                           import: document picker -> staging -> resources/
  app/Strings.swift        every user-facing string in the Swift shell, in one place
  app/Horizon-Bridging-Header.h
                           the C surface Swift sees (horizon_ios_main, zlib)
  app/Info.plist, app/LaunchScreen.storyboard
                           bundle id dev.horizon.ios (placeholder), no scene manifest, Files sharing on
crates/horizon-client/src/
  game.rs  game/*.rs       the desktop game loop, unchanged in behaviour; the library on iOS calls game::run_with()
  game/touch.rs            touch controls and the platform mouse in the game loop
  game/surface.rs          the window's surface coming and going
  touch.rs                 touch controls as pure logic (fingers in, actions out; unit tests)
  mobile.rs                first-run phone defaults
  winit_host.rs            the event loop per platform
crates/horizon-client-render/src/
  platform.rs              events from the platform side, requests to it, thermal state
```

The game itself is the desktop client's: `game.rs` and its modules are compiled into the `horizon-client`
library, and the iOS shell calls `horizon_client::game::run_with()` — the direct successor of the Android
port's `run_game()`. The `horizon-ios` static library is the thin layer around it: it sets up the game
folder, logging and crash reports, checks the resources, and starts that loop on the main thread, which
winit's iOS backend requires.

Two phases, one process: winit's iOS backend calls `UIApplicationMain` itself and refuses to run after
one has already run, so each launch is one phase or the other. `main.swift` hands the main thread straight
to `horizon_ios_main()`. If the game returns because the resources are missing (or with an error), `main.swift`
starts UIKit with the Swift launcher, which offers the import and shows `last-error.txt`. After a successful
import the player relaunches the app; on that launch the game starts directly. The launcher's Start button
only shows the "relaunch to play" note. This is the v1 limitation; the follow-up is presenting the picker
from inside the game's own winit window.

## Build

Build on a Mac (or in GitHub Actions); the `.ipa` is built for `arm64` only. There is no local Linux build:
`ring` and `mimalloc` need Apple's iOS SDK, which only a Mac has.

### Install once (Mac)

1. **Xcode** from the App Store, and **XcodeGen** (`brew install xcodegen`).
2. **Rust** with the iOS target:
   ```sh
   rustup target add aarch64-apple-ios
   ```

### Build the .ipa

```sh
ios/build-ipa.sh            # -> dist/horizon-ios-unsigned.ipa
```

The script builds the Rust static library (`ios/build-rust.sh`, which seeds `ios/Cargo.lock` from the desktop
`Cargo.lock` so every dependency has the version the desktop game is tested with), generates the Xcode project,
builds it unsigned (`CODE_SIGNING_ALLOWED=NO`), and packages `Payload/Horizon.app` into the `.ipa` with `ditto`.

The first build compiles the whole game for ARM64 and takes a while.

### Build the .ipa on GitHub (no local setup)

`.github/workflows/ios.yml` builds the `.ipa` in GitHub Actions on every push to the **`ios-port`** branch and on demand. It needs no secrets,
no repository variables and none of the upstream author's accounts, so it works on a fork. It runs `cargo check` and `clippy -D warnings`
for the iOS target, runs `ios/build-ipa.sh`, checks that the `.ipa` holds only the engine (`tools/check_ipa.py`: no assets, game files, prepared
resources or jars; the job fails otherwise), and uploads `horizon-ios-unsigned-<short sha>.ipa`
as an artifact kept for 14 days. It also maintains the `ios-latest` pre-release as the fixed download link.
The `.ipa` is unsigned, so SideStore/AltStore signs it on install with a free Apple ID.

On your fork:

1. Open the fork on GitHub, **Actions** tab, and press **I understand my workflows, go ahead and enable them** (forks have Actions off until you do).
2. Put the code on a branch named `ios-port` (`git push origin <your branch>:ios-port`). Pushing it starts the workflow.
3. To run it by hand: **Actions, ios, Run workflow**, pick the branch. GitHub lists a manually run workflow only if the workflow file is on the
   fork's **default branch**, so either make `ios-port` the default branch (Settings, Branches) or merge the workflow file into `main` of the fork.
4. When the run is green, open it and download the artifact (**horizon-ios-unsigned-&lt;sha&gt;**, at the bottom of the run page). On the phone, open the
   run in the browser (signed in to GitHub), download the artifact (it arrives as a `.zip`), unzip it with the Files app, and hand the `.ipa` to
   SideStore/AltStore. Or on a computer: `gh run download <run id> -n horizon-ios-unsigned-<sha>`.

The first run builds everything from scratch; later runs reuse the caches.

## Device setup

1. **SideStore** (or AltStore) on the phone, signed in with a free Apple ID. SideStore is installed with
   SideInstaller, from **sideinstaller.net only** (sideinstaller.com is not the real site). The free
   certificate lasts 7 days and SideStore refreshes it on-device; SideStore itself counts as one of the
   3 apps a free account may have installed at once.
2. Open the `.ipa` with SideStore/AltStore and install. iOS may ask about an app from an unknown developer:
   choose *Install anyway* (SideStore signs it with your Apple ID first).

No developer options, no cables, no `adb` — there is no iOS equivalent.

## Resources

The game needs the files `horizon-prepare` builds from **your own** Minecraft Java Edition 26.3 install (the same
step as on the desktop, see [Build from source](README.md#build-from-source) and
[the native client guide](docs/native-client.md)). Nothing is downloaded and nothing of Minecraft's is stored in this repository.

On the computer (the Steam Deck works):

```sh
# Prepare, then zip the prepared folder's contents (the manifest at the zip root).
horizon-prepare --prepare
horizon-prepare sounds   # optional, about 380 MB; without them the game is silent
cd <prepared folder> && zip -r ~/horizon-resources.zip .
```

The folder you zip must be the one that holds `horizon-resources.json` (the `.local/horizon-client/26.3` of a
prepared checkout, or the launcher's prepared folder). The launcher screen shows "Resources found" once
`resources/horizon-resources.json` is there. Without it the game stops at start with
a message saying what to copy where (on the launcher screen, and in `last-error.txt`).

The game folder is the app's `Documents/` (visible in the Files app, so saves and logs can be pulled out):

| Path | What |
| --- | --- |
| `resources/` | the prepared resources (you import these) |
| `.local/horizon-client/26.3` | a symlink to `resources/` (a bridge until the game defaulted to `resources/` itself; a real prepared folder there wins) |
| `saves/` | worlds (vanilla Anvil format; copy one out of Files to open it on the desktop) |
| `options.txt`, `horizon-graphics.txt`, `horizon-controller.txt`, ... | settings, as on the desktop |
| `logs/latest.log`, `crash-reports/` | the log of the run and crash reports |
| `last-error.txt`, `running.marker` | what the launcher screen reads |
| `horizon-args.txt` | optional command-line arguments, one or more per line (see below) |
| `horizon-touch.txt` | optional touch layout tuning (see below) |
| `horizon-env.txt` | optional environment variables for the game (see below) |

## Running

Open **Horizon** on the phone: with resources present the game starts straight into the loading screen, then the
title menu. Without them the launcher screen shows the state instead (see [Quick start](#quick-start-no-cables-no-command-line)).
The phone defaults below are **not** seeded yet: the shell does not call `mobile::seed` in v1, so a first launch runs
desktop defaults — lower the render distance in the options if it struggles.

Command-line arguments (the desktop options `--world`, `--creative`, `--render-distance N`, `--seed`, `--server HOST`...) go in
`horizon-args.txt` in the game folder, whitespace separated, `#` starting a comment line:

```
# straight into a creative world
--world Test --creative
```

Quit from the title screen by leaving the app (Home, then swipe it away), or use *Save and Quit to Title* in the pause menu and
close the app. Closing the app saves the world first (see [Lifecycle](#lifecycle-background-rotate)).

## Controls

### Touch (in the world)

| Control | Does |
| --- | --- |
| Left thumb, anywhere on the left 42% (below the top quarter) | **Floating stick**: the base appears where the thumb lands. Push the stick all the way forward to sprint. |
| Drag anywhere else | **Look** (the Mouse Sensitivity option scales it) |
| **Break** (bottom right) | Attack and mine; *hold to keep breaking* |
| **Place** | Place a block / use the item; hold to keep using (eating, bows) |
| **Jump** | Jump; double tap in creative to fly, as with Space |
| **Sneak** | Toggles sneaking (it lights while on) |
| **Menu** (top left) | Pause menu |
| **Chat** | Opens chat with the on-screen keyboard |
| **Inv** / **Drop** (top right) | Inventory / throw the item in hand |
| Tap a **hotbar** slot | Select it; **swipe** along the hotbar to step the selection |

### Touch (in menus, inventories, the title screen)

The first finger is the mouse (a tap is a click, a drag drags items and sliders). A toolbar runs down the left edge:

| Button | Does |
| --- | --- |
| **Back** | Escape (close the screen, go back a menu) |
| **Shift** | Toggle: holds Shift for your taps (quick-move stacks in inventories) |
| **Right** | Makes your *next* tap a right click (split a stack, place one item) |
| **Up** / **Down** | Scroll lists (hold to repeat) |
| **Keys** | Opens or closes the on-screen keyboard; it types into chat, signs, books, search boxes, world names and the options' fields |

There is no system back gesture on iOS; the toolbar's **Back** is the only Escape. Chat opens the on-screen keyboard by itself.

### Bluetooth keyboard

Untested: a paired keyboard may work through winit's iOS backend as on the desktop (the default key bindings;
text straight into fields). Report what happens if you try it.

### Debug switches without rebuilding

`horizon-env.txt` in the game folder sets environment variables: one `NAME=value` per line, `#` for comments,
read at start. The desktop game's switches work, for example:

```
HORIZON_SPIKES=25        # log the timed spans of every frame slower than 25 ms
HORIZON_NO_AUDIO=1       # no sound
WGPU_BACKEND=metal
HORIZON_SPRITE_SPLIT=64  # smaller block-texture arrays, if the GPU is short of layers
```

### Tuning the touch layout

`horizon-touch.txt` in the game folder overrides the layout (one `key:value` per line, values are clamped to sane ranges;
sizes in dp):

```
buttonSize:0.20     # a button's edge as a fraction of the screen's shorter side (default 0.16, kept between buttonMin and buttonMax)
buttonMin:48
buttonMax:72
margin:20
gap:10
stickRadius:64
stickZone:0.42      # how much of the screen's left a thumb can start the stick in
deadzone:0.14
sprintPush:0.97
lookGain:1.6        # look speed of a drag
```

The defaults are in `Layout::default()` in `crates/horizon-client/src/touch.rs`, where the arrangement (`arrange`) is plain code and has
unit tests for phone screens (held sideways and upright): nothing overlaps, nothing leaves the
screen, nothing covers the hotbar, and buttons stay between 48 and 72 dp.

## Defaults for a phone

On the first start (only where the file does not exist), `mobile::seed` writes lighter settings than the desktop's; anything you
change in the options is yours from then on. **Not wired in v1**: the shell does not call `mobile::seed` yet, so a first
launch runs desktop defaults until that one-line call lands in the shell.

| Setting | Desktop | iOS | Why |
| --- | --- | --- | --- |
| Render distance | 12 | **6** | mesh count and GPU memory grow with its square |
| Simulation distance | 12 | **5** (vanilla's minimum) | mob and block ticking |
| Max FPS | 120 | **60** | heat and battery |
| Mipmap levels | 4 | **2** | texture memory and bandwidth |
| Biome blend | 2 | **1** | meshing time |
| Particles | all | **decreased** | fill rate |
| Entity distance | 100% | **75%** | draw calls |
| Entity shadows | on | **off** | extra passes |
| Clouds | fancy | **fast** | fill rate |
| Menu background blur | 5 | **2** | a full-screen blur is costly on a mobile GPU |
| Auto jump | off | **on** | there is no key to hold for a jump |
| Battery mode (`horizon-graphics.txt`) | off | **on** | at most 60 FPS in a world and 30 in menus, half the chunk worker threads |
| GUI scale | auto | auto | the phone picks it from the screen |

**Frame limiter and heat.** The limiter takes the lowest of the options' cap and battery mode's cap, and a thermal
cap on top of those: nothing up to *light*, **45 FPS** at *moderate*, **30** at *severe*, **20** from *critical*
(`platform::thermal_fps_cap`). The thermal status is not fed yet on iOS — the Swift side does not report
`ProcessInfo.thermalState` to the shell in v1 — so the thermal cap never applies until that wiring lands.
With no surface (the app in the background) the loop sleeps instead of spinning.

## Lifecycle: background, rotate

iOS destroys the `CAMetalLayer`/surface when the app goes to the background, and the window changes shape on
rotation. The game handles it with the renderer kept as is:

- **`suspended`** (app backgrounded): the swapchain is dropped *before the callback returns*, since the window
  is gone after it; held keys and fingers are released; the world is autosaved; nothing is drawn and the loop waits.
- **`resumed`** (app foregrounded): a new surface is created for the window's size now and configured; the device, pipelines and textures are the old ones.
  If the loading screen was still up it carries on from there.
- **Resized** (rotation): the swapchain is reconfigured.
- Losing focus pauses the game (the *Pause on lost focus* option).

iOS may suspend or kill a backgrounded app without the `suspended` callback running to completion; the autosave
on suspend is the mitigation, and a memory-warning kill leaves the last autosave as what remains.

## Logs and debugging

The game's log is in the game folder (`Documents/logs/latest.log`), visible in the Files app; **Share the log** on
the launcher screen sends its tail as text. The shell's own lifecycle lines go to `os_log` (Console.app on a Mac);
the game's lines do not reach `os_log` yet — the `platform` log sink is not wired to the shell in v1.

The renderer logs what it requests and what it gets (`Horizon renderer: requesting the device ... with features [...] and the adapter's limits (...)`,
and a retry without optional features if the driver refuses them); a crash in the graphics driver leaves no panic, so the last
`Horizon renderer:` line is the place it stopped.

On screen: if the game ends with an error or a panic, the app hands over to the launcher screen, which shows the message (and the crash
report's name); if the process dies without a message, the launcher shows the end of the log.

## Known issues and limits

Untested on a device: everything. In particular the first run on real hardware can show surprises in these areas, in the order the
checklist meets them: the Rust build for `aarch64-apple-ios` (dependencies with C code such as `ring` and `mimalloc`, and `cpal`),
the Metal device (features/limits of the A15), the surface lifecycle, touch delivery, text entry.

Known limits and decisions that will bite:

- **Text entry** uses the game's own on-screen keyboard (tap *Keys*). iOS's soft keyboard is not wired in: the
  Rust side never sets the keyboard bit, and winit's iOS backend is not known to deliver text input. The game's OSK is the fallback either way.
- **No system clipboard** (copy and paste work inside the game only; `UIPasteboard` bridging is a follow-up).
- **No thermal feed yet**: the thermal cap exists in the game, but the Swift side does not report the thermal state, so the phone draws at the options' cap even as it warms.
- **No log sink yet**: the game's lines go to `logs/latest.log` only, not to `os_log`/Console.app.
- **Controllers:** `gilrs` and the DualSense code are not built on iOS. A Bluetooth gamepad is not used yet.
- **Mods with code** are native now, built by the launcher on a computer; whether one can be delivered into an iOS app sandbox and loaded at all (signing, dynamic loading) is undecided. Content-only mods (`.hmod`) should work through the content path.
- **Shader packs, LOD distance, and high render distances** are the desktop's heavy features; they work in principle, but a phone's budget is far smaller.
- **Display cut-outs and safe areas:** the game draws full screen; the HUD and touch controls do not avoid the Dynamic Island, the home indicator, or rounded corners yet.
- **Memory:** iOS may kill a backgrounded app; the last autosave is what remains.
- **Worlds are backed up with the app** (the game home is `Documents/`): a big world bloats the iCloud backup. That is accepted for v1.
- **The launcher's Start button only shows a "relaunch to play" note**: each launch is one phase (game or launcher), so an import needs a relaunch. Presenting the picker from inside the game is a follow-up.
- The iPhone has no mouse; iPad trackpad/mouse support is a follow-up (the `platform` pointer queue exists for it).
- Multiplayer, LAN, and online sign-in are the desktop code and untested here; v1 scope is single-player + LAN.
- Audio (`cpal` 0.16, CoreAudio backend) is unverified on iOS; if it fails to build or link, the escape hatch is building the shell without the `audio` feature.
- Gaps in the touch controls: no pick-block (creative middle click), no swap-hands button, no perspective toggle (F5), no F3.

## What changed in shared code

The desktop must not change; every iOS difference is behind `cfg(any(target_os = "android", target_os = "ios"))`, `cfg!(...)` that is constant false elsewhere,
or code that only a phone event can reach. The mobile layer is shared with Android (the fork's `android`-only cfgs extended to `android | ios`):

- **New modules** (compiled everywhere, used by Android and iOS): `touch` and `mobile` in `horizon-client-lib`;
  `platform` in `horizon-client-render` (logging lives there, so the sink forwarding reaches it without a crate cycle);
  `winit_host` in `horizon-client` (iOS uses plain `EventLoop::new()` like the desktop — no iOS branch needed);
  `game/touch` and `game/surface` in `horizon-client/src/game/`. The touch controls are off on the desktop
  (`HORIZON_TOUCH=1` turns them on for testing there).
- `lib.rs`: **unchanged** — upstream already exposes `game` unconditionally, so no `run_game()` wrapper is needed; the
  shell calls `horizon_client::game::run_with()`.
- `game.rs`: the event loop comes from `winit_host::event_loop()` (identical `EventLoop::new()` off Android); `resumed`/`suspended` handle a
  surface that comes back (added to `Game`, `Loading`, `Split` and `App` — upstream had `resumed` but no `suspended` at all); `Touch` events;
  the loading screen takes a finger as a mouse; the loop sleeps without a surface; the launch arguments can
  come from `horizon-args.txt`; the default resources folder is `resources` (Android and iOS); the mouse grab is virtual on a phone.
  The touch helpers take `&dyn EventLoopCtl` and send synthetic events through `handle_window_event`, since the
  loop's work moved into `idle()` on current upstream.
- `gfx.rs`: `suspend_surface` / `reattach_surface` / `has_surface` (reattach reuses upstream's `recreate_surface`, which
  keeps a spare surface, then resizes); on Android and iOS the device request retries without optional features.
- `game/options.rs`: the frame limit takes the thermal cap (none on the desktop). `game/pad.rs`: the move vector can come from the touch stick; `pad_action` is `pub(super)`.
  `game/pad_menu.rs`: three methods `pub(super)` and the keyboard's top-or-bottom test factored out. `osk.rs`: tap hit testing.
- `logging.rs`: a log sink (fed by the shell) and, on a phone, the other crates' warnings; locks tolerated. `crash.rs`: `mark_main_thread()`,
  so a shell's game thread counts as the main one. `save.rs`: the session lock is skipped where the file system has none.
  `clipboard.rs`: the game's own copy on Android and iOS.
- `horizon-client-input/Cargo.toml`: `arboard` is not an Android or iOS dependency (it pulls `wayland-backend`, which does not compile for iOS).
- Root `Cargo.toml`: `ios` is excluded from the workspace (it has its own, like the launcher and `android/native`), so the desktop `Cargo.lock` is untouched.
- **Not ported**: the fork's `lib.rs` edits (upstream supersedes them), the `mods` feature and `horizon-mods/src/stub.rs`
  (upstream has no wasmtime runtime any more; code mods are native now), and anything in the fork that was upstream drift.

## Decisions

- **A static library, not a dynamic one.** iOS does not load arbitrary dynamic libraries from the app bundle the way Android loads a
  `cdylib`; the Xcode app links the `staticlib` instead. One C entry point, `horizon_ios_main(Documents path)`, is the analogue of `android_main`.
- **No `@main` AppDelegate.** winit's iOS backend calls `UIApplicationMain` itself and asserts no one else has; `main.swift` is the entry
  point and hands the main thread straight to Rust. The price is the two-phases-one-process launcher (see above).
- **No Xcode project file in the repo, XcodeGen instead.** The project is generated from `ios/project.yml`; the workflow and `build-ipa.sh`
  run `xcodegen generate` first. No Gradle equivalent is needed.
- **Game home is `Documents/`**, not Application Support: the player can see (and rescue) saves and logs in the Files app, and
  `UIFileSharingEnabled` / `LSSupportsOpeningDocumentsInPlace` are on for the same reason. The trade-off is worlds in the iCloud backup.
- **Unsigned `.ipa`, SideStore/AltStore signing.** App Store distribution is not realistic for an unofficial reimplementation that needs the
  player's own assets; a free Apple ID signs on install. No Apple ID or certificate lives in the repo or CI.
- **Swift only where iOS demands it**, talking to Rust through one C entry point and the `platform` bridge (thermal status, wants bits,
  pointer events, exit request) once the Swift feeders land. The only `unsafe` code is in the shell (the os_log FFI, the environment).
- **The game's own on-screen keyboard** (made for controllers) for text, because it is drawn where it does not cover the field and needs nothing from iOS.
- **Touch controls as pure logic** with tests (`touch.rs`), and a thin layer that turns their actions into the same calls the controller layer makes
  (`pad_action("key.jump", ...)`, synthetic pointer events), so key bindings, toggles and every screen's mouse behaviour are reused.
- **The wgpu backend** is Metal on iOS; the device is requested with the adapter's own limits, so a mobile GPU cannot fail on limits; only optional features have a fallback.
- **The symlink bridge** (`.local/horizon-client/26.3` → `resources/`) stays in the shell as a fallback now that the game defaults to `resources/` itself; remove it when the shell is next touched.

## What next

1. Run [IOS_TESTING.md](IOS_TESTING.md) on the phone and fix what the first steps find (compile errors first).
2. Wire the Swift feeders for `platform.rs`: `ProcessInfo.thermalState` → thermal status, and an exit export on terminate → `platform::request_exit()`.
3. Wire the shell's os_log writer to `platform::set_log_sink`, so the game's lines reach Console.app.
4. Present the document picker from inside the game, so an import does not need a relaunch.
5. Safe-area handling for the Dynamic Island and home indicator (feed `safeAreaInsets` into the touch layout's `Screen`).
6. Gamepads: map iOS controller input to the controller layer. iOS clipboard through `UIPasteboard`; pick-block, swap-hands and perspective buttons.
7. Native code mods on iOS: decide whether a launcher-built native mod can be delivered and loaded at all.
