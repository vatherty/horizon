# iPhone-only acceptance tests

Target: iPhone 13 Pro (A15, 6 GB system RAM), SideStore. These are test steps,
not measured results. No computer, Instruments, or Xcode is required of the player.

## Receive and install

1. Receive the finished IPA link from the build/upload agent. In Safari, download
   the engine-only unsigned IPA to Files, then install/sign it through SideStore.
   Keep an export of important worlds in Files before replacing an experimental build.
2. Use a prepared resource archive from your own licensed Java Edition resources.
   Import it through Horizon's document picker. The app does not prepare Java
   resources itself; the build/upload agent must arrange an existing lawful archive
   if one is not already on the phone. No Minecraft files are in these patches or IPA.
3. Close and reopen Horizon after import: the importer and winit gameplay use
   separate launches. First launch without resources should show the importer;
   a bad archive should leave the previous imported resources usable.
4. With missing settings files, confirm render distance 6, simulation distance 5,
   max FPS 60, battery mode on, world resolution 75%. HUD/text remain native.
   Change a setting, reopen, and confirm it survives. Also test with just one
   settings file missing; the other must remain unchanged.
5. Enter a fresh scratch world. Check terrain, animated textures, water,
   transparency, held items, native HUD, menus and a screenshot. Check placement
   and use of a boat, painting variants, containers and inventory. Save/reopen.

## Lifecycle, save and network

1. In a private world, walk to a recognizable spot, change inventory and place
   a block. Press Home; return after 10 seconds, then after a minute. Check the
   world resumes, input clears, animation is normal, and there is no client tick
   burst. Repeat lock/unlock. Test starting from the pause screen: it must remain
   paused on return.
2. Rotate while inactive, then return in each supported orientation. Repeat
   rapidly and test Home during loading and while entering a world. A detached
   or zero-size surface must recover without a black screen or stretched HUD.
3. After a normal save/return to title, reopen the world and check the spot,
   block and inventory. Then repeat after Home/return. Lifecycle save requests
   are asynchronous on the server owner thread: iOS can suspend/terminate before
   completion, so forced termination is not a guaranteed durability test.
4. Publish the world to LAN and connect another phone/client if available. Home
   the host and return. LAN publishing must not use private-world pause semantics;
   iOS may suspend the whole process and interrupt networking. Verify reconnect,
   state and saves. Record this as unavailable if no second client is available.
5. Repeat Home/return before and after opening LAN. Test an already-paused world,
   repeated background notifications, and disconnecting the guest while inactive.

## Runtime power caps

1. Start cool with max FPS 60 and battery mode on: gameplay baseline 60, menus 30.
   Enable Low Power Mode in iOS Settings or Control Center while Horizon runs,
   return, and check the local summary's independent OS mode/effective cap.
2. Choose a lower player cap. Toggle Low Power Mode on/off: recovery must keep
   the lower player cap. Confirm neither battery setting nor saved max FPS changed.
3. Compare naturally warmed gameplay with a later cooled run. Record the thermal
   state and effective cap from the summary. Cooling must remove only the thermal
   restriction. Do not treat fair/serious/critical Apple integer values as Android
   status codes. Allow time for the OS to report cooling.
4. Repeat mode changes during loading and while backgrounded. On return both
   signals should be resampled. Leave ProMotion/120 FPS experiments off for baseline.

## Comparable cold and warmed runs

1. Record build SHA, iOS version, resource generation, world/position/view,
   screen brightness, charging state, Low Power Mode and graphics settings.
2. With the phone cool, relaunch, enter the same scratch world and follow the
   same route/view for a fixed duration. Keep output/world dimensions and FPS
   cap identical between builds. Save the bounded local performance summary
   from Horizon's Documents folder in Files after its write interval.
3. Repeat after sustained play, recording thermal state and cap. Repeat at least
   twice; separate startup, world arrival and steady-state intervals. Compare
   CPU/frame/GPU samples only where available, with sample counts. A missing GPU
   timestamp or memory observation is unavailable, not zero.
4. Enable one documented opt-in experiment at a time in horizon-env.txt using a
   text editor on the phone, then relaunch. Repeat the same runs and validate
   screenshots, hand/HUD, resize, settings changes and resource reload. Restore
   the baseline file between experiments.
5. Report crashes/black screens and unexpected cap recovery, along with
   logs/latest.log, crash reports, startup trace if explicitly enabled, summary,
   and screenshots via Files. Summary data are local and never automatically uploaded.

75% linear world resolution is about 56% of native world pixels. Scaling adds an
intermediate target/copy; it does not establish an FPS, RAM or battery saving.
The full allocated mip chain remains present with mipmapLevels=2. The texture
budget counts top-mip color data, not total texture/process/Metal memory. Resident,
physical footprint, allocator and renderer estimates overlap and must not be added.
