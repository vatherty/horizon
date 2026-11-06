//! Admission for a mesh batch; the caller supplies elapsed time so tests can
//! advance a fake clock through even a small batch without a GPU.
use std::time::Duration;

pub struct MeshUploadBudget {
    limit: usize,
    byte_limit: u64,
    deadline: Option<Duration>,
    count: usize,
    bytes: u64,
}
impl MeshUploadBudget {
    pub fn new(limit: usize, byte_limit: u64, deadline: Option<Duration>) -> Self {
        Self {
            limit,
            byte_limit,
            deadline,
            count: 0,
            bytes: 0,
        }
    }
    pub fn allows(&self, bytes: u64, elapsed: Duration) -> bool {
        // At least one mesh progresses, even if it alone is over budget.
        self.count == 0
            || ((self.limit == 0 || self.count < self.limit)
                && (self.byte_limit == 0 || self.bytes.saturating_add(bytes) <= self.byte_limit)
                && self.deadline.is_none_or(|deadline| elapsed < deadline))
    }
    pub fn uploaded(&mut self, bytes: u64) {
        self.count += 1;
        self.bytes = self.bytes.saturating_add(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_small_batch_stops_after_a_slow_first_upload_and_later_progresses() {
        for batch_size in 1..=8 {
            let mut pending = vec![1; batch_size];
            let mut frames = 0;
            while !pending.is_empty() {
                let mut budget = MeshUploadBudget::new(0, 0, Some(Duration::from_millis(2)));
                let mut elapsed = Duration::ZERO;
                while let Some(bytes) = pending.last() {
                    if !budget.allows(*bytes, elapsed) {
                        break;
                    }
                    budget.uploaded(pending.pop().unwrap());
                    elapsed += Duration::from_millis(3);
                }
                frames += 1;
            }
            assert_eq!(frames, batch_size);
        }
    }
    #[test]
    fn explicit_count_and_byte_budgets_remain_upper_bounds() {
        let mut count = MeshUploadBudget::new(2, 0, None);
        assert!(count.allows(12, Duration::ZERO));
        count.uploaded(12);
        count.uploaded(12);
        assert!(!count.allows(1, Duration::ZERO));
        let mut bytes = MeshUploadBudget::new(0, 10, None);
        assert!(bytes.allows(20, Duration::from_secs(1)));
        bytes.uploaded(20);
        assert!(!bytes.allows(1, Duration::ZERO));
    }
}
