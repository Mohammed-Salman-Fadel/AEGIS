//! Role: process-level Ctrl+C signal handling for graceful CLI shutdown.
//! Called by: `main.rs`, `commands.rs`, and `menu.rs`.
//! Calls into: the host operating system signal APIs on Windows and standard IO error inspection.
//! Owns: the Ctrl+C requested flag, graceful-exit sentinel, and interruption helpers.
//! Does not own: terminal rendering, command routing, or business logic.
//! Next TODOs: add Unix signal support if the CLI becomes a first-class multi-platform interactive shell.

use std::io;

const CTRL_C_EXIT_SENTINEL: &str = "__AEGIS_CTRL_C_EXIT__";

#[cfg(windows)]
mod platform {
    use std::sync::atomic::{AtomicBool, Ordering};

    static CTRL_C_REQUESTED: AtomicBool = AtomicBool::new(false);

    type Bool = i32;
    type Dword = u32;
    type HandlerRoutine = unsafe extern "system" fn(Dword) -> Bool;

    const CTRL_C_EVENT: Dword = 0;

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn SetConsoleCtrlHandler(handler: Option<HandlerRoutine>, add: Bool) -> Bool;
    }

    pub(super) fn install_handler() -> Result<(), String> {
        let result = unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), 1) };
        if result == 0 {
            Err("Windows console control handler registration failed.".to_string())
        } else {
            Ok(())
        }
    }

    pub(super) fn ctrl_c_requested() -> bool {
        CTRL_C_REQUESTED.load(Ordering::SeqCst)
    }

    pub(super) fn clear_ctrl_c_request() {
        CTRL_C_REQUESTED.store(false, Ordering::SeqCst);
    }

    unsafe extern "system" fn ctrl_handler(ctrl_type: Dword) -> Bool {
        if ctrl_type == CTRL_C_EVENT {
            CTRL_C_REQUESTED.store(true, Ordering::SeqCst);
            1
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
mod platform {
    pub(super) fn install_handler() -> Result<(), String> {
        Ok(())
    }

    pub(super) fn ctrl_c_requested() -> bool {
        false
    }

    pub(super) fn clear_ctrl_c_request() {}
}

pub fn install_handler() -> Result<(), String> {
    platform::install_handler()
}

pub fn ctrl_c_requested() -> bool {
    platform::ctrl_c_requested()
}

pub fn clear_ctrl_c_request() {
    platform::clear_ctrl_c_request();
}

pub fn ctrl_c_exit_error() -> String {
    clear_ctrl_c_request();
    CTRL_C_EXIT_SENTINEL.to_string()
}

pub fn is_ctrl_c_error(error: &str) -> bool {
    error == CTRL_C_EXIT_SENTINEL
}

pub fn was_ctrl_c(error: &io::Error) -> bool {
    ctrl_c_requested()
        || error.kind() == io::ErrorKind::Interrupted
        || error.raw_os_error() == Some(995)
}
