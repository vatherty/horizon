//! The window only posts lifecycle requests. The integrated server owns the
//! world and consumes them between ticks, including requests made while busy.
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// Foreground frame time, separate from FixedStep's simulation clock. Injecting
/// `now` also lets resume behavior be tested without waiting for wall time.
pub(super) struct FrameTime(Instant);

impl FrameTime {
    pub fn new(now: Instant) -> Self {
        Self(now)
    }

    pub fn advance(&mut self, now: Instant) -> Duration {
        now.saturating_duration_since(std::mem::replace(&mut self.0, now))
    }

    pub fn resume(&mut self, now: Instant) {
        self.0 = now;
    }
}

const MENU_PAUSED: u8 = 1;
const BACKGROUND: u8 = 2;
const SAVE: u8 = 4;
const RESET_WALL_TIME: u8 = 8;

#[derive(Default)]
pub(super) struct LifecycleRequests(AtomicU8);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Actions {
    pub paused: bool,
    pub save: bool,
    pub reset_wall_time: bool,
}

impl LifecycleRequests {
    pub fn set_menu_paused(&self, paused: bool) {
        if paused {
            self.0.fetch_or(MENU_PAUSED, Ordering::Relaxed);
        } else {
            self.0.fetch_and(!MENU_PAUSED, Ordering::Relaxed);
        }
    }

    /// Never waits for a tick, save, mutex or reply. A quick return to the app
    /// clears the background pause but retains the pending save request.
    pub fn set_backgrounded(&self, backgrounded: bool, menu_paused: bool) {
        let mut state = self.0.load(Ordering::Relaxed);
        loop {
            // A screen may have opened since the last redraw published its
            // pause bit. Capture its current intent in this same atomic update.
            let mut next = state & !MENU_PAUSED;
            if menu_paused {
                next |= MENU_PAUSED;
            }
            if (state & BACKGROUND != 0) != backgrounded {
                next = if backgrounded {
                    next | BACKGROUND | SAVE | RESET_WALL_TIME
                } else {
                    (next & !BACKGROUND) | RESET_WALL_TIME
                };
            }
            if next == state {
                return;
            }
            match self
                .0
                .compare_exchange_weak(state, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return,
                Err(updated) => state = updated,
            }
        }
    }

