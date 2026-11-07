//! Touch controls and the platform mouse in the game loop (docs: `ANDROID.md`, `IOS.md`).
//!
//! `horizon_client::touch` turns fingers into [`Action`]s and knows nothing of the game;
//! this is the other half: it feeds it the window's touches, carries its actions out
//! through the same paths a keyboard, a mouse and a controller use (the controller's
//! action names for buttons, synthetic pointer events for menus), and draws the controls.
//! `horizon_client::platform` carries the real mouse from the platform side of the app,
//! which winit's Android backend cannot see (iOS has no mouse; the queue stays idle there).
//!
//! On the desktop the touch controls are off unless `HORIZON_TOUCH=1`; there the window
//! system turns a touch into a mouse click anyway.
use super::{Game, Screen};

mod draw;
use super::{event_loop::EventLoopCtl, input_trace::InputOrigin};
use horizon_client::{
    Result, gamepad,
    held::Source,
    hud::HudBatch,
    osk::Hit,
    platform::{self, PointerButton, PointerEvent},
    touch::{self as controls, Action, Button, Layout, Mode, Phase, TouchControls},
};
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    keyboard::{KeyCode, ModifiersState},
};

/// The touch controls' state in the game.
pub(super) struct TouchState {
    controls: TouchControls,
    /// The layout from `horizon-touch.txt` before the Touch options are
    /// applied over it (they scale its look gain and button size).
    base_layout: Layout,
    enabled: bool,
    /// Seconds are counted from here.
    since: Instant,
    /// Where the menu pointer is, in physical pixels.
    pointer: (f32, f32),
    /// The press that began on the on-screen keyboard is its own: its release is not a click.
    swallowing: bool,
    held_buttons: Vec<Button>,
    stick_sprint: bool,
    sprint_input: bool,
}

impl Default for TouchState {
    fn default() -> Self {
        // `horizon-touch.txt` in the game folder (the working directory) tunes it.
        let base_layout = std::env::current_dir()
            .map(|dir| Layout::load(&dir))
            .unwrap_or_default();
        Self {
            controls: TouchControls::new(
                base_layout.clone(),
                controls::Screen::plain(1280.0, 720.0, 1.0),
            ),
            base_layout,
            enabled: cfg!(any(target_os = "android", target_os = "ios"))
                || std::env::var("HORIZON_TOUCH").is_ok_and(|v| v == "1"),
            since: Instant::now(),
            pointer: (0.0, 0.0),
            swallowing: false,
            held_buttons: Vec::new(),
            stick_sprint: false,
            sprint_input: false,
        }
    }
}

/// The window's safe-area insets in physical pixels, for the touch controls
/// to keep off the unsafe edges. Only iOS has any: winit exposes no safe-area
/// call, but its iOS backend defines the window's inner position and size
/// from the safe area, so the insets are what the full surface extends past
/// them on each side. If the platform reports nothing sane, landscape
/// iPhones keep fixed insets instead (24 dp at the sides, 16 dp below).
#[cfg(target_os = "ios")]
fn safe_insets(
    window: Option<&winit::window::Window>,
    width: f32,
    height: f32,
    density: f32,
) -> controls::Insets {
    if let Some(window) = window
        && let Ok(position) = window.inner_position()
    {
        let inner = window.inner_size();
        let insets = controls::Insets {
            left: (position.x as f32).max(0.0),
            top: (position.y as f32).max(0.0),
            right: (width - position.x as f32 - inner.width as f32).max(0.0),
            bottom: (height - position.y as f32 - inner.height as f32).max(0.0),
        };
        // A sane safe area never eats half the screen; anything wider is
        // the platform reporting positions, not insets.
        if insets.left + insets.right < width / 2.0 && insets.top + insets.bottom < height / 2.0 {
            return insets;
        }
    }
    controls::Insets {
        left: 24.0 * density,
        top: 0.0,
        right: 24.0 * density,
        bottom: 16.0 * density,
    }
}

/// No platform but iOS reports a safe area to winit (Android's edges are
/// the layout's own margin's problem, as before).
#[cfg(not(target_os = "ios"))]
fn safe_insets(
    _window: Option<&winit::window::Window>,
    _width: f32,
    _height: f32,
    _density: f32,
) -> controls::Insets {
    controls::Insets::default()
}

