#![windows_subsystem = "windows"]

mod autostart;
mod config;
mod icon;
mod settings;
mod state;
mod tray;
mod win;

use std::cell::RefCell;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow,
    DispatchMessageW, GetMessageW, HICON, IsDialogMessageW, KillTimer, MSG, MSGFLT_ALLOW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetTimer, TranslateMessage,
    WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_TIMER, WNDCLASSEXW, WS_EX_TOOLWINDOW,
};

use config::{ALL, Config};
use state::{ES_CONTINUOUS, State};
use win::{now_ms, set_execution_state, wide};

const TIMER_ID: usize = 1;

struct App {
    hwnd: HWND,
    state: State,
    config: Config,
    icons: [HICON; 2], // [0] = inactive, [1] = active
    wm_taskbar_created: u32,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| {
        let mut borrow = cell.borrow_mut();
        borrow.as_mut().map(f)
    })
}

fn current_icon_and_tip(app: &App, now: u64) -> (HICON, String) {
    let idx = usize::from(app.state.active);
    (app.icons[idx], tray::tooltip(&app.state, now))
}

fn apply(app: &App) {
    let now = now_ms();
    set_execution_state(app.state.flags());
    let (icon, tip) = current_icon_and_tip(app, now);
    tray::update(app.hwnd, icon, &tip);
    unsafe {
        if app.state.needs_timer() {
            SetTimer(app.hwnd, TIMER_ID, 1000, None);
        } else {
            KillTimer(app.hwnd, TIMER_ID);
        }
    }
}

fn on_settings_save(new_cfg: Config) {
    let _ = new_cfg.save();
    autostart::set(new_cfg.autostart);
    with_app(|app| {
        app.config = new_cfg.clone();
        app.state
            .apply_config(new_cfg.keep_display, new_cfg.duration, now_ms());
        apply(app);
    });
}

fn allow_taskbar_broadcast(hwnd: HWND, msg: u32) -> bool {
    if msg == 0 {
        return false;
    }
    unsafe { ChangeWindowMessageFilterEx(hwnd, msg, MSGFLT_ALLOW, null_mut()) != 0 }
}

