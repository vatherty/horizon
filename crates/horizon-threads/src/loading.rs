//! A shared budget for nested resource loading. A parent never waits to spawn:
//! when every slot is busy its child runs inline at join, on the parent's slot.
//! Model/sprite maps inherit the same budget instead of starting another pool.
use std::{
    cell::RefCell,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
    thread::{Scope, ScopedJoinHandle},
};

thread_local! {
    static CURRENT: RefCell<Option<Budget>> = const { RefCell::new(None) };
}

#[derive(Clone)]
pub struct Budget(Arc<Inner>);

struct Inner {
    limit: usize,
    active: AtomicUsize,
    wait: Mutex<()>,
    wake: Condvar,
}

impl Budget {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Inner {
            limit: limit.max(1),
            active: AtomicUsize::new(0),
            wait: Mutex::new(()),
            wake: Condvar::new(),
        }))
    }

    /// The enclosing generation's budget, or the process's default for reloads.
    pub fn current() -> Self {
        CURRENT
            .with(|current| current.borrow().clone())
            .unwrap_or_else(|| {
                static DEFAULT: OnceLock<Budget> = OnceLock::new();
                DEFAULT
                    .get_or_init(|| Budget::new(crate::worker_count()))
                    .clone()
            })
    }

    /// Reuse an enclosing resource generation, including pre-resource mod
    /// overlays; otherwise start one with the selected settings count.
    pub fn current_or_new(limit: usize) -> Self {
        CURRENT
            .with(|current| current.borrow().clone())
            .unwrap_or_else(|| Self::new(limit))
    }

    pub fn limit(&self) -> usize {
        self.0.limit
    }

    fn try_slot(&self) -> bool {
        let mut active = self.0.active.load(Ordering::Relaxed);
        while active < self.0.limit {
            match self.0.active.compare_exchange_weak(
                active,
                active + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(observed) => active = observed,
            }
        }
        false
    }

    fn try_permit(&self) -> Option<Permit> {
        self.try_slot().then(|| Permit(self.clone()))
    }

    fn acquire_slot(&self) {
        loop {
            if self.try_slot() {
                return;
            }
            let mut wait = self.0.wait.lock().unwrap_or_else(|e| e.into_inner());
            while self.0.active.load(Ordering::Acquire) >= self.0.limit {
                wait = self.0.wake.wait(wait).unwrap_or_else(|e| e.into_inner());
            }
        }
    }

    fn entered(&self) -> bool {
        CURRENT.with(|current| {
            current
                .borrow()
                .as_ref()
                .is_some_and(|b| Arc::ptr_eq(&b.0, &self.0))
        })
    }

    /// Enter a root job; nested work on this thread reuses its existing slot.
    pub fn run<T>(&self, f: impl FnOnce() -> T) -> T {
        if self.entered() {
            return f();
        }
        self.acquire_slot();
        Permit(self.clone()).run(f)
    }

    fn release(&self) {
        let _wait = self.0.wait.lock().unwrap_or_else(|e| e.into_inner());
        self.0.active.fetch_sub(1, Ordering::AcqRel);
        self.0.wake.notify_one();
    }

    /// A parent waiting for another root (or a scoped child) is not leaf work.
    /// Release its slot for the wait, including on unwinding, then reacquire it
    /// before the parent resumes work. Never hold a slot while joining a root.
    pub fn suspend<T>(&self, f: impl FnOnce() -> T) -> T {
        if !self.entered() {
            return f();
        }
        struct Resume(Budget);
        impl Drop for Resume {
            fn drop(&mut self) {
                self.0.acquire_slot();
                CURRENT.with(|current| *current.borrow_mut() = Some(self.0.clone()));
            }
        }
        CURRENT.with(|current| *current.borrow_mut() = None);
        self.release();
        let _resume = Resume(self.clone());
        f()
    }
}

