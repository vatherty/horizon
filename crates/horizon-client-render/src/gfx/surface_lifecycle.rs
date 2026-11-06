//! Native attachment is independent of target size. A missing native window
//! must never be mistaken for an intentional offscreen renderer.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Native,
    Detached,
    Offscreen,
}

pub(super) struct SurfaceLifecycle {
    target: Target,
    size: [u32; 2],
    drawable: bool,
}

/// Zero-sized/minimized and oversized windows keep their last targets, rather
/// than passing a size the device cannot take to Surface::configure.
pub(super) fn drawable_size(width: u32, height: u32, max: u32) -> bool {
    width > 0 && height > 0 && width <= max && height <= max
}

impl SurfaceLifecycle {
    pub fn new(offscreen: bool, size: [u32; 2], max: u32) -> Self {
        Self {
            target: if offscreen {
                Target::Offscreen
            } else {
                Target::Native
            },
            size,
            drawable: drawable_size(size[0], size[1], max),
        }
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn suspended(&self) -> bool {
        self.target == Target::Detached || !self.drawable
    }

    /// Returns whether this is the first native detach. Offscreen targets have
    /// no native surface to lose and keep their normal resize behavior.
    pub fn detach(&mut self) -> bool {
        if self.target != Target::Native {
            return false;
        }
        self.target = Target::Detached;
        self.drawable = false;
        true
    }

    pub fn attach(&mut self) {
        if self.target == Target::Detached {
            self.target = Target::Native;
        }
    }

    /// The allocation/configuration the renderer should perform. None means
    /// retain every target: detached, invalid and repeated sizes do no GPU work.
    pub fn resize(&mut self, size: [u32; 2], max: u32) -> Option<Target> {
        if self.target == Target::Detached {
            return None;
        }
        let was_drawable = self.drawable;
        self.drawable = drawable_size(size[0], size[1], max);
        if !self.drawable || (was_drawable && self.size == size) {
            return None;
        }
        self.size = size;
        Some(self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detached_resize_never_allocates_or_clears_suspension() {
        let mut surface = SurfaceLifecycle::new(false, [640, 480], 8192);
        assert!(surface.detach());
        for size in [[800, 600], [0, 0], [600, 800], [640, 480]] {
            assert_eq!(surface.resize(size, 8192), None);
            assert!(surface.suspended());
            assert_eq!(surface.target(), Target::Detached);
        }
        assert!(!surface.detach());
        surface.attach();
        assert_eq!(surface.resize([600, 800], 8192), Some(Target::Native));
        assert!(!surface.suspended());
        surface.attach();
        assert_eq!(surface.resize([600, 800], 8192), None);
    }

    #[test]
    fn offscreen_resize_keeps_its_target_across_native_lifecycle_events() {
        let mut surface = SurfaceLifecycle::new(true, [640, 480], 8192);
        assert!(!surface.detach());
        surface.attach();
        assert_eq!(surface.resize([800, 600], 8192), Some(Target::Offscreen));
        assert!(!surface.suspended());
        assert_eq!(surface.resize([800, 600], 8192), None);
    }

    #[test]
    fn zero_and_oversized_resumes_wait_for_a_drawable_size() {
        let mut surface = SurfaceLifecycle::new(false, [640, 480], 8192);
        surface.detach();
        surface.attach();
        for size in [[0, 480], [640, 0], [0, 0], [8193, 480]] {
            assert_eq!(surface.resize(size, 8192), None);
            assert!(surface.suspended());
        }
        assert_eq!(surface.resize([640, 480], 8192), Some(Target::Native));
        assert!(!surface.suspended());
        assert_eq!(surface.resize([640, 480], 8192), None);
    }
}
