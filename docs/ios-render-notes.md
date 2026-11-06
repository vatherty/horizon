# iOS rendering and memory changes

This patch changes allocation/lifetime and diagnostic work. It does not establish a device FPS,
startup-time, memory, thermal, or battery improvement. No Minecraft resources or Apple SDK are
present in the Linux workspace; real title/world screenshots and on-device measurements remain
required. Software-GPU fixture pixels are a narrower check.

| Item | Implemented behavior | Default / limits |
| --- | --- | --- |
| Prepared textures | Apple workers prepare 512-layer arrays before device creation; other workers prepare 2048. Adapter limits still choose the final split. Repartitioning moves aligned arrays or copies one destination array at a time, without concatenating the atlas into another full-chain allocation. Global layer order/settings, forcefield and LabPBR chains remain intact. | Enabled. `HORIZON_SPRITE_SPLIT` retains its explicit override, including on Metal. Eight shader array bindings remain the hard limit; excess sprites produce the existing error. |
| Static block entities | Shader module, transform bind-group layout, pipeline layout, and render pipeline use the renderer's immutable cache. Every cache key includes actual device identity and the complete descriptor; referenced GPU handles distinguish layouts/modules. | Enabled. Per-renderer uniforms, textures and bind groups remain separate. A new device never receives another device's cached object. |
| Startup renderer lifetime | A `RendererSeed` retains the device, queue, adapter, immutable cache, present mode and transferable surface. Startup boot/title/world replacements drop obsolete renderers before constructing the next one. Ordinary resource reloads retain the previous renderer until success for recovery. | Enabled. Existing first-title-frame handoff is unchanged. No GPU-completion wait was added; the existing submit-thread handoff remains. Submitted GPU commands retain their referenced resources until safe. A failed startup replacement still reports the startup error. |
| iOS process memory | An SDK-compiled C boundary uses `task_info(MACH_TASK_BASIC_INFO)` for resident bytes, `task_info(TASK_VM_INFO)` for `phys_footprint`, and `os_proc_available_memory()` for the current process's available allocation headroom. The SDK supplies structs/counts. Rust exposes three independent `Option<u64>` values. | Read only. Individual failures are unavailable (`None`), never guessed or zero-filled. The project targets iOS 17; available-memory calls have an iOS 13 availability guard. iOS never calls macOS `proc_pidinfo`. |
| GPU frame timing | Timing resources and marker passes are lazy. Readbacks remain asynchronous, emitted in submission order, with requested/submitted/completed/invalid/busy counters. Already submitted samples finish after demand stops. | Off by default. `HORIZON_GPU_TIMING=sampled` times one in 120 frames; `full` or `1` times every frame. F3/pass diagnostics request full timing. Existing Metal start/end markers and delayed resolution are retained for measured frames. Unsupported timestamp queries remain unavailable. |
| Arena experiment | Smaller initial capacities (at most 4 MiB), 32 MiB page ceilings, growth steps at most 8 MiB, and 8 MiB compaction/shrink thresholds. Existing upload copies, stable ranges, empty-page trim, and staging reuse remain in place. | **Off:** iOS only, `HORIZON_IOS_SMALL_ARENAS=1`. More pages/copies/draw batches may cost performance. No claim of lower resident/physical memory follows from capacity alone. |
| Overlay depth experiment | A native-output-sized first-person hand target is attachment-only, clear/discard, and reused until dimensions change. Advertised memoryless support and depth-format transient usage gate `TRANSIENT_ATTACHMENT`; unsupported/rejected requests use the renderer's existing regular depth without a duplicate fallback allocation or repeated failed allocation attempts. | **Off:** iOS only, `HORIZON_IOS_TRANSIENT_DEPTH=1`. Used only with a separately scaled world and HDR off. HDR's subsequent depth load, direct world depth, sampled/copied depth, crosshair/HUD depth, and world depth retention/discard rules keep their existing path. The original regular depth is still retained, so this does not establish a net memory reduction. |
| Dynamic world resolution experiment | GPU samples drive a scale bounded by the user's chosen ceiling and a 50% floor (or a lower user ceiling). Three samples above 110% of the effective frame budget reduce scale; eight below 75% permit a small increase. Changes are at least five seconds apart and targets rebuild lazily only when dimensions change. | **Off:** iOS only, `HORIZON_IOS_DYNAMIC_RESOLUTION=1`. Samples one in 120 frames when full diagnostics are not active. The existing effective FPS limiter supplies the budget. Title and scaled-screenshot frames do not drive feedback. Existing world-post allocation failure recovery is retained. No per-frame target reallocation is introduced. |

