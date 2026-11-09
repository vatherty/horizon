//! Touch controls for phones and tablets (Android and iOS): a floating left-thumb stick
//! for movement, right-side drag to look, buttons for jump, sneak, break (hold
//! to break) and place, a tappable and swipeable hotbar, inventory, chat and pause
//! buttons, and in menus a pointer from the first finger plus a small toolbar
//! (back, shift, right click, scroll, keyboard).
//!
//! The world itself takes fingers too, the way Bedrock does: a quick tap on the
//! world uses the item in hand at the crosshair, and holding a finger on the
//! world breaks what the crosshair points at, for as long as the finger stays
//! down — dragging while holding turns the view and the mining follows it. A
//! drag that sets off before the hold lands is only a look, never a break.
//!
//! This is pure logic: fingers go in (`touch`), [`Action`]s come out for the game
//! loop to carry out, and [`TouchControls::view`] says what to draw. It knows
//! nothing of winit or the game, so it is tested on any machine. All sizes in the
//! [`Layout`] are in dp (density-independent pixels, 160 per inch); the screen it
//! is arranged for is in physical pixels, like the touch positions.
//!
//! The layout is data, not code: change [`Layout::default`] or build another one
//! (the game picks it in `game/touch.rs`) and everything is arranged from it, on
//! the cover screen of a foldable (short and wide, or tall and narrow) and on the
//! unfolded one alike.

/// Hotbar slots.
pub const HOTBAR_SLOTS: usize = 9;

/// Seconds a finger must stay on the world, without a big drag, before it
/// starts breaking what the crosshair points at. A lift sooner is a tap.
pub const HOLD_TO_BREAK: f64 = 0.25;

/// How far (dp) a finger on the world may wander from where it went down
/// before it counts as a look drag rather than a press on the world.
pub const WORLD_DRAG: f32 = 12.0;

/// An on-screen button.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    // In the world.
    Jump,
    Sneak,
    Sprint,
    FlyUp,
    FlyDown,
    /// Double-jump using the existing flight gesture.
    FlyToggle,
    PickBlock,
    SwapHands,
    Perspective,
    SaveToolbar,
    LoadToolbar,
    QuickActions,
    Emote,
    PlayerList,
    /// Attack and mine: held down it keeps mining.
    Break,
    /// Place a block or use the item in hand: held down it keeps using it.
    Place,
    Inventory,
    Chat,
    Pause,
    /// Throws the item in hand.
    Drop,
    // In menus.
    /// What Escape does.
    Back,
    /// Holds Shift for the next clicks (quick-move in inventories).
    Shift,
    /// Sticky Control modifier, also available for picking block data in the world.
    Control,
    MenuHotbar(usize),
    MenuSwapHands,
    MenuDrop,
    MenuDropStack,
    MenuClone,
    MenuCollect,
    /// Makes the next drag a middle click (creative fill distribution).
    MenuMiddleClick,
    /// Makes the next tap a right click (split a stack).
    RightClick,
    ScrollUp,
    ScrollDown,
    /// Opens the on-screen keyboard over a text field.
    Keyboard,
}

impl Button {
    /// Translation key for the compact caption below the icon.
    pub fn label_key(self) -> &'static str {
        match self {
            Button::Jump => "horizon.touch.jump",
            Button::Sneak => "horizon.touch.sneak",
            Button::Sprint => "horizon.touch.sprint",
            Button::FlyUp => "horizon.touch.fly_up",
            Button::FlyDown => "horizon.touch.fly_down",
            Button::FlyToggle => "horizon.touch.fly",
            Button::PickBlock => "horizon.touch.pick",
            Button::SwapHands | Button::MenuSwapHands => "horizon.touch.swap",
            Button::Perspective => "horizon.touch.camera",
            Button::SaveToolbar => "horizon.touch.save_toolbar",
            Button::LoadToolbar => "horizon.touch.load_toolbar",
            Button::QuickActions => "horizon.touch.quick_actions",
            Button::Emote => "horizon.touch.emote",
            Button::PlayerList => "horizon.touch.players",
            Button::Break => "horizon.touch.mine",
            Button::Place => "horizon.touch.use",
            Button::Inventory => "horizon.touch.inventory",
            Button::Chat => "horizon.touch.chat",
            Button::Pause => "horizon.touch.pause",
            Button::Drop | Button::MenuDrop => "horizon.touch.drop",
            Button::Back => "horizon.touch.back",
            Button::Shift => "horizon.touch.quick_move",
            Button::Control => "horizon.touch.control",
            Button::RightClick => "horizon.touch.split",
            Button::MenuMiddleClick => "horizon.touch.fill",
            Button::ScrollUp => "horizon.touch.scroll_up",
            Button::ScrollDown => "horizon.touch.scroll_down",
            Button::Keyboard => "horizon.touch.keyboard",
            Button::MenuDropStack => "horizon.touch.drop_stack",
            Button::MenuClone => "horizon.touch.clone",
            Button::MenuCollect => "horizon.touch.collect",
            Button::MenuHotbar(0) => "horizon.touch.slot_1",
            Button::MenuHotbar(1) => "horizon.touch.slot_2",
            Button::MenuHotbar(2) => "horizon.touch.slot_3",
            Button::MenuHotbar(3) => "horizon.touch.slot_4",
            Button::MenuHotbar(4) => "horizon.touch.slot_5",
            Button::MenuHotbar(5) => "horizon.touch.slot_6",
            Button::MenuHotbar(6) => "horizon.touch.slot_7",
            Button::MenuHotbar(7) => "horizon.touch.slot_8",
            Button::MenuHotbar(_) => "horizon.touch.slot_9",
        }
    }
}

/// What the touch controls are for right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Walking around: stick, look, buttons, hotbar.
    Playing,
    /// An inventory or creative screen: pointer, slot actions and modifiers.
    Inventory,
    /// Any other screen: pointer, back, scrolling and keyboard.
    Menu,
}

/// A finger's life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// What the game should do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// The movement stick, `[left, forward]` like a controller's, length at most 1.
    Move([f32; 2]),
    /// Turn the view by a drag, in dp (down and right are positive).
    Look {
        dx: f32,
        dy: f32,
    },
    /// Sprint on or off (the stick pushed all the way forward).
    Sprint(bool),
    /// A button went down. Menu toolbar buttons act on this alone.
    Press(Button),
    Release(Button),
    /// Select this hotbar slot (0..9).
    SelectSlot(usize),
    /// Move the selection by this many slots.
    StepSlot(i32),
    /// The menu pointer is here, in physical pixels.
    PointerMove {
        x: f32,
        y: f32,
    },
    /// The menu pointer pressed or released a button.
    PointerButton {
        right: bool,
        down: bool,
    },
    /// The Shift toggle changed.
    Shift(bool),
    Control(bool),
    /// Middle-button menu drag.
    PointerMiddle(bool),
    /// Scroll a menu by this many lines (positive: up, like the wheel).
    Scroll(f32),
}

/// An axis-aligned rectangle in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn centre(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    fn inflate(&self, by: f32) -> Rect {
        Rect {
            x: self.x - by,
            y: self.y - by,
            w: self.w + 2.0 * by,
            h: self.h + 2.0 * by,
        }
    }
}

/// How the controls are laid out and how they feel. Sizes in dp.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// A button's edge as a fraction of the screen's shorter side, kept between the
    /// two limits that follow: big on a tablet, still fitting a phone's short side.
    pub button_fraction: f32,
    pub button_min: f32,
    pub button_max: f32,
    /// Space between the buttons and the screen's edge, and between buttons.
    pub margin: f32,
    pub gap: f32,
    /// Radius of the movement stick.
    pub stick_radius: f32,
    /// Where a touch starts the stick instead of a look: the left part of the screen
    /// (a fraction of its width), below the top quarter.
    pub stick_zone: f32,
    /// Stick travel (0..1) that counts as standing still.
    pub deadzone: f32,
    /// Pushing the stick this far forward sprints.
    pub sprint_push: f32,
    /// A finger that moves less than this has not moved yet.
    pub slop: f32,
    /// How far above the hotbar a tap still counts as on it.
    pub hotbar_reach: f32,
    /// Seconds between scroll steps while a scroll button is held.
    pub scroll_repeat: f32,
    /// Mouse counts per dp of a look drag: with 1.6 and the default Mouse Sensitivity, a drag
    /// across a 400 dp wide screen turns the view about 100 degrees.
    pub look_gain: f32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            button_fraction: 0.16,
            button_min: 48.0,
            button_max: 72.0,
            margin: 20.0,
            gap: 10.0,
            stick_radius: 64.0,
            stick_zone: 0.42,
            deadzone: 0.14,
            sprint_push: 0.97,
            slop: 2.0,
            hotbar_reach: 14.0,
            scroll_repeat: 0.12,
            look_gain: 1.6,
        }
    }
}

impl Layout {
    /// The file in the game folder that overrides the layout, a line per setting as
    /// `key:value`. Unknown lines and unreadable values are skipped; every value is kept
    /// within a sensible range, so a typo cannot make the controls unusable.
    pub const FILE: &'static str = "horizon-touch.txt";

    pub fn load(dir: &std::path::Path) -> Self {
        std::fs::read_to_string(dir.join(Self::FILE))
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Self {
        let mut layout = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let Ok(v) = value.trim().parse::<f32>() else {
                continue;
            };
            if !v.is_finite() {
                continue;
            }
            match key.trim() {
                "buttonSize" => layout.button_fraction = v.clamp(0.05, 0.4),
                "buttonMin" => layout.button_min = v.clamp(24.0, 120.0),
                "buttonMax" => layout.button_max = v.clamp(24.0, 160.0),
                "margin" => layout.margin = v.clamp(0.0, 80.0),
                "gap" => layout.gap = v.clamp(0.0, 40.0),
                "stickRadius" => layout.stick_radius = v.clamp(30.0, 160.0),
                "stickZone" => layout.stick_zone = v.clamp(0.2, 0.7),
                "deadzone" => layout.deadzone = v.clamp(0.0, 0.5),
                "sprintPush" => layout.sprint_push = v.clamp(0.6, 1.5),
                "lookGain" => layout.look_gain = v.clamp(0.2, 8.0),
                _ => {}
            }
        }
        // The size limits stay in order whatever the file says.
        layout.button_max = layout.button_max.max(layout.button_min);
        layout
    }
}

/// The screen's edges that are not safe to draw controls on, in physical
/// pixels: a notch or Dynamic Island, the home indicator, rounded corners.
/// Controls keep at least this far from each edge, on top of the layout's
/// own margin where that is bigger.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// The screen the controls are arranged for. Physical pixels, except `density`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub width: f32,
    pub height: f32,
    /// Pixels per dp.
    pub density: f32,
    /// Where the game draws its hotbar.
    pub hotbar: Rect,
    /// The width of one hotbar slot.
    pub slot: f32,
    /// The unsafe edges of this screen.
    pub insets: Insets,
}

