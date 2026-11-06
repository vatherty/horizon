# iOS performance and lifecycle implementation checklist

Initial checkout: `88231c8b0953a60a1361b071e0a38574ad77348a` on `vatherty/horizon` branch
`ios-port` (verified against the remote before work). That base includes both
Metal-selection commits and the latest change back to `macos-15`. No history was
reset, rebased or force-pushed. During validation the remote advanced to
`8dcf7f78e91e6f1a504516db0dbc90652480b0fc` (the IPA runner changed to
`macos-latest`); that commit was merged and retained. The distribution patch base is this newer
remote commit, `8dcf7f78e91e6f1a504516db0dbc90652480b0fc`, so its CI change is not duplicated
in the application series. Three workers used separate worktrees, with an
integration branch preserving their commits.

Classification: **Fix** = implemented fix/default with host evidence; **Experiment**
= functional opt-in path, off by default, awaiting device measurements;
**Blocked validation** = implementation is present but a required Apple SDK or
resource/device validation cannot be completed in this environment. A host policy
test is not evidence that UIKit/Metal linkage or device behavior passed.

## Prerequisites

| Item | Classification | Implementation and evidence |
|---|---|---|
| iOS selects Metal | Fix already landed; preserved | Base `592e18fe7` and existing `ios_uses_metal_for_every_saved_graphics_api` regression; no duplicate backend patch. |
| Seed defaults before checks/startup | Blocked validation (code implemented) | `ios/src/lib.rs` calls `mobile::seed` after logging, before resource preflight or `run_with`; target SDK compile is pending. |
| Missing-file-only 6/5/60/battery/75% | Fix | Existing distance/cap/battery defaults preserved; only the missing 75% world-scale default added. `mobile` tests serialize/read settings and exercise first/partial/repeated seeding. Native HUD uses the existing separate overlay path. |
| Existing settings unchanged | Fix | No runtime power handler writes settings. Seeding tests preserve both existing settings files byte-for-byte, including when only one file is missing. |

## Agent 1: lifecycle, heat and power

| Requested item | Classification | Implementation and evidence |
|---|---|---|
| 1. Detached resize/resume | Fix | Explicit Native/Detached/Offscreen state; detached resize retains targets/config/suspension. Surface-policy tests cover valid/zero/oversized/repeated transitions, real offscreen sizing and resume. Native UIKit delivery remains a phone check. |
| 2. Background pause/save | Fix | Atomic request consumed between server ticks; private pause/save, published LAN/local-seat save without private pause. Tests cover repeated events, quick resume, current pause before redraw and blocked owner. Real save/reopen/LAN parity needs resources. Explicit recording snapshots execute save+copy in one owner task so the stricter LAN pause policy cannot tear copies. |
| 3. Resume timing | Fix | Foreground FrameTime excludes the background gap; frame measurement and software pacing restart. Fake-clock test feeds 25ms after six hours and preserves FixedStep fraction/rate/freeze/step. Vanilla simulation cadence/RNG remain unchanged. |
| 4. Retained thermal monitor | Blocked validation (code implemented) | Process singleton starts in main.swift before Rust/winit; initial/change/activation samples and scalar bridge. Explicit nominal/fair/serious/critical mapping tested in Rust. Swift/UIKit compilation and real linkage await authorized macOS CI. No second application loop. |
| 5. Separate OS Low Power Mode | Fix, Apple bridge validation pending | Independent atomic signal; lowest player/inactivity/battery/thermal/OS cap. Intersection/recovery tests preserve lower player choices. Swift initial/notification/activation feeder is part of the pending SDK link. |
| 6. Pacing/ProMotion evaluation | Fix/default preserved | 60 gameplay/30 menus baseline; existing single limiter and winit scheduling. No extra display link, default 120 or ProMotion plist opt-in. Higher player-selected rates remain an experiment to measure, not an efficiency claim. |

## Agent 2: GPU, textures, memory and reuse

