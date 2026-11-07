//! What the host platform tells the game and what the game asks of it.
//!
//! Android and iOS use this today. The platform side of the app (Android's Java
//! `NativeActivity` subclass, or the iOS shell's C exports) and the shell library
//! push events in through the functions here, from whatever thread
//! they run on, and the game loop reads them once per iteration. Nothing here
//! touches the platform itself, so it builds and tests everywhere, and on the
//! desktop it stays idle: no event is ever pushed and nothing asks for
//! anything.
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// Whether the game is running as the Android app.
pub const fn is_android() -> bool {
    cfg!(target_os = "android")
}

/// Whether the game is running as a phone app (Android or iOS).
pub const fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

// ---- logging -------------------------------------------------------------------------------

/// Where log lines go besides the log file: on Android, logcat; on iOS, os_log.
pub type LogSink = fn(priority: u8, message: &str);

static LOG_SINK: std::sync::OnceLock<LogSink> = std::sync::OnceLock::new();

/// Sets the sink once; later calls are ignored.
pub fn set_log_sink(sink: LogSink) {
    let _ = LOG_SINK.set(sink);
}

/// Whether a sink is set, so a caller can skip building the line.
pub fn has_log_sink() -> bool {
    LOG_SINK.get().is_some()
}

/// Sends a log line to the sink, if there is one. `priority` is Android's: 2 verbose,
/// 3 debug, 4 info, 5 warn, 6 error.
pub fn log_to_sink(priority: u8, message: &str) {
    if let Some(sink) = LOG_SINK.get() {
        sink(priority, message);
    }
}

// ---- thermal state -------------------------------------------------------------------------

/// `PowerManager.THERMAL_STATUS_*`.
pub mod thermal {
    pub const NONE: i32 = 0;
    pub const LIGHT: i32 = 1;
    pub const MODERATE: i32 = 2;
    pub const SEVERE: i32 = 3;
    pub const CRITICAL: i32 = 4;
    pub const EMERGENCY: i32 = 5;
    pub const SHUTDOWN: i32 = 6;
}

static THERMAL_STATUS: AtomicI32 = AtomicI32::new(thermal::NONE);

/// The device's thermal status as the platform last reported it.
pub fn set_thermal_status(status: i32) {
    THERMAL_STATUS.store(status, Ordering::Relaxed);
}

pub fn thermal_status() -> i32 {
    THERMAL_STATUS.load(Ordering::Relaxed)
}

/// The stable scalar bridge from Apple's `ProcessInfo.ThermalState`: 0 nominal,
/// 1 fair, 2 serious, 3 critical. These are deliberately translated rather than
/// passed to the Android status scale, where 1 and 2 have different meanings.
pub fn set_apple_thermal_status(state: i32) {
    set_thermal_status(apple_thermal_status(state));
}

fn apple_thermal_status(state: i32) -> i32 {
    match state {
        0 => thermal::NONE,
        1 => thermal::MODERATE,
        2 => thermal::SEVERE,
        _ => thermal::CRITICAL,
    }
}

/// The most frames per second the game should draw at a thermal status:
/// nothing until the device is moderately warm, then 45, 30 and 20. The
/// limiter takes the lower of this and the player's own limit, so a cool
/// device keeps whatever the options say.
pub fn thermal_fps_cap(status: i32) -> Option<u32> {
    match status {
        i32::MIN..=thermal::LIGHT => None,
        thermal::MODERATE => Some(45),
        thermal::SEVERE => Some(30),
        _ => Some(20),
    }
}

// OS power policy is independent of the player's Battery Mode setting. Clearing
// it restores the current player/thermal cap without ever changing options files.
static LOW_POWER_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_low_power_mode(enabled: bool) {
    LOW_POWER_MODE.store(enabled, Ordering::Relaxed);
}

pub fn low_power_mode() -> bool {
    LOW_POWER_MODE.load(Ordering::Relaxed)
}

/// The common rendering cap for gameplay and loading. `in_world` means active
/// gameplay, not a menu drawn over a world. The caller first applies its vanilla
/// inactivity limit; every power policy may only lower that result.
pub fn effective_fps_cap(
    player_limit: u32,
    battery_mode: bool,
    in_world: bool,
    focused: bool,
) -> u32 {
    intersect_fps_caps(
        player_limit,
        battery_mode,
        in_world,
        focused,
        thermal_status(),
        low_power_mode(),
    )
}

