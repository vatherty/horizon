//! Read-only process memory diagnostics.
/// OS observations, in bytes. These overlap and must never be added together,
/// or to allocator/GPU capacities. `None` means unavailable, not zero.
#[derive(Clone, Copy, Debug, Default)]
pub struct Observations {
    pub resident: Option<u64>,
    /// The OS's physical footprint, including memory beyond the resident set.
    pub footprint: Option<u64>,
    /// Memory the current process may still allocate (not system free RAM).
    pub available: Option<u64>,
}

/// Supported iOS SDK APIs, compiled against the target's Mach headers.
#[cfg(target_os = "ios")]
pub fn observations() -> Observations {
    unsafe extern "C" {
        fn horizon_process_memory_ios(
            resident: *mut u64,
            footprint: *mut u64,
            available: *mut u64,
        ) -> u32;
    }
    let (mut resident, mut footprint, mut available) = (0, 0, 0);
    // SAFETY: the SDK shim writes one u64 to each valid, distinct output and
    // returns a validity bit for each independently successful observation.
    let known =
        unsafe { horizon_process_memory_ios(&mut resident, &mut footprint, &mut available) };
    Observations {
        resident: (known & 1 != 0).then_some(resident),
        footprint: (known & 2 != 0).then_some(footprint),
        available: (known & 4 != 0).then_some(available),
    }
}

#[cfg(not(target_os = "ios"))]
pub fn observations() -> Observations {
    Observations {
        resident: resident_bytes(),
        ..Default::default()
    }
}

#[cfg(target_os = "ios")]
pub fn resident_bytes() -> Option<u64> {
    observations().resident
}

/// The process's memory counters.
#[cfg(windows)]
fn counters() -> Option<windows_sys::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    // SAFETY: all-zero is a valid value of this plain C struct, and the call writes at most `size`
    // bytes into it; the current process's pseudo-handle needs no closing.
    unsafe {
        let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        counters.cb = size;
        (K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) != 0).then_some(counters)
    }
}

/// The memory this process holds in RAM, in bytes.
#[cfg(windows)]
pub fn resident_bytes() -> Option<u64> {
    Some(counters()?.WorkingSetSize as u64)
}

/// The memory this process holds in RAM, in bytes.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn resident_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: reads one system constant.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    Some(pages * u64::try_from(page).ok()?)
}

/// The memory this process holds in RAM, in bytes.
/// macOS's libproc API; iOS uses the supported Mach SDK calls above.
#[cfg(all(target_vendor = "apple", target_os = "macos"))]
pub fn resident_bytes() -> Option<u64> {
    let size = std::mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
    // SAFETY: all-zero is a valid value of this plain C struct, and the call writes at most `size`
    // bytes into it.
    unsafe {
        let mut info: libc::proc_taskinfo = std::mem::zeroed();
        let written = libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            (&raw mut info).cast(),
            size,
        );
        (written == size).then_some(info.pti_resident_size)
    }
}

/// The memory this process holds in RAM, in bytes: not known on this system.
#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "android",
    target_os = "ios",
    all(target_vendor = "apple", target_os = "macos")
)))]
pub fn resident_bytes() -> Option<u64> {
    None
}

#[cfg(all(
    test,
    any(
        windows,
        target_os = "linux",
        all(target_vendor = "apple", target_os = "macos")
    )
))]
mod tests {
    use super::*;

    #[test]
    fn touched_memory_counts_as_resident() {
        const SIZE: usize = 256 << 20;
        let before = resident_bytes().unwrap();
        let block = vec![1u8; SIZE];
        let during = resident_bytes().unwrap();
        std::hint::black_box(&block);
        assert!(
            during >= before + SIZE as u64 / 2,
            "{before} then {during} bytes"
        );
    }
}