| Requested item | Classification | Implementation and evidence |
|---|---|---|
| 1. Avoid texture repartition copy | Fix | Apple CPU preparation chooses Metal 512 policy early, honors explicit split/adapter limits, and resplits per-array slices without a whole-chain concat. Boundary/PBR/mipmap/animation/order tests cover equivalent output and differing splits. Existing cache generation inputs retained. |
| 2. Static block-entity pipeline reuse | Fix | Shader, bind-group layout, dependent pipeline layout and render pipeline go through immutable reuse cache; device identity joins descriptor keys. Uniform buffers/groups remain per renderer. Reuse/pixel/device tests validate cache boundaries. |
| 3. Boot/title/world overlap | Fix | RendererSeed retains only instance/device/queue/reuse/surface handoff, allowing obsolete targets/atlases to drop before replacement. First-title-frame wait retained; reload recovery keeps old renderer until success. Existing surface synchronization retained; no reclamation-only GPU wait added. Actual iOS peak overlap awaits measurements. |
| 4. iOS memory observations | Blocked validation (SDK implementation present) | Conditional C shim includes Apple mach/os headers; SDK defines layouts/counts for RSS and phys_footprint, plus os_proc_available_memory. Rust returns distinct Option observations and never adds overlapping totals. Conditional cc build must pass Apple SDK check/link; Linux fallback does not validate it. |
| 5. GPU timing controls | Fix | Default no basic timestamps/marker passes; opt-in sampled/full timing and diagnostic demand. Async slot completion and submission order retained, with requested/submitted/completed/invalid/busy counts. Policy and GPU slot/order tests; actual Metal timestamps remain a device check. |
| 6a. Mobile arena policy | Experiment | HORIZON_IOS_SMALL_ARENAS=1 selects smaller growth/reclamation thresholds; existing retired/in-flight lifetime safeguards retained. Default unchanged; policy/growth/reclamation tests. |
| 6b. Transient overlay depth | Experiment | HORIZON_IOS_TRANSIENT_DEPTH=1 uses native-size attachment-only depth for scaled-world hand rendering when HDR does not consume it. Unsupported or rejected transient requests use the existing regular depth, without allocating a duplicate fallback. The original native depth remains retained; releasing it needs a wider HUD/screenshot/HDR lifecycle audit. Clear/discard only; sampled world/HDR depth remains regular. Target lifecycle/pass validation tested on host; real Metal render/screenshot validation pending. |
| 6c. Dynamic world resolution | Experiment | HORIZON_IOS_DYNAMIC_RESOLUTION=1 enables sampled GPU feedback, bounded user ceiling/floor, hysteresis and at least five seconds between changes. Targets update lazily; no per-frame realloc. Fake-clock/controller tests cover pressure/cooling/ceiling/invalid values. Actual A15 quality/performance and resize/reload/screenshots pending. |

## Agent 3: startup, CPU, uploads and diagnostics

| Requested item | Classification | Implementation and evidence |
|---|---|---|
| 1. iOS startup measurements | Blocked validation (code implemented) | Capture early origin/mark_start, establish Documents/logs, read horizon-env, then init existing opt-in startup trace before resource loading. Consent/upload behavior unchanged. Actual entry compiled/linked only by Apple CI, pending. |
| 2. Loading pacing | Fix | Deadline gates both requests and delivered redraws, with WaitUntil and shared cap/menu ceiling. Completion/error polling stays bounded and input/resize/resume responsive; no UIKit sleep. Fake-clock repeated-wake, hidden-pending, slow-cap and resume tests. |
| 3. Startup leaf-work budget | Fix | Shared generation budget covers outer loaders, panorama, models/sprites maps and block validation. Nonblocking spawn defers inline; joins release parent slots. Tests cover actual leaf peak, ordered values/errors, nested work, panic/cancellation cleanup and repeated join reference reclamation. No A15 core-count constant. |
| 4. Small upload deadlines | Fix | All automatic mesh batches, including 1..8, honor existing 2/6ms slice after at least one progress. Explicit count/byte policy retained. Fake elapsed tests and real pending-queue replacement/unload/nearest-first tests. One costly upload may overrun the slice. |
| 5. Integrated asset reuse | Fix | Immutable complete GameCpuAssets cached per PackResources generation, shared through Arc; mutable UI/trim/waypoint state remains per Game. Tests remove the source and exercise retained boat/painting/model availability, independent Creative state and new-generation invalidation. Resource-backed placement/save/inventory parity pending prepared resources. |
| 6. Bounded Files summary | Fix | See startup notes for exact local summary schema/path/interval. Bounded frame samples and infrequent writes/memory snapshots; build/backend/dimensions/caps/power/CPU/GPU/sample counts/workers/pending fields, with unavailable measurements explicit. No per-frame disk stream or automatic uploads. |

## Limits and evidence

Revision-specific commands/results are in [ios-validation.md](ios-validation.md).
Implementation detail and experimental controls are in
[ios-lifecycle-notes.md](ios-lifecycle-notes.md),
[ios-render-notes.md](ios-render-notes.md), and
[ios-startup-notes.md](ios-startup-notes.md).
[Phone-only acceptance steps](ios-phone-tests.md) cover import, first launch,
world entry, Home/lock/rotation, saves/LAN, runtime caps and comparable cold/warm runs.

No Instruments results, A15 FPS gains, memory savings or battery savings have
been measured. 75% linear resolution is about 56% world pixels and adds a target/copy;
mipmapLevels=2 still allocates the full chain; texture budget is top-mip color data.
Depth discard, Metal render ordering, staging reuse, in-flight lifetimes and
disabled iOS asynchronous submission have been retained.
