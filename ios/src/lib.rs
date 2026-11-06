//! Horizon on iOS: the static library the Xcode app links (`ios/README.md`).
//!
//! The Swift side (`ios/app/main.swift`) is the process entry point: it calls
//! [`horizon_ios_main`] on the main thread with the app's Documents folder. That
//! function sets up the game's folders, logging and crash reports and runs the
//! game's own loop (`horizon_client::game::run_with`, the same one the desktop
//! binary runs through `entry::main`). It returns when the game ends; the codes
//! are [`EXIT_OK`], [`EXIT_ERROR`] and [`EXIT_NO_RESOURCES`].
//!
//! The one thing this crate cannot do on its own is show UI before the game
//! starts. Winit's iOS backend calls `UIApplicationMain` itself when the event
//! loop runs, and refuses to run after it has already been called
//! (`winit-0.30.13/src/platform_impl/ios/event_loop.rs`, `EventLoop::run`), so
//! the document-picker import lives in the Swift launcher phase: `main.swift`
//! starts it when this function reports the resources missing, and the player
//! relaunches after importing (ios/README.md, "Two phases, one process").
//!
//! This crate is the only place with `unsafe` code: the C entry point and
//! the environment setup below.
//!
//! Shell lifecycle lines go to the file log (`logs/latest.log`) like the
//! game's: `os_log_with_type` is not a linkable symbol on iOS (it is absent
//! from `libc` entirely; only `os_log_create` is exported), so there is no
//! `os_log` sink in this crate.
#![cfg(target_os = "ios")]

/// The desktop binary's allocator (mimalloc), mirroring `src/main.rs`: this
/// static library is linked into the Xcode app binary, so it is the whole app's
/// allocator. `--features system-allocator` leaves the system heap in place
/// (the escape hatch if mimalloc ever fails to build for iOS).
#[cfg(not(feature = "system-allocator"))]
#[global_allocator]
static ALLOCATOR: horizon_threads::Allocator = horizon_threads::ALLOCATOR;

use std::ffi::{CStr, c_char, c_int};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use horizon_client::{crash, logging};

/// The game ended (near-unreachable on iOS: the event loop never returns).
pub const EXIT_OK: c_int = 0;
/// The game stopped with an error; the message is in `last-error.txt`.
pub const EXIT_ERROR: c_int = 1;
/// The prepared resources are not on the phone; the Swift launcher takes over.
pub const EXIT_NO_RESOURCES: c_int = 2;

/// A file in the game folder the Swift launcher reads: why the last run ended,
/// and whether it ended at all.
const ERROR_FILE: &str = "last-error.txt";
const RUNNING_FILE: &str = "running.marker";
/// Environment variables to set before the game starts, one `NAME=value` per line
/// (`#` starts a comment): the debugging switches of the desktop game
/// (`HORIZON_SPIKES=20`, `WGPU_BACKEND=metal`, `HORIZON_NO_AUDIO=1`, ...) without
/// rebuilding the app. The Files app cannot set them for an app, so they come
/// from a file in the game folder.
const ENV_FILE: &str = "horizon-env.txt";
/// The prepared folder's marker: the file that says "these are the resources".
const MANIFEST: &str = "horizon-resources.json";
/// The prepared folder's version (horizon-core's `VANILLA_VERSION`, `26.3`).
const PREPARED_VERSION: &str = "26.3";

/// Apple's thermal-state sample, on a stable scale defined in the C header.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_set_thermal_state(state: c_int) {
    horizon_client::platform::set_apple_thermal_status(state);
}

/// OS Low Power Mode is independent of the player's saved Battery Mode.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_set_low_power_mode(enabled: c_int) {
    horizon_client::platform::set_low_power_mode(enabled != 0);
}

/// The prepared resources are not on the phone.
pub const RESOURCES_MISSING: c_int = 0;
/// The prepared resources are imported, in `resources/`.
pub const RESOURCES_IN_RESOURCES: c_int = 1;
/// The prepared resources are in the prepared folder (`.local/horizon-client/26.3`).
pub const RESOURCES_IN_PREPARED: c_int = 2;
/// The prepared resources are where `horizon-args.txt`'s `--resources` points.
pub const RESOURCES_FROM_ARGS: c_int = 3;

/// Where the prepared resources are, by the game's own rules — the answer
/// [`check_resources`] reaches inside a run, as one of the `RESOURCES_*`
/// codes. The Swift launcher asks with this instead of repeating the path
/// rules in Swift: its own check once knew only `resources/`, so it called
/// the resources missing on phones where the game would have found them in
/// the prepared folder or at an args override. Pure path checks (plus
/// reading `horizon-args.txt`): no folders are created and no game state is
/// touched, so the launcher may call it on any launch, as often as it likes.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_resources_state(documents: *const c_char) -> c_int {
    let Some(documents) = cstr_to_path(documents) else {
        return RESOURCES_MISSING;
    };
    resources_state(&documents)
}

