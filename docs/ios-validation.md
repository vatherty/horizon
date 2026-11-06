# iOS integration validation

Initial checkout: 88231c8b0953a60a1361b071e0a38574ad77348a.
Distribution patch base: 8dcf7f78e91e6f1a504516db0dbc90652480b0fc.
The newer remote CI commit was merged; macos-latest is preserved. Both Git pushes
and connected GitHub API branch creation rejected new validation branches with
HTTP 403. No remote branch was overwritten and no force-push/reset/rebase was used.

The final Rust implementation is present by fa5ccff6f; subsequent integration
changes are documentation and preservation of the newer CI runner. The bundle
manifest records the final packaged HEAD/tree. Exactly three workers used isolated
worktrees and their commits were integrated in dependency order. The clean worker
worktrees were removed after integration to reclaim build disk space; their branch
refs/commits remain in the repository and their patches are in the series.

## Host checks

Linux x86_64, Rust 1.99.1, Mesa 25 llvmpipe, two build jobs, incremental compilation
disabled. Tests used line-tables-only debug information, as the repository expects.
Runs used fresh scratch HORIZON_HOME, launcher, XDG data/config/runtime and game
folders. No real player saves/settings or Minecraft resources were used.

| Check | Result |
|---|---|
| python3 -m unittest discover -s tests | 690 tests, one skipped, no failures; repeated in pr_ready. |
| cargo fmt --all -- --check |Pass. |
| cargo fmt --check --manifest-path ios/Cargo.toml |Pass; formatting only, not an iOS compilation. |
| cargo clippy --workspace --all-targets --locked -- -D warnings |Pass on final source. |
| cargo clippy -p horizon-client --all-targets --locked --no-default-features -- -D warnings |Pass on final source (part of the same pr_ready Clippy gate). |
| cargo test --locked -p horizon-client --lib game:: -- --test-threads=2 | 1,019 passed, zero failed, 41 ignored, 6 filtered. |
| cargo test --locked -p horizon-client-lib --lib mobile:: -- --test-threads=2 | 3 passed, zero failed. |
| cargo test --locked -p horizon-client-render --lib -- --test-threads=2 | 1,513 passed, zero failed, 47 ignored; includes real software-GPU fixtures and the two crash/line-table tests. |
| cargo test --locked -p horizon-threads --lib -- --test-threads=2 | 21 passed, zero failed. |
| cargo test --locked --workspace --doc |Pass; the runner reported zero doc tests, so this adds no behavioral coverage. |
| cargo test --locked -p horizon-client-lib --lib mod_content::tests:: -- --ignored | 25 passed, zero failed. |
| public tree / public history / git diff --check |Pass; no game files in patch sources. |

The gameplay run includes the new lifecycle, fake-clock resume, Loading scheduling,
small-queue replacement/order, bounded-summary tests, and
startup_cpu_tests::worker_prepares_owned_models_and_inventory_before_handoff.
That synthetic test uses production asset preparation, removes its source root,
checks shared boat/chest-boat and painting inputs, independent Creative selection,
and generation invalidation. Full placement/save/inventory parity still needs
prepared vanilla resources.

The renderer run includes detached/offscreen/zero-size policy tests, cap recovery,
511/512/513 and 2047/2048/2049 texture/PBR boundaries, animation/cache updates across
Metal-sized partitions, independent mutable-uniform fixture pixels, device/source/
format cache separation, arena contents through growth/reclaim, asynchronous GPU
timing ordering/counts, transient attachment validation/fallback, and resolution
hysteresis/ceiling tests. The adapter is software Vulkan, not A15/Metal.

## Required full self-check and honest failures

The bundle contains the complete initial and corrected pr_ready reports/logs.
The corrected command was:

```sh
python3 tools/pr_ready.py --base 88231c8b0953a60a1361b071e0a38574ad77348a
```

It is **not an all-green self-check**:

- Full workspace test linking exhausted the 7.9 GB filesystem. ld terminated with
  SIGBUS while linking datapack_biome_sync, followed by an explicit ENOSPC creating
  the gameplay test temporary directory. This was a build failure, not an assertion
  failure. No full-workspace test pass is claimed. The focused libraries above were
  then built/run successfully after reclaiming generated artifacts.
- The unchanged desktop launcher was selected through shared Cargo inputs. Its
  Clippy/tests are blocked by missing Fontconfig development metadata (fontconfig.pc).
  Launcher formatting passed. The launcher is outside the iOS app workspace; this
  failure is retained rather than suppressed.
- pr_ready reported 441 affected base parity harnesses with no vanilla data. The
  final parity listing reports 449 dependent entries, including shard variants and
  two explicitly unselected nightly/manual entries (frame_identity and lan_integration);
  447 selected entries map to 439 base names. The full pending list is included.
  None were executed or counted as passing.

The initial self-check also lacked a raw C include path for the rootlessly installed
libudev headers. Adding C_INCLUDE_PATH fixed that host setup problem; the corrected
workspace/server Clippy gate passed. Two earlier interim renderer tests failed when
DEBUG=0 removed line tables needed by crash-report tests. The final renderer run
restored line tables and passed both tests. Interim missing loader/boat-field calls
were corrected before final Clippy and gameplay tests. No tests were removed or
weakened to hide these failures.

Temporary cache relocation and deletion of completed build executables were used
to fit host validation in the limited filesystem. They do not change test inputs
or shipped source and are not device-memory observations.

## Apple SDK, IPA and resource/device blockers

This Linux environment has no Xcode/UIKit SDK. bash ios/build-ipa.sh stops at its
required xcodegen check. The aarch64-apple-ios Rust standard library alone cannot
validate the native app. New-branch Git push and connected API creation both
returned 403, preventing submission of the patched tree to the available macOS
workflow. The older successful iOS shell run 42139410424 validates only its old
commit, not the new Swift monitor or Mach/os memory boundary.

Required next gate on authorized macOS CI:

1. cargo check and Clippy -D warnings for aarch64-apple-ios using ios/Cargo.toml.
2. Compile the conditional process_memory_ios.c against real Mach/os headers.
3. Typecheck PlatformPowerMonitor.swift, link its scalar Rust exports, build/link
   the complete Swift/C/Rust application through xcodebuild, and pass the engine-only
   IPA content check. Neither a staticlib nor an empty cfg-gated Linux iOS test
   substitutes for this gate.
4. Resource-backed server/save/reopen, boat/painting placement, inventory, reload,
   recording snapshot and LAN tests where lawful prepared data exist.
5. The phone acceptance matrix in ios-phone-tests.md, including cold/warm comparisons,
   Home/lock/rotation, private/LAN behavior and power/thermal-cap restoration.

No new IPA, Instruments trace, measured FPS gain, heat reduction, battery saving
or process/Metal memory saving is claimed. Experiments remain off by default.
See ios-build-handoff.md for the upload/build agent's cloud workflow; the player
needs only the iPhone, Files, Safari and SideStore.

## Patch application verification

The cumulative patch is applied to a separate temporary Git index initialized at
the distribution base. The format-patch series is applied with git am --3way in a
clean detached base worktree. Both resulting trees must equal the packaged HEAD's
tree; the bundle manifest records these results. The series retains all worker
commits and attribution. The initial checkout's CI work and the later remote
runner change are already in the recorded base rather than duplicated in patches.
