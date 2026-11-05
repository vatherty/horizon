# Testing Horizon on the phone

A checklist for the iOS build ([IOS.md](IOS.md)), ordered from "the app launches" to "walk around a world".
Run the steps in order and stop at the first one that fails: each later step needs the earlier ones. For a failure, report the
step number, what you saw, and the **evidence** listed under it.

Nothing in this port has run on a device yet, so step 0 may be a few compile errors rather than a phone problem. Send the first
error (the text from `error[` down to the next blank line), not the whole log.

## Before you start

There is no `adb` on iOS. The evidence comes from the phone itself:

- **Files app**: the game folder is `Documents/` in the Horizon app — `logs/latest.log` and `crash-reports/` are there.
- **Share the log** on the launcher screen sends the tail of `latest.log` as text.
- **Console.app** on a Mac shows the shell's own lifecycle lines (`os_log`); the game's lines are in `latest.log` only.
- Screenshots: power + volume up.

## Steps

### Build and install

**0. The .ipa builds.** `ios/build-ipa.sh` on a Mac ends with `Built .../dist/horizon-ios-unsigned.ipa`, or the
`ios` workflow on GitHub goes green and offers the `horizon-ios-unsigned-<sha>` artifact.
- Fails in `cargo build` for `aarch64-apple-ios`: send the first compiler error. A failure in `ring` or
  `mimalloc` names the missing iOS SDK (it builds on the Mac runner, not on Linux).
- Fails in `xcodegen` or `xcodebuild`: send that tool's message (the likeliest gap is a missing linker framework).

**1. It installs.** SideStore/AltStore installs the `.ipa` (signing it with a free Apple ID) and the Horizon icon appears.
- Fails: send the exact message SideStore/AltStore shows (a 7-day certificate that expired, the 3-app free-account
  limit, or an unsigned `.ipa` that was not handed to a signer).

### The app launches

**2. The launcher screen opens.** Tapping the Horizon icon runs the game, which finds no resources and hands over
to the launcher: a dark screen with "Horizon", "Minecraft Java Edition 26.3, rewritten in Rust" and your iPhone's model.
- Crashes at once: send `latest.log` if it exists (Files app, `logs/latest.log`) and what the launcher showed, if anything.

**3. It says the resources are missing.** Before importing anything, the launcher shows "The game's resources are not on this phone yet"
with how to prepare and send them.
- Shows "Resources found" already: you imported them earlier; go on. Shows nothing about resources: send a screenshot.

**4. Start without resources shows the error.** Tap *Start anyway (shows the error)*. The screen goes dark for a moment and returns to the
launcher with "The last run ended with an error: The game's resources are missing...".
- This is a test of the static library loading, `horizon_ios_main`, logging and the error path: it must work before anything else does.
- Fails: the app closes instead (send `latest.log`), or the launcher stays with no error. Tap *Dismiss* afterwards.

### Resources