fn intersect_fps_caps(
    player_limit: u32,
    battery_mode: bool,
    in_world: bool,
    focused: bool,
    thermal: i32,
    os_low_power: bool,
) -> u32 {
    let mut cap = player_limit.max(1);
    if battery_mode {
        cap = cap.min(if in_world && focused { 60 } else { 30 });
    }
    if let Some(thermal) = thermal_fps_cap(thermal) {
        cap = cap.min(thermal);
    }
    if os_low_power {
        cap = cap.min(30);
    }
    cap
}

// ---- render resolution ------------------------------------------------------------------

/// The longest edge, in pixels, the world may render at on iOS: 720p class
/// while any power saving is on (the phone seed's default), 1080p class as the
/// pushed option, and never more — a phone panel's native size (2532x1170, let
/// alone the desktop-flavoured 3840x2100 a windowed default asks for) is more
/// pixels than the game needs to draw. `None` where no cap applies: every
/// platform other than iOS keeps the resolution scale exactly as the player
/// set it. The window and the HUD keep the panel's size; only the world's
/// render target is capped (`gfx::world_post`).
pub fn render_long_edge_cap(battery_mode: bool) -> Option<u32> {
    if !cfg!(target_os = "ios") {
        return None;
    }
    Some(phone_render_long_edge_cap(battery_mode, low_power_mode()))
}

fn phone_render_long_edge_cap(battery_mode: bool, os_low_power: bool) -> u32 {
    if battery_mode || os_low_power {
        1280
    } else {
        1920
    }
}

// ---- pointer events ------------------------------------------------------------------------

/// A mouse button, as Android's `MotionEvent.BUTTON_*` bits name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// A mouse event from the platform. Android's NativeActivity input queue
/// carries a mouse as a touch pointer (winit 0.30 drops its hover, wheel and
/// button events), so the Java side forwards the real mouse here. iOS has no
/// mouse: nothing is ever pushed there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    /// The pointer is at this window position, in physical pixels.
    Moved {
        x: f32,
        y: f32,
    },
    /// Relative motion while the pointer is captured.
    Relative {
        dx: f32,
        dy: f32,
    },
    Button {
        button: PointerButton,
        down: bool,
    },
    /// The wheel, in lines (`AXIS_HSCROLL`, `AXIS_VSCROLL`).
    Scroll {
        x: f32,
        y: f32,
    },
}

/// More than this many unread events and the oldest are dropped: a game
/// that stopped reading must not grow the queue without end.
const MAX_QUEUED: usize = 4096;

static POINTER: Mutex<VecDeque<PointerEvent>> = Mutex::new(VecDeque::new());
static POINTER_SEEN: AtomicBool = AtomicBool::new(false);

/// Queues a mouse event for the game loop.
pub fn push_pointer(event: PointerEvent) {
    POINTER_SEEN.store(true, Ordering::Relaxed);
    let mut queue = POINTER.lock().unwrap_or_else(|e| e.into_inner());
    if queue.len() >= MAX_QUEUED {
        queue.pop_front();
    }
    queue.push_back(event);
}

/// Takes every queued mouse event, oldest first.
pub fn drain_pointer() -> Vec<PointerEvent> {
    let mut queue = POINTER.lock().unwrap_or_else(|e| e.into_inner());
    queue.drain(..).collect()
}

/// A mouse has sent an event since the app started.
pub fn pointer_seen() -> bool {
    POINTER_SEEN.load(Ordering::Relaxed)
}

// ---- leaving ---------------------------------------------------------------------------------

static EXIT: AtomicBool = AtomicBool::new(false);

/// The platform is ending the app (Android: the activity is being destroyed, and its
/// `onDestroy` waits for the game thread to finish; iOS: the scene is disconnecting):
/// the game saves and leaves its loop.
pub fn request_exit() {
    EXIT.store(true, Ordering::Relaxed);
}

pub fn exit_requested() -> bool {
    EXIT.load(Ordering::Relaxed)
}

// ---- requests to the platform --------------------------------------------------------------

/// What the game wants from the platform, as bits the platform side polls.
const WANT_CAPTURE: i32 = 1;
const WANT_KEYBOARD: i32 = 2;

static WANTS: AtomicI32 = AtomicI32::new(0);

fn set_want(bit: i32, on: bool) {
    if on {
        WANTS.fetch_or(bit, Ordering::Relaxed);
    } else {
        WANTS.fetch_and(!bit, Ordering::Relaxed);
    }
}

/// The game grabs the mouse (mouse look): the platform should capture the pointer.
pub fn set_want_capture(on: bool) {
    set_want(WANT_CAPTURE, on);
}