fn handle_context_menu(show: impl FnOnce(HWND, &State)) {
    if let Some((hwnd, state)) = with_app(|app| (app.hwnd, app.state)) {
        show(hwnd, &state);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg != 0 {
        let readd = with_app(|app| {
            if msg == app.wm_taskbar_created {
                let (icon, tip) = current_icon_and_tip(app, now_ms());
                tray::add(app.hwnd, icon, &tip);
                true
            } else {
                false
            }
        });
        if readd == Some(true) {
            return 0;
        }
    }

    match msg {
        tray::WM_TRAY => {
            let event = (lparam as u32) & 0xFFFF;
            if event == WM_LBUTTONUP {
                with_app(|app| {
                    app.state.toggle(now_ms());
                    apply(app);
                });
            } else if event == WM_CONTEXTMENU {
                handle_context_menu(tray::show_menu);
            }
            0
        }
        WM_TIMER => {
            if wparam == TIMER_ID {
                with_app(|app| {
                    let _ = app.state.tick(now_ms());
                    apply(app);
                });
            }
            0
        }
        WM_COMMAND => {
            let id = (wparam as u32) & 0xFFFF;
            if id == tray::ID_TOGGLE {
                with_app(|app| {
                    app.state.toggle(now_ms());
                    apply(app);
                });
            } else if id == tray::ID_DISPLAY {
                with_app(|app| {
                    let v = !app.state.keep_display;
                    app.state.set_keep_display(v);
                    app.config.keep_display = v;
                    let _ = app.config.save();
                    apply(app);
                });
            } else if id >= tray::ID_DUR_BASE
                && (id - tray::ID_DUR_BASE) < ALL.len() as u32
            {
                let dur = ALL[(id - tray::ID_DUR_BASE) as usize];
                with_app(|app| {
                    app.state.set_duration(dur, now_ms());
                    app.config.duration = dur;
                    let _ = app.config.save();
                    apply(app);
                });
            } else if id == tray::ID_SETTINGS {
                if let Some((owner, cfg)) = with_app(|app| (app.hwnd, app.config.clone())) {
                    settings::open(owner, &cfg, on_settings_save);
                }
            } else if id == tray::ID_EXIT {
                let dlg = settings::hwnd();
                if !dlg.is_null() {
                    unsafe { DestroyWindow(dlg) };
                }
                unsafe { DestroyWindow(hwnd) };
            }
            0
        }
        WM_DESTROY => {
            with_app(|app| {
                unsafe { KillTimer(app.hwnd, TIMER_ID) };
                tray::remove(app.hwnd);
                set_execution_state(ES_CONTINUOUS);
                for hicon in app.icons {
                    if !hicon.is_null() {
                        unsafe { DestroyIcon(hicon) };
                    }
                }
            });
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn main() {
    let mutex_name = wide("Local\\Caffein.SingleInstance");
    let mutex = unsafe { CreateMutexW(null(), 1, mutex_name.as_ptr()) };
    if mutex.is_null() || unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        if !mutex.is_null() {
            unsafe { CloseHandle(mutex) };
        }
        return;
    }

    let hinstance = unsafe { GetModuleHandleW(null()) };
    let class_name = wide("CaffeinTrayClass");
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: null_mut(),
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };

    let title = wide("Caffein");
    // Hidden top-level window (not HWND_MESSAGE) so TaskbarCreated broadcasts are received.
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinstance,
            null(),
        )
    };
    if hwnd.is_null() {
        unsafe { CloseHandle(mutex) };
        return;
    }

    let mut config = Config::load();
    config.autostart = autostart::is_enabled();
    if config.autostart {
        autostart::set(true);
    }

    let mut state = State::new(config.keep_display, config.duration);
    if config.activate_on_launch {
        state.activate(now_ms());
    }

    let icons = [icon::make(false), icon::make(true)];
    let taskbar_msg_name = wide("TaskbarCreated");
    let wm_taskbar_created = unsafe { RegisterWindowMessageW(taskbar_msg_name.as_ptr()) };
    allow_taskbar_broadcast(hwnd, wm_taskbar_created);

    APP.with(|cell| {
        *cell.borrow_mut() = Some(App {
            hwnd,
            state,
            config,
            icons,
            wm_taskbar_created,
        });
    });

    with_app(|app| {
        let (icon, tip) = current_icon_and_tip(app, now_ms());
        tray::add(app.hwnd, icon, &tip);
        apply(app);
    });

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    unsafe {
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            let dlg = settings::hwnd();
            if !dlg.is_null() && IsDialogMessageW(dlg, &msg) != 0 {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        CloseHandle(mutex);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::Duration;

    #[test]
    fn context_menu_allows_reentrant_timer_without_borrow_panic() {
        APP.with(|cell| {
            *cell.borrow_mut() = Some(App {
                hwnd: null_mut(),
                state: State::new(true, Duration::M15),
                config: Config::default(),
                icons: [null_mut(), null_mut()],
                wm_taskbar_created: 0,
            });
        });

        handle_context_menu(|_, _| {
            with_app(|app| {
                let _ = app.state.tick(now_ms());
            });
        });

        APP.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }

    #[test]
    fn allows_taskbar_created_through_uipi() {
        let hinstance = unsafe { GetModuleHandleW(null()) };
        let cls = wide("CaffeinTestUipiClass");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(DefWindowProcW),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: cls.as_ptr(),
            hIconSm: null_mut(),
        };
        unsafe { RegisterClassExW(&wc) };
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                cls.as_ptr(),
                cls.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                hinstance,
                null(),
            )
        };
        assert!(!hwnd.is_null());
        let msg_name = wide("TaskbarCreated");
        let msg = unsafe { RegisterWindowMessageW(msg_name.as_ptr()) };
        let ok = allow_taskbar_broadcast(hwnd, msg);
        unsafe { DestroyWindow(hwnd) };
        assert!(ok);
    }
}
