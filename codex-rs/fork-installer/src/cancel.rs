//! Turns termination requests into an orderly rollback instead of an abrupt exit.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// Shared cancellation state; holds the exit code of the request, or zero.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicUsize>);

/// The installer stopped because it was asked to; any changes were rolled back.
#[derive(Debug)]
pub struct Cancelled {
    pub exit_code: u8,
}

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("installation cancelled")
    }
}

impl std::error::Error for Cancelled {}

impl Cancel {
    pub fn check(&self) -> Result<(), Cancelled> {
        match self.0.load(Ordering::SeqCst) {
            0 => Ok(()),
            code => Err(Cancelled {
                exit_code: u8::try_from(code).unwrap_or(1),
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn request(&self, exit_code: u8) {
        self.0.store(usize::from(exit_code), Ordering::SeqCst);
    }

    /// Route SIGTERM/SIGINT (Ctrl+C/Ctrl+Break on Windows) to this state. A
    /// second Unix signal terminates immediately, in case rollback hangs.
    pub fn install_handlers(&self) -> std::io::Result<()> {
        #[cfg(unix)]
        {
            use signal_hook::consts::SIGINT;
            use signal_hook::consts::SIGTERM;
            use std::sync::atomic::AtomicBool;
            let seen = Arc::new(AtomicBool::new(false));
            for (signal, code) in [(SIGTERM, 143), (SIGINT, 130)] {
                signal_hook::flag::register_conditional_shutdown(signal, code, Arc::clone(&seen))?;
                signal_hook::flag::register(signal, Arc::clone(&seen))?;
                signal_hook::flag::register_usize(signal, Arc::clone(&self.0), code as usize)?;
            }
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Console::CTRL_BREAK_EVENT;
            use windows_sys::Win32::System::Console::CTRL_C_EVENT;
            use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

            static HANDLER_STATE: std::sync::OnceLock<Arc<AtomicUsize>> =
                std::sync::OnceLock::new();

            unsafe extern "system" fn handler(event: u32) -> i32 {
                if event != CTRL_C_EVENT && event != CTRL_BREAK_EVENT {
                    return 0;
                }
                if let Some(state) = HANDLER_STATE.get() {
                    state.store(130, Ordering::SeqCst);
                }
                1
            }

            if HANDLER_STATE.set(Arc::clone(&self.0)).is_ok()
                && unsafe { SetConsoleCtrlHandler(Some(handler), 1) } == 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    }
}
