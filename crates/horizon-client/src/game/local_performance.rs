//! Local diagnostics, independent of telemetry consent/outboxes. One replaced
//! JSON file, fixed histograms, and at most one queued write every 30 seconds.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::mpsc::{SyncSender, sync_channel},
    time::{Duration, Instant},
};

pub(super) const FILE: &str = "performance-summary.json";
const INTERVAL: Duration = Duration::from_secs(30);
const BINS: usize = 256;

#[derive(Clone)]
struct Metric {
    bins: [u64; BINS],
    count: u64,
    nanos: u128,
    max: Duration,
}
impl Default for Metric {
    fn default() -> Self {
        Self {
            bins: [0; BINS],
            count: 0,
            nanos: 0,
            max: Duration::ZERO,
        }
    }
}
impl Metric {
    fn record(&mut self, value: Duration) {
        let bin = value.as_millis().min((BINS - 1) as u128) as usize;
        self.bins[bin] = self.bins[bin].saturating_add(1);
        self.count = self.count.saturating_add(1);
        self.nanos = self.nanos.saturating_add(value.as_nanos());
        self.max = self.max.max(value);
    }
    fn percentile_ms(&self, percent: u64) -> f64 {
        let target = self.count.saturating_mul(percent).div_ceil(100);
        let mut count = 0u64;
        for (bin, samples) in self.bins.iter().enumerate() {
            count = count.saturating_add(*samples);
            if count >= target {
                return if bin == BINS - 1 {
                    self.max.as_secs_f64() * 1000.0
                } else {
                    (bin + 1) as f64
                };
            }
        }
        0.0
    }
    fn summary(&self) -> Value {
        if self.count == 0 {
            return Value::Null;
        }
        json!({"samples": self.count, "mean_ms": self.nanos as f64 / self.count as f64 / 1e6,
            "max_ms": self.max.as_secs_f64() * 1000.0,
            "p50_ms_upper_bound": self.percentile_ms(50),
            "p95_ms_upper_bound": self.percentile_ms(95),
            "p99_ms_upper_bound": self.percentile_ms(99)})
    }
}

#[derive(Default)]
pub(super) struct LocalPerformance(Option<Session>);
struct Session {
    writer: SyncSender<Value>,
    started: Instant,
    next: Instant,
    last_frame: Option<Instant>,
    presented: u64,
    frame: Metric,
    cpu: Metric,
    submission: Metric,
    gpu: Metric,
}
impl LocalPerformance {
    pub fn new(path: Option<PathBuf>, now: Instant) -> Self {
        let Some(path) = path else {
            return Self::default();
        };
        let (writer, receiver) = sync_channel::<Value>(1);
        let Ok(_) = std::thread::Builder::new()
            .name("Local performance".into())
            .spawn(move || {
                while let Ok(snapshot) = receiver.recv() {
                    let _ = write_snapshot(&path, &snapshot);
                }
            })
        else {
            return Self::default();
        };
        Self(Some(Session {
            writer,
            started: now,
            next: now,
            last_frame: None,
            presented: 0,
            frame: Metric::default(),
            cpu: Metric::default(),
            submission: Metric::default(),
            gpu: Metric::default(),
        }))
    }
    pub fn enabled(&self) -> bool {
        self.0.is_some()
    }
    pub fn resume(&mut self) {
        if let Some(session) = &mut self.0 {
            session.last_frame = None;
        }
    }
    pub fn record_at(
        &mut self,
        now: Instant,
        cpu: Option<Duration>,
        submission_wall: Duration,
        gpu: &[f32],
    ) {
        let Some(session) = &mut self.0 else {
            return;
        };
        if let Some(previous) = session.last_frame {
            session
                .frame
                .record(now.saturating_duration_since(previous));
        }
        session.last_frame = Some(now);
        session.presented = session.presented.saturating_add(1);
        if let Some(cpu) = cpu {
            session.cpu.record(cpu);
        }
        session.submission.record(submission_wall);
        for &ms in gpu {
            if ms.is_finite()
                && ms > 0.0
                && let Ok(duration) = Duration::try_from_secs_f64(f64::from(ms) / 1000.0)
            {
                session.gpu.record(duration);
            }
        }
    }
    pub fn due(&self, now: Instant) -> bool {
        self.0.as_ref().is_some_and(|session| now >= session.next)
    }
    pub fn publish(&mut self, now: Instant, details: Value) {
        let Some(session) = &mut self.0 else {
            return;
        };
        if now < session.next {
            return;
        }
        session.next = now + INTERVAL;
        let snapshot = json!({"schema": 1, "scope": "local_only", "uptime_seconds": now.saturating_duration_since(session.started).as_secs_f64(),
            "presented_frames": session.presented, "histogram_bucket_ms": 1,
            "frames": session.frame.summary(), "thread_cpu": session.cpu.summary(),
            "submission_wall_excluding_surface_wait": session.submission.summary(), "gpu": session.gpu.summary(), "current": details});
        let _ = session.writer.try_send(snapshot);
    }
}

fn write_snapshot(path: &std::path::Path, snapshot: &Value) -> std::io::Result<()> {
    let bytes = serde_json::to_vec_pretty(snapshot)?;
    // The fixed schema is a few KiB. Refuse unexpectedly unbounded metadata.
    if bytes.len() > 32 * 1024 {
        return Err(std::io::Error::other("performance summary too large"));
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn histograms_are_bounded_and_missing_cpu_and_gpu_are_unavailable() {
        let (writer, receiver) = sync_channel(1);
        let now = Instant::now();
        let mut report = LocalPerformance(Some(Session {
            writer,
            started: now,
            next: now,
            last_frame: None,
            presented: 0,
            frame: Metric::default(),
            cpu: Metric::default(),
            submission: Metric::default(),
            gpu: Metric::default(),
        }));
        for frame in 0..100_000 {
            report.record_at(
                now + Duration::from_millis(frame * 20),
                None,
                Duration::from_millis(3),
                &[0.0, f32::NAN],
            );
        }
        report.publish(now, json!({"phase":"world"}));
        let snapshot = receiver.recv().unwrap();
        assert_eq!(snapshot["frames"]["mean_ms"], 20.0);
        assert_eq!(snapshot["frames"]["p99_ms_upper_bound"], 21.0);
        assert!(snapshot["thread_cpu"].is_null());
        assert!(snapshot["gpu"].is_null());
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < 2048);
        assert!(!report.due(now + Duration::from_secs(29)));
        assert!(report.due(now + Duration::from_secs(30)));
        report.resume();
        report.record_at(
            now + Duration::from_secs(12_000),
            Some(Duration::from_millis(2)),
            Duration::from_millis(4),
            &[1.5],
        );
        report.publish(now + Duration::from_secs(12_000), json!({}));
        let snapshot = receiver.recv().unwrap();
        assert_eq!(snapshot["frames"]["samples"], 99_999);
        assert_eq!(snapshot["thread_cpu"]["mean_ms"], 2.0);
        assert_eq!(snapshot["gpu"]["mean_ms"], 1.5);
    }

    #[test]
    fn the_local_report_replaces_one_file() {
        let root =
            std::env::temp_dir().join(format!("horizon-local-summary-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(FILE);
        write_snapshot(&path, &json!({"frames":1})).unwrap();
        write_snapshot(&path, &json!({"frames":2})).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(&path).unwrap()).unwrap()["frames"],
            2
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
