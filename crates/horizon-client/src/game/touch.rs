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
use super::{event_loop::EventLoopCtl, input_trace::InputOrigin};
use horizon_client::{
    Result, gamepad,
    hud::{HudAtlas, HudBatch, rgba},
    osk::Hit,
    platform::{self, PointerButton, PointerEvent},
    touch::{self as controls, Action, Button, Layout, Mode, Phase, TouchControls},
};
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    keyboard::KeyCode,
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
            _ => return None,
        })
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
        if self.title.is_none()
            && self.screen == Screen::Playing
            && self.sign_edit.is_none()
            && self.book.is_none()
            // In bed the screen has a "Leave Bed" button to tap.
            && self.sleeping.is_none()
        {
            Mode::Playing
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
                    y: (batch.height - 22.0) * s,
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

    /// Lets go of everything the touch controls hold (the window lost its surface, or the app its
    /// focus). A menu pointer press cannot be undone without an event loop; it ends with its finger.
    pub(super) fn touch_release(&mut self) {
        for action in self.touch.controls.release_all() {
            let result = match action {
                Action::Press(button) => self.touch_button(button, true),
                Action::Release(button) => self.touch_button(button, false),
                Action::Sprint(on) => self.pad_action("key.sprint", on),
                Action::Shift(false) => {
                    self.physical_keys.remove(&KeyCode::ShiftLeft);
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
                    self.pad_action("key.sprint", on)?;
                }
            }
            Action::Press(button) => self.touch_button(button, true)?,
            Action::Release(button) => self.touch_button(button, false)?,
            Action::SelectSlot(slot) => {
                if self.screen == Screen::Playing {
                    self.selected = slot;
                }
            }
            Action::StepSlot(steps) => {
                if self.screen == Screen::Playing {
                    for _ in 0..steps.unsigned_abs() {
                        self.selected = gamepad::hotbar_step(self.selected, steps > 0);
                    }
                }
            }
            Action::PointerMove { x, y } => {
                self.touch.pointer = (x, y);
                self.touch_cursor_to(event_loop, x, y);
            }
            Action::PointerButton { right, down } => {
                self.touch_pointer_button(event_loop, right, down);
            }
            Action::Shift(on) => {
                // `has_shift_down` reads the physical keys.
                if on {
                    self.physical_keys.insert(KeyCode::ShiftLeft);
                } else {
                    self.physical_keys.remove(&KeyCode::ShiftLeft);
                }
            }
            Action::Scroll(lines) => self.touch_wheel(event_loop, 0.0, lines),
        }
        Ok(())
    }

    fn touch_button(&mut self, button: Button, down: bool) -> Result<()> {
        if let Some(name) = Self::touch_action_name(button) {
            self.pad_action(name, down)?;
            // Chat from a touch screen comes with its keyboard.
            if down && button == Button::Chat && self.screen == Screen::Chat {
                self.touch_keyboard(true);
            }
            return Ok(());
        }
        match button {
            Button::Pause | Button::Back if down => self.pad_back()?,
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

    /// The buttons and the stick, over the HUD. Plain fills and the game's font, so the
    /// menu theme's animation clock is left alone.
    pub(super) fn draw_touch(&self, batch: &mut HudBatch) {
        if !self.touch.enabled {
            return;
        }
        let view = self.touch.controls.view();
        let atlas = &self.gui.atlas;
        let s = batch.scale;
        let plate = rgba(24, 24, 28, 120);
        let lit = rgba(255, 255, 255, 150);
        if self.touch.controls.mode() == Mode::Playing {
            let stick = view.stick;
            let (cx, cy, r) = (stick.base.0 / s, stick.base.1 / s, stick.radius / s);
            let alpha = if stick.active { 90 } else { 45 };
            round_rect(
                batch,
                atlas,
                cx - r,
                cy - r,
                2.0 * r,
                2.0 * r,
                r,
                rgba(24, 24, 28, alpha),
            );
            let (kx, ky, kr) = (stick.knob.0 / s, stick.knob.1 / s, r * 0.45);
            let alpha = if stick.active { 170 } else { 70 };
            round_rect(
                batch,
                atlas,
                kx - kr,
                ky - kr,
                2.0 * kr,
                2.0 * kr,
                kr,
                rgba(230, 230, 235, alpha),
            );
        }
        for b in &view.buttons {
            let r = b.rect;
            let (x, y, w, h) = (r.x / s, r.y / s, r.w / s, r.h / s);
            // Sneak stays lit while the player sneaks (it toggles).
            let lit_now =
                b.lit || (b.button == Button::Sneak && self.keys.contains(&KeyCode::ShiftLeft));
            round_rect(
                batch,
                atlas,
                x,
                y,
                w,
                h,
                (h / 4.0).max(2.0),
                if lit_now { lit } else { plate },
            );
            let label = b.button.label();
            let width = HudBatch::text_width(atlas, label);
            let ink = if lit_now {
                rgba(20, 20, 20, 255)
            } else {
                rgba(255, 255, 255, 255)
            };
            batch.text(
                atlas,
                label,
                (x + (w - width) / 2.0).floor(),
                (y + (h - 8.0) / 2.0).floor(),
                ink,
                !lit_now,
            );
        }
    }
}

/// A rounded rectangle from one-pixel rows, in GUI pixels.
fn round_rect(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color: u32,
) {
    let radius = radius.min(w / 2.0).min(h / 2.0).max(0.0);
    let rows = h.ceil() as i32;
    for i in 0..rows {
        let mid = i as f32 + 0.5;
        // How far this row is from the nearest top or bottom edge.
        let edge = mid.min(h - mid);
        let inset = if edge < radius {
            radius
                - (radius * radius - (radius - edge) * (radius - edge))
                    .max(0.0)
                    .sqrt()
        } else {
            0.0
        };
        batch.fill(
            atlas,
            x + inset,
            y + i as f32,
            (w - 2.0 * inset).max(0.0),
            1.0,
            color,
        );
    }
}