/// A finger as the mouse: the events a pointer at its place and a left button would send. The
/// startup menu, shown while the resources load, takes only a mouse.
pub(super) fn touch_as_mouse(touch: winit::event::Touch) -> Vec<WindowEvent> {
    let device_id = touch.device_id;
    let at = WindowEvent::CursorMoved {
        device_id,
        position: touch.location,
    };
    let button = |state| WindowEvent::MouseInput {
        device_id,
        state,
        button: MouseButton::Left,
    };
    match touch.phase {
        TouchPhase::Started => vec![at, button(ElementState::Pressed)],
        TouchPhase::Moved => vec![at],
        TouchPhase::Ended | TouchPhase::Cancelled => vec![at, button(ElementState::Released)],
    }
}

impl Game<'_> {
    /// The controller action each world button stands for.
    fn touch_action_name(button: Button) -> Option<&'static str> {
        Some(match button {
            Button::Jump => "key.jump",
            Button::Sneak => "key.sneak",
            Button::Break => "key.attack",
            Button::Place => "key.use",
            Button::Inventory => "key.inventory",
            Button::Chat => "key.chat",
            Button::Drop => "key.drop",
            Button::PickBlock => "key.pickItem",
            Button::SwapHands => "key.swapOffhand",
            Button::Perspective => "key.togglePerspective",
            Button::QuickActions => "key.quickActions",
            Button::Emote => "key.emote",
            Button::PlayerList => "key.playerlist",
            _ => return None,
        })
    }

    /// GUI units reserved above the phone's home indicator, shared by drawing and hits.
    pub(super) fn touch_hud_bottom_inset(&self, batch: &HudBatch) -> f32 {
        if !self.touch.enabled {
            return 0.0;
        }
        let density = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32)
            .max(0.5);
        let insets = safe_insets(
            self.window.as_deref(),
            batch.width * batch.scale,
            batch.height * batch.scale,
            density,
        );
        (insets
            .bottom
            .max(self.touch.controls.layout().margin * density)
            / batch.scale)
            .ceil()
    }

    /// Touch holds also protect mappings from keyboard modifier recovery and
    /// provide the creative toolbar activators without pretending a physical key is down.
    pub(super) fn touch_holds_action(&self, action: &str) -> bool {
        let held = |b| self.touch.held_buttons.contains(&b);
        self.touch.enabled
            && match action {
                "key.sneak" => held(Button::Sneak) || held(Button::FlyDown),
                "key.sprint" => held(Button::Sprint) || self.touch.stick_sprint,
                "key.saveToolbarActivator" => held(Button::SaveToolbar),
                "key.loadToolbarActivator" => held(Button::LoadToolbar),
                _ => false,
            }
    }

    pub(super) fn touch_shift_down(&self) -> bool {
        self.touch.enabled && self.touch.controls.shift()
    }

    pub(super) fn touch_control_down(&self) -> bool {
        self.touch.enabled && self.touch.controls.control()
    }

    fn touch_modifiers(&self) -> ModifiersState {
        let mut modifiers = self.modifiers;
        modifiers.set(ModifiersState::SHIFT, self.has_shift_down());
        modifiers.set(ModifiersState::CONTROL, self.has_control_down());
        modifiers
    }

    /// The movement stick as the controller's left stick, while a finger holds it.
    pub(super) fn touch_move(&self) -> Option<[f32; 2]> {
        if !self.touch.enabled || self.pad_in_menu() {
            return None;
        }
        self.touch.controls.move_vector()
    }

    /// The touch controls for the open screen: the world's, or a menu's.
    fn touch_mode(&self) -> Mode {
        let overlay = self.dialog_open()
            || self.link_prompt.is_some()
            || self.command_prompt.is_some()
            || self.code_of_conduct.is_some()
            || self.server_packs.prompt.is_some();
        if self.title.is_none()
            && !overlay
            && self.screen == Screen::Playing
            && self.sign_edit.is_none()
            && self.book.is_none()
            // In bed the screen has a "Leave Bed" button to tap.
            && self.sleeping.is_none()
        {
            Mode::Playing
        } else if matches!(self.screen, Screen::Inventory | Screen::Creative) && !overlay {
            Mode::Inventory
        } else {
            Mode::Menu
        }
    }

    /// Tells the controls the window's size and the hotbar's place, and which kind of screen is open.
    fn touch_sync(&mut self, event_loop: &dyn EventLoopCtl) {
        if let Some(r) = &self.renderer {
            let (width, height) = (r.config.width, r.config.height);
            let batch = HudBatch::new([width, height]);
            let s = batch.scale;
            let density = self
                .window
                .as_ref()
                .map_or(1.0, |w| w.scale_factor() as f32)
                .max(0.5);
            self.touch.controls.set_screen(controls::Screen {
                width: width as f32,
                height: height as f32,
                density,
                // `Gui` draws the hotbar 182 by 22 GUI pixels, centred at the bottom.
                hotbar: controls::Rect {
                    x: (batch.width / 2.0 - 91.0) * s,
                    y: ((batch.height - self.touch_hud_bottom_inset(&batch)).floor() - 22.0) * s,
                    w: 182.0 * s,
                    h: 22.0 * s,
                },
                slot: 20.0 * s,
                insets: safe_insets(self.window.as_deref(), width as f32, height as f32, density),
            });
        }
        // The Touch options (under Mouse Settings) ride on the layout file's
        // values: they scale its look gain and button size, and switch world
        // taps and holds, so a change in the options applies at once.
        let mut layout = self.touch.base_layout.clone();
        layout.look_gain =
            (layout.look_gain * self.settings.touch_look_multiplier()).clamp(0.2, 8.0);
        layout.button_fraction =
            (layout.button_fraction * self.settings.touch_button_scale()).clamp(0.05, 0.4);
        if *self.touch.controls.layout() != layout {
            self.touch.controls.set_layout(layout);
        }
        let released = self
            .touch
            .controls
            .set_world_touch(self.settings.touch_world_interact);
        self.touch_actions(event_loop, released);
        let actions = self.touch.controls.set_mode(self.touch_mode());
        self.touch_actions(event_loop, actions);
    }

    /// Release HUD holds and menu drags on focus/surface loss.
    pub(super) fn touch_release(&mut self) {
        for action in self.touch.controls.release_all() {
            let result = match action {
                Action::Press(button) => self.touch_button(button, true),
                Action::Release(button) => self.touch_button(button, false),
                Action::Sprint(on) => self.touch_sprint(on),
                Action::PointerButton { right, down: false } => {
                    if matches!(self.screen, Screen::Inventory | Screen::Creative)
                        && !self.touch.swallowing
                    {
                        self.inventory_mouse_release(right);
                    }
                    self.touch.swallowing = false;
                    Ok(())
                }
                Action::PointerMiddle(false) => {
                    self.inventory_pick_release();
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(e) = result {
                log::warn!("Horizon: touch control: {e}");
            }
        }
    }

    /// A finger touched the window, moved on it or left it.
    pub(super) fn touch_event(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        touch: winit::event::Touch,
    ) {
        if !self.touch.enabled {
            return;
        }
        self.last_input = Instant::now();
        self.touch_sync(event_loop);
        // Touching the world takes the view, as a click into the window does.
        if self.touch.controls.mode() == Mode::Playing && !self.captured && self.focused {
            self.capture(true);
        }
        let phase = match touch.phase {
            TouchPhase::Started => Phase::Started,
            TouchPhase::Moved => Phase::Moved,
            TouchPhase::Ended => Phase::Ended,
            TouchPhase::Cancelled => Phase::Cancelled,
        };
        let now = self.touch.since.elapsed().as_secs_f64();
        let actions = self.touch.controls.touch(
            touch.id,
            phase,
            touch.location.x as f32,
            touch.location.y as f32,
            now,
        );
        self.touch_actions(event_loop, actions);
    }

    /// Once per loop iteration: the mouse's events from the platform, the touch controls' timers
    /// and the platform's requests (the pointer capture follows the game's own grab).
    pub(super) fn touch_frame(&mut self, event_loop: &dyn EventLoopCtl) {
        // The platform is ending the app: save and leave, as closing the window does.
        if platform::exit_requested() {
            match self.window.as_ref().map(|w| w.id()) {
                Some(id) => self.handle_window_event(
                    event_loop,
                    id,
                    WindowEvent::CloseRequested,
                    InputOrigin::User,
                ),
                None => event_loop.exit(),
            }
            return;
        }
        for event in platform::drain_pointer() {
            self.platform_pointer(event_loop, event);
        }
        if cfg!(any(target_os = "android", target_os = "ios")) {
            platform::set_want_capture(self.captured && self.focused);
        }
        if !self.touch.enabled {
            return;
        }
        self.touch_sync(event_loop);
        let now = self.touch.since.elapsed().as_secs_f64();
        let actions = self.touch.controls.update(now);
        self.touch_actions(event_loop, actions);
    }

    /// A mouse event from the platform, handled like the window's own mouse events.
    fn platform_pointer(&mut self, event_loop: &dyn EventLoopCtl, event: PointerEvent) {
        self.last_input = Instant::now();
        match event {
            PointerEvent::Moved { x, y } => {
                // A captured pointer reports only relative motion.
                if !self.captured {
                    self.touch.pointer = (x, y);
                    self.touch_cursor_to(event_loop, x, y);
                }
            }
            PointerEvent::Relative { dx, dy } => {
                if self.captured && self.focused && self.screen == Screen::Playing {
                    // As `device_event` does for MouseMotion with the pointer grabbed.
                    self.mouse
                        .on_move(0.0, 0.0, f64::from(dx), f64::from(dy), true, true);
                }
            }
            PointerEvent::Button { button, down } => {
                let button = match button {
                    PointerButton::Left => MouseButton::Left,
                    PointerButton::Right => MouseButton::Right,
                    PointerButton::Middle => MouseButton::Middle,
                };
                // Pressing into the window with the mouse takes the view, like a click on the desktop.
                if down && !self.captured && self.focused && self.touch_mode() == Mode::Playing {
                    self.capture(true);
                }
                self.touch_mouse_button(event_loop, button, down);
            }
            PointerEvent::Scroll { x, y } => self.touch_wheel(event_loop, x, y),
        }
    }

    /// Carries out what the controls decided.
    fn touch_actions(&mut self, event_loop: &dyn EventLoopCtl, actions: Vec<Action>) {
        for action in actions {
            if let Err(e) = self.touch_action(event_loop, action) {
                log::warn!("Horizon: touch control: {e}");
            }
        }
    }

    fn touch_action(&mut self, event_loop: &dyn EventLoopCtl, action: Action) -> Result<()> {
        match action {
            // Read every tick through `pad_move`.
            Action::Move(_) => {}
            Action::Look { dx, dy } => {
                if self.captured && self.focused && self.screen == Screen::Playing {
                    // The Mouse Sensitivity option scales it further, as for a mouse.
                    let gain = f64::from(self.touch.controls.layout().look_gain);
                    self.mouse
                        .add_motion(f64::from(dx) * gain, f64::from(dy) * gain);
                }
            }
            Action::Sprint(on) => {
                if self.screen == Screen::Playing {
                    self.touch_sprint(on)?;
                }
            }
            Action::Press(button) => self.touch_button(button, true)?,
            Action::Release(button) => self.touch_button(button, false)?,
            Action::SelectSlot(slot) => {
                if self.screen == Screen::Playing
                    && let Some(&key) = super::HOTBAR_DIGITS.get(slot)
                {
                    self.key_pressed(key, key)?;
                }
            }
            Action::StepSlot(steps) => {
                if self.screen == Screen::Playing {
                    // Follow the normal wheel path for spectator menus too.
                    self.touch_wheel(event_loop, 0.0, -steps as f32);
                }
            }
            Action::PointerMove { x, y } => {
                self.touch.pointer = (x, y);
                self.touch_cursor_to(event_loop, x, y);
            }
            Action::PointerButton { right, down } => {
                self.touch_pointer_button(event_loop, right, down);
            }
            // The controls own touch modifier latches; physical modifiers stay
            // owned by the OS. Screen queries combine both sources.
            Action::Shift(_) | Action::Control(_) => {}
            Action::PointerMiddle(down) => {
                if down {
                    self.inventory_pick_press();
                } else {
                    self.inventory_pick_release();
                }
                if let Some(key) = self.container {
                    self.container_changed(key);
                }
            }
            Action::Scroll(lines) => self.touch_wheel(event_loop, 0.0, lines),
        }
        Ok(())
    }

    fn touch_sprint(&mut self, stick: bool) -> Result<()> {
        self.touch.stick_sprint = stick;
        let on = stick || self.touch.held_buttons.contains(&Button::Sprint);
        if on == self.touch.sprint_input {
            return Ok(());
        }
        self.touch.sprint_input = on;
        self.mapped_key(
            KeyCode::ControlLeft,
            on,
            false,
            KeyCode::ControlLeft,
            Source::Touch,
        )
    }

    fn touch_button(&mut self, button: Button, down: bool) -> Result<()> {
        if down {
            self.touch.held_buttons.push(button);
        } else if let Some(i) = self.touch.held_buttons.iter().position(|b| *b == button) {
            self.touch.held_buttons.remove(i);
        }
        let held = |b| self.touch.held_buttons.contains(&b);
        match button {
            Button::Jump | Button::FlyUp => {
                let on = held(Button::Jump) || held(Button::FlyUp);
                return self.mapped_key(KeyCode::Space, on, false, KeyCode::Space, Source::Touch);
            }
            // Flight descent is held input even with toggle-crouch enabled. It
            // shares the logical key with sneak, but owns a separate device bit.
            Button::FlyDown => {
                if down {
                    self.keys.press(KeyCode::ShiftLeft, Source::Touch);
                } else if !self.toggles.is_down(horizon_client::keymapping::SNEAK) {
                    self.keys.release(&KeyCode::ShiftLeft, Source::Touch);
                }
                return Ok(());
            }
            Button::Sprint => return self.touch_sprint(self.touch.stick_sprint),
            Button::SaveToolbar | Button::LoadToolbar | Button::FlyToggle => return Ok(()),
            _ => {}
        }
        if let Some(name) = Self::touch_action_name(button) {
            let modifiers = self.modifiers;
            self.modifiers = self.touch_modifiers();
            let result = match gamepad::route(name) {
                Some(gamepad::Route::Key(key)) => {
                    self.mapped_key(key, down, false, key, Source::Touch)
                }
                Some(gamepad::Route::Sneak) => self.mapped_key(
                    KeyCode::ShiftLeft,
                    down,
                    false,
                    KeyCode::ShiftLeft,
                    Source::Touch,
                ),
                Some(gamepad::Route::Mouse(button)) => self.play_mouse(button, down),
                _ => Ok(()),
            };
            self.modifiers = modifiers;
            result?;
            if down && button == Button::Chat && self.screen == Screen::Chat {
                self.touch_keyboard(true);
            }
            return Ok(());
        }
        // Armed toolbar actions run after the next target-slot tap moves the
        // pointer. Use the existing inventory handlers, including server packets.
        if down
            && matches!(self.screen, Screen::Inventory | Screen::Creative)
            && !self.dialog_open()
        {
            match button {
                Button::MenuHotbar(slot) => {
                    if let Some(&key) = super::HOTBAR_DIGITS.get(slot) {
                        self.inventory_hotkey(key);
                    }
                }
                Button::MenuSwapHands => {
                    self.inventory_hotkey(KeyCode::KeyF);
                }
                Button::MenuDrop | Button::MenuDropStack => {
                    let previous = self.modifiers;
                    self.modifiers
                        .set(ModifiersState::CONTROL, button == Button::MenuDropStack);
                    self.inventory_hotkey(KeyCode::KeyQ);
                    self.modifiers = previous;
                }
                Button::MenuClone => {
                    self.inventory_pick_press();
                    self.inventory_pick_release();
                    if let Some(key) = self.container {
                        self.container_changed(key);
                    }
                }
                Button::MenuCollect => {
                    if let Some(slot) = self.inventory_target().0 {
                        // Pick up a seed stack if needed, then the same menu
                        // PickupAll operation used by double-click collection.
                        if self.carried.is_none() {
                            self.menu_click(Some(slot), 0, horizon_client::menu::Input::Pickup);
                        }
                        self.menu_click(Some(slot), 0, horizon_client::menu::Input::PickupAll);
                        if let Some(key) = self.container {
                            self.container_changed(key);
                        }
                    }
                }
                _ => {}
            }
        }
        match button {
            Button::Pause | Button::Back if down => {
                if self.command_prompt.is_some() {
                    self.command_prompt_key(KeyCode::Escape);
                } else if self.link_prompt.is_some() {
                    self.answer_link_prompt(false);
                } else if self.code_of_conduct.is_some() { /* This screen disallows Escape. */
                } else if self.server_packs.prompt.is_some() {
                    self.answer_pack_prompt(false)?;
                } else if self.dialog_open() {
                    self.dialog_key(KeyCode::Escape, None, None)?;
                } else {
                    self.pad_back()?;
                }
            }
            Button::Keyboard if down => {
                let open = self.pad.menu.osk.is_some();
                self.touch_keyboard(!open);
            }
            _ => {}
        }
        Ok(())
    }

    /// Opens or closes the on-screen keyboard (the controller's, which a tap can type on).
    fn touch_keyboard(&mut self, open: bool) {
        if open {
            if self.pad.menu.osk.is_none() {
                self.pad_open_keyboard();
            }
        } else {
            self.pad.menu.osk = None;
        }
    }

    /// The menu pointer pressed or let go. A press on the on-screen keyboard types its key instead.
    fn touch_pointer_button(&mut self, event_loop: &dyn EventLoopCtl, right: bool, down: bool) {
        if down && self.touch_osk_tap() {
            self.touch.swallowing = true;
            return;
        }
        if !down && self.touch.swallowing {
            self.touch.swallowing = false;
            return;
        }
        let button = if right {
            MouseButton::Right
        } else {
            MouseButton::Left
        };
        self.touch_mouse_button(event_loop, button, down);
    }

    /// A press at the menu pointer on the on-screen keyboard: types the key under it.
    /// True when the press landed on the keyboard (a key or its plate).
    fn touch_osk_tap(&mut self) -> bool {
        if self.pad.menu.osk.is_none() {
            return false;
        }
        let Some(size) = self
            .renderer
            .as_ref()
            .map(|r| [r.config.width, r.config.height])
        else {
            return false;
        };
        let batch = HudBatch::new(size);
        let at_top = self.pad_osk_at_top(&batch);
        // `Ui`'s units: the screen scaled so it reads like a 1040 by 700 window.
        let (pw, ph) = (batch.width * batch.scale, batch.height * batch.scale);
        let density = (pw / 1040.0).min(ph / 700.0).clamp(1.0, 4.0);
        let (x, y) = (
            self.touch.pointer.0 / density,
            self.touch.pointer.1 / density,
        );
        let Some(keyboard) = self.pad.menu.osk.as_mut() else {
            return false;
        };
        match keyboard.hit(pw / density, ph / density, at_top, x, y) {
            Some(Hit::Key(row, col)) => {
                keyboard.focus(row, col);
                if let Some(output) = keyboard.press() {
                    self.pad_osk_output(output);
                }
                true
            }
            Some(Hit::Plate) => true,
            None => false,
        }
    }

    // ---- synthetic window events: what the pad's menu cursor sends, too ----

    fn touch_cursor_to(&mut self, event_loop: &dyn EventLoopCtl, x: f32, y: f32) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        self.handle_window_event(
            event_loop,
            id,
            WindowEvent::CursorMoved {
                device_id: DeviceId::dummy(),
                position: PhysicalPosition::new(f64::from(x), f64::from(y)),
            },
            InputOrigin::User,
        );
    }

    fn touch_mouse_button(
        &mut self,
        event_loop: &dyn EventLoopCtl,
        button: MouseButton,
        down: bool,
    ) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        let modifiers = self.modifiers;
        self.modifiers = self.touch_modifiers();
        self.handle_window_event(
            event_loop,
            id,
            WindowEvent::MouseInput {
                device_id: DeviceId::dummy(),
                state: if down {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
                button,
            },
            InputOrigin::User,
        );
        self.modifiers = modifiers;
    }

    fn touch_wheel(&mut self, event_loop: &dyn EventLoopCtl, x: f32, y: f32) {
        let Some(id) = self.window.as_ref().map(|w| w.id()) else {
            return;
        };
        self.handle_window_event(
            event_loop,
            id,
            WindowEvent::MouseWheel {
                device_id: DeviceId::dummy(),
                delta: MouseScrollDelta::LineDelta(x, y),
                phase: TouchPhase::Moved,
            },
            InputOrigin::User,
        );
    }

    // ---- drawing ----
    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        if !self.touch.enabled {
            return;
        }
        draw::draw(
            batch,
            &self.gui.atlas,
            &self.touch.controls.view(),
            self.touch.controls.mode() == Mode::Playing,
            self.keys.contains(&KeyCode::ShiftLeft),
            self.keys.contains(&KeyCode::ControlLeft),
            self.player.as_ref().is_some_and(|p| p.flying),
        );
    }
}
