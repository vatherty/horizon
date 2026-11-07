# Touch HUD

The [generated reference](touch-hud-reference.svg) is presented on a 2532×1170
landscape canvas. Its [original generated artwork](touch-hud-reference.png) is
preserved at the image generator's native resolution (1846×852).

The implementation uses the reference's translucent charcoal plates, pale borders,
white pixel icons, circular movement stick, right-side action cluster, top-right
Chat/Camera/Pause, and Swap/Pick + Inventory/Drop beside all nine hotbar slots.
Captions use translation keys. Touch remains automatic on iOS/Android; desktop
previewing can enable it with `HORIZON_TOUCH=1`.

## World interactions

| Control | Existing input behavior |
| --- | --- |
| Movement stick | Analog movement; full forward push also sprints |
| Clear world area | Drag to look; optional tap to use, hold to mine |
| Mine | Hold to attack/mine |
| Use | Hold to place, interact, consume, charge, block, or use the held item |
| Jump | Jump, swim up, mount jump, start gliding; double tap toggles permitted flight |
| Sneak | Sneak/dismount; respects the player's toggle-crouch option |
| Sprint | Sprint while held; respects the toggle-sprint option |
| Fly / Fly up / Fly down | Fly synthesizes a timed double-jump; hold ascent/descent. Descent remains held input even with toggle-crouch enabled |
| Inventory | Player, creative, container and ridden mount inventories use existing screens |
| Chat / Camera / Pause | Existing chat + touch keyboard, perspective cycle, pause menu |
| Drop | Drop one; Ctrl + Drop drops the stack |
| Hotbar 1–9 | Select slots; spectator menu and creative saved-toolbar shortcuts follow existing digit handlers |
| Pick / Swap | Pick block/entity and swap hands; Ctrl + Pick includes creative block data |
| Save bar / Load bar | Hold then tap a hotbar slot to save/load that creative toolbar |
| Actions / Emote / Players | Datapack quick actions, enabled emote wheel, held player list |
| Up / Down | Wheel behavior, including spectator selection/speed |

The look surface remains clear of drawn plates. Action buttons and look can be
held with the movement stick simultaneously. Shared actions release only after
the last owning finger leaves. Focus loss, cancellation, and screen changes
release touch input and clear armed inventory tools.

## Inventory interactions

Normal taps pick up/place stacks. Dragging distributes a carried stack. Double
tapping gathers matching items. Every actual inventory screen, including crafting,
containers, creative tabs, recipe books and stations, retains its existing touch
pointer handling.

- **Move** latches Shift for quick-move taps and matching-stack double taps.
- **Split** arms right-click for the next tap or drag (half-stack/single-item distribution).
- **Fill** arms middle-click for creative clone/fill dragging.
- **Ctrl** latches the control modifier without overwriting the physical keyboard state.
- **Swap, Drop, Stack, Clone, Collect and 1–9** are action-first tools: tap the tool,
  then the target slot. The highlighted tool consumes that tap without first picking
  up the slot's item. Tap the tool again to cancel. Clone and Collect use the same
  menu operations as mouse input; they preserve game-mode and server restrictions.
- **Back, Up, Down, Keys** close screens, scroll lists, and expose the existing touch
  keyboard for search, chat, signs, books, and other text fields.

Inventory tools use side rails in landscape and a top grid in portrait, avoiding
the centered recipe book and inventory. Generic menus show only navigation tools.

## Comparison with the reference

[Landscape HUD preview](touch-hud-preview.png) ·
[Inventory preview](touch-menu-preview.png) ·
[Portrait inventory preview](touch-portrait-menu-preview.png)

These are production-layout renders: the preview compiles the actual control
arrangement and actual HUD painter. It substitutes a monospace font and numbered
hotbar placeholders over a neutral backdrop. It is **not a live game screenshot**.

The primary grouping and icon/plate treatment follow the reference. Deliberate
remaining differences: the native nine-slot hotbar is narrower (29% rather than
about 40% of this phone's width), survival indicators stay in the engine's existing
bottom HUD, and an additional utility row exposes the less frequent interactions.
The full bottom HUD, including item models and touch hit areas, is lifted by one
shared safe-area inset. Native desktop HUD geometry is unchanged.

A live world/iPhone comparison still requires prepared Minecraft resources and an
Apple build/device; neither is available in this Linux workspace. No world,
inventory, movement, combat, or server simulation logic was changed.

## Reproduce validation

```sh
rustc --edition=2024 --test crates/horizon-client/src/touch.rs -o /tmp/touch-tests
/tmp/touch-tests
cargo test -p horizon-client-input --no-default-features --lib held::tests
cargo clippy -p horizon-client --no-default-features --lib --bin horizon-client -- -D warnings
cargo fmt --all -- --check
python3 tools/check_public_tree.py

rustc --edition=2024 tools/preview_touch_hud.rs -o /tmp/preview-touch
/tmp/preview-touch /tmp/touch.svg
/tmp/preview-touch /tmp/inventory.svg --menu
/tmp/preview-touch /tmp/portrait.svg --menu --size 1170x2532
/tmp/preview-touch /tmp/small.svg --size 1334x750 --density 2
```

Tests cover phone safe areas, landscape/tablet/portrait geometry, complete control
exposure, menu action targeting, flight timing, movement/look together, overlapping
mining holds, right/middle dragging, cancellation and input ownership.

Validation in this workspace: **41 touch tests and 3 input-ownership tests passed**;
client Clippy with warnings denied, workspace rustfmt, `git diff --check`, and the
public-tree check passed. The production previews were rendered in Chromium at
2532×1170 and 1170×2532; the smaller landscape layout was also inspected.