impl Screen {
    /// A screen without a hotbar to avoid (tests, and menus).
    pub fn plain(width: f32, height: f32, density: f32) -> Self {
        Screen {
            width,
            height,
            density,
            hotbar: Rect {
                x: width / 2.0,
                y: height,
                w: 0.0,
                h: 0.0,
            },
            slot: 20.0 * density,
            insets: Insets::default(),
        }
    }
}

/// A point in the window's full pixel space (the "panel"), mapped into the
/// space of a screen laid out at `screen` size. The two spaces differ where
/// the surface spans less than the window: on iOS the surface covers only
/// the safe area inside the panel, the window reports fingers in panel
/// pixels, and the surface is stretched over the panel when presented — so
/// a finger and the control it is seen on meet only after this mapping.
/// Where the spaces agree (a desktop window, Android) it is the identity.
pub fn panel_point(panel: (f32, f32), screen: (f32, f32), x: f32, y: f32) -> (f32, f32) {
    let axis = |full: f32, laid: f32, v: f32| if full > 0.0 { v * laid / full } else { v };
    (axis(panel.0, screen.0, x), axis(panel.1, screen.1, y))
}

impl Insets {
    /// The safe-area insets of a window whose safe area starts at `position`
    /// and spans `inner` inside a full window of `outer` (all panel pixels),
    /// expressed in a layout space of `screen` size: what the panel extends
    /// past the safe area on each side, scaled into the space with
    /// [`panel_point`]. `None` when the numbers describe no sane safe area:
    /// an inset pair eating half the panel is the platform reporting
    /// positions, not insets.
    pub fn from_panel(
        position: (f32, f32),
        inner: (f32, f32),
        outer: (f32, f32),
        screen: (f32, f32),
    ) -> Option<Insets> {
        let panel = Insets {
            left: position.0.max(0.0),
            top: position.1.max(0.0),
            right: (outer.0 - position.0 - inner.0).max(0.0),
            bottom: (outer.1 - position.1 - inner.1).max(0.0),
        };
        if panel.left + panel.right >= outer.0 / 2.0 || panel.top + panel.bottom >= outer.1 / 2.0 {
            return None;
        }
        let (left, top) = panel_point(outer, screen, panel.left, panel.top);
        let (right, bottom) = panel_point(outer, screen, panel.right, panel.bottom);
        Some(Insets {
            left,
            top,
            right,
            bottom,
        })
    }
}

/// Where everything is, for one screen.
#[derive(Clone, Debug, PartialEq)]
struct Arrangement {
    buttons: Vec<(Button, Rect)>,
    /// The stick's resting place and radius.
    home: (f32, f32),
    radius: f32,
}

fn arrange(layout: &Layout, screen: Screen, mode: Mode) -> Arrangement {
    let dp = screen.density.max(0.1);
    let short = screen.width.min(screen.height) / dp;
    let b = (short * layout.button_fraction).clamp(layout.button_min, layout.button_max) * dp;
    let m = layout.margin * dp;
    // Each edge keeps the layout's margin or the screen's unsafe inset,
    // whichever reaches further.
    let m_left = m.max(screen.insets.left.max(0.0));
    let m_top = m.max(screen.insets.top.max(0.0));
    let m_right = m.max(screen.insets.right.max(0.0));
    let m_bottom = m.max(screen.insets.bottom.max(0.0));
    let g = layout.gap * dp;
    let small = b * 0.78;
    let radius = (layout.stick_radius * dp).max(b);
    let hotbar = screen.hotbar;
    let right = screen.width - m_right;
    let bottom = screen.height - m_bottom;
    let mut home = (m_left + radius, bottom - radius);
    let mut buttons = Vec::new();
    let rect = |x, y, w, h| Rect { x, y, w, h };
    match mode {
        Mode::Playing => {
            // The reference's bottom row flanks the nine-slot bar. On narrower
            // screens it becomes a centred row immediately above that bar.
            let side_width = 2.0 * small + g;
            let beside = hotbar.w > 0.0
                && hotbar.x - g - side_width >= m_left
                && hotbar.x + hotbar.w + g + side_width <= right;
            let utility_y = if beside || hotbar.w <= 0.0 {
                bottom - small
            } else {
                hotbar.y - g - small
            };
            let left = if beside {
                hotbar.x - g - side_width
            } else {
                (m_left + (right - m_left - 4.0 * small - 3.0 * g) / 2.0).max(m_left)
            };
            for (i, button) in [Button::SwapHands, Button::PickBlock]
                .into_iter()
                .enumerate()
            {
                buttons.push((
                    button,
                    rect(left + i as f32 * (small + g), utility_y, small, small),
                ));
            }
            let inventory_x = if beside {
                hotbar.x + hotbar.w + g
            } else {
                left + 2.0 * (small + g)
            };
            buttons.push((
                Button::Inventory,
                rect(inventory_x, utility_y, small, small),
            ));
            buttons.push((
                Button::Drop,
                rect(inventory_x + small + g, utility_y, small, small),
            ));

            // Keep both thumbs above the utility row and the safe-area hotbar.
            let cluster_bottom = utility_y - g;
            let cluster_left = right - 2.0 * b - g;
            buttons.push((Button::Break, rect(cluster_left, cluster_bottom - b, b, b)));
            buttons.push((Button::Place, rect(right - b, cluster_bottom - b, b, b)));
            buttons.push((
                Button::Sneak,
                rect(cluster_left, cluster_bottom - 2.0 * b - g, b, b),
            ));
            buttons.push((
                Button::Jump,
                rect(cluster_left, cluster_bottom - 3.0 * b - 2.0 * g, b, b),
            ));
            buttons.push((
                Button::FlyDown,
                rect(right - small, cluster_bottom - 2.0 * b - g, small, small),
            ));
            buttons.push((
                Button::FlyUp,
                rect(
                    right - small,
                    cluster_bottom - 3.0 * b - 2.0 * g,
                    small,
                    small,
                ),
            ));
            buttons.push((
                Button::FlyToggle,
                rect(
                    cluster_left - small - g,
                    cluster_bottom - 3.0 * b - 2.0 * g,
                    small,
                    small,
                ),
            ));
            let stick_right = m_left + 2.0 * radius;
            let stick_clear = hotbar.w > 0.0 && stick_right + g <= left;
            home.1 = if stick_clear {
                bottom - radius
            } else {
                cluster_bottom - radius
            };
            buttons.push((
                Button::Sprint,
                rect(m_left, home.1 - radius - small - g, small, small),
            ));
            for (i, button) in [Button::Chat, Button::Perspective, Button::Pause]
                .into_iter()
                .enumerate()
            {
                buttons.push((
                    button,
                    rect(
                        right - (3 - i) as f32 * small - (2 - i) as f32 * g,
                        m_top,
                        small,
                        small,
                    ),
                ));
            }
            // Less frequent vanilla actions have their own compact utility row,
            // below the status area. Wrapping keeps portrait layouts usable too.
            let extras = [
                Button::Control,
                Button::SaveToolbar,
                Button::LoadToolbar,
                Button::QuickActions,
                Button::Emote,
                Button::PlayerList,
                Button::ScrollUp,
                Button::ScrollDown,
            ];
            let extra_x = m_left + small + g;
            let cols =
                (((right - extra_x + g) / (small + g)).floor() as usize).clamp(1, extras.len());
            for (i, button) in extras.into_iter().enumerate() {
                buttons.push((
                    button,
                    rect(
                        extra_x + (i % cols) as f32 * (small + g),
                        m_top + small + g + (i / cols) as f32 * (small + g),
                        small,
                        small,
                    ),
                ));
            }
        }
        Mode::Menu => {
            let menu_small = small.min((screen.height - m_top - m_bottom - 3.0 * g) / 4.0);
            for (i, button) in [
                Button::Back,
                Button::ScrollUp,
                Button::ScrollDown,
                Button::Keyboard,
            ]
            .into_iter()
            .enumerate()
            {
                buttons.push((
                    button,
                    rect(
                        m_left,
                        m_top + i as f32 * (menu_small + g),
                        menu_small,
                        menu_small,
                    ),
                ));
            }
        }
        Mode::Inventory => {
            // Rails sit at the edges, leaving the inventory and its pointer clear.
            // The final row is the nine number-key swaps for the pointed-at slot.
            let menu_small = small.min((screen.height - m_top - m_bottom - 8.0 * g) / 8.0);
            let row_y = bottom - menu_small;
            let left_order = [
                Button::Back,
                Button::Shift,
                Button::RightClick,
                Button::Control,
                Button::ScrollUp,
                Button::ScrollDown,
                Button::Keyboard,
            ];
            let right_order = [
                Button::MenuSwapHands,
                Button::MenuDrop,
                Button::MenuDropStack,
                Button::MenuClone,
                Button::MenuCollect,
                Button::MenuMiddleClick,
            ];
            if screen.height > screen.width {
                // An open recipe book spans the portrait screen's width. Put
                // its controls above the centred inventory instead of alongside
                // its slots, where an edge rail would intercept slot touches.
                let cols = (((right - m_left + g) / (menu_small + g)).floor() as usize).clamp(4, 5);
                let grid_x =
                    (m_left + right - cols as f32 * menu_small - (cols - 1) as f32 * g) / 2.0;
                for (i, button) in left_order.into_iter().chain(right_order).enumerate() {
                    buttons.push((
                        button,
                        rect(
                            grid_x + (i % cols) as f32 * (menu_small + g),
                            m_top + (i / cols) as f32 * (menu_small + g),
                            menu_small,
                            menu_small,
                        ),
                    ));
                }
            } else {
                for (i, button) in left_order.into_iter().enumerate() {
                    buttons.push((
                        button,
                        rect(
                            m_left,
                            m_top + i as f32 * (menu_small + g),
                            menu_small,
                            menu_small,
                        ),
                    ));
                }
                for (i, button) in right_order.into_iter().enumerate() {
                    buttons.push((
                        button,
                        rect(
                            right - menu_small,
                            m_top + i as f32 * (menu_small + g),
                            menu_small,
                            menu_small,
                        ),
                    ));
                }
            }
            let slot_gap = (g / 2.0).min((right - m_left) / 40.0);
            let slot_w = menu_small.min((right - m_left - 8.0 * slot_gap) / 9.0);
            let row_x = (m_left + right - 9.0 * slot_w - 8.0 * slot_gap) / 2.0;
            for slot in 0..HOTBAR_SLOTS {
                buttons.push((
                    Button::MenuHotbar(slot),
                    rect(
                        row_x + slot as f32 * (slot_w + slot_gap),
                        row_y,
                        slot_w,
                        menu_small,
                    ),
                ));
            }
        }
    }

    Arrangement {
        buttons,
        home,
        radius,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Stick,
    Look,
    Button(Button),
    Hotbar,
    Pointer,
    /// A finger the controls have no use for (a second one in a menu).
    Ignored,
}

/// What a finger resting on the world (a `Role::Look` finger in the game)
/// is doing with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum World {
    /// Still deciding: a quick lift is a tap (use), staying down past
    /// [`HOLD_TO_BREAK`] starts breaking, a big drag makes it a look.
    Pending,
    /// A look drag, or a finger world touches no longer apply to: it never
    /// taps or breaks.
    Looking,
    /// Holding `key.attack` on the world: breaking until it lifts.
    Breaking,
}