`Renderer::performance_summary()` reports output/world target dimensions, color format, backend,
current world scale, resident section count, arena capacity, timing capability/counters and resolution
change count without walking meshes. `process_memory::observations()` reports OS observations.
**Do not add RSS, footprint, available headroom, live allocator bytes or GPU/arena capacities**: they
measure different, overlapping things. Metal's allocator report remains unavailable through wgpu;
arena capacity is not a substitute for driver allocation or process footprint. Sparse GPU samples
describe the sampled frames, not an unbiased FPS or percentile measurement.

## Verification

Implementation check: `cargo check -p horizon-client-render -p horizon-threads --no-default-features`
passed on Linux with incremental builds disabled. The following focused regressions exercise the
behavior without proprietary resources:

- `prepared_texture_uploads_preserve_layer_order_and_settings`: direct versus repartitioned mip data
  for exact and partial arrays and different per-layer cutout strategies.
- `prepared_texture_partition_boundaries_preserve_companion_maps`: 511/512/513 and 2047/2048/2049
  layers, both split directions, forcefield and LabPBR preservation, platform policy and override.
- `metal_boundary_layers_keep_animation_and_cache_order`: animated frames/mips across 512-layer
  boundaries, unchanged-tick cache suppression and changed-frame order, compared with CPU reference.
- `device_and_descriptor_boundaries_keep_pixels_and_mutable_uniforms_distinct`: real GPU pipeline
  reuse, device/format/source separation, and independent red/green/blue fixture pixel readbacks.
- `mobile_policy_bounds_growth_preserves_uploads_and_reclaims_empty_pages`: copied buffer contents
  survive growth; actual capacities, empty-page release and subsequent allocation are checked.
- `target_reuses_size_replaces_on_resize_and_validates_discard_pass`: native overlay depth reuse,
  dimension replacement, transient/ordinary attachment validation and invalid-size fallback.
- `timing_is_opt_in_sampled_or_explicitly_demanded`,
  `completed_samples_keep_submission_order_and_counts`, and the existing readback/device-loss tests.
- `dynamic_resolution::tests`: fake-clock pressure/hysteresis, minimum interval, disabled/invalid
  samples, a lower user ceiling and a ceiling below 50%.

The focused `gfx::` suite passed on Mesa llvmpipe: **374 passed, 14 existing ignored, 0 failed**
(`cargo test -p horizon-client-render --no-default-features --lib gfx:: -- --test-threads=2`).
After adding per-sample world-feedback eligibility, all four timing tests passed again. The Linux
process-residency allocation test passed. `cargo clippy -p horizon-client-render -p horizon-threads
--no-default-features --all-targets -- -D warnings` passed before the final depth-fallback refinement;
that refinement has a dedicated follow-up fixture run. Builds used `CARGO_INCREMENTAL=0`, two
build jobs and debug information disabled. GPU tests explicitly skip if no suitable adapter exists;
a skip is not pixel validation. The Linux test adapter is not an Apple GPU.

## Remaining platform validation

The iOS SDK compile/link must compile `process_memory_ios.c` with the actual target SDK and link the
final app; Linux cfg checking does not validate Apple symbols, availability or Mach headers. Neither
the C boundary nor process footprint/headroom was executed on iOS here. The optional memoryless
allocation was validated through wgpu on a software adapter only, not established as an Apple
memory/bandwidth saving. Real suspension/resumption, resize/rotation, first title, resource reload,
native/scaled screenshots, shader packs, HDR and error recovery need an iOS device with prepared
resources. No platform API blocker was found for attachment-only transient depth in wgpu 30; using
it for depth that is sampled, copied, stored or loaded by a later pass would violate that API.
Replacing/releasing the renderer's original native depth requires a wider audit of the direct-world,
HUD/crosshair, screenshot and HDR lifecycle. The hand-only experiment deliberately retains it.