    /// Owner-thread snapshot. Published LAN and other local seats retain their
    /// normal simulation/network ticks, but still receive the lifecycle save.
    pub fn take(&self, was_paused: bool, published: bool, split_players: bool) -> Actions {
        let state = self
            .0
            .fetch_and(!(SAVE | RESET_WALL_TIME), Ordering::Relaxed);
        let paused = !published && !split_players && state & (MENU_PAUSED | BACKGROUND) != 0;
        Actions {
            paused,
            save: state & SAVE != 0 || (paused && !was_paused),
            reset_wall_time: state & RESET_WALL_TIME != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, mpsc::channel};

    #[test]
    fn private_world_saves_once_and_holds_until_foreground() {
        let requests = LifecycleRequests::default();
        requests.set_backgrounded(true, false);
        requests.set_backgrounded(true, false);
        assert_eq!(
            requests.take(false, false, false),
            Actions {
                paused: true,
                save: true,
                reset_wall_time: true,
            }
        );
        assert_eq!(
            requests.take(true, false, false),
            Actions {
                paused: true,
                save: false,
                reset_wall_time: false,
            }
        );
        requests.set_backgrounded(false, false);
        assert_eq!(
            requests.take(true, false, false),
            Actions {
                paused: false,
                save: false,
                reset_wall_time: true,
            }
        );
    }

    #[test]
    fn foreground_preserves_a_preexisting_pause_screen() {
        let requests = LifecycleRequests::default();
        requests.set_menu_paused(true);
        assert!(requests.take(false, false, false).paused);
        requests.set_backgrounded(true, true);
        assert!(requests.take(true, false, false).save);
        requests.set_backgrounded(false, true);
        let resumed = requests.take(true, false, false);
        assert!(resumed.paused);
        assert!(!resumed.save);
        requests.set_menu_paused(false);
        assert!(!requests.take(true, false, false).paused);
    }

    #[test]
    fn lan_and_other_local_players_save_without_pausing() {
        for (published, split) in [(true, false), (false, true), (true, true)] {
            let requests = LifecycleRequests::default();
            requests.set_menu_paused(true);
            requests.set_backgrounded(true, true);
            assert_eq!(
                requests.take(false, published, split),
                Actions {
                    paused: false,
                    save: true,
                    reset_wall_time: true,
                }
            );
            assert!(!requests.take(false, published, split).save);
            // Closing LAN restores the still-requested private pause.
            assert!(requests.take(false, false, false).paused);
        }
    }

    #[test]
    fn quick_resume_cannot_cancel_a_save_waiting_for_the_owner() {
        let requests = LifecycleRequests::default();
        requests.set_backgrounded(true, false);
        requests.set_backgrounded(false, false);
        requests.set_backgrounded(false, false);
        assert_eq!(
            requests.take(false, false, false),
            Actions {
                paused: false,
                save: true,
                reset_wall_time: true,
            }
        );
        assert_eq!(
            requests.take(false, false, false),
            Actions {
                paused: false,
                save: false,
                reset_wall_time: false,
            }
        );
    }

    #[test]
    fn lifecycle_callback_returns_while_the_owner_is_still_saving() {
        let requests = Arc::new(LifecycleRequests::default());
        let owner = requests.clone();
        let (saving, started) = channel();
        let (release, waiting) = channel();
        requests.set_backgrounded(true, false);
        let thread = std::thread::spawn(move || {
            assert!(owner.take(false, false, false).save);
            saving.send(()).unwrap();
            waiting.recv().unwrap();
            owner.take(true, false, false)
        });
        started.recv().unwrap();
        // These calls must finish before the saving owner can be released.
        requests.set_backgrounded(false, false);
        requests.set_backgrounded(true, false);
        release.send(()).unwrap();
        let next = thread.join().unwrap();
        assert!(next.save && next.paused && next.reset_wall_time);
    }

    #[test]
    fn a_pause_opened_since_the_last_redraw_survives_resume() {
        let requests = LifecycleRequests::default();
        requests.set_menu_paused(false); // Last rendered screen was Playing.
        requests.set_backgrounded(true, true); // Pause screen just opened.
        let background = requests.take(false, false, false);
        assert!(background.paused && background.save);
        requests.set_backgrounded(false, true);
        let resumed = requests.take(true, false, false);
        assert!(resumed.paused);
        assert!(!resumed.save);
    }

    #[test]
    fn resumed_frames_exclude_suspended_time_and_preserve_simulation_state() {
        let start = Instant::now();
        let mut frame = FrameTime::new(start);
        let mut simulation = horizon_client::clock::FixedStep::client();
        simulation.set_rate(10.0);
        assert_eq!(
            simulation.advance(frame.advance(start + Duration::from_millis(25))),
            0
        );
        assert_eq!(simulation.fraction(), 0.25);
        simulation.set_frozen(true);
        assert!(simulation.start_step(3));
        let step = simulation.step_state();

        let resumed = start + Duration::from_secs(6 * 60 * 60);
        frame.resume(resumed);
        assert_eq!(simulation.fraction(), 0.25);
        let elapsed = frame.advance(resumed + Duration::from_millis(25));
        assert_eq!(elapsed, Duration::from_millis(25));
        assert_eq!(simulation.advance(elapsed), 0, "no suspended tick backlog");
        assert_eq!(simulation.fraction(), 0.5);
        assert_eq!(simulation.rate(), 10.0);
        assert!(simulation.frozen());
        assert_eq!(simulation.step_state(), step);
    }
}