#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    role: Role,
    last: (f32, f32),
    /// Where it went down.
    start: (f32, f32),
    /// When it went down, in seconds (the world hold counts from here).
    down_at: f64,
    /// Hotbar swipe distance not yet turned into steps.
    swipe: f32,
    /// A look drag has gone past the slop (so small shakes do not turn the view).
    moved: bool,
    world: World,
    /// This finger holds a button it slid onto, not one it landed on: sliding
    /// off lets go of it again (a finger that landed on a button keeps it).
    slid: bool,
}

#[derive(Clone, Copy, Debug)]
struct Stick {
    id: u64,
    anchor: (f32, f32),
    at: (f32, f32),
}

/// What to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub buttons: Vec<ButtonView>,
    pub stick: StickView,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonView {
    pub button: Button,
    pub rect: Rect,
    /// A finger is on it, or a toggle is on.
    pub lit: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StickView {
    /// The base, and whether a finger is using it (else it rests faintly at home).
    pub base: (f32, f32),
    pub radius: f32,
    pub knob: (f32, f32),
    pub active: bool,
}

/// The touch controls' state.
#[derive(Clone, Debug)]
pub struct TouchControls {
    layout: Layout,
    screen: Screen,
    mode: Mode,
    arranged: Arrangement,
    fingers: Vec<Finger>,
    stick: Option<Stick>,
    /// The `Move` last sent.
    moving: [f32; 2],
    sprinting: bool,
    held: Vec<Button>,
    shift: bool,
    control: bool,
    middle_next: bool,
    /// A discrete inventory shortcut waiting for the player to tap its slot.
    pending_menu: Option<Button>,
    pointer_middle: bool,
    /// Next phase of the existing double-jump flight gesture.
    fly_pulse: Option<(u8, f64)>,
    right_next: bool,
    /// The menu pointer's right-click state when it went down.
    pointer_right: bool,
    scroll_at: f64,
    /// Tap and hold on the world itself to use and break (the Touch options).
    world_touch: bool,
}

impl TouchControls {
    pub fn new(layout: Layout, screen: Screen) -> Self {
        let mode = Mode::Playing;
        let arranged = arrange(&layout, screen, mode);
        Self {
            layout,
            screen,
            mode,
            arranged,
            fingers: Vec::new(),
            stick: None,
            moving: [0.0; 2],
            sprinting: false,
            held: Vec::new(),
            shift: false,
            control: false,
            middle_next: false,
            pending_menu: None,
            pointer_middle: false,
            fly_pulse: None,
            right_next: false,
            pointer_right: false,
            scroll_at: 0.0,
            world_touch: true,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The window changed size, or the hotbar moved.
    pub fn set_screen(&mut self, screen: Screen) {
        if screen != self.screen {
            self.screen = screen;
            self.arranged = arrange(&self.layout, screen, self.mode);
        }
    }

    /// Another layout (from the options, say).
    pub fn set_layout(&mut self, layout: Layout) {
        self.layout = layout;
        self.arranged = arrange(&self.layout, self.screen, self.mode);
    }

    /// Whether fingers on the world itself tap to use and hold to break.
    pub fn world_touch(&self) -> bool {
        self.world_touch
    }

    /// Turns world taps and holds on or off (the Touch options). Turning it
    /// off lets go of a world hold at once and spends every pending press:
    /// a finger already down never taps or starts breaking afterwards.
    /// Returns what that releases.
    pub fn set_world_touch(&mut self, on: bool) -> Vec<Action> {
        if self.world_touch == on {
            return Vec::new();
        }
        self.world_touch = on;
        let mut out = Vec::new();
        if !on {
            for i in 0..self.fingers.len() {
                if self.fingers[i].world == World::Breaking {
                    self.fingers[i].world = World::Looking;
                    out.extend(self.release_button(Button::Break));
                } else if self.fingers[i].world == World::Pending {
                    self.fingers[i].world = World::Looking;
                }
            }
        }
        out
    }

    /// Marks a button held and says so.
    fn press_button(&mut self, button: Button, out: &mut Vec<Action>) {
        let first = !self.held.contains(&button);
        self.held.push(button);
        if first {
            out.push(Action::Press(button));
        }
    }

    /// Lets go of one hold on a button, if any.
    fn release_button(&mut self, button: Button) -> Vec<Action> {
        if let Some(at) = self.held.iter().position(|b| *b == button) {
            self.held.remove(at);
            if self.held.contains(&button) {
                Vec::new()
            } else {
                vec![Action::Release(button)]
            }
        } else {
            Vec::new()
        }
    }

    /// Switches between the world's controls and the menus', letting go of
    /// everything held first. Returns what that releases.
    pub fn set_mode(&mut self, mode: Mode) -> Vec<Action> {
        if mode == self.mode {
            return Vec::new();
        }
        let out = self.release_all();
        self.mode = mode;
        self.arranged = arrange(&self.layout, self.screen, mode);
        out
    }

    /// Lets go of every finger, button and the stick.
    pub fn release_all(&mut self) -> Vec<Action> {
        let mut out = Vec::new();
        if self.fingers.iter().any(|f| matches!(f.role, Role::Pointer)) {
            if self.pointer_middle {
                out.push(Action::PointerMiddle(false));
            } else {
                out.push(Action::PointerButton {
                    right: self.pointer_right,
                    down: false,
                });
            }
        }
        self.fingers.clear();
        for button in std::mem::take(&mut self.held) {
            if !out.contains(&Action::Release(button)) {
                out.push(Action::Release(button));
            }
        }
        if self.stick.take().is_some() || self.moving != [0.0; 2] {
            self.moving = [0.0; 2];
            out.push(Action::Move([0.0; 2]));
        }
        if self.sprinting {
            self.sprinting = false;
            out.push(Action::Sprint(false));
        }
        if self.shift {
            self.shift = false;
            out.push(Action::Shift(false));
        }
        if self.control {
            self.control = false;
            out.push(Action::Control(false));
        }
        if self.fly_pulse.take().is_some() {
            out.extend(self.release_button(Button::Jump));
        }
        self.right_next = false;
        self.middle_next = false;
        self.pending_menu = None;
        self.pointer_middle = false;
        out
    }

    /// The stick as the game's move vector, while a finger holds it off centre.
    pub fn move_vector(&self) -> Option<[f32; 2]> {
        (self.stick.is_some() && self.moving != [0.0; 2]).then_some(self.moving)
    }

    /// Whether any finger is on the controls.
    pub fn active(&self) -> bool {
        !self.fingers.is_empty()
    }

    pub fn control(&self) -> bool {
        self.control
    }

    pub fn shift(&self) -> bool {
        self.shift
    }

    /// What to draw for the current mode.
    pub fn view(&self) -> View {
        let buttons = self
            .arranged
            .buttons
            .iter()
            .map(|&(button, rect)| ButtonView {
                button,
                rect,
                lit: self.held.contains(&button)
                    || (button == Button::Shift && self.shift)
                    || (button == Button::RightClick && self.right_next)
                    || (button == Button::Control && self.control)
                    || (button == Button::MenuMiddleClick && self.middle_next)
                    || self.pending_menu == Some(button),
            })
            .collect();
        let radius = self.arranged.radius;
        let stick = match self.stick {
            Some(s) => {
                let (dx, dy) = (s.at.0 - s.anchor.0, s.at.1 - s.anchor.1);
                let len = dx.hypot(dy);
                let k = if len > radius { radius / len } else { 1.0 };
                StickView {
                    base: s.anchor,
                    radius,
                    knob: (s.anchor.0 + dx * k, s.anchor.1 + dy * k),
                    active: true,
                }
            }
            None => StickView {
                base: self.arranged.home,
                radius,
                knob: self.arranged.home,
                active: false,
            },
        };
        View { buttons, stick }
    }

    fn button_at(&self, x: f32, y: f32) -> Option<Button> {
        let reach = self.layout.gap * self.screen.density / 2.0;
        self.arranged
            .buttons
            .iter()
            .find(|(_, r)| r.inflate(reach).contains(x, y))
            .map(|&(b, _)| b)
    }

    fn on_hotbar(&self, x: f32, y: f32) -> bool {
        let hb = self.screen.hotbar;
        if hb.w <= 0.0 {
            return false;
        }
        let reach = self.layout.hotbar_reach * self.screen.density;
        Rect {
            x: hb.x,
            y: hb.y - reach,
            w: hb.w,
            h: hb.h + reach + self.screen.density * 40.0,
        }
        .contains(x, y)
    }

    /// The slot under a horizontal position, from the hotbar's left edge (vanilla's
    /// slots are 20 GUI pixels apart, the first starting 1 pixel in).
    fn slot_at(&self, x: f32) -> usize {
        let unit = self.screen.slot / 20.0;
        let slot = ((x - self.screen.hotbar.x) / unit - 1.0) / 20.0;
        (slot.floor().max(0.0) as usize).min(HOTBAR_SLOTS - 1)
    }

    fn in_stick_zone(&self, x: f32, y: f32) -> bool {
        x < self.screen.width * self.layout.stick_zone && y > self.screen.height * 0.25
    }

    /// A finger touched, moved or lifted. `now` is in seconds.
    pub fn touch(&mut self, id: u64, phase: Phase, x: f32, y: f32, now: f64) -> Vec<Action> {
        match phase {
            Phase::Started => self.started(id, x, y, now),
            Phase::Moved => self.moved(id, x, y),
            // A cancelled touch is a lift, but never a tap.
            Phase::Ended => self.ended(id, true, now),
            Phase::Cancelled => self.ended(id, false, now),
        }
    }

    fn started(&mut self, id: u64, x: f32, y: f32, now: f64) -> Vec<Action> {
        // A finger id reused without an end (a lost event): start it afresh.
        let mut out = if self.fingers.iter().any(|f| f.id == id) {
            self.ended(id, false, now)
        } else {
            Vec::new()
        };
        let mut role = Role::Look;
        match self.mode {
            Mode::Playing => {
                if let Some(button) = self.button_at(x, y) {
                    role = Role::Button(button);
                    if button == Button::Control {
                        self.control = !self.control;
                        out.push(Action::Control(self.control));
                    } else if button == Button::FlyToggle {
                        if self.fly_pulse.is_none() {
                            self.press_button(Button::Jump, &mut out);
                            self.fly_pulse = Some((1, now + 0.08));
                        }
                    } else if matches!(button, Button::ScrollUp | Button::ScrollDown) {
                        out.extend(self.menu_button(button, now));
                    } else {
                        self.press_button(button, &mut out);
                    }
                } else if self.on_hotbar(x, y) {
                    role = Role::Hotbar;
                    out.push(Action::SelectSlot(self.slot_at(x)));
                } else if self.stick.is_none() && self.in_stick_zone(x, y) {
                    role = Role::Stick;
                    let r = self.arranged.radius;
                    let anchor = (
                        x.clamp(
                            r.min(self.screen.width / 2.0),
                            (self.screen.width - r).max(r),
                        ),
                        y.clamp(
                            r.min(self.screen.height / 2.0),
                            (self.screen.height - r).max(r),
                        ),
                    );
                    self.stick = Some(Stick {
                        id,
                        anchor,
                        at: (x, y),
                    });
                }
            }
            Mode::Menu | Mode::Inventory => {
                if let Some(button) = self.button_at(x, y) {
                    role = Role::Button(button);
                    out.extend(self.menu_button(button, now));
                } else if self.fingers.iter().any(|f| matches!(f.role, Role::Pointer)) {
                    role = Role::Ignored;
                } else if let Some(button) = self.pending_menu.take() {
                    // Target the slot without picking its item up first. Toolbar
                    // activation alone cannot hover a slot on a touch screen.
                    role = Role::Ignored;
                    out.push(Action::PointerMove { x, y });
                    out.push(Action::Press(button));
                    out.push(Action::Release(button));
                } else {
                    role = Role::Pointer;
                    self.pointer_right = self.right_next;
                    self.pointer_middle = self.middle_next;
                    out.push(Action::PointerMove { x, y });
                    if self.pointer_middle {
                        out.push(Action::PointerMiddle(true));
                    } else {
                        out.push(Action::PointerButton {
                            right: self.pointer_right,
                            down: true,
                        });
                    }
                }
            }
        }
        self.fingers.push(Finger {
            id,
            role,
            last: (x, y),
            start: (x, y),
            down_at: now,
            swipe: 0.0,
            moved: false,
            world: if role == Role::Look && self.world_touch {
                World::Pending
            } else {
                World::Looking
            },
            slid: false,
        });
        out
    }

    fn menu_button(&mut self, button: Button, now: f64) -> Vec<Action> {
        match button {
            Button::MenuHotbar(_)
            | Button::MenuSwapHands
            | Button::MenuDrop
            | Button::MenuDropStack
            | Button::MenuClone
            | Button::MenuCollect => {
                self.pending_menu = if self.pending_menu == Some(button) {
                    None
                } else {
                    Some(button)
                };
                self.right_next = false;
                self.middle_next = false;
                Vec::new()
            }
            Button::Shift => {
                self.shift = !self.shift;
                vec![Action::Shift(self.shift)]
            }
            Button::Control => {
                self.control = !self.control;
                vec![Action::Control(self.control)]
            }
            Button::RightClick => {
                self.pending_menu = None;
                self.right_next = !self.right_next;
                self.middle_next = false;
                Vec::new()
            }
            Button::MenuMiddleClick => {
                self.pending_menu = None;
                self.middle_next = !self.middle_next;
                self.right_next = false;
                Vec::new()
            }
            Button::ScrollUp => {
                self.scroll_at = now + 0.4;
                vec![Action::Scroll(1.0)]
            }
            Button::ScrollDown => {
                self.scroll_at = now + 0.4;
                vec![Action::Scroll(-1.0)]
            }
            other => {
                let mut out = Vec::new();
                self.press_button(other, &mut out);
                out
            }
        }
    }

    fn moved(&mut self, id: u64, x: f32, y: f32) -> Vec<Action> {
        let dp = self.screen.density.max(0.1);
        let slop = self.layout.slop * dp;
        let slot = self.screen.slot.max(1.0);
        let Some(i) = self.fingers.iter().position(|f| f.id == id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        match self.fingers[i].role {
            Role::Look => {
                // Sliding onto a button claims the finger for that button
                // (the stick finger never has this role, and menus keep
                // their pointer). A world hold ends before the press.
                if self.mode == Mode::Playing
                    && let Some(button) = self.button_at(x, y)
                    && matches!(
                        button,
                        Button::Break
                            | Button::Place
                            | Button::Jump
                            | Button::Sneak
                            | Button::Sprint
                            | Button::FlyUp
                            | Button::FlyDown
                    )
                {
                    if self.fingers[i].world == World::Breaking {
                        self.fingers[i].world = World::Looking;
                        out.extend(self.release_button(Button::Break));
                    }
                    self.fingers[i].role = Role::Button(button);
                    self.fingers[i].slid = true;
                    self.fingers[i].last = (x, y);
                    self.press_button(button, &mut out);
                    return out;
                }
                let finger = &mut self.fingers[i];
                let (sx, sy) = (x - finger.start.0, y - finger.start.1);
                // A big drag before the hold lands is a look, never a break.
                if finger.world == World::Pending && sx.hypot(sy) > WORLD_DRAG * dp {
                    finger.world = World::Looking;
                }
                if !finger.moved {
                    let len = sx.hypot(sy);
                    if len < slop {
                        return out;
                    }
                    finger.moved = true;
                    // Only the travel past the slop turns the view, so the
                    // first step is the size of any other, not a lurch. The
                    // slop itself is spent once: later moves count from
                    // where the finger actually is.
                    let keep = (len - slop) / len;
                    out.push(Action::Look {
                        dx: sx * keep / dp,
                        dy: sy * keep / dp,
                    });
                    finger.last = (x, y);
                } else {
                    out.push(Action::Look {
                        dx: (x - finger.last.0) / dp,
                        dy: (y - finger.last.1) / dp,
                    });
                    finger.last = (x, y);
                }
            }
            Role::Hotbar => {
                let finger = &mut self.fingers[i];
                finger.swipe += x - finger.last.0;
                finger.last = (x, y);
                while finger.swipe >= slot {
                    finger.swipe -= slot;
                    out.push(Action::StepSlot(1));
                }
                while finger.swipe <= -slot {
                    finger.swipe += slot;
                    out.push(Action::StepSlot(-1));
                }
            }
            Role::Pointer => {
                self.fingers[i].last = (x, y);
                out.push(Action::PointerMove { x, y });
            }
            Role::Stick => {
                self.fingers[i].last = (x, y);
                if let Some(stick) = self.stick.as_mut().filter(|s| s.id == id) {
                    stick.at = (x, y);
                }
                out.extend(self.stick_update());
            }
            Role::Button(button) => {
                self.fingers[i].last = (x, y);
                // A finger that slid onto its button follows the finger:
                // off it lets go, onto the next one hands over. A finger
                // that landed on its button keeps it wherever it wanders.
                if self.fingers[i].slid {
                    match self.button_at(x, y) {
                        Some(next) if next == button => {}
                        Some(next) => {
                            out.extend(self.release_button(button));
                            self.fingers[i].role = Role::Button(next);
                            self.press_button(next, &mut out);
                        }
                        None => {
                            out.extend(self.release_button(button));
                            let finger = &mut self.fingers[i];
                            finger.role = Role::Look;
                            finger.slid = false;
                            // It has done its pressing: it only looks now.
                            finger.world = World::Looking;
                            finger.moved = true;
                            finger.start = (x, y);
                            finger.last = (x, y);
                        }
                    }
                }
            }
            Role::Ignored => self.fingers[i].last = (x, y),
        }
        out
    }

    /// Recomputes the move vector and sprint after the stick moved.
    fn stick_update(&mut self) -> Vec<Action> {
        let Some(stick) = self.stick else {
            return Vec::new();
        };
        let r = self.arranged.radius.max(1.0);
        let (dx, dy) = (
            (stick.at.0 - stick.anchor.0) / r,
            (stick.at.1 - stick.anchor.1) / r,
        );
        let raw = dx.hypot(dy);
        let len = raw.min(1.0);
        let dead = self.layout.deadzone;
        let vector = if raw <= dead {
            [0.0; 2]
        } else {
            // Rescaled so the vector starts from nothing at the dead zone's edge.
            let k = (len - dead) / (1.0 - dead) / raw;
            // Screen right is the player's right: [left, forward] negates x, and
            // screen up (negative y) is forward.
            [-dx * k, -dy * k]
        };
        let mut out = Vec::new();
        if vector != self.moving {
            self.moving = vector;
            out.push(Action::Move(vector));
        }
        let forward = vector[1];
        let sprint = if self.sprinting {
            forward > 0.5
        } else {
            raw >= self.layout.sprint_push && forward > 0.8
        };
        if sprint != self.sprinting {
            self.sprinting = sprint;
            out.push(Action::Sprint(sprint));
        }
        out
    }

    fn ended(&mut self, id: u64, tap: bool, now: f64) -> Vec<Action> {
        let Some(i) = self.fingers.iter().position(|f| f.id == id) else {
            return Vec::new();
        };
        let finger = self.fingers.remove(i);
        let mut out = Vec::new();
        match finger.role {
            Role::Button(button) => {
                out.extend(self.release_button(button));
            }
            Role::Stick => {
                self.stick = None;
                if self.moving != [0.0; 2] {
                    self.moving = [0.0; 2];
                    out.push(Action::Move([0.0; 2]));
                }
                if self.sprinting {
                    self.sprinting = false;
                    out.push(Action::Sprint(false));
                }
            }
            Role::Pointer => {
                if self.pointer_middle {
                    out.push(Action::PointerMiddle(false));
                    self.middle_next = false;
                } else {
                    out.push(Action::PointerButton {
                        right: self.pointer_right,
                        down: false,
                    });
                    if self.pointer_right {
                        self.right_next = false;
                    }
                }
            }
            Role::Look => {
                if finger.world == World::Breaking {
                    // The hold is over: stop breaking.
                    out.extend(self.release_button(Button::Break));
                } else if tap
                    && finger.world == World::Pending
                    && self.mode == Mode::Playing
                    && self.world_touch
                    && now - finger.down_at < HOLD_TO_BREAK
                {
                    // A tap on the world: use what the crosshair points at.
                    out.push(Action::Press(Button::Place));
                    out.push(Action::Release(Button::Place));
                }
            }
            Role::Hotbar | Role::Ignored => {}
        }
        out
    }

    /// Called every frame: starts a world hold that has been down long
    /// enough, and repeats a held scroll button.
    pub fn update(&mut self, now: f64) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some((phase, at)) = self.fly_pulse
            && now >= at
        {
            if phase == 2 {
                self.press_button(Button::Jump, &mut out);
            } else {
                out.extend(self.release_button(Button::Jump));
            }
            self.fly_pulse = (phase < 3).then_some((phase + 1, now + 0.08));
        }
        if self.mode == Mode::Playing && self.world_touch {
            for i in 0..self.fingers.len() {
                let finger = &self.fingers[i];
                if finger.role == Role::Look
                    && finger.world == World::Pending
                    && now - finger.down_at >= HOLD_TO_BREAK
                {
                    self.fingers[i].world = World::Breaking;
                    self.press_button(Button::Break, &mut out);
                }
            }
        }
        for finger in &self.fingers {
            let Role::Button(button) = finger.role else {
                continue;
            };
            let direction = match button {
                Button::ScrollUp => 1.0,
                Button::ScrollDown => -1.0,
                _ => continue,
            };
            if now >= self.scroll_at {
                self.scroll_at = now + f64::from(self.layout.scroll_repeat);
                out.push(Action::Scroll(direction));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Fold 5's cover screen held sideways: 2316 x 904 px at 2.625 px/dp, with the
    /// hotbar of GUI scale 3 centred at the bottom.
    fn cover() -> Screen {
        let (w, h, s) = (2316.0, 904.0, 3.0);
        Screen {
            width: w,
            height: h,
            density: 2.625,
            hotbar: Rect {
                x: w / 2.0 - 91.0 * s,
                y: h - 22.0 * s,
                w: 182.0 * s,
                h: 22.0 * s,
            },
            slot: 20.0 * s,
            insets: Insets::default(),
        }
    }

    /// The unfolded screen: 2176 x 1812 px, GUI scale 6.
    fn unfolded() -> Screen {
        let (w, h, s) = (2176.0, 1812.0, 6.0);
        Screen {
            width: w,
            height: h,
            density: 2.625,
            hotbar: Rect {
                x: w / 2.0 - 91.0 * s,
                y: h - 22.0 * s,
                w: 182.0 * s,
                h: 22.0 * s,
            },
            slot: 20.0 * s,
            insets: Insets::default(),
        }
    }

    /// The cover screen held upright: 904 x 2316 px.
    fn portrait() -> Screen {
        let (w, h, s) = (904.0, 2316.0, 2.0);
        Screen {
            width: w,
            height: h,
            density: 2.625,
            hotbar: Rect {
                x: w / 2.0 - 91.0 * s,
                y: h - 22.0 * s,
                w: 182.0 * s,
                h: 22.0 * s,
            },
            slot: 20.0 * s,
            insets: Insets::default(),
        }
    }

    fn phone() -> Screen {
        Screen {
            width: 2532.0,
            height: 1170.0,
            density: 3.0,
            hotbar: Rect {
                x: 902.0,
                y: 1012.0,
                w: 728.0,
                h: 88.0,
            },
            slot: 80.0,
            insets: Insets {
                left: 141.0,
                right: 141.0,
                top: 0.0,
                bottom: 63.0,
            },
        }
    }

    /// The same iPhone as the game actually builds it: winit reports the
    /// safe area as the window (inner position (141, 0), inner size
    /// 2250 x 1110) inside the 2532 x 1170 panel, so the controls are laid
    /// out on the 2250 x 1110 surface, with the hotbar the HUD batch of GUI
    /// scale 4 draws for it (bottom inset 15 GUI pixels) and the insets
    /// `Insets::from_panel` makes of the window's numbers.
    fn iphone() -> Screen {
        let panel = (2532.0, 1170.0);
        let surface = (2250.0, 1110.0);
        Screen {
            width: surface.0,
            height: surface.1,
            density: 3.0,
            hotbar: Rect {
                x: 762.0,
                y: 964.0,
                w: 728.0,
                h: 88.0,
            },
            slot: 80.0,
            insets: Insets::from_panel((141.0, 0.0), surface, panel, surface)
                .expect("the iPhone's safe area is sane"),
        }
    }

    /// A tap as it now reaches the controls: the layout point `at` is what
    /// the player sees, stretched onto the panel; the finger is reported in
    /// panel pixels and mapped back into the layout's space at the door.
    fn panel_tap(
        c: &mut TouchControls,
        screen: Screen,
        id: u64,
        at: (f32, f32),
        now: f64,
    ) -> Vec<Action> {
        let panel = (2532.0, 1170.0);
        let seen = (
            at.0 * panel.0 / screen.width,
            at.1 * panel.1 / screen.height,
        );
        let (x, y) = panel_point(panel, (screen.width, screen.height), seen.0, seen.1);
        c.touch(id, Phase::Started, x, y, now)
    }

    fn rect_of(c: &TouchControls, button: Button) -> Rect {
        c.view()
            .buttons
            .iter()
            .find(|b| b.button == button)
            .unwrap()
            .rect
    }

    #[test]
    fn every_control_fits_on_every_screen_and_none_overlap() {
        for screen in [cover(), unfolded(), portrait(), phone()] {
            for mode in [Mode::Playing, Mode::Inventory, Mode::Menu] {
                let mut c = TouchControls::new(Layout::default(), screen);
                c.set_mode(mode);
                let view = c.view();
                for (i, a) in view.buttons.iter().enumerate() {
                    let r = a.rect;
                    assert!(r.x >= 0.0 && r.y >= 0.0, "{:?} off the top left", a.button);
                    assert!(
                        r.x + r.w <= screen.width && r.y + r.h <= screen.height,
                        "{:?} off the screen {screen:?}: {r:?}",
                        a.button
                    );
                    for b in &view.buttons[i + 1..] {
                        let s = b.rect;
                        let apart = r.x + r.w <= s.x
                            || s.x + s.w <= r.x
                            || r.y + r.h <= s.y
                            || s.y + s.h <= r.y;
                        assert!(apart, "{:?} overlaps {:?}", a.button, b.button);
                    }
                    // Nothing sits on the hotbar.
                    let hb = screen.hotbar;
                    if mode == Mode::Playing {
                        let clear = r.x + r.w <= hb.x
                            || hb.x + hb.w <= r.x
                            || r.y + r.h <= hb.y
                            || hb.y + hb.h <= r.y;
                        assert!(clear, "{:?} on the hotbar of {screen:?}", a.button);
                    }
                }
                let stick = view.stick;
                assert!(stick.base.0 - stick.radius >= -0.5, "stick off the left");
                assert!(
                    stick.base.1 + stick.radius <= screen.height + 0.5,
                    "stick off the bottom"
                );
            }
        }
    }

    #[test]
    fn buttons_are_big_enough_for_a_thumb() {
        for screen in [cover(), unfolded(), portrait()] {
            let c = TouchControls::new(Layout::default(), screen);
            let jump = rect_of(&c, Button::Jump);
            let dp = jump.w / screen.density;
            assert!((48.0..=72.0).contains(&dp), "jump is {dp} dp wide");
        }
    }

    #[test]
    fn a_button_presses_on_touch_and_releases_on_lift() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let (x, y) = rect_of(&c, Button::Break).centre();
        assert_eq!(
            c.touch(1, Phase::Started, x, y, 0.0),
            vec![Action::Press(Button::Break)]
        );
        assert!(
            c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::Break && b.lit)
        );
        // Held, dragged off it, still held.
        assert!(
            c.touch(1, Phase::Moved, x - 400.0, y - 100.0, 0.1)
                .is_empty()
        );
        assert_eq!(
            c.touch(1, Phase::Ended, x, y, 0.2),
            vec![Action::Release(Button::Break)]
        );
        assert!(!c.view().buttons.iter().any(|b| b.lit));
    }

    #[test]
    fn two_buttons_at_once() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let jump = rect_of(&c, Button::Jump).centre();
        let brk = rect_of(&c, Button::Break).centre();
        c.touch(1, Phase::Started, jump.0, jump.1, 0.0);
        c.touch(2, Phase::Started, brk.0, brk.1, 0.0);
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.1),
            vec![Action::Release(Button::Jump)]
        );
        assert_eq!(
            c.touch(2, Phase::Ended, 0.0, 0.0, 0.1),
            vec![Action::Release(Button::Break)]
        );
    }

    #[test]
    fn the_stick_floats_where_the_thumb_lands() {
        let mut c = TouchControls::new(Layout::default(), cover());
        // Down in the left zone: nothing yet, the stick is anchored there.
        assert!(c.touch(1, Phase::Started, 400.0, 600.0, 0.0).is_empty());
        let view = c.view();
        assert!(view.stick.active);
        assert_eq!(view.stick.base, (400.0, 600.0));
        // Straight up by half the radius: forward, no strafing.
        let r = view.stick.radius;
        let out = c.touch(1, Phase::Moved, 400.0, 600.0 - r * 0.6, 0.1);
        let Some(Action::Move(v)) = out.first() else {
            panic!("no move in {out:?}");
        };
        assert!(v[0].abs() < 1e-6 && v[1] > 0.3 && v[1] < 0.6, "{v:?}");
        assert_eq!(c.move_vector(), Some(*v));
        // Right of centre strafes right, which is negative in [left, forward].
        let out = c.touch(1, Phase::Moved, 400.0 + r * 0.8, 600.0, 0.2);
        let Some(Action::Move(v)) = out.first() else {
            panic!("no move in {out:?}");
        };
        assert!(v[0] < -0.5 && v[1].abs() < 1e-6, "{v:?}");
        // Lifting stops.
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.3),
            vec![Action::Move([0.0; 2])]
        );
        assert_eq!(c.move_vector(), None);
        assert!(!c.view().stick.active);
    }

    #[test]
    fn small_stick_wobble_is_a_dead_zone() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 400.0, 600.0, 0.0);
        assert!(c.touch(1, Phase::Moved, 405.0, 597.0, 0.1).is_empty());
        assert_eq!(c.move_vector(), None);
    }

    #[test]
    fn pushing_the_stick_all_the_way_forward_sprints_until_it_eases() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 400.0, 600.0, 0.0);
        let r = c.view().stick.radius;
        let out = c.touch(1, Phase::Moved, 400.0, 600.0 - r * 1.2, 0.1);
        assert!(out.contains(&Action::Sprint(true)), "{out:?}");
        // Easing back to half still sprints (hysteresis), a quarter stops.
        let out = c.touch(1, Phase::Moved, 400.0, 600.0 - r * 0.7, 0.2);
        assert!(!out.contains(&Action::Sprint(false)), "{out:?}");
        let out = c.touch(1, Phase::Moved, 400.0, 600.0 - r * 0.3, 0.3);
        assert!(out.contains(&Action::Sprint(false)), "{out:?}");
    }

    #[test]
    fn dragging_on_the_right_looks_after_the_slop() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let dp = 2.625;
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        // A shake under 2 dp turns nothing.
        assert!(c.touch(1, Phase::Moved, 1503.0, 401.0, 0.1).is_empty());
        // Past it: the travel beyond the slop (40 dp of drag turns 38), so
        // the first step is no bigger than the ones that follow.
        let out = c.touch(1, Phase::Moved, 1500.0 + 40.0 * dp, 400.0, 0.2);
        assert_eq!(out.len(), 1);
        match out[0] {
            Action::Look { dx, dy } => {
                assert!((dx - 38.0).abs() < 1e-3 && dy.abs() < 1e-3, "{dx} {dy}");
            }
            ref other => panic!("{other:?}"),
        }
        let out = c.touch(1, Phase::Moved, 1500.0 + 50.0 * dp, 400.0 + 10.0 * dp, 0.3);
        match out[0] {
            Action::Look { dx, dy } => {
                assert!(
                    (dx - 10.0).abs() < 1e-3 && (dy - 10.0).abs() < 1e-3,
                    "{dx} {dy}"
                );
            }
            ref other => panic!("{other:?}"),
        }
        assert!(c.touch(1, Phase::Ended, 0.0, 0.0, 0.4).is_empty());
    }

    #[test]
    fn stick_and_look_work_together() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 400.0, 600.0, 0.0);
        c.touch(2, Phase::Started, 1400.0, 400.0, 0.0);
        let r = c.view().stick.radius;
        assert!(!c.touch(1, Phase::Moved, 400.0, 600.0 - r, 0.1).is_empty());
        let out = c.touch(2, Phase::Moved, 1500.0, 400.0, 0.1);
        assert!(matches!(out[0], Action::Look { .. }));
        // Lifting the looking finger leaves the stick going.
        c.touch(2, Phase::Ended, 0.0, 0.0, 0.2);
        assert!(c.move_vector().is_some());
        // A second finger in the stick zone, with the stick taken, looks.
        c.touch(3, Phase::Started, 300.0, 500.0, 0.3);
        let out = c.touch(3, Phase::Moved, 600.0, 500.0, 0.4);
        assert!(matches!(out[0], Action::Look { .. }));
    }

    #[test]
    fn the_hotbar_selects_on_tap_and_steps_on_swipe() {
        let mut c = TouchControls::new(Layout::default(), unfolded());
        let s = unfolded();
        // The middle of slot 4 (the fifth): 1 + 20 * 4 + 10 GUI pixels in.
        let x = s.hotbar.x + (1.0 + 20.0 * 4.0 + 10.0) * 6.0;
        let y = s.hotbar.y + s.hotbar.h / 2.0;
        assert_eq!(
            c.touch(1, Phase::Started, x, y, 0.0),
            vec![Action::SelectSlot(4)]
        );
        // Slide right two and a half slots: two steps.
        let out = c.touch(1, Phase::Moved, x + s.slot * 2.5, y, 0.1);
        assert_eq!(out, vec![Action::StepSlot(1), Action::StepSlot(1)]);
        // And back left past where it began.
        let out = c.touch(1, Phase::Moved, x - s.slot * 1.0, y, 0.2);
        assert_eq!(out, vec![Action::StepSlot(-1); 3]);
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.3);
        // The ends of the hotbar are slots 0 and 8.
        assert_eq!(
            c.touch(1, Phase::Started, s.hotbar.x + 4.0, y, 0.4),
            vec![Action::SelectSlot(0)]
        );
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.5);
        assert_eq!(
            c.touch(1, Phase::Started, s.hotbar.x + s.hotbar.w - 4.0, y, 0.6),
            vec![Action::SelectSlot(8)]
        );
    }

    #[test]
    fn a_tap_just_above_the_hotbar_still_hits_it() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let s = cover();
        let out = c.touch(
            1,
            Phase::Started,
            s.hotbar.x + 200.0,
            s.hotbar.y - 10.0,
            0.0,
        );
        assert!(matches!(out[..], [Action::SelectSlot(_)]), "{out:?}");
    }

    #[test]
    fn leaving_the_world_releases_everything_held() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let brk = rect_of(&c, Button::Break).centre();
        c.touch(1, Phase::Started, brk.0, brk.1, 0.0);
        c.touch(2, Phase::Started, 400.0, 600.0, 0.0);
        let r = c.view().stick.radius;
        c.touch(2, Phase::Moved, 400.0, 600.0 - r * 1.2, 0.1);
        let out = c.set_mode(Mode::Inventory);
        assert!(out.contains(&Action::Release(Button::Break)), "{out:?}");
        assert!(out.contains(&Action::Move([0.0; 2])), "{out:?}");
        assert!(out.contains(&Action::Sprint(false)), "{out:?}");
        assert!(!c.active());
        assert_eq!(c.move_vector(), None);
        assert!(c.set_mode(Mode::Inventory).is_empty());
    }

    #[test]
    fn a_menu_tap_is_a_left_click_at_the_finger() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.set_mode(Mode::Inventory);
        let out = c.touch(1, Phase::Started, 1200.0, 450.0, 0.0);
        assert_eq!(
            out,
            vec![
                Action::PointerMove {
                    x: 1200.0,
                    y: 450.0
                },
                Action::PointerButton {
                    right: false,
                    down: true
                }
            ]
        );
        assert_eq!(
            c.touch(1, Phase::Moved, 1210.0, 460.0, 0.1),
            vec![Action::PointerMove {
                x: 1210.0,
                y: 460.0
            }]
        );
        // A second finger is ignored until the first lifts.
        assert!(c.touch(2, Phase::Started, 900.0, 300.0, 0.15).is_empty());
        assert!(c.touch(2, Phase::Moved, 950.0, 300.0, 0.16).is_empty());
        c.touch(2, Phase::Ended, 0.0, 0.0, 0.17);
        assert_eq!(
            c.touch(1, Phase::Ended, 1210.0, 460.0, 0.2),
            vec![Action::PointerButton {
                right: false,
                down: false
            }]
        );
    }

    #[test]
    fn the_right_click_toggle_applies_to_one_tap() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.set_mode(Mode::Inventory);
        let toggle = rect_of(&c, Button::RightClick).centre();
        assert!(
            c.touch(1, Phase::Started, toggle.0, toggle.1, 0.0)
                .is_empty()
        );
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.05);
        assert!(
            c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::RightClick && b.lit)
        );
        let out = c.touch(2, Phase::Started, 1200.0, 450.0, 0.1);
        assert_eq!(
            out[1],
            Action::PointerButton {
                right: true,
                down: true
            }
        );
        assert_eq!(
            c.touch(2, Phase::Ended, 0.0, 0.0, 0.2),
            vec![Action::PointerButton {
                right: true,
                down: false
            }]
        );
        // Used up.
        let out = c.touch(3, Phase::Started, 1200.0, 450.0, 0.3);
        assert_eq!(
            out[1],
            Action::PointerButton {
                right: false,
                down: true
            }
        );
    }

    #[test]
    fn the_shift_toggle_stays_until_tapped_again() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.set_mode(Mode::Inventory);
        let shift = rect_of(&c, Button::Shift).centre();
        assert_eq!(
            c.touch(1, Phase::Started, shift.0, shift.1, 0.0),
            vec![Action::Shift(true)]
        );
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.05);
        assert!(c.shift());
        assert_eq!(
            c.touch(1, Phase::Started, shift.0, shift.1, 0.1),
            vec![Action::Shift(false)]
        );
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.15);
        // Leaving the menu lets go of it.
        c.touch(1, Phase::Started, shift.0, shift.1, 0.2);
        c.touch(1, Phase::Ended, 0.0, 0.0, 0.25);
        assert!(c.set_mode(Mode::Playing).contains(&Action::Shift(false)));
    }

    #[test]
    fn scroll_buttons_repeat_while_held() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.set_mode(Mode::Inventory);
        let down = rect_of(&c, Button::ScrollDown).centre();
        assert_eq!(
            c.touch(1, Phase::Started, down.0, down.1, 1.0),
            vec![Action::Scroll(-1.0)]
        );
        // The first repeat waits 0.4 s, the next ones the layout's interval.
        assert!(c.update(1.2).is_empty());
        assert_eq!(c.update(1.45), vec![Action::Scroll(-1.0)]);
        assert!(c.update(1.5).is_empty());
        assert_eq!(c.update(1.6), vec![Action::Scroll(-1.0)]);
        c.touch(1, Phase::Ended, 0.0, 0.0, 1.7);
        assert!(c.update(2.5).is_empty());
    }

    #[test]
    fn back_is_a_press_that_releases_with_the_finger() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.set_mode(Mode::Inventory);
        let back = rect_of(&c, Button::Back).centre();
        assert_eq!(
            c.touch(1, Phase::Started, back.0, back.1, 0.0),
            vec![Action::Press(Button::Back)]
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.1),
            vec![Action::Release(Button::Back)]
        );
    }

    #[test]
    fn the_layout_file_overrides_what_it_names_and_clamps_the_rest() {
        let layout = Layout::parse(
            "buttonSize:0.2\nlookGain: 3\nmargin:9999\nbogus:1\nstickZone:abc\nnot a line\nbuttonMin:100\nbuttonMax:50\n",
        );
        assert_eq!(layout.button_fraction, 0.2);
        assert_eq!(layout.look_gain, 3.0);
        assert_eq!(layout.margin, 80.0, "clamped");
        assert_eq!(
            layout.stick_zone,
            Layout::default().stick_zone,
            "unreadable: kept"
        );
        assert!(layout.button_max >= layout.button_min);
        assert_eq!(Layout::parse(""), Layout::default());
        // A bigger button size makes bigger buttons.
        let big = TouchControls::new(layout.clone(), cover());
        let normal = TouchControls::new(Layout::default(), cover());
        assert!(rect_of(&big, Button::Jump).w >= rect_of(&normal, Button::Jump).w);
    }

    #[test]
    fn resizing_rearranges() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let before = rect_of(&c, Button::Break);
        c.set_screen(unfolded());
        let after = rect_of(&c, Button::Break);
        assert_ne!(before, after);
        assert!(after.x + after.w <= unfolded().width);
    }

    #[test]
    fn a_lost_end_does_not_wedge_a_finger_id() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let brk = rect_of(&c, Button::Break).centre();
        c.touch(1, Phase::Started, brk.0, brk.1, 0.0);
        let out = c.touch(1, Phase::Started, brk.0, brk.1, 0.1);
        assert_eq!(
            out,
            vec![Action::Release(Button::Break), Action::Press(Button::Break)]
        );
    }

    #[test]
    fn a_tap_on_the_world_uses_what_the_crosshair_points_at() {
        let mut c = TouchControls::new(Layout::default(), cover());
        assert!(c.touch(1, Phase::Started, 1500.0, 400.0, 0.0).is_empty());
        // Down and up again quickly, barely moved: a use, pressed and let go.
        assert_eq!(
            c.touch(1, Phase::Ended, 1502.0, 401.0, 0.1),
            vec![Action::Press(Button::Place), Action::Release(Button::Place)]
        );
    }

    #[test]
    fn a_cancelled_world_touch_is_not_a_tap() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        assert!(c.touch(1, Phase::Cancelled, 1500.0, 400.0, 0.1).is_empty());
    }

    #[test]
    fn holding_on_the_world_breaks_until_the_finger_lifts() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        // Not yet: the hold has to last.
        assert!(c.update(0.2).is_empty());
        assert_eq!(c.update(0.26), vec![Action::Press(Button::Break)]);
        // It does not press twice, and the Break button shows it.
        assert!(c.update(0.4).is_empty());
        assert!(
            c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::Break && b.lit)
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 1500.0, 400.0, 0.5),
            vec![Action::Release(Button::Break)]
        );
    }

    #[test]
    fn holding_and_dragging_keeps_breaking_while_it_looks() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let dp = 2.625;
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        assert_eq!(c.update(0.3), vec![Action::Press(Button::Break)]);
        // Dragging turns the view and the break stays held.
        let out = c.touch(1, Phase::Moved, 1500.0 + 40.0 * dp, 400.0, 0.4);
        assert!(matches!(out[..], [Action::Look { .. }]), "{out:?}");
        assert!(
            c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::Break && b.lit)
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.5),
            vec![Action::Release(Button::Break)]
        );
    }

    #[test]
    fn a_big_drag_before_the_hold_is_only_a_look() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let dp = 2.625;
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        // Away past the world-drag distance at once: a look, and no hold
        // ever starts under it, however long it stays down.
        let out = c.touch(1, Phase::Moved, 1500.0 + 40.0 * dp, 400.0, 0.1);
        assert!(matches!(out[..], [Action::Look { .. }]), "{out:?}");
        assert!(c.update(0.6).is_empty());
        assert!(c.touch(1, Phase::Ended, 0.0, 0.0, 0.7).is_empty());
    }

    #[test]
    fn a_small_wander_still_holds_and_never_taps_after_a_drag() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let dp = 2.625;
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        // 5 dp of wander: under the world-drag distance, so the hold lands.
        c.touch(1, Phase::Moved, 1500.0 + 5.0 * dp, 400.0, 0.1);
        assert_eq!(c.update(0.3), vec![Action::Press(Button::Break)]);
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.4),
            vec![Action::Release(Button::Break)]
        );
    }

    #[test]
    fn world_touch_off_means_no_taps_and_no_holds() {
        let mut c = TouchControls::new(Layout::default(), cover());
        assert!(c.set_world_touch(false).is_empty());
        assert!(!c.world_touch());
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        assert!(c.update(0.5).is_empty());
        assert!(c.touch(1, Phase::Ended, 1500.0, 400.0, 0.6).is_empty());
        // A fresh tap with it off is only a look finger too.
        c.touch(2, Phase::Started, 1500.0, 400.0, 1.0);
        assert!(c.touch(2, Phase::Ended, 1500.0, 400.0, 1.1).is_empty());
    }

    #[test]
    fn turning_world_touch_off_mid_hold_lets_go() {
        let mut c = TouchControls::new(Layout::default(), cover());
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        assert_eq!(c.update(0.3), vec![Action::Press(Button::Break)]);
        assert_eq!(
            c.set_world_touch(false),
            vec![Action::Release(Button::Break)]
        );
        assert!(c.touch(1, Phase::Ended, 1500.0, 400.0, 0.4).is_empty());
    }

    #[test]
    fn sliding_onto_a_button_presses_it_and_sliding_off_lets_go() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let brk = rect_of(&c, Button::Break).centre();
        let place = rect_of(&c, Button::Place).centre();
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        // Onto Break: it presses without the finger ever lifting.
        assert_eq!(
            c.touch(1, Phase::Moved, brk.0, brk.1, 0.1),
            vec![Action::Press(Button::Break)]
        );
        // Across to Place: one lets go as the other presses.
        assert_eq!(
            c.touch(1, Phase::Moved, place.0, place.1, 0.2),
            vec![Action::Release(Button::Break), Action::Press(Button::Place)]
        );
        // Off the buttons entirely: let go, and the finger only looks now.
        assert_eq!(
            c.touch(1, Phase::Moved, 1500.0, 400.0, 0.3),
            vec![Action::Release(Button::Place)]
        );
        assert!(c.touch(1, Phase::Ended, 1500.0, 400.0, 0.4).is_empty());
    }

    #[test]
    fn sliding_onto_a_button_ends_a_world_hold_first() {
        let mut c = TouchControls::new(Layout::default(), cover());
        let jump = rect_of(&c, Button::Jump).centre();
        c.touch(1, Phase::Started, 1500.0, 400.0, 0.0);
        assert_eq!(c.update(0.3), vec![Action::Press(Button::Break)]);
        assert_eq!(
            c.touch(1, Phase::Moved, jump.0, jump.1, 0.4),
            vec![Action::Release(Button::Break), Action::Press(Button::Jump)]
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.5),
            vec![Action::Release(Button::Jump)]
        );
    }

    #[test]
    fn the_safe_area_pushes_the_controls_in() {
        let mut screen = cover();
        screen.insets = Insets {
            left: 100.0,
            top: 80.0,
            right: 120.0,
            bottom: 80.0,
        };
        let c = TouchControls::new(Layout::default(), screen);
        // Every hit target stays within the same safe rectangle as its drawing.
        for button in c.view().buttons {
            let r = button.rect;
            assert!(r.x >= 100.0 && r.y >= 80.0, "{button:?}");
            assert!(r.x + r.w <= screen.width - 120.0, "{button:?}");
            assert!(r.y + r.h <= screen.height - 80.0, "{button:?}");
        }
        let pause = rect_of(&c, Button::Pause);
        assert!((pause.x + pause.w - (screen.width - 120.0)).abs() < 0.01);
        assert!((pause.y - 80.0).abs() < 0.01);
        let stick = c.view().stick;
        assert!(
            (stick.base.0 - stick.radius - 100.0).abs() < 0.01,
            "{stick:?}"
        );
    }

    #[test]
    fn panel_mapping_is_the_identity_where_the_spaces_agree() {
        assert_eq!(
            panel_point((1920.0, 1080.0), (1920.0, 1080.0), 700.0, 300.0),
            (700.0, 300.0)
        );
        // And it undoes the panel stretch exactly: a layout point, seen where
        // the presentation puts it, maps back onto itself.
        let screen = iphone();
        let panel = (2532.0, 1170.0);
        let at = (2000.0, 500.0);
        let seen = (
            at.0 * panel.0 / screen.width,
            at.1 * panel.1 / screen.height,
        );
        let back = panel_point(panel, (screen.width, screen.height), seen.0, seen.1);
        assert!((back.0 - at.0).abs() < 0.01 && (back.1 - at.1).abs() < 0.01);
    }

    #[test]
    fn iphone_insets_are_the_panels_scaled_into_the_surface() {
        let insets = iphone().insets;
        // 141 panel pixels at each side and 60 below, scaled by 2250/2532
        // and 1110/1170 — not the 0 the surface-based basis made of the
        // right side by subtracting the safe width from itself.
        assert!((insets.left - 125.3).abs() < 0.1, "{insets:?}");
        assert!((insets.right - 125.3).abs() < 0.1, "{insets:?}");
        assert_eq!(insets.top, 0.0);
        assert!((insets.bottom - 56.9).abs() < 0.1, "{insets:?}");
        // A "safe area" starting past half the panel is no safe area.
        assert!(
            Insets::from_panel(
                (1300.0, 0.0),
                (2250.0, 1110.0),
                (2532.0, 1170.0),
                (2250.0, 1110.0)
            )
            .is_none()
        );
    }

    #[test]
    fn iphone_taps_land_on_the_controls_they_are_seen_on() {
        let screen = iphone();
        let mut c = TouchControls::new(Layout::default(), screen);
        // Place, far right: read in panel pixels unmapped, this tap landed
        // some 260 px further right still — past the button altogether.
        let place = rect_of(&c, Button::Place).centre();
        assert_eq!(
            panel_tap(&mut c, screen, 1, place, 0.0),
            vec![Action::Press(Button::Place)]
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 0.0, 0.0, 0.1),
            vec![Action::Release(Button::Place)]
        );
        // The hotbar's seventh slot: unmapped, this tap selected the ninth.
        let slot = 6;
        let unit = screen.slot / 20.0;
        let seventh = (
            screen.hotbar.x + unit * (1.0 + 20.0 * slot as f32 + 10.0),
            screen.hotbar.y + screen.hotbar.h / 2.0,
        );
        assert_eq!(
            panel_tap(&mut c, screen, 2, seventh, 0.2),
            vec![Action::SelectSlot(slot)]
        );
        // The stick's home on the left is unmoved: a finger there takes the
        // stick and presses nothing.
        let home = c.view().stick.base;
        let out = panel_tap(&mut c, screen, 3, home, 0.3);
        assert!(
            out.iter().all(|a| !matches!(a, Action::Press(_))),
            "{out:?}"
        );
        assert!(c.view().stick.active);
    }
    #[test]
    fn all_world_buttons_dispatch_and_flight_is_the_existing_double_jump() {
        let mut c = TouchControls::new(Layout::default(), phone());
        for button in [
            Button::Sprint,
            Button::FlyUp,
            Button::FlyDown,
            Button::PickBlock,
            Button::SwapHands,
            Button::Perspective,
            Button::SaveToolbar,
            Button::LoadToolbar,
            Button::QuickActions,
            Button::Emote,
            Button::PlayerList,
        ] {
            let (x, y) = rect_of(&c, button).centre();
            assert_eq!(
                c.touch(1, Phase::Started, x, y, 0.0),
                vec![Action::Press(button)]
            );
            assert_eq!(
                c.touch(1, Phase::Cancelled, x, y, 0.1),
                vec![Action::Release(button)]
            );
        }
        let (x, y) = rect_of(&c, Button::FlyToggle).centre();
        assert_eq!(
            c.touch(1, Phase::Started, x, y, 0.0),
            vec![Action::Press(Button::Jump)]
        );
        assert!(c.touch(1, Phase::Ended, x, y, 0.01).is_empty());
        assert_eq!(c.update(0.08), vec![Action::Release(Button::Jump)]);
        assert_eq!(c.update(0.16), vec![Action::Press(Button::Jump)]);
        assert_eq!(c.update(0.24), vec![Action::Release(Button::Jump)]);
        assert!(c.update(1.0).is_empty());
    }

    #[test]
    fn menu_shortcuts_preserve_the_slot_pointer_and_middle_drag_ends_on_cancel() {
        let mut c = TouchControls::new(Layout::default(), phone());
        c.set_mode(Mode::Inventory);
        // Arm each operation, then tap the target. The shortcut must run at
        // that slot with no normal click to pick up or displace its contents.
        for button in [
            Button::MenuSwapHands,
            Button::MenuDrop,
            Button::MenuDropStack,
            Button::MenuClone,
            Button::MenuCollect,
            Button::MenuHotbar(0),
            Button::MenuHotbar(8),
        ] {
            let (x, y) = rect_of(&c, button).centre();
            assert!(c.touch(1, Phase::Started, x, y, 0.2).is_empty());
            assert!(c.touch(1, Phase::Ended, x, y, 0.3).is_empty());
            assert!(c.view().buttons.iter().any(|b| b.button == button && b.lit));
            assert_eq!(
                c.touch(2, Phase::Started, 1266.0, 500.0, 0.4),
                vec![
                    Action::PointerMove {
                        x: 1266.0,
                        y: 500.0
                    },
                    Action::Press(button),
                    Action::Release(button)
                ]
            );
            assert!(c.touch(2, Phase::Moved, 1300.0, 500.0, 0.5).is_empty());
            assert!(c.touch(2, Phase::Ended, 1300.0, 500.0, 0.6).is_empty());
            assert!(!c.view().buttons.iter().any(|b| b.button == button && b.lit));
        }
        let (x, y) = rect_of(&c, Button::MenuMiddleClick).centre();
        assert!(c.touch(1, Phase::Started, x, y, 0.4).is_empty());
        c.touch(1, Phase::Ended, x, y, 0.5);
        assert_eq!(
            c.touch(2, Phase::Started, 1200.0, 500.0, 0.6),
            vec![
                Action::PointerMove {
                    x: 1200.0,
                    y: 500.0
                },
                Action::PointerMiddle(true)
            ]
        );
        assert_eq!(
            c.touch(2, Phase::Moved, 1300.0, 500.0, 0.7),
            vec![Action::PointerMove {
                x: 1300.0,
                y: 500.0
            }]
        );
        assert_eq!(
            c.touch(2, Phase::Cancelled, 1300.0, 500.0, 0.8),
            vec![Action::PointerMiddle(false)]
        );
        assert!(
            !c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::MenuMiddleClick && b.lit)
        );
    }

    #[test]
    fn leaving_a_screen_releases_control_and_an_unfinished_flight_gesture() {
        let mut c = TouchControls::new(Layout::default(), phone());
        let (x, y) = rect_of(&c, Button::Control).centre();
        assert_eq!(
            c.touch(1, Phase::Started, x, y, 0.0),
            vec![Action::Control(true)]
        );
        c.touch(1, Phase::Ended, x, y, 0.1);
        let (x, y) = rect_of(&c, Button::FlyToggle).centre();
        c.touch(2, Phase::Started, x, y, 0.2);
        let actions = c.set_mode(Mode::Inventory);
        assert!(actions.contains(&Action::Control(false)));
        assert!(actions.contains(&Action::Release(Button::Jump)));
        assert!(c.update(1.0).is_empty());
    }
    #[test]
    fn shared_mining_holds_release_only_after_the_last_finger() {
        let mut c = TouchControls::new(Layout::default(), phone());
        let (x, y) = rect_of(&c, Button::Break).centre();
        assert_eq!(
            c.touch(1, Phase::Started, x, y, 0.0),
            vec![Action::Press(Button::Break)]
        );
        assert!(c.touch(2, Phase::Started, x, y, 0.1).is_empty());
        assert!(c.touch(1, Phase::Ended, x, y, 0.2).is_empty());
        // A world hold joins the remaining dedicated mine-button finger.
        c.touch(3, Phase::Started, 1500.0, 500.0, 0.3);
        assert!(c.update(0.6).is_empty());
        assert!(c.touch(2, Phase::Cancelled, x, y, 0.7).is_empty());
        assert_eq!(
            c.touch(3, Phase::Ended, 1500.0, 500.0, 0.8),
            vec![Action::Release(Button::Break)]
        );
        c.touch(4, Phase::Started, x, y, 0.9);
        c.touch(5, Phase::Started, x, y, 1.0);
        assert_eq!(c.release_all(), vec![Action::Release(Button::Break)]);
    }

    #[test]
    fn flight_gesture_does_not_release_a_jump_finger() {
        let mut c = TouchControls::new(Layout::default(), phone());
        let (x, y) = rect_of(&c, Button::Jump).centre();
        c.touch(1, Phase::Started, x, y, 0.0);
        let (fx, fy) = rect_of(&c, Button::FlyToggle).centre();
        assert!(c.touch(2, Phase::Started, fx, fy, 0.0).is_empty());
        assert!(c.update(0.08).is_empty());
        assert!(c.update(0.16).is_empty());
        assert!(c.update(0.24).is_empty());
        assert_eq!(
            c.touch(1, Phase::Ended, x, y, 0.3),
            vec![Action::Release(Button::Jump)]
        );
    }

    #[test]
    fn a_world_look_cannot_slide_into_a_modifier_or_flight_toggle() {
        for button in [Button::Control, Button::FlyToggle] {
            let mut c = TouchControls::new(Layout::default(), phone());
            c.touch(1, Phase::Started, 1500.0, 500.0, 0.0);
            let (x, y) = rect_of(&c, button).centre();
            let out = c.touch(1, Phase::Moved, x, y, 0.1);
            assert!(
                out.iter()
                    .all(|action| matches!(action, Action::Look { .. }))
            );
            assert!(!c.view().buttons.iter().any(|b| b.button == button && b.lit));
        }
    }

    #[test]
    fn spectator_scroll_buttons_repeat_in_the_world() {
        let mut c = TouchControls::new(Layout::default(), phone());
        for (button, direction) in [(Button::ScrollUp, 1.0), (Button::ScrollDown, -1.0)] {
            let (x, y) = rect_of(&c, button).centre();
            assert_eq!(
                c.touch(1, Phase::Started, x, y, 0.0),
                vec![Action::Scroll(direction)]
            );
            assert!(c.update(0.2).is_empty());
            assert_eq!(c.update(0.4), vec![Action::Scroll(direction)]);
            c.touch(1, Phase::Ended, x, y, 0.5);
            assert!(c.update(0.6).is_empty());
        }
    }
    #[test]
    fn an_armed_inventory_action_can_cancel_and_never_leaks_to_another_screen() {
        let mut c = TouchControls::new(Layout::default(), phone());
        c.set_mode(Mode::Inventory);
        let (x, y) = rect_of(&c, Button::MenuDropStack).centre();
        for id in [1, 2] {
            assert!(c.touch(id, Phase::Started, x, y, 0.0).is_empty());
            assert!(c.touch(id, Phase::Ended, x, y, 0.1).is_empty());
        }
        assert_eq!(
            c.touch(3, Phase::Started, 1266.0, 500.0, 0.2),
            vec![
                Action::PointerMove {
                    x: 1266.0,
                    y: 500.0
                },
                Action::PointerButton {
                    right: false,
                    down: true
                }
            ]
        );
        c.touch(3, Phase::Ended, 1266.0, 500.0, 0.3);
        c.touch(4, Phase::Started, x, y, 0.4);
        c.touch(4, Phase::Ended, x, y, 0.5);
        assert!(c.release_all().is_empty());
        assert!(
            !c.view()
                .buttons
                .iter()
                .any(|b| b.button == Button::MenuDropStack && b.lit)
        );
        let actions = c.touch(5, Phase::Started, 1266.0, 500.0, 0.6);
        assert!(actions.contains(&Action::PointerButton {
            right: false,
            down: true
        }));
        assert!(!actions.contains(&Action::Press(Button::MenuDropStack)));
        c.touch(5, Phase::Ended, 1266.0, 500.0, 0.7);
        c.touch(6, Phase::Started, x, y, 0.8);
        c.set_mode(Mode::Playing);
        c.set_mode(Mode::Inventory);
        let actions = c.touch(7, Phase::Started, 1266.0, 500.0, 0.9);
        assert!(actions.contains(&Action::PointerButton {
            right: false,
            down: true
        }));
    }
    #[test]
    fn portrait_menu_leaves_the_full_width_inventory_and_recipe_book_clear() {
        let mut screens = [
            portrait(),
            Screen {
                width: 1170.0,
                height: 2532.0,
                density: 3.0,
                slot: 80.0,
                ..Screen::plain(1170.0, 2532.0, 3.0)
            },
        ];
        screens[1].insets = Insets {
            top: 141.0,
            bottom: 63.0,
            ..Insets::default()
        };
        for screen in screens {
            let mut c = TouchControls::new(Layout::default(), screen);
            c.set_mode(Mode::Inventory);
            let gui_scale = screen.slot / 20.0;
            let inventory_top = (screen.height - 166.0 * gui_scale) / 2.0;
            let inventory_bottom = inventory_top + 166.0 * gui_scale;
            for button in c.view().buttons {
                let r = button.rect;
                if matches!(button.button, Button::MenuHotbar(_)) {
                    assert!(r.y >= inventory_bottom, "{button:?} covers the inventory");
                } else {
                    assert!(
                        r.y + r.h <= inventory_top,
                        "{button:?} covers the recipe book"
                    );
                }
            }
        }
    }
    #[test]
    fn generic_menus_show_only_navigation_and_keep_touch_pointer_input() {
        let mut c = TouchControls::new(Layout::default(), phone());
        c.set_mode(Mode::Menu);
        let buttons: Vec<_> = c.view().buttons.into_iter().map(|b| b.button).collect();
        assert_eq!(
            buttons,
            vec![
                Button::Back,
                Button::ScrollUp,
                Button::ScrollDown,
                Button::Keyboard
            ]
        );
        assert_eq!(
            c.touch(1, Phase::Started, 1266.0, 500.0, 0.0),
            vec![
                Action::PointerMove {
                    x: 1266.0,
                    y: 500.0
                },
                Action::PointerButton {
                    right: false,
                    down: true
                }
            ]
        );
        assert_eq!(
            c.touch(1, Phase::Ended, 1266.0, 500.0, 0.1),
            vec![Action::PointerButton {
                right: false,
                down: false
            }]
        );
        let (x, y) = rect_of(&c, Button::ScrollDown).centre();
        assert_eq!(
            c.touch(2, Phase::Started, x, y, 0.2),
            vec![Action::Scroll(-1.0)]
        );
        assert_eq!(c.update(0.61), vec![Action::Scroll(-1.0)]);
    }
}
