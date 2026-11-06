//! Opt-in GPU-feedback experiment; the user's scale is always the ceiling.
use std::time::Duration;

pub(super) struct Controller {
    pub enabled: bool,
    ceiling: f32,
    scale: f32,
    budget_ms: f32,
    pressure: i8,
    last_change: Option<Duration>,
    pub changes: u64,
}
impl Controller {
    pub fn from_environment() -> Self {
        Self::new(
            cfg!(target_os = "ios")
                && std::env::var("HORIZON_IOS_DYNAMIC_RESOLUTION").as_deref() == Ok("1"),
        )
    }
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ceiling: 1.0,
            scale: 1.0,
            budget_ms: 1000.0 / 60.0,
            pressure: 0,
            last_change: None,
            changes: 0,
        }
    }
    pub fn set_ceiling(&mut self, scale: f32) -> f32 {
        let ceiling = scale.clamp(0.25, 2.0);
        if self.ceiling != ceiling {
            self.ceiling = ceiling;
            self.scale = self.scale.min(ceiling);
            self.pressure = 0;
        }
        if self.enabled { self.scale } else { ceiling }
    }
    pub fn set_budget(&mut self, fps: u32) {
        self.budget_ms = 1000.0 / fps.clamp(15, 240) as f32;
    }
    pub fn observe(&mut self, ms: f32, now: Duration) -> Option<f32> {
        if !self.enabled || !ms.is_finite() || ms <= 0.0 {
            return None;
        }
        if self
            .last_change
            .is_some_and(|t| now.saturating_sub(t) < Duration::from_secs(5))
        {
            return None;
        }
        self.pressure = if ms > self.budget_ms * 1.10 {
            self.pressure.min(0) - 1
        } else if ms < self.budget_ms * 0.75 {
            self.pressure.max(0) + 1
        } else {
            0
        };
        let floor = self.ceiling.min(0.5);
        let next = if self.pressure <= -3 {
            ((self.scale * 0.9 * 20.0).floor() / 20.0).max(floor)
        } else if self.pressure >= 8 {
            (self.scale + 0.05).min(self.ceiling)
        } else {
            return None;
        };
        self.pressure = 0;
        if (next - self.scale).abs() < 0.001 {
            return None;
        }
        self.scale = next;
        self.last_change = Some(now);
        self.changes += 1;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sustained_pressure_changes_infrequently_within_user_ceiling() {
        let mut c = Controller::new(true);
        c.set_ceiling(0.8);
        assert_eq!(c.observe(30.0, Duration::ZERO), None);
        assert_eq!(c.observe(30.0, Duration::from_secs(1)), None);
        assert_eq!(c.observe(30.0, Duration::from_secs(2)), Some(0.7));
        for second in 3..7 {
            assert_eq!(c.observe(30.0, Duration::from_secs(second)), None);
        }
        for second in 7..10 {
            c.observe(30.0, Duration::from_secs(second));
        }
        assert_eq!(c.scale, 0.6);
        for second in 14..80 {
            c.observe(1.0, Duration::from_secs(second));
            assert!(c.scale <= 0.8);
        }
        assert_eq!(c.scale, 0.8);
        assert_eq!(c.set_ceiling(0.25), 0.25);
        for second in 80..100 {
            assert_eq!(c.observe(99.0, Duration::from_secs(second)), None);
        }
    }
    #[test]
    fn disabled_invalid_and_hysteresis_samples_do_not_allocate_new_sizes() {
        let mut c = Controller::new(false);
        assert_eq!(c.set_ceiling(1.5), 1.5);
        for _ in 0..30 {
            assert_eq!(c.observe(99.0, Duration::ZERO), None);
        }
        c.enabled = true;
        c.set_budget(30);
        for ms in [f32::NAN, f32::INFINITY, 0.0, -1.0, 33.0, 35.0] {
            assert_eq!(c.observe(ms, Duration::from_secs(30)), None);
        }
        assert_eq!(c.changes, 0);
    }
}
