//! Small native additions to the custom title bar. No hooks or background tasks.

/// Open the foreground Spark Code window's native Windows system menu.
///
/// Call on the GUI/event-loop thread in response to a title-bar right click or
/// Alt+Space. The menu is anchored at the window's upper-left corner. It is
/// deliberately a no-op if another process has focus, or on non-Windows hosts.
pub fn show_system_menu() {
    #[cfg(target_os = "windows")]
    windows::show_system_menu();
}

#[cfg(any(target_os = "windows", test))]
fn is_window_command(command: i32) -> bool {
    // Size, Move, Minimize, Maximize, Close, Restore. Do not forward arbitrary
    // commands if another component ever adds items to this borrowed menu.
    matches!(command, 0xF000 | 0xF010 | 0xF020 | 0xF030 | 0xF060 | 0xF120)
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{ffi::c_void, ptr};

    type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> Handle;
        fn GetWindowThreadProcessId(window: Handle, process_id: *mut u32) -> u32;
        fn GetSystemMenu(window: Handle, revert: i32) -> Handle;
        fn GetWindowRect(window: Handle, rect: *mut Rect) -> i32;
        fn GetWindowLongW(window: Handle, index: i32) -> i32;
        fn IsIconic(window: Handle) -> i32;
        fn IsZoomed(window: Handle) -> i32;
        fn EnableMenuItem(menu: Handle, item: u32, flags: u32) -> u32;
        fn TrackPopupMenu(
            menu: Handle,
            flags: u32,
            x: i32,
            y: i32,
            reserved: i32,
            window: Handle,
            excluded_rect: *const Rect,
        ) -> i32;
        fn PostMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    fn belongs_to_current_process(window: Handle) -> bool {
        let mut owner = 0;
        // SAFETY: User32 validates the opaque handle; owner is a valid output.
        !window.is_null()
            && unsafe { GetWindowThreadProcessId(window, &mut owner) } != 0
            && owner == std::process::id()
    }

    pub(super) fn show_system_menu() {
        // SAFETY: All handles come from User32, output pointers refer to live
        // local values, and the borrowed menu is never destroyed. Ownership is
        // checked before menu access and again after its modal message loop.
        unsafe {
            let window = GetForegroundWindow();
            if !belongs_to_current_process(window) {
                return;
            }
            let menu = GetSystemMenu(window, 0);
            let mut rect = Rect::default();
            if menu.is_null() || GetWindowRect(window, &mut rect) == 0 {
                return;
            }

            let minimized = IsIconic(window) != 0;
            let maximized = IsZoomed(window) != 0;
            let style = GetWindowLongW(window, -16) as u32; // GWL_STYLE
            // A custom invocation must reflect current state on every opening,
            // including after maximize/restore via a keyboard or taskbar action.
            for (command, enabled) in [
                (0xF120, minimized || maximized),   // SC_RESTORE
                (0xF010, !maximized && !minimized), // SC_MOVE
                (0xF000, !maximized && !minimized && style & 0x0004_0000 != 0), // WS_THICKFRAME
                (0xF020, !minimized && style & 0x0002_0000 != 0), // WS_MINIMIZEBOX
                (0xF030, !maximized && style & 0x0001_0000 != 0), // WS_MAXIMIZEBOX
            ] {
                // MF_BYCOMMAND | MF_ENABLED/MF_GRAYED, preserving native text.
                EnableMenuItem(menu, command, if enabled { 0 } else { 1 });
            }

            // TPM_RETURNCMD | TPM_RIGHTBUTTON: obtain the user's selected
            // command and allow either mouse button as in a normal title bar.
            let command = TrackPopupMenu(
                menu,
                0x0100 | 0x0002,
                rect.left,
                rect.top,
                0,
                window,
                ptr::null(),
            );
            if super::is_window_command(command) && belongs_to_current_process(window) {
                // WM_SYSCOMMAND is posted, so normal window-loop handling owns
                // the transition, including the application's close handling.
                PostMessageW(window, 0x0112, command as usize, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_normal_window_commands_are_forwarded() {
        for command in [0xF000, 0xF010, 0xF020, 0xF030, 0xF060, 0xF120] {
            assert!(super::is_window_command(command));
        }
        // Cancel/error, application-defined commands and non-window actions.
        for command in [0, -1, 1, 0x1234, 0xF130, 0xF140, 0xF170, 0xF180] {
            assert!(!super::is_window_command(command));
        }
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn other_platforms_are_a_noop() {
        super::show_system_menu();
    }
}
