# Horizon for iOS

A native Swift shell around the Rust/winit/wgpu engine. The target for this
performance/lifecycle series is iPhone 13 Pro (A15, 6 GB system RAM), installed
through SideStore. System RAM is not an app memory allowance.

The engine-only unsigned IPA is built in macOS GitHub Actions and signed by
SideStore during installation. The player only needs the phone. Build/application
commands below are for the build/upload agent, who can use a cloud checkout.
No Minecraft resources, saves or Apple signing credentials belong in this repository.

See [the implementation checklist](../docs/ios-performance.md),
[validation results](../docs/ios-validation.md), and
[phone-only acceptance steps](../docs/ios-phone-tests.md). Device performance and
battery/memory savings have not been measured by this patch series.

## Layout

| Path | Purpose |
|---|---|
| `ios/src/lib.rs` | Rust staticlib entry, Documents home, logging, trace initialization and resource preflight |
| `ios/app/main.swift` | Process entry, power/thermal monitor and handoff to winit |
| `ios/app/LauncherAppDelegate.swift`, `LauncherViewController.swift` | Import/error phase and document picker |
| `ios/app/ResourceImporter.swift`, `ZipExtractor.swift` | Staged resource import with path validation |
| `ios/app/Horizon-Bridging-Header.h` | Scalar C interface between Swift and Rust |
| `ios/project.yml` | XcodeGen project; iOS 17+, unsigned build |
| `ios/build-rust.sh`, `build-ipa.sh` | Static library and IPA builders |
| `.github/workflows/ios.yml` | Apple target check/clippy, complete Swift/C/Rust link, IPA content check, artifacts |

## The Rust entry point

`horizon_ios_main(Documents path)` calls `horizon_client::game::run_with()` on
the main thread. It performs the shell setup normally done by `entry::main()`
without exiting the process on a preflight error: return code 0 means ended,
1 means error, 2 means resources missing. The importer reads `last-error.txt`.

The shell establishes Documents as the game home/current directory and reads
`horizon-env.txt` before opt-in startup tracing. Logging directories exist before
trace initialization; the early monotonic origin includes shell setup. No consent
or automatic-upload behavior changes. `horizon-args.txt` accepts launch arguments
on iOS. Mobile defaults are seeded before resource checks and startup, once per
missing settings file; an existing file is left untouched.

Missing-file defaults: render distance 6, simulation distance 5, max FPS 60,
battery mode on, world resolution 75%, native-resolution HUD. Gameplay baseline
is 60 FPS and menus 30 FPS. Player, battery, thermal and OS Low Power Mode limits
intersect at the lowest cap; OS transitions never rewrite saved options or battery
mode. No additional display link or second frame scheduler is installed.

75% linear resolution means roughly 56% of native world pixels, plus an intermediate
target and copy. It does not guarantee FPS or RAM savings. `mipmapLevels=2` limits
sampling and currently does not reduce the allocated full chain. The texture budget
covers top-mip color data rather than all GPU/process memory.

## Two phases, one process

Winit 0.30.13 owns `UIApplicationMain`, its window and view controller. It requires
a main-thread event loop and cannot start after UIKit's application loop has
already started. The shell therefore has no `@main` AppDelegate and never starts
an extra UIKit loop for gameplay.

On a launch with resources, `main.swift` starts the retained process-lifetime
power/thermal observer and hands control directly to Rust/winit. The observer is
not restricted to `LauncherAppDelegate`, which is used only when the resource
preflight returns or startup fails before entering the application loop.

On a missing-resource launch, Swift starts the importer/error launcher. Import
then requires closing and reopening the app; the same process cannot transition
from the importer's `UIApplicationMain` into winit. No scene manifest is used.

Winit delivers active/inactive notifications to the Rust lifecycle. Suspension
detaches native rendering without converting it to intentional offscreen rendering.
A private integrated world receives a nonblocking owner-thread pause/save request;
published LAN worlds retain network simulation semantics and are not privately
paused. iOS can suspend the whole process regardless. Save work does not block
UIKit, and asynchronous writes are not guaranteed to finish before termination.
Resume resets wall-clock pacing without changing vanilla simulation frequency.

## The resource-zip contract

The player supplies resources from their own licensed Java Edition 26.3 install.
The prepared folder contains `horizon-resources.json` at its root. Import searches
inside a zip (depth up to 4) or folder (up to 3) for that marker, stages to
`Documents/resources.importing`, then replaces `Documents/resources`. Invalid
imports leave the current resources intact. Zip entries escaping the game folder,
encrypted archives and multi-disk archives are rejected. Stored/deflate and zip64
are supported. The phone app does not run the Java resource preparer; arrange a
lawful prepared archive separately if the player does not already have one.

A compatibility symlink `.local/horizon-client/26.3 -> ../../resources` is retained.
An existing real prepared folder wins and is not replaced. The mobile argument and
resource defaults already cover iOS; the compatibility bridge is not a prerequisite
for the mobile renderer.

Documents is exposed through Files (`UIFileSharingEnabled` and
`LSSupportsOpeningDocumentsInPlace`). It holds:

| Path | Contents |
|---|---|
| `resources/` | Imported resources, never included in patches/IPA |
| `saves/` | Worlds |
| `options.txt`, `horizon-graphics.txt` | Player settings |
| `horizon-env.txt`, `horizon-args.txt`, `horizon-touch.txt` | Optional switches, launch args, touch settings |
| `logs/latest.log`, `crash-reports/` | Local diagnostics |
| `last-error.txt`, `running.marker` | Launcher status |

Local performance-summary and experiment controls are documented in the checklist
and worker notes linked there. Missing measurements remain unavailable. Process
resident, footprint, allocator and renderer observations overlap: never add them
as if they were independent memory allocations.

## Build and CI handoff

A build agent on macOS with Xcode, Rust and XcodeGen runs:

```sh
bash ios/build-ipa.sh
# dist/horizon-ios-unsigned.ipa
```

The workflow retains the newer remote `macos-latest` runner change. It runs Apple target check/clippy and the real
Xcode application build/link; a cfg-disabled Linux shell test is not iOS validation.
An existing successful run does not validate a later patch revision.

Pushes to `ios-port` build and update the `ios-latest` pre-release as before.
For an isolated integration branch, dispatch `ios.yml` on that branch. The optional
`publish_release` input defaults to false: the IPA artifact is uploaded without
replacing the fixed phone download. After validation the build/upload agent may
explicitly publish the tested revision. Never reset or force-push `ios-port`.

In Safari, download the finished IPA to Files and import it into SideStore to sign
and install. Import the lawful prepared-resource archive inside Horizon, close the
app and reopen. The player need not apply patches or use a computer.

## Validation limits

See the revision-specific validation report. Linux formatting, policy/concurrency
unit tests and software-GPU tests cannot establish Apple SDK linkage, A15 rendered
pixels, actual process headroom, heat, battery life or sustainable frame rate. The
SDK check and full IPA link must run on macOS, followed by the phone acceptance
steps. Experiments remain off by default until comparable phone measurements and
correctness checks justify enabling them.
