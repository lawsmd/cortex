use windows::Win32::Foundation::{FALSE, HWND, TRUE};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
use windows_core::BOOL;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid WindowHandle")]
    InvalidWindowHandle,
    #[error("Unknown error")]
    Other(#[from] windows::core::Error),
}

/// Extension trait for Windows specific logic on a [`winit::window::Window`].
pub trait WindowExt {
    /// "Cloaks" the window. A cloaked window is one that is invisible, but can still be drawn to.
    fn set_cloaked(&self, cloaked: bool) -> Result<(), Error>;
}

impl WindowExt for Window {
    fn set_cloaked(&self, cloaked: bool) -> Result<(), Error> {
        let Ok(RawWindowHandle::Win32(handle)) = self
            .window_handle()
            .map(|window_handle| window_handle.as_raw())
        else {
            return Err(Error::InvalidWindowHandle);
        };

        let value = if cloaked { TRUE } else { FALSE };
        unsafe {
            DwmSetWindowAttribute(
                HWND(handle.hwnd.get() as _),
                DWMWA_CLOAK,
                &value as *const BOOL as *const _,
                size_of::<BOOL>() as u32,
            )?
        }

        Ok(())
    }
}

// CORTEX-BEGIN: cloak-watchdog
use std::time::Duration;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};

/// Safety net for the cloak / first-frame-render failure class.
///
/// Every window is created *cloaked* (invisible but still compositable) and is
/// only uncloaked after the first frame renders successfully (see the uncloak
/// block in `window.rs::Window::render`). If the UI thread wedges before first
/// paint — e.g. a render-path deadlock — the window stays cloaked forever:
/// invisible, unclosable (no `WM_CLOSE` ever pumped), and with nothing logged.
/// That exact failure brought Cortex down on 2026-06-15 (a re-entrant lock in
/// the smart-clear button) and was almost undiagnosable precisely because it
/// was silent.
///
/// This watchdog runs off the UI thread. After `timeout` it checks whether the
/// window is still cloaked and, if so, force-uncloaks it and logs an error.
/// That converts a silent invisible brick into a visible window plus a log
/// line you can act on — it does NOT un-wedge a hung UI thread, but it makes
/// the failure obvious instead of a mystery. A healthy startup uncloaks well
/// within the timeout, so this is a harmless no-op in the normal case.
pub fn spawn_cloak_watchdog(window: &Window, timeout: Duration) {
    let Ok(RawWindowHandle::Win32(handle)) = window
        .window_handle()
        .map(|window_handle| window_handle.as_raw())
    else {
        return;
    };
    // Capture the raw HWND as an integer. `HWND` is not `Send`, but the handle
    // value is just a number, and DWM attribute calls are valid cross-thread
    // for a window owned by this process.
    let hwnd_value = handle.hwnd.get();

    let spawned = std::thread::Builder::new()
        .name("cortex-cloak-watchdog".to_owned())
        .spawn(move || {
            std::thread::sleep(timeout);
            let hwnd = HWND(hwnd_value as _);

            // Only act if the window is still cloaked. DWMWA_CLOAKED returns a
            // non-zero bitmask (APP / SHELL / INHERITED) while cloaked.
            let mut cloaked: u32 = 0;
            let queried = unsafe {
                DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAKED,
                    &mut cloaked as *mut u32 as *mut _,
                    size_of::<u32>() as u32,
                )
            };
            if queried.is_err() || cloaked == 0 {
                // Healthy startup uncloaked it already, or the window is gone.
                return;
            }

            log::error!(
                "Window still cloaked {timeout:?} after launch — the first frame never \
                 rendered (UI thread likely wedged before first paint). Force-uncloaking so \
                 the window is at least visible; it may still be unresponsive."
            );
            let value = FALSE;
            let _ = unsafe {
                DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAK,
                    &value as *const BOOL as *const _,
                    size_of::<BOOL>() as u32,
                )
            };
        });
    if let Err(e) = spawned {
        log::warn!("Failed to spawn cloak watchdog thread: {e:#?}");
    }
}
// CORTEX-END: cloak-watchdog
