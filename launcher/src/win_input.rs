// SPDX-License-Identifier: MIT
// God of War III Remastered - Windows Input Bridge & Window Manager

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::thread;
    use std::time::Duration;

    static HOOK_RUNNING: AtomicBool = AtomicBool::new(false);
    static GAME_PID: AtomicU32 = AtomicU32::new(0);
    static HIDE_CURSOR_ENABLED: AtomicBool = AtomicBool::new(false);
    static HOOK_HANDLE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

    type HWND = *mut c_void;
    type HHOOK = isize;
    type HINSTANCE = isize;
    type LRESULT = isize;
    type WPARAM = usize;
    type LPARAM = isize;
    type HOOKPROC = unsafe extern "system" fn(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT;

    const WH_MOUSE_LL: i32 = 14;
    const WM_LBUTTONDOWN: usize = 0x0201;
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_RBUTTONDOWN: usize = 0x0204;
    const WM_RBUTTONUP: usize = 0x0205;
    const WM_MBUTTONDOWN: usize = 0x0207;
    const WM_MBUTTONUP: usize = 0x0208;
    const WM_MOUSEWHEEL: usize = 0x020A;
    const WM_XBUTTONDOWN: usize = 0x020B;
    const WM_XBUTTONUP: usize = 0x020C;
    const WM_QUIT: u32 = 0x0012;

    const VK_F13: u8 = 0x7C;
    const VK_F14: u8 = 0x7D;
    const VK_F15: u8 = 0x7E;
    const VK_F16: u8 = 0x7F;
    const VK_F17: u8 = 0x80;
    const VK_F18: u8 = 0x81;
    const VK_F19: u8 = 0x82;

    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const SW_RESTORE: i32 = 9;
    const HWND_TOPMOST: HWND = -1isize as HWND;
    const HWND_NOTOPMOST: HWND = -2isize as HWND;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_SHOWWINDOW: u32 = 0x0040;

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MSLLHOOKSTRUCT {
        pt: POINT,
        mouse_data: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: u32,
        w_param: WPARAM,
        l_param: LPARAM,
        time: u32,
        pt: POINT,
    }

    #[link(name = "user32")]
    #[link(name = "winmm")]
    unsafe extern "system" {
        fn SetWindowsHookExA(id_hook: i32, lpfn: HOOKPROC, h_mod: HINSTANCE, dw_thread_id: u32) -> HHOOK;
        fn UnhookWindowsHookEx(hhk: HHOOK) -> i32;
        fn CallNextHookEx(hhk: HHOOK, n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
        fn GetForegroundWindow() -> HWND;
        fn GetWindowThreadProcessId(hwnd: HWND, lpdw_process_id: *mut u32) -> u32;
        fn GetWindowTextA(hwnd: HWND, lp_string: *mut u8, n_max_count: i32) -> i32;
        fn keybd_event(b_vk: u8, b_scan: u8, dw_flags: u32, dw_extra_info: usize);
        fn GetMessageA(lp_msg: *mut MSG, hwnd: HWND, w_msg_filter_min: u32, w_msg_filter_max: u32) -> i32;
        fn PostThreadMessageA(id_thread: u32, msg: u32, w_param: WPARAM, l_param: LPARAM) -> i32;
        fn GetCurrentThreadId() -> u32;
        fn ShowWindow(hwnd: HWND, n_cmd_show: i32) -> i32;
        fn SetForegroundWindow(hwnd: HWND) -> i32;
        fn BringWindowToTop(hwnd: HWND) -> i32;
        fn SetWindowPos(hwnd: HWND, hwnd_insert_after: HWND, x: i32, y: i32, cx: i32, cy: i32, u_flags: u32) -> i32;
        fn EnumWindows(lp_enum_func: unsafe extern "system" fn(hwnd: HWND, lparam: LPARAM) -> i32, lparam: LPARAM) -> i32;
        fn timeBeginPeriod(u_period: u32) -> u32;
        fn timeEndPeriod(u_period: u32) -> u32;
    }

    unsafe fn is_game_window(hwnd: HWND) -> bool {
        if hwnd.is_null() {
            return false;
        }
        let expected_pid = GAME_PID.load(Ordering::SeqCst);
        let mut win_pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut win_pid);
        }
        if expected_pid > 0 && win_pid == expected_pid {
            return true;
        }

        let mut title_buf = [0u8; 128];
        let len = unsafe { GetWindowTextA(hwnd, title_buf.as_mut_ptr(), 127) };
        if len > 0 {
            let title = String::from_utf8_lossy(&title_buf[..len as usize]);
            if title.contains("God of War") || title.contains("gow3") {
                return true;
            }
        }
        false
    }

    unsafe extern "system" fn low_level_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            unsafe {
                let fg = GetForegroundWindow();
                if is_game_window(fg) {
                    let p_mouse = &*(lparam as *const MSLLHOOKSTRUCT);
                    match wparam {
                        WM_LBUTTONDOWN => {
                            keybd_event(VK_F13, 0x68, 0, 0);
                        }
                        WM_LBUTTONUP => {
                            keybd_event(VK_F13, 0x68, KEYEVENTF_KEYUP, 0);
                        }
                        WM_RBUTTONDOWN => {
                            keybd_event(VK_F14, 0x69, 0, 0);
                        }
                        WM_RBUTTONUP => {
                            keybd_event(VK_F14, 0x69, KEYEVENTF_KEYUP, 0);
                        }
                        WM_MBUTTONDOWN => {
                            keybd_event(VK_F15, 0x6A, 0, 0);
                        }
                        WM_MBUTTONUP => {
                            keybd_event(VK_F15, 0x6A, KEYEVENTF_KEYUP, 0);
                        }
                        WM_XBUTTONDOWN => {
                            let xbtn = (p_mouse.mouse_data >> 16) as u16;
                            if xbtn == 1 {
                                keybd_event(VK_F16, 0x6B, 0, 0);
                            } else if xbtn == 2 {
                                keybd_event(VK_F17, 0x6C, 0, 0);
                            }
                        }
                        WM_XBUTTONUP => {
                            let xbtn = (p_mouse.mouse_data >> 16) as u16;
                            if xbtn == 1 {
                                keybd_event(VK_F16, 0x6B, KEYEVENTF_KEYUP, 0);
                            } else if xbtn == 2 {
                                keybd_event(VK_F17, 0x6C, KEYEVENTF_KEYUP, 0);
                            }
                        }
                        WM_MOUSEWHEEL => {
                            let delta = (p_mouse.mouse_data >> 16) as i16;
                            if delta > 0 {
                                keybd_event(VK_F18, 0x6D, 0, 0);
                                keybd_event(VK_F18, 0x6D, KEYEVENTF_KEYUP, 0);
                            } else if delta < 0 {
                                keybd_event(VK_F19, 0x6E, 0, 0);
                                keybd_event(VK_F19, 0x6E, KEYEVENTF_KEYUP, 0);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let hhk = HOOK_HANDLE.load(Ordering::SeqCst);
        unsafe { CallNextHookEx(hhk, code, wparam, lparam) }
    }

    struct EnumParam {
        target_pid: u32,
        found_hwnd: HWND,
    }

    unsafe extern "system" fn enum_win_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let p = unsafe { &mut *(lparam as *mut EnumParam) };
        let mut pid: u32 = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
            if (p.target_pid > 0 && pid == p.target_pid) || is_game_window(hwnd) {
                p.found_hwnd = hwnd;
                return 0; // stop enumerating
            }
        }
        1 // continue
    }

    pub fn find_game_hwnd(pid: u32) -> Option<HWND> {
        let mut param = EnumParam {
            target_pid: pid,
            found_hwnd: std::ptr::null_mut(),
        };
        unsafe {
            EnumWindows(enum_win_proc, &mut param as *mut _ as LPARAM);
        }
        if !param.found_hwnd.is_null() {
            Some(param.found_hwnd)
        } else {
            None
        }
    }

    pub fn bring_game_window_to_front(pid: u32) {
        if let Some(hwnd) = find_game_hwnd(pid) {
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
                BringWindowToTop(hwnd);
                SetForegroundWindow(hwnd);
                SetWindowPos(hwnd, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
            }
        }
    }

    pub struct WinInputGuard {
        thread_id: u32,
    }

    impl Drop for WinInputGuard {
        fn drop(&mut self) {
            HOOK_RUNNING.store(false, Ordering::SeqCst);
            if self.thread_id != 0 {
                unsafe {
                    PostThreadMessageA(self.thread_id, WM_QUIT, 0, 0);
                    timeEndPeriod(1);
                }
            }
        }
    }

    pub fn start_mouse_hook(pid: u32, hide_cursor: bool) -> WinInputGuard {
        GAME_PID.store(pid, Ordering::SeqCst);
        HIDE_CURSOR_ENABLED.store(hide_cursor, Ordering::SeqCst);
        HOOK_RUNNING.store(true, Ordering::SeqCst);

        let (tx, rx) = std::sync::mpsc::channel();

        thread::spawn(move || {
            unsafe {
                timeBeginPeriod(1);

                let tid = GetCurrentThreadId();
                let hook = SetWindowsHookExA(WH_MOUSE_LL, low_level_mouse_proc, 0, 0);
                HOOK_HANDLE.store(hook, Ordering::SeqCst);
                let _ = tx.send(tid);

                if hook != 0 {
                    let mut msg: MSG = std::mem::zeroed();
                    while HOOK_RUNNING.load(Ordering::SeqCst) && GetMessageA(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        // Hook message loop
                    }
                    UnhookWindowsHookEx(hook);
                    HOOK_HANDLE.store(0, Ordering::SeqCst);
                }
                timeEndPeriod(1);
            }
        });

        let thread_id = rx.recv_timeout(Duration::from_millis(500)).unwrap_or(0);
        WinInputGuard { thread_id }
    }
}

#[cfg(target_os = "windows")]
pub use imp::*;

#[cfg(not(target_os = "windows"))]
mod stub {
    pub struct WinInputGuard;
    pub fn start_mouse_hook(_pid: u32, _hide_cursor: bool) -> WinInputGuard {
        WinInputGuard
    }
    pub fn bring_game_window_to_front(_pid: u32) {}
    pub fn find_game_hwnd(_pid: u32) -> Option<isize> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::*;