struct Permit(Budget);
impl Permit {
    fn run<T>(self, f: impl FnOnce() -> T) -> T {
        struct Restore(Option<Budget>);
        impl Drop for Restore {
            fn drop(&mut self) {
                CURRENT.with(|current| *current.borrow_mut() = self.0.take());
            }
        }
        let _restore = Restore(CURRENT.with(|current| current.replace(Some(self.0.clone()))));
        f()
    }
}
impl Drop for Permit {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A scoped child. Unavailable slots are deferred, never blocked on while a
/// parent owns a slot; `Scope` still joins live children on error/cancellation.
pub enum Task<'scope, T> {
    Thread(ScopedJoinHandle<'scope, T>),
    Later(Box<dyn FnOnce() -> T + Send + 'scope>),
}
impl<'scope, T: Send + 'scope> Task<'scope, T> {
    pub fn spawn(
        scope: &'scope Scope<'scope, '_>,
        parallel: bool,
        name: &str,
        f: impl FnOnce() -> T + Send + 'scope,
    ) -> Self {
        let budget = Budget::current();
        if let Some(permit) = parallel.then(|| budget.try_permit()).flatten() {
            return Self::Thread(
                std::thread::Builder::new()
                    .name(format!("load-{name}"))
                    .spawn_scoped(scope, move || permit.run(f))
                    .expect("resource loader thread"),
            );
        }
        Self::Later(Box::new(move || budget.run(f)))
    }

    pub fn join(self) -> T {
        match self {
            Self::Thread(handle) => Budget::current().suspend(|| {
                handle
                    .join()
                    .unwrap_or_else(|e| std::panic::resume_unwind(e))
            }),
            Self::Later(f) => f(),
        }
    }
}

/// Parallel leaf work, with results in input order even when work finishes out
/// of order. The caller also works; nested maps spend only remaining slots.
pub fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    map_limit(items, usize::MAX, f)
}

/// A resource-specific memory ceiling can further restrict its leaf work.
pub fn map_limit<T: Sync, R: Send>(
    items: &[T],
    maximum: usize,
    f: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    let budget = Budget::current();
    budget.run(|| {
        let next = AtomicUsize::new(0);
        let work = || {
            let mut out = Vec::new();
            loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(i) else {
                    return out;
                };
                out.push((i, f(item)));
            }
        };
        let mut out = std::thread::scope(|scope| {
            let tasks: Vec<_> = (1..budget.limit().min(maximum.max(1)).min(items.len()))
                .map(|_| Task::spawn(scope, true, "files", &work))
                .collect();
            let mut out = work();
            for task in tasks {
                out.extend(task.join());
            }
            out
        });
        out.sort_unstable_by_key(|(i, _)| *i);
        out.into_iter().map(|(_, value)| value).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn repeated_joins_release_generation_and_a_single_slot_can_join_a_root() {
        let budget = Budget::new(1);
        let baseline = Arc::strong_count(&budget.0);
        for _ in 0..100 {
            budget.run(|| {
                let root_budget = budget.clone();
                let root = std::thread::spawn(move || root_budget.run(|| 7));
                assert_eq!(budget.suspend(|| root.join().unwrap()), 7);
            });
        }
        assert_eq!(Arc::strong_count(&budget.0), baseline);
        assert_eq!(budget.0.active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn independent_leaves_use_all_available_slots() {
        let budget = Budget::new(3);
        let barrier = std::sync::Barrier::new(3);
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let values = budget.run(|| {
            map(&[3, 1, 2], |x| {
                peak.fetch_max(active.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                barrier.wait();
                active.fetch_sub(1, Ordering::SeqCst);
                x * 2
            })
        });
        assert_eq!(values, [6, 2, 4]);
        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn nested_leaves_obey_budget_and_keep_results_and_errors_ordered() {
        for limit in [1, 2, 4] {
            let budget = Budget::new(limit);
            let active = AtomicUsize::new(0);
            let peak = AtomicUsize::new(0);
            let values = budget.run(|| {
                map(&[0, 1, 2, 3], |outer| {
                    map(&[0, 1, 2, 3], |inner| {
                        let running = active.fetch_add(1, Ordering::SeqCst) + 1;
                        peak.fetch_max(running, Ordering::SeqCst);
                        std::thread::yield_now();
                        active.fetch_sub(1, Ordering::SeqCst);
                        if *inner == 2 {
                            Err(*outer)
                        } else {
                            Ok(outer * 10 + inner)
                        }
                    })
                })
            });
            assert_eq!(values[3], [Ok(30), Ok(31), Err(3), Ok(33)]);
            assert!(peak.load(Ordering::SeqCst) <= limit);
            assert_eq!(budget.0.active.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn cancellation_joins_started_children_and_releases_slots_after_panic() {
        let budget = Budget::new(2);
        let finished = AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget.run(|| {
                std::thread::scope(|scope| {
                    let _child = Task::spawn(scope, true, "cancel", || {
                        finished.store(true, Ordering::SeqCst)
                    });
                    panic!("cancel generation");
                });
            })
        }));
        assert!(result.is_err());
        assert!(finished.load(Ordering::SeqCst));
        assert_eq!(budget.run(|| map(&[1, 2, 3], |x| x * 2)), [2, 4, 6]);
        assert_eq!(budget.0.active.load(Ordering::SeqCst), 0);
    }
}
