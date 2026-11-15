#![windows_subsystem = "windows"]

mod autostart;
mod config;
mod icon;
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
    CreateWindowExW, DefWindowProcW, DestroyIcon, DestroyWindow, DispatchMessageW, GetMessageW,
    HICON, KillTimer, MSG, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SetTimer,
    TranslateMessage, WM_COMMAND, WM_CONTEXTMENU, WM_DESTROY, WM_LBUTTONUP, WM_TIMER, WNDCLASSEXW,
    WS_EX_TOOLWINDOW,
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
                with_app(|app| {
                    tray::show_menu(app.hwnd, &app.state);
                });
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
                // Task 5 sẽ nối vào
            } else if id == tray::ID_EXIT {
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

    let config = Config::load();
    let mut state = State::new(config.keep_display, config.duration);
    if config.activate_on_launch {
        state.activate(now_ms());
    }

    let icons = [icon::make(false), icon::make(true)];
    let taskbar_msg_name = wide("TaskbarCreated");
    let wm_taskbar_created = unsafe { RegisterWindowMessageW(taskbar_msg_name.as_ptr()) };

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
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        CloseHandle(mutex);
    }
}
