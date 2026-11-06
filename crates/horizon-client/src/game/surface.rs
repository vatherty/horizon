//! The window's surface coming and going (Android and iOS): the platform destroys the native
//! window when the app goes to the background and often when the screen changes shape (a fold or
//! unfold, a rotation, split screen), and creates a new one, of whatever size, when it comes back.
//! The renderer keeps its device, pipelines and textures through that; only the swapchain goes.
use super::Game;
use std::time::Instant;
use winit::event_loop::ActiveEventLoop;

impl Game<'_> {
    /// `ApplicationHandler::suspended`: the native window is about to be destroyed, so the surface
    /// has to be dropped before this returns. Nothing happens off Android and iOS, where it is
    /// never called.
    pub(super) fn surface_lost(&mut self) {
        if !cfg!(any(target_os = "android", target_os = "ios")) {
            return;
        }
        if self.lifecycle_suspended {
            return;
        }
        self.lifecycle_suspended = true;
        // Only the server owner may snapshot/save this world. This atomic
        // request returns even when that thread is busy saving or loading.
        self.background_integrated(true);
        log::info!("Horizon: the window's surface was destroyed");
        if let Some(renderer) = &mut self.renderer {
            renderer.suspend_surface();
        }
        // A game in the background makes no noise (the music would play on).
        self.audio.stop_all();
        // Fingers and keys that were down will never be released.
        self.keys.clear();
        self.physical_keys.clear();
        self.physical_buttons.clear();
        self.left = false;
        self.right = false;
        self.touch_release();
    }

    /// `ApplicationHandler::resumed` with the window already made: a new surface of the window's
    /// size now.
    pub(super) fn surface_back(&mut self, event_loop: &ActiveEventLoop) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        if !renderer.has_surface() {
            let size = window.inner_size();
            match renderer.reattach_surface(size.width, size.height) {
                Ok(()) => log::info!(
                    "Horizon: the window's surface is back ({}x{})",
                    size.width,
                    size.height
                ),
                Err(e) => {
                    log::error!("Horizon: could not attach the window's surface again: {e}");
                    self.error = Some(e);
                    event_loop.exit();
                    return;
                }
            }
        }
        if self.lifecycle_suspended {
            self.lifecycle_suspended = false;
            let now = Instant::now();
            // Feed only foreground time to FixedStep::advance. Do not reset
            // its partial tick, pause/freeze/step state, rate or simulation.
            self.previous.resume(now);
            self.local_performance.resume();
            self.frame_end = now;
            self.frame_ticks = 0.0;
            self.frame_drawn = false;
            self.fps_at = now;
            self.fps_frames = 0;
            self.frame_clock = Default::default();
            self.limiter = horizon_client::window::FramerateLimiter::new(0);
            self.background_integrated(false);
        }
    }

    fn background_integrated(&self, backgrounded: bool) {
        let menu_paused = self.screen_requests_pause();
        if let Some(server) = &self.integrated {
            server.set_backgrounded(backgrounded, menu_paused);
        }
        if let Some(starting) = &self.integrated_starting {
            starting.handle.set_backgrounded(backgrounded, menu_paused);
        }
    }
}
