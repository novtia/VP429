use crate::engine::ProxyEngine;
use std::sync::Arc;

#[cfg(windows)]
mod imp {
    use crate::engine::ProxyEngine;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::{Arc, OnceLock};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
        DispatchMessageW, FindWindowW, GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage,
        RegisterClassW, SetForegroundWindow, ShowWindow, TrackPopupMenu, TranslateMessage,
        IDI_APPLICATION, MF_SEPARATOR, MF_STRING, MSG, SW_HIDE, SW_RESTORE, SW_SHOW,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_APP, WM_COMMAND, WM_LBUTTONDBLCLK, WM_LBUTTONUP,
        WM_RBUTTONUP, WNDCLASSW,
    };

    const WM_TRAY_CALLBACK: u32 = WM_APP + 42;
    const TRAY_UID: u32 = 1001;
    const CMD_SHOW: usize = 2001;
    const CMD_ROTATE: usize = 2002;
    const CMD_QUIT: usize = 2003;

    static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);
    static TRAY_CTX: OnceLock<(Arc<ProxyEngine>, tokio::runtime::Handle)> = OnceLock::new();

    pub fn is_window_visible() -> bool {
        WINDOW_VISIBLE.load(Ordering::Relaxed)
    }

    pub fn trim_working_set() {
        unsafe {
            let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn capture_main_window() -> HWND {
        let cached = MAIN_HWND.load(Ordering::SeqCst) as HWND;
        if !cached.is_null() {
            return cached;
        }
        let title = wide("VP429");
        let hwnd = unsafe { FindWindowW(null(), title.as_ptr()) };
        if !hwnd.is_null() {
            MAIN_HWND.store(hwnd as isize, Ordering::SeqCst);
        }
        hwnd
    }

    pub fn hide_to_tray() {
        let hwnd = capture_main_window();
        if !hwnd.is_null() {
            WINDOW_VISIBLE.store(false, Ordering::Relaxed);
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
            trim_working_set();
        }
    }

    pub fn show_main_window() {
        let hwnd = capture_main_window();
        if !hwnd.is_null() {
            WINDOW_VISIBLE.store(true, Ordering::Relaxed);
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }

    pub fn remove_tray_icon() {
        let tray_hwnd = TRAY_HWND.load(Ordering::SeqCst) as HWND;
        if !tray_hwnd.is_null() {
            unsafe {
                let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = tray_hwnd;
                nid.uID = TRAY_UID;
                Shell_NotifyIconW(NIM_DELETE, &nid);
            }
        }
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_TRAY_CALLBACK => {
                let event = (lparam as u32) & 0xFFFF;
                if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                    show_main_window();
                } else if event == WM_RBUTTONUP {
                    let hmenu = CreatePopupMenu();
                    if !hmenu.is_null() {
                        let s_show = wide("显示主窗口");
                        let s_rot = wide("切换下一节点");
                        let s_quit = wide("退出 VP429");
                        AppendMenuW(hmenu, MF_STRING, CMD_SHOW, s_show.as_ptr());
                        AppendMenuW(hmenu, MF_STRING, CMD_ROTATE, s_rot.as_ptr());
                        AppendMenuW(hmenu, MF_SEPARATOR, 0, null());
                        AppendMenuW(hmenu, MF_STRING, CMD_QUIT, s_quit.as_ptr());

                        let mut pt = POINT { x: 0, y: 0 };
                        GetCursorPos(&mut pt);
                        SetForegroundWindow(hwnd);
                        TrackPopupMenu(
                            hmenu,
                            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
                            pt.x,
                            pt.y,
                            0,
                            hwnd,
                            null(),
                        );
                        DestroyMenu(hmenu);
                    }
                }
                0
            }
            WM_COMMAND => {
                let cmd = wparam & 0xFFFF;
                match cmd {
                    CMD_SHOW => show_main_window(),
                    CMD_ROTATE => {
                        if let Some((eng, rt)) = TRAY_CTX.get() {
                            let eng = eng.clone();
                            rt.spawn(async move {
                                let _ = eng.scheduler.rotate_next("托盘菜单切换节点").await;
                            });
                        }
                    }
                    CMD_QUIT => {
                        remove_tray_icon();
                        if let Some((eng, rt)) = TRAY_CTX.get() {
                            rt.block_on(async {
                                eng.shutdown().await;
                            });
                        }
                        PostQuitMessage(0);
                        std::process::exit(0);
                    }
                    _ => {}
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    pub fn spawn_tray(engine: Arc<ProxyEngine>, rt: tokio::runtime::Handle) {
        let _ = TRAY_CTX.set((engine, rt));
        std::thread::spawn(move || unsafe {
            let hinst = GetModuleHandleW(null());
            let class_name = wide("VP429_TrayMessageWindow");

            let wc = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinst,
                hIcon: null_mut(),
                hCursor: null_mut(),
                hbrBackground: null_mut(),
                lpszMenuName: null(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                hinst,
                null(),
            );
            if hwnd.is_null() {
                return;
            }
            TRAY_HWND.store(hwnd as isize, Ordering::SeqCst);

            // Load embedded Icon 02 from exe resource ID 1 (fallback to default app icon in debug)
            let mut hicon = LoadIconW(hinst, 1usize as *const u16);
            if hicon.is_null() {
                hicon = LoadIconW(null_mut(), IDI_APPLICATION);
            }

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = TRAY_UID;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAY_CALLBACK;
            nid.hIcon = hicon;

            let tip = "VP429";
            for (i, ch) in tip.encode_utf16().take(127).enumerate() {
                nid.szTip[i] = ch;
            }
            Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            remove_tray_icon();
        });
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::engine::ProxyEngine;
    use std::sync::Arc;
    pub fn spawn_tray(_engine: Arc<ProxyEngine>, _rt: tokio::runtime::Handle) {}
    pub fn hide_to_tray() {}
    pub fn remove_tray_icon() {}
    pub fn is_window_visible() -> bool {
        true
    }
}

pub fn spawn_tray(engine: Arc<ProxyEngine>, rt: tokio::runtime::Handle) {
    imp::spawn_tray(engine, rt);
}

pub fn hide_to_tray() {
    imp::hide_to_tray();
}

pub fn remove_tray_icon() {
    imp::remove_tray_icon();
}

pub fn is_window_visible() -> bool {
    imp::is_window_visible()
}
