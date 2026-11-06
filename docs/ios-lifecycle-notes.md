# iOS lifecycle, thermal state and power caps

The lifecycle changes protect native rendering and move background saving to
the integrated server's owning thread. They do not change world tick rates,
random calls, network protocol, `/tick` freeze/step state or saved settings.

## Native surface behavior

`Renderer` records native attachment separately from intentional offscreen
rendering. Suspending a native renderer drops its surface and replacement spare.
While detached, resize events retain the last configuration, allocate no color
or depth targets, and leave rendering suspended. Only an explicit resume can
reattach the native window. `recreate_surface` cannot bypass native suspension.

An intentional offscreen renderer retains its target through native lifecycle
events and still resizes as an offscreen renderer. Invalid sizes retain the last
targets; repeated drawable sizes avoid reallocation. A zero-sized native resume
may attach the window handle but skips swapchain configuration until a drawable
resize arrives. `has_surface` is false while rendering remains suspended.

## Background saving and pausing

The UIKit/winit lifecycle callback atomically publishes the current menu pause
intent and background state. It does not snapshot the world, wait for a server
reply, join a thread or perform disk I/O. This also reaches an integrated server
that is still opening its world. Current menu intent is captured even if the
pause screen opened after the last redraw.

Between ticks, the server owner consumes the request and invokes the existing
autosave path (`save_game_with(false)`). A private single-player world pauses;
connections continue through the existing paused-server path. A published LAN
world or world with other local seats requests the same save and keeps its
ordinary simulation/network behavior while the process can run. Becoming private
again applies any still-active pause intent. Remote clients do not save the
remote world's data.

Repeated background notifications do not enqueue repeated saves. A foreground
transition does not cancel an unconsumed background save. A second background
transition while the owner is saving retains another save request. Returning to
the app preserves the pause screen and its pause intent.

Client redraws and UI polling wait while the app is suspended. Resume restarts
the foreground frame clock, FPS measurement window and frame limiter. The owner
also restarts its wall-time deadline. Suspended wall time is excluded without
resetting `FixedStep` or discarding its interpolation, rate, freeze or step state.

The lifecycle save is best effort. The OS may suspend or terminate the process
before it is serviced or before asynchronous writes finish. There is no claim
of guaranteed persistence at termination or continued LAN hosting in suspension.

## Thermal and OS power signals

`PlatformPowerMonitor.shared` is retained for the process lifetime and starts in
`main.swift` before `horizon_ios_main`. It samples immediately, observes thermal
and OS power changes, and samples again on application activation. It operates
during the game's winit-owned UIKit phase as well as the importer phase;
`LauncherAppDelegate` alone cannot cover the game.

The Swift switch exports a scalar bridge with an explicit Apple mapping:

| Apple state | Bridge | Rust status | Thermal cap |
| --- | ---: | --- | ---: |
| nominal | 0 | NONE | none |
| fair | 1 | MODERATE | 45 FPS |
| serious | 2 | SEVERE | 30 FPS |
| critical or unknown | 3 | CRITICAL | 20 FPS |

OS Low Power Mode is a separate atomic signal and adds a 30 FPS cap. The common
`platform::effective_fps_cap` helper intersects the player's/inactivity limit,
Battery Mode (60 FPS in focused gameplay, 30 in menus/unfocused), thermal cap,
and OS power cap. Clearing an external signal restores the intersection with
the player's current choices. No monitor or cap policy writes settings. Loading
uses the same policy with its own 30 FPS ceiling; gameplay retains its existing
single frame limiter.

ProMotion/120 FPS is excluded from the baseline: first-run gameplay stays at
60 FPS and menus/loading at 30 FPS. No high-refresh display entitlement, display
link or competing scheduler is enabled. Higher player-selected caps are an
explicit experiment and still intersect all active power limits.

## Recording snapshots

An explicitly requested recording with a world snapshot saves, flushes and
copies the world as one integrated-owner task. The server cannot tick between
those operations, including when it is published to LAN. This recording action
may wait for its owner task; the native lifecycle path remains nonblocking.
A missing reply or timeout returns an error instead of starting a recording
with an unconfirmed snapshot (the queued job may still complete afterward).

## Validation and remaining device work

Resource-free tests cover detached resize/resume, intentional offscreen behavior,
zero/oversized/repeated sizes, private and LAN/local-seat pause/save decisions,
quick resume, repeat events, a callback returning while the owner is blocked,
pending menu pause, and cap intersection/recovery. A fake clock advances six
hours through suspension, then confirms only 25 ms reaches the actual
`FixedStep`, preserving its fraction, tick rate, freeze and step count.

Standalone Rust validation passed 5 platform tests, 3 surface-policy tests, and
17 lifecycle/clock/text tests (7 lifecycle tests plus existing clock/text tests).
The wrappers include the production Rust modules; the clock wrapper only
replaces the log sink. Formatting and public-tree commit checks passed.

Apple SDK compilation and device measurements remain pending: this Linux
environment has no UIKit SDK or Xcode/Swift toolchain. Resource-backed save/load
and LAN end-to-end checks also need the player's prepared vanilla resources,
which are not present here. Device follow-up should cover an already-open pause
screen, private and published worlds, rotation while backgrounded, repeated
background/foreground events, thermal escalation/recovery, Low Power Mode
changes, and reopening the save after completed writes. No measured battery or
temperature improvement is claimed from the policy tests.
The recording save/flush/copy integration likewise needs a resource-backed world;
its existing resource-free copy-tree test covers the file-copy helper only.
