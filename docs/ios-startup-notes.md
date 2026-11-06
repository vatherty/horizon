# iOS startup, loading, uploads, and local diagnostics

These changes affect resource preparation and presentation scheduling. They do
not change simulation ticks, random draws, network messages, telemetry consent,
or telemetry uploads.

| Item | Implementation | Evidence and remaining validation |
| --- | --- | --- |
| Early startup measurement | `horizon_ios_main` captures an `Instant` and calls `telemetry::mark_start` before shell setup. `start` creates Documents, tmp, and logs, enters Documents, reads `horizon-env.txt`, and then initializes the optional startup trace using the captured origin. | The same origin includes folder/env setup; a first-launch relative `logs/startup.jsonl` has its parent directory before trace opening. Actual iOS SDK linking and on-device trace collection remain pending. |
| Mobile defaults | The iOS shell calls `mobile::seed` after logging initialization and before resource preflight. Existing player files are preserved by that API; failures are logged and preflight continues. | Default values and missing-file behavior are covered by the mobile-default implementation's tests. |
| Loading redraw pacing | A deadline gates requested and unsolicited OS redraws. `WaitUntil` replaces `Poll`; the effective cap intersects the player, battery, thermal, and OS power policy with the mobile loading cap of 30 FPS (desktop menu cap 60). | Two fake-clock tests cover repeated wakes/redraws, a hidden pending redraw, a slow player cap, and immediate scheduling after reset/resume. Resource/error polling remains at most 33 ms apart while active; normal input events continue to be dispatched. Resize, new title artwork, and resume reset the schedule. No main-thread sleep is added. |
| Nested startup CPU work | One `horizon_threads::loading::Budget` covers core/rest tables, atlas/CIT PNG decoding, panorama faces, texture preparation, models/sprites, block-facts validation, and launch-world/sync-data preloads. It starts before asynchronous mod preparation; synchronous mod preparation also enters the selected budget. The existing three-table memory ceiling remains in force. Child spawning reserves an available slot or defers work inline; waiting parents release their slot. The rest loader and preload roots inherit their generation's budget. Pack reload preparation and world-selection preloads explicitly select a budget too. | Four standalone tests exercise limits 1/2/4, full slot use, nested work with ordered values/errors, panic cleanup, and 100 repeated single-slot root joins with reclaimed generation references. Deferred scoped work is discarded on unwind; already started scoped children are joined. No A15/core-count assumption is used. Runtime generation/server/network pool policies are unchanged. |
| Small mesh batches | Every batch enters the same replacement/unload queue and nearest-first ordering, including batches of at most eight meshes. Automatic admission checks elapsed time after each upload and permits at least one mesh per frame. | Three standalone tests cover batch sizes 1–8 under a fake 3 ms upload/2 ms deadline, eventual progress, explicit count/byte limits, replacement, unload, and nearest-first order. Explicit user limits retain their existing semantics. The 2 ms normal/6 ms arrival automatic deadlines are unchanged. |
| Integrated-server CPU assets | Each immutable `PackResources` generation owns one cached CPU preparation. The client and server share `Arc<GameCpuAssets>` with complete boat/chest-boat availability, painting variants and placeable order, frame models, and entity models. Each Game has separate mutable creative, trim, and waypoint state. | The synthetic startup test prepares on a worker, removes the source root, obtains the shared server snapshot, verifies boat and painting gameplay inputs, and verifies separate creative selection. A fresh generation cannot return the old cache. Resource reload creates a fresh cache; this does not introduce a process-global path cache. Full resource-backed server/pack-reload parity runs remain pending because prepared resources are absent. |
| Local performance report | iOS writes `Documents/performance-summary.json`, visible in Files. A background writer replaces that one file using a temporary file. Fixed 256-bin histograms retain no frame list. One bounded channel holds at most one pending snapshot; snapshots are offered on the first rendered frame and then at most every 30 seconds. JSON is limited to 32 KiB. | Two standalone tests record 100,000 frames, verify unavailable CPU/GPU remain `null`, verify sampling cadence and suspended-time exclusion, and verify replacement leaves only one file. The report does not enter the telemetry outbox. Device power/thermal transitions and Metal timing still require device validation. |

The local report includes build, backend, output and world pixel dimensions,
world scale, effective FPS cap, thermal state, OS Low Power Mode, battery mode,
configured workers, pending jobs/uploads, resident sections, arena capacity,
process memory observations, resolution changes, and GPU timing completion/error
counters. Frame intervals, measured thread CPU, submission wall time excluding
surface wait, and valid completed GPU samples are separate metrics. Missing
thread CPU/GPU measurements are `null`, not zero. Percentiles are upper bounds
from cumulative session samples in 1 ms histogram bins; the last bin uses the observed maximum. Resident,
footprint, available, and GPU allocation observations overlap and must not be
added together.

To collect a startup trace on the phone, put this in Documents/horizon-env.txt
before launching:

```text
HORIZON_STARTUP_TRACE=logs/startup.jsonl
```

Tracing remains opt-in. It is separate from the bounded local performance file.
For desktop diagnostic testing, `HORIZON_LOCAL_PERFORMANCE_FILE=/absolute/path/performance-summary.json`
enables the same local report; desktop runs otherwise leave it disabled. No
telemetry consent setting is changed by either control.

Standalone validation used `rustc --edition=2024 --test` for `loading.rs`,
`loading_schedule.rs`, `gfx/upload/budget.rs` plus `mesh_uploads.rs`, and
`local_performance.rs` (with the already built `serde_json` dependency). They
passed 4, 2, 3, and 2 tests respectively. Workspace compile, Clippy, the synthetic
gameplay asset test, and resource-dependent harness results are recorded by the
integration checks; standalone tests do not establish Apple SDK or device
correctness.