/// A text field has focus: the platform should show its soft keyboard.
pub fn set_want_keyboard(on: bool) {
    set_want(WANT_KEYBOARD, on);
}

/// The request bits for the platform side's poll (bit 0: capture, bit 1: keyboard).
pub fn wants() -> i32 {
    WANTS.load(Ordering::Relaxed)
}

pub fn wants_capture() -> bool {
    wants() & WANT_CAPTURE != 0
}

pub fn wants_keyboard() -> bool {
    wants() & WANT_KEYBOARD != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_follow_the_thermal_status() {
        assert_eq!(thermal_fps_cap(thermal::NONE), None);
        assert_eq!(thermal_fps_cap(thermal::LIGHT), None);
        assert_eq!(thermal_fps_cap(thermal::MODERATE), Some(45));
        assert_eq!(thermal_fps_cap(thermal::SEVERE), Some(30));
        assert_eq!(thermal_fps_cap(thermal::CRITICAL), Some(20));
        assert_eq!(thermal_fps_cap(thermal::SHUTDOWN), Some(20));
        assert_eq!(thermal_fps_cap(-1), None);
    }

    #[test]
    fn phone_render_cap_is_720p_under_power_saving_and_1080p_pushed() {
        assert_eq!(phone_render_long_edge_cap(true, false), 1280);
        assert_eq!(phone_render_long_edge_cap(false, true), 1280);
        assert_eq!(phone_render_long_edge_cap(true, true), 1280);
        assert_eq!(phone_render_long_edge_cap(false, false), 1920);
    }

    #[test]
    fn apple_states_enter_the_cap_at_their_own_severity() {
        assert_eq!(thermal_fps_cap(apple_thermal_status(0)), None);
        assert_eq!(thermal_fps_cap(apple_thermal_status(1)), Some(45));
        assert_eq!(thermal_fps_cap(apple_thermal_status(2)), Some(30));
        assert_eq!(thermal_fps_cap(apple_thermal_status(3)), Some(20));
        assert_eq!(thermal_fps_cap(apple_thermal_status(99)), Some(20));
    }

    #[test]
    fn independent_caps_intersect_and_recover_to_the_players_current_choice() {
        let cap = intersect_fps_caps;
        assert_eq!(cap(120, true, true, true, thermal::NONE, false), 60);
        assert_eq!(cap(120, true, false, true, thermal::NONE, false), 30);
        assert_eq!(cap(120, true, true, false, thermal::NONE, false), 30);
        assert_eq!(cap(25, true, true, true, thermal::MODERATE, true), 25);
        assert_eq!(cap(120, false, true, true, thermal::MODERATE, false), 45);
        assert_eq!(cap(120, false, true, true, thermal::MODERATE, true), 30);
        assert_eq!(cap(120, true, true, true, thermal::CRITICAL, true), 20);
        // Power clears while still hot, then thermal recovers. A new player
        // choice made while hot is retained throughout both transitions.
        assert_eq!(cap(50, true, true, true, thermal::CRITICAL, false), 20);
        assert_eq!(cap(50, true, true, true, thermal::NONE, false), 50);
        assert_eq!(cap(120, false, true, true, thermal::NONE, false), 120);
    }

    #[test]
    fn pointer_events_come_out_in_order_and_the_queue_is_bounded() {
        drain_pointer();
        push_pointer(PointerEvent::Moved { x: 1.0, y: 2.0 });
        push_pointer(PointerEvent::Button {
            button: PointerButton::Left,
            down: true,
        });
        assert!(pointer_seen());
        assert_eq!(
            drain_pointer(),
            vec![
                PointerEvent::Moved { x: 1.0, y: 2.0 },
                PointerEvent::Button {
                    button: PointerButton::Left,
                    down: true
                }
            ]
        );
        assert!(drain_pointer().is_empty());
        for i in 0..MAX_QUEUED + 10 {
            push_pointer(PointerEvent::Scroll {
                x: 0.0,
                y: i as f32,
            });
        }
        let all = drain_pointer();
        assert_eq!(all.len(), MAX_QUEUED);
        assert_eq!(all[0], PointerEvent::Scroll { x: 0.0, y: 10.0 });
    }

    #[test]
    fn requests_set_and_clear_their_own_bits() {
        set_want_capture(true);
        set_want_keyboard(true);
        assert_eq!(wants() & 3, 3);
        set_want_capture(false);
        assert!(!wants_capture());
        assert!(wants_keyboard());
        set_want_keyboard(false);
        assert_eq!(wants() & 3, 0);
    }
}
