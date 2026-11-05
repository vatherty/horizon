//! The winit event loop, built the way the platform needs it. On the desktop it is
//! `EventLoop::new()`. On Android the loop must be given the `AndroidApp` that the
//! glue passed to `android_main`, which the `horizon-android` library stores here
//! first.
use winit::event_loop::EventLoop;

#[cfg(target_os = "android")]
pub use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
static APP: std::sync::Mutex<Option<AndroidApp>> = std::sync::Mutex::new(None);

/// Remembers the app that `android_main` was given.
#[cfg(target_os = "android")]
pub fn set_android_app(app: AndroidApp) {
    *APP.lock().unwrap_or_else(|e| e.into_inner()) = Some(app);
}

/// The app that `android_main` was given.
#[cfg(target_os = "android")]
pub fn android_app() -> Option<AndroidApp> {
    APP.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The game's event loop.
pub fn event_loop() -> Result<EventLoop<()>, Box<dyn std::error::Error>> {
    #[cfg(target_os = "android")]
    {
        use winit::platform::android::EventLoopBuilderExtAndroid;
        let app = android_app().ok_or("the Android app was not handed to the game")?;
        Ok(EventLoop::builder().with_android_app(app).build()?)
    }
    #[cfg(not(target_os = "android"))]
    Ok(EventLoop::new()?)
}