**5. The resources import.** Prepare on the Deck with the Horizon launcher (Prepare button), zip the prepared folder's contents
(`cd <prepared folder> && zip -r ~/horizon-resources.zip .`), send the zip to the phone (AirDrop, Files, a messaging app),
tap **Import resources from a zip** and pick it. The launcher screen then says "Resources found in ...".
- "no horizon-resources.json in what was copied": the zip is of the wrong folder; zip the folder that holds `horizon-resources.json` (the manifest at the zip's root).
- The import fails with another message: send it. Check free space too (the resources are large and are copied, then the zip stays in Files).
- Prepare fails on the Deck: the same step on the desktop would fail too (send the red message under the buttons); usually JDK 25 or Minecraft 26.3 not launched once.
- After "Imported", **close the app and open it again**: each launch is one phase (game or launcher), so the game only starts on the next launch.

### The game starts

**6. The game window opens.** On the relaunch the screen goes full screen, then shows the loading screen
(the Horizon artwork and a progress bar).
- Black screen forever or the app closes: evidence = `latest.log`. Note the **last** `Horizon renderer:` line.

**7. Metal works.** `latest.log` shows `Horizon renderer: <GPU name> (Metal, ...)`, then
`requesting the device ... with features [...]`, `device ready; configuring the surface (WxH ..., ...)`,
`presenting the first frame`.
- Stops before `device ready`: send those lines (the driver does not offer what the renderer asks for;
  the log says what was requested and what the adapter offers). If a line says "retrying without the optional features", note it.
- `No Metal graphics device found`: send the log lines around it.

**8. The title menu appears and a tap works.** After loading you see the title menu, and the toolbar down the left edge (Back, Shift, Right,
Up, Down, Keys). Tap *Singleplayer*.
- The menu is there but taps do nothing: touch -> pointer events are not reaching it (send a note and the log).
- The picture is wrong (colours washed out, upside down, stretched, or under the Dynamic Island): send a screenshot.

### In a world

**9. Create a world with the keyboard.** In *Create New World* tap the name field, tap **Keys**: the on-screen keyboard appears; tap letters,
*Done*. The name changes. Create the world.
- The keyboard does not appear or taps on keys do nothing: send a screenshot.

**10. The world loads and draws.** You see terrain, the HUD (hotbar at the bottom) and the touch controls: stick base at the bottom left, four
buttons (Break, Place, Jump, Sneak) bottom right, Menu/Chat at the top left, Inv/Drop at the top right. Note the frame rate (turn on the
overlay: Options, Video, Overlay "fps").
- Black or garbage terrain, or a crash on load: send `latest.log` and the crash report.
- Controls missing or off the screen (or under the Dynamic Island / home indicator): send a screenshot and your screen size and orientation.

**11. Move and look.** Left thumb on the left side drags the stick (the player walks, sprints when you push it to the top). Drag on the right
side to look. **Jump** jumps.
- Walking works but looking does not (or the reverse): note which.

**12. Break and place.** Hold **Break** on a block: it breaks (cracks, then drops an item that you pick up). **Place** puts a block.
- Report if holding does not keep breaking, or placing needs several taps.

**13. Hotbar and inventory.** Tap hotbar slots; swipe along the hotbar. **Inv** opens the inventory: tap a stack, tap an empty slot (it moves).
Toggle **Right** and tap a stack (splits it), toggle **Shift** and tap (moves it across). **Back** closes it.
- Report which of these does not work.

**14. Chat.** Tap **Chat**; the on-screen keyboard opens. Type `/time set day`, *Done*. The command runs. (Close the keyboard with **Keys** or *Back*.)

**15. Pause.** **Menu** opens the pause menu; tap *Back to Game*; tap *Options*, move a slider, scroll a list with **Up**/**Down**.
(There is no system back gesture on iOS; the toolbar's **Back** is the only Escape.)

### The iOS lifecycle

**16. Home and back.** In the world, press Home (swipe up), wait ten seconds, come back via the app switcher. The game is paused (or running) and **draws again**,
the touch controls work, the world is where you left it.
- Frozen or black: send `latest.log` from before pressing Home (look for `the window's surface was destroyed` and `is back`).

**17. Rotate.** Turn the phone: the picture re-fits and the controls re-arrange, with no restart. Try it on the pause menu too.

**18. Lock and unlock.** Lock the phone in the world, wait ten seconds, unlock: the game draws again and the world is where you left it
(the same surface-destroyed/back path as step 16, from the lock screen instead of Home).

**19. Quit and resume.** Pause menu, *Save and Quit to Title*, then swipe Home and open the app again from the launcher:
the game starts at the title menu and the world is in the list with your changes.

### Keyboard

**20. Bluetooth keyboard.** Pair it; in the world WASD moves, Space jumps, E opens the inventory, T chats and typing works, Escape pauses.
- Untested on iOS: report whether keys arrive at all before anything else.

**21. Mouse: not applicable on iPhone.** The iPhone has no mouse or trackpad, and the game has no pointer to capture there.
iPad trackpad/mouse support is a follow-up (the `platform` pointer queue exists for it).

### Walking around

**22. Five minutes in a world.** Set the render distance to 8 and then 12 (Options, Video); walk, fly (creative), build, go into a cave; let it rain.
Note: the frame rate, whether it gets hot, whether chunks pop in late.
- Evidence: the fps overlay values and `latest.log` (look for `slow frame`, `video memory is full`, warnings from wgpu).
- The thermal cap is **not** fed on iOS in v1, so the frame limiter does not drop as the phone warms; note how hot it gets instead.

**23. Leave the game running ten minutes in your hand.** Report how the phone feels, the battery drop, and any crash or the app being killed.

## Report template

```
Step: <number and title>
Phone / iOS version: <e.g. iPhone 13 Pro, iOS 27>
Screen: <portrait / landscape>
What I saw:
Evidence: <latest.log, a screenshot>
```

What the logs mean:

| Line | Meaning |
| --- | --- |
| `Horizon for iOS ... starting; game folder ...` | the static library started (step 4 passed) |
| `Horizon renderer: ...` | the renderer's progress; a crash in the driver leaves the last one |
| `Horizon: the window's surface was destroyed` / `... is back (WxH)` | the surface lifecycle (steps 16-18) |
| `Horizon: touch control: ...` | an action of the touch controls failed |
| `Horizon: no sound: ...` | audio could not start (the game goes on silent) |
