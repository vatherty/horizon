//! Deadline state independent of winit, so repeated event-loop wakes and OS
//! redraws cannot turn the loading screen into a polling render loop.
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct LoadingSchedule {
    next: Option<Instant>,
    pending: bool,
}
impl LoadingSchedule {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn request(&mut self, now: Instant) -> bool {
        if self.pending || self.next.is_some_and(|next| now < next) {
            return false;
        }
        self.pending = true;
        true
    }
    pub fn begin_draw(&mut self, now: Instant) -> bool {
        self.pending = false;
        self.next.is_none_or(|next| now >= next)
    }
    pub fn drawn(&mut self, now: Instant, period: Duration) {
        self.next = Some(now + period);
    }
    pub fn wake_at(&self, now: Instant, period: Duration) -> Instant {
        // A hidden window may never deliver its requested redraw. Continue
        // polling loader/error results at the normal cadence in that case.
        self.next
            .filter(|next| *next > now)
            .map_or(now + period, |next| next.min(now + period))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_wakes_and_os_redraws_wait_for_the_deadline() {
        let now = Instant::now();
        let period = Duration::from_millis(33);
        let mut schedule = LoadingSchedule::default();
        assert!(schedule.request(now));
        assert!(!schedule.request(now));
        assert_eq!(schedule.wake_at(now, period), now + period);
        assert!(schedule.begin_draw(now));
        schedule.drawn(now, period);
        for ms in 1..33 {
            let at = now + Duration::from_millis(ms);
            assert!(!schedule.begin_draw(at));
            assert!(!schedule.request(at));
            assert_eq!(schedule.wake_at(at, period), now + period);
        }
        assert!(schedule.request(now + period));
        assert!(schedule.begin_draw(now + period));
        schedule.drawn(now + period, period);
        schedule.reset();
        assert!(schedule.request(now + period));
    }

    #[test]
    fn a_hidden_redraw_and_slower_cap_still_poll_results_and_resume_is_immediate() {
        let now = Instant::now();
        let poll = Duration::from_millis(33);
        let mut schedule = LoadingSchedule::default();
        assert!(schedule.request(now));
        for frame in 1..10 {
            let at = now + poll * frame;
            assert!(!schedule.request(at));
            assert_eq!(schedule.wake_at(at, poll), at + poll);
        }
        schedule.reset();
        assert!(schedule.begin_draw(now));
        schedule.drawn(now, Duration::from_secs(1));
        assert_eq!(schedule.wake_at(now, poll), now + poll);
        assert!(!schedule.request(now + poll));
        schedule.reset();
        assert!(schedule.request(now + poll));
        assert!(schedule.begin_draw(now + poll));
    }
}