/// The app's Documents folder, from `main.swift`, as a C string.
#[unsafe(no_mangle)]
pub extern "C" fn horizon_ios_main(documents: *const c_char) -> c_int {
    let origin = Instant::now();
    horizon_client::telemetry::mark_start();
    static STARTED: AtomicBool = AtomicBool::new(false);
    // The Swift launcher starts the game at most once per process: a second run
    // would find the game's process-wide state in use.
    if STARTED.swap(true, Ordering::SeqCst) {
        // No log sink exists yet this early; the exit code is the signal.
        return EXIT_ERROR;
    }
    let Some(documents) = cstr_to_path(documents) else {
        return EXIT_ERROR;
    };
    // Panics are caught here so that the launcher can show what happened instead
    // of the process aborting.
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| start(&documents, origin)));
    let (code, failure) = match outcome {
        Ok(Ok(code)) => (code, None),
        Ok(Err(Failure::Error(message))) => (EXIT_ERROR, Some(message)),
        Ok(Err(Failure::NoResources(message))) => (EXIT_NO_RESOURCES, Some(message)),
        Err(payload) => {
            let what = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            (
                EXIT_ERROR,
                Some(format!(
                    "The game crashed: {what}\n\nA crash report is in the crash-reports folder of the game folder, and the log in logs/latest.log."
                )),
            )
        }
    };
    logging::flush();
    let _ = fs::remove_file(documents.join(RUNNING_FILE));
    if let Some(message) = failure {
        log::error!("Horizon: {message}");
        let _ = fs::write(documents.join(ERROR_FILE), &message);
    }
    code
}

/// Why a run ended: an error, or the prepared resources missing (a different
/// exit code — [`EXIT_NO_RESOURCES`] — so the Swift launcher takes over instead
/// of showing a generic failure).
enum Failure {
    Error(String),
    NoResources(String),
}

/// The C string `main.swift` passes, or `None` when it is null or not UTF-8.
fn cstr_to_path(ptr: *const c_char) -> Option<PathBuf> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: the caller passes a NUL-terminated C string that outlives the call.
    let text = unsafe { CStr::from_ptr(ptr) }.to_str().ok()?;
    Some(PathBuf::from(text))
}

/// The prepared resources, as the game looks for them: the imported folder.
fn resources_dir(documents: &Path) -> PathBuf {
    documents.join("resources")
}

/// The prepared folder's natural place, where `horizon-prepare` puts it.
fn prepared_dir(documents: &Path) -> PathBuf {
    documents
        .join(".local")
        .join("horizon-client")
        .join(PREPARED_VERSION)
}

fn start(documents: &Path, origin: Instant) -> Result<c_int, Failure> {
    // First, as in `entry::main`: the allocator's purge delay, before any thread
    // exists. (The `system-allocator` feature is the escape hatch if mimalloc
    // ever fails to build for iOS; ios/README.md.)
    horizon_threads::configure_allocator();
    fs::create_dir_all(documents)
        .map_err(|e| Failure::Error(format!("Could not create {}: {e}", documents.display())))?;
    let container = documents.parent().unwrap_or(documents);
    let tmp = documents.join("tmp");
    let _ = fs::create_dir_all(&tmp);
    // Relative trace paths such as logs/startup.jsonl must work on the first
    // launch, before logging::init creates its own files.
    fs::create_dir_all(documents.join("logs"))
        .map_err(|e| Failure::Error(format!("Could not create the logs folder: {e}")))?;
    // The game takes its folder from the working directory and `HORIZON_HOME`,
    // and a phone's defaults for both are not writable.
    // SAFETY: the game's own threads do not exist yet, and nothing else running
    // reads the environment.
    unsafe {
        std::env::set_var("HORIZON_HOME", documents);
        std::env::set_var("HOME", container);
        std::env::set_var("TMPDIR", &tmp);
        // The default panic hook prints a backtrace (to the log) with this set.
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    std::env::set_current_dir(documents)
        .map_err(|e| Failure::Error(format!("Could not enter {}: {e}", documents.display())))?;
    let from_file = env_file(documents);
    // Read horizon-env.txt after choosing writable folders, before opening an
    // optional startup trace. The origin still includes the shell setup.
    horizon_client::spans::startup::init(origin);
    let _entry = horizon_client::spans::startup::step("ios.entry");
    fs::write(documents.join(RUNNING_FILE), "running\n").ok();
    let _ = fs::remove_file(documents.join(ERROR_FILE));
    logging::set_game_home(documents.to_path_buf());
    logging::init(documents, "Render thread");
    log::info!(
        "Horizon {} for iOS starting; game folder {}",
        crash::version(),
        documents.display()
    );
    crash::install(documents.to_path_buf(), "client");
    match horizon_client::mobile::seed(documents) {
        Ok(files) if !files.is_empty() => log::info!("Horizon: seeded mobile defaults: {files:?}"),
        Ok(_) => {}
        Err(error) => log::warn!("Horizon: could not seed mobile defaults: {error}"),
    }
    if !from_file.is_empty() {
        log::info!("Horizon: environment from {ENV_FILE}: {from_file:?}");
    }
    ensure_prepared_symlink(documents);
    if let Err(message) = check_resources(documents) {
        return Err(Failure::NoResources(message));
    }
    if let Err(error) = horizon_client::game::run_with() {
        log::error!("Horizon: {error}");
        let report = crash::report_error(documents, "client", &*error);
        let mut message = format!("The game stopped with an error:\n{error}");
        if let Some(report) = report {
            message.push_str(&format!("\n\nCrash report: {}", report.display()));
        }
        return Err(Failure::Error(message));
    }
    let _ = fs::remove_file(documents.join(RUNNING_FILE));
    Ok(EXIT_OK)
}

/// Sets the variables in `horizon-env.txt`, returning their names.
fn env_file(documents: &Path) -> Vec<String> {
    let text = fs::read_to_string(documents.join(ENV_FILE)).unwrap_or_default();
    let mut names = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            let name = name.trim();
            if !name.is_empty() && !name.contains('\0') && !value.contains('\0') {
                // SAFETY: as in `start`: the game's threads are not running yet.
                unsafe { std::env::set_var(name, value.trim()) };
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// `--resources DIR` from `horizon-args.txt` in the game folder, if the player
/// put the resources somewhere else. The game itself only honours this file
/// once the mobile layer's `launch_args()` cfg covers iOS; the shell honours it
/// in its own preflight already, so the launcher does not report the resources
/// missing when the player pointed elsewhere.
fn args_resources(documents: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(documents.join("horizon-args.txt")).ok()?;
    let mut words = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .flat_map(|line| line.split_whitespace());
    while let Some(word) = words.next() {
        if word == "--resources" {
            let dir = words.next()?;
            let path = PathBuf::from(dir);
            return Some(if path.is_absolute() {
                path
            } else {
                documents.join(path)
            });
        }
    }
    None
}

/// The prepared folder's natural place (`.local/horizon-client/26.3`) as a link
/// to `resources/`: upstream's `Options::parse` only looks beside the executable
/// and at `.local/horizon-client/<version>`, and the mobile layer's
/// `cfg(target_os = "android")` branch that defaults the resources to
/// `resources/` does not cover iOS yet. The link lets the unmodified game find
/// an import today; the integrator removes it when that branch becomes
/// `android | ios` (ios/README.md, "Integrator"). A real prepared folder in
/// `.local` wins over the link: it is the player's own copy, left alone.
fn ensure_prepared_symlink(documents: &Path) {
    let resources = resources_dir(documents);
    if !resources.join(MANIFEST).is_file() {
        return;
    }
    let prepared = prepared_dir(documents);
    if prepared.symlink_metadata().is_ok() {
        return;
    }
    if let Some(parent) = prepared.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return;
    }
    match std::os::unix::fs::symlink("../../resources", &prepared) {
        Ok(()) => log::info!("Horizon: linked {} to resources/", prepared.display()),
        Err(e) => log::warn!(
            "Horizon: could not link {} to resources/: {e}",
            prepared.display()
        ),
    }
}

/// Where the prepared resources are: one of the `RESOURCES_*` codes. The
/// order is [`check_resources`]'s: an args override the player asked for
/// wins, then the import in `resources/`, then a prepared folder of their own.
fn resources_state(documents: &Path) -> c_int {
    if let Some(dir) = args_resources(documents)
        && dir.join(MANIFEST).is_file()
    {
        return RESOURCES_FROM_ARGS;
    }
    if resources_dir(documents).join(MANIFEST).is_file() {
        return RESOURCES_IN_RESOURCES;
    }
    if prepared_dir(documents).join(MANIFEST).is_file() {
        return RESOURCES_IN_PREPARED;
    }
    RESOURCES_MISSING
}

/// Stops with a readable message when the prepared resources are not on the
/// phone, which is the first thing that goes wrong. The Swift launcher shows
/// this message and offers the import; the code it gets back is
/// [`EXIT_NO_RESOURCES`].
fn check_resources(documents: &Path) -> Result<(), String> {
    if resources_state(documents) != RESOURCES_MISSING {
        return Ok(());
    }
    Err(format!(
        "The game's resources are missing.\n\n\
         Horizon does not include Minecraft's files. Prepare them on a computer from your own \
         Minecraft Java Edition install with horizon-prepare (the Horizon launcher's Prepare \
         button), zip the prepared folder (the one that holds {MANIFEST}), send the zip to this \
         phone, then tap Import resources below and pick it.\n\n\
         The resources belong in the Documents folder, in a folder called resources."
    ))
}
