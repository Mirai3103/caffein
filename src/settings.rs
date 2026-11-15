use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_WINDOW, CreateFontIndirectW, DeleteObject, GetDC, GetDeviceCaps, GetSysColorBrush,
    HBRUSH, HDC, HFONT, LOGPIXELSX, ReleaseDC, SetBkMode, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON,
    BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, CBS_DROPDOWNLIST,
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetDlgItem, IDCANCEL, IDOK, NONCLIENTMETRICSW,
    RegisterClassExW, SPI_GETNONCLIENTMETRICS, SPI_GETWORKAREA, SW_RESTORE, SW_SHOW,
    SendMessageW, SetForegroundWindow, ShowWindow, SystemParametersInfoW, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY, WM_SETFONT, WNDCLASSEXW, WS_CAPTION,
    WS_CHILD, WS_EX_DLGMODALFRAME, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::autostart;
use crate::config::{ALL, Config, Duration};
use crate::win::wide;

const ID_CHK_AUTOSTART: i32 = 201;
const ID_CHK_ACTIVATE: i32 = 202;
const ID_CHK_DISPLAY: i32 = 203;
const ID_CMB_DURATION: i32 = 204;

const BST_UNCHECKED: usize = 0;
const BST_CHECKED: usize = 1;

struct DialogState {
    config: Config,
    on_save: fn(Config),
    font: HFONT,
}

thread_local! {
    static SETTINGS_HWND: Cell<HWND> = const { Cell::new(null_mut()) };
    static CLASS_REGISTERED: Cell<bool> = const { Cell::new(false) };
    static DIALOG_STATE: RefCell<Option<DialogState>> = const { RefCell::new(None) };
}

pub fn hwnd() -> HWND {
    SETTINGS_HWND.with(|c| c.get())
}

fn system_dpi() -> i32 {
    unsafe {
        let hdc = GetDC(null_mut());
        let dpi = GetDeviceCaps(hdc, LOGPIXELSX as i32);
        ReleaseDC(null_mut(), hdc);
        if dpi > 0 { dpi } else { 96 }
    }
}

fn scale_px(px: i32, dpi: i32) -> i32 {
    (px * dpi + 48) / 96
}

fn create_message_font() -> HFONT {
    unsafe {
        let mut ncm: NONCLIENTMETRICSW = std::mem::zeroed();
        ncm.cbSize = std::mem::size_of::<NONCLIENTMETRICSW>() as u32;
        if SystemParametersInfoW(
            SPI_GETNONCLIENTMETRICS,
            ncm.cbSize,
            (&raw mut ncm).cast(),
            0,
        ) != 0
        {
            CreateFontIndirectW(&ncm.lfMessageFont)
        } else {
            null_mut()
        }
    }
}

unsafe fn set_ctrl_font(ctrl: HWND, font: HFONT) {
    if !font.is_null() && !ctrl.is_null() {
        unsafe { SendMessageW(ctrl, WM_SETFONT, font as usize, 1) };
    }
}

unsafe fn create_child(
    parent: HWND,
    class: &str,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
    font: HFONT,
) -> HWND {
    let cls = wide(class);
    let txt = wide(text);
    let hinstance = unsafe { GetModuleHandleW(null()) };
    let child = unsafe {
        CreateWindowExW(
            0,
            cls.as_ptr(),
            txt.as_ptr(),
            style,
            x,
            y,
            w,
            h,
            parent,
            id as isize as _,
            hinstance,
            null(),
        )
    };
    unsafe { set_ctrl_font(child, font) };
    child
}

unsafe extern "system" fn settings_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let dpi = system_dpi();
            let s = |v: i32| scale_px(v, dpi);
            let font = create_message_font();

            let cfg = DIALOG_STATE.with(|cell| {
                let mut borrow = cell.borrow_mut();
                if let Some(ds) = borrow.as_mut() {
                    ds.font = font;
                    Some(ds.config.clone())
                } else {
                    None
                }
            });

            if let Some(cfg) = cfg {
                let chk_style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX as u32;
                let chk_auto = unsafe {
                    create_child(
                        hwnd,
                        "BUTTON",
                        "Khởi động cùng Windows",
                        chk_style,
                        s(16),
                        s(16),
                        s(288),
                        s(22),
                        ID_CHK_AUTOSTART,
                        font,
                    )
                };
                let chk_act = unsafe {
                    create_child(
                        hwnd,
                        "BUTTON",
                        "Tự bật giữ thức khi mở app",
                        chk_style,
                        s(16),
                        s(44),
                        s(288),
                        s(22),
                        ID_CHK_ACTIVATE,
                        font,
                    )
                };
                let chk_disp = unsafe {
                    create_child(
                        hwnd,
                        "BUTTON",
                        "Giữ màn hình sáng (mặc định)",
                        chk_style,
                        s(16),
                        s(72),
                        s(288),
                        s(22),
                        ID_CHK_DISPLAY,
                        font,
                    )
                };

                unsafe {
                    SendMessageW(
                        chk_auto,
                        BM_SETCHECK,
                        if cfg.autostart { BST_CHECKED } else { BST_UNCHECKED },
                        0,
                    );
                    SendMessageW(
                        chk_act,
                        BM_SETCHECK,
                        if cfg.activate_on_launch {
                            BST_CHECKED
                        } else {
                            BST_UNCHECKED
                        },
                        0,
                    );
                    SendMessageW(
                        chk_disp,
                        BM_SETCHECK,
                        if cfg.keep_display {
                            BST_CHECKED
                        } else {
                            BST_UNCHECKED
                        },
                        0,
                    );
                }

                unsafe {
                    create_child(
                        hwnd,
                        "STATIC",
                        "Thời lượng mặc định:",
                        WS_CHILD | WS_VISIBLE,
                        s(16),
                        s(110),
                        s(145),
                        s(20),
                        -1,
                        font,
                    );
                }

                let cmb = unsafe {
                    create_child(
                        hwnd,
                        "COMBOBOX",
                        "",
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                        s(164),
                        s(106),
                        s(140),
                        s(160),
                        ID_CMB_DURATION,
                        font,
                    )
                };

                for (i, &dur) in ALL.iter().enumerate() {
                    let label = wide(dur.label());
                    unsafe {
                        SendMessageW(cmb, CB_ADDSTRING, 0, label.as_ptr() as LPARAM);
                    }
                    if dur == cfg.duration {
                        unsafe { SendMessageW(cmb, CB_SETCURSEL, i, 0) };
                    }
                }

                unsafe {
                    create_child(
                        hwnd,
                        "BUTTON",
                        "Lưu",
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
                        s(136),
                        s(164),
                        s(80),
                        s(28),
                        IDOK,
                        font,
                    );
                    create_child(
                        hwnd,
                        "BUTTON",
                        "Hủy",
                        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
                        s(224),
                        s(164),
                        s(80),
                        s(28),
                        IDCANCEL,
                        font,
                    );
                    SetFocus(chk_auto);
                }
            }
            0
        }
        WM_CTLCOLORSTATIC => {
            let hdc = wparam as HDC;
            unsafe {
                SetBkMode(hdc, TRANSPARENT as i32);
                GetSysColorBrush(COLOR_WINDOW) as LRESULT
            }
        }
        WM_COMMAND => {
            let id = ((wparam as u32) & 0xFFFF) as i32;
            if id == IDOK {
                let checked = |ctrl_id: i32| -> bool {
                    let ctrl = unsafe { GetDlgItem(hwnd, ctrl_id) };
                    unsafe { SendMessageW(ctrl, BM_GETCHECK, 0, 0) as usize == BST_CHECKED }
                };
                let cmb = unsafe { GetDlgItem(hwnd, ID_CMB_DURATION) };
                let sel = unsafe { SendMessageW(cmb, CB_GETCURSEL, 0, 0) };
                let duration = usize::try_from(sel)
                    .ok()
                    .and_then(|idx| ALL.get(idx).copied())
                    .unwrap_or(Duration::Infinite);

                let new_cfg = Config {
                    autostart: checked(ID_CHK_AUTOSTART),
                    activate_on_launch: checked(ID_CHK_ACTIVATE),
                    keep_display: checked(ID_CHK_DISPLAY),
                    duration,
                };

                let callback = DIALOG_STATE.with(|cell| cell.borrow().as_ref().map(|ds| ds.on_save));
                if let Some(on_save) = callback {
                    on_save(new_cfg);
                }
                unsafe { DestroyWindow(hwnd) };
                return 0;
            } else if id == IDCANCEL {
                unsafe { DestroyWindow(hwnd) };
                return 0;
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CLOSE => {
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_DESTROY => {
            DIALOG_STATE.with(|cell| {
                if let Some(ds) = cell.borrow_mut().take() {
                    if !ds.font.is_null() {
                        unsafe { DeleteObject(ds.font) };
                    }
                }
            });
            SETTINGS_HWND.with(|c| c.set(null_mut()));
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn open(owner: HWND, cfg: &Config, on_save: fn(Config)) {
    let existing = hwnd();
    if !existing.is_null() {
        unsafe {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
        }
        return;
    }

    let hinstance = unsafe { GetModuleHandleW(null()) };
    let class_name = wide("CaffeinSettingsClass");

    CLASS_REGISTERED.with(|reg| {
        if !reg.get() {
            unsafe {
                let icc = INITCOMMONCONTROLSEX {
                    dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
                    dwICC: ICC_STANDARD_CLASSES,
                };
                InitCommonControlsEx(&icc);

                let wc = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfnWndProc: Some(settings_wnd_proc),
                    cbClsExtra: 0,
                    cbWndExtra: 0,
                    hInstance: hinstance,
                    hIcon: null_mut(),
                    hCursor: null_mut(),
                    hbrBackground: (COLOR_WINDOW + 1) as isize as HBRUSH,
                    lpszMenuName: null(),
                    lpszClassName: class_name.as_ptr(),
                    hIconSm: null_mut(),
                };
                RegisterClassExW(&wc);
            }
            reg.set(true);
        }
    });

    let mut initial_cfg = cfg.clone();
    initial_cfg.autostart = autostart::is_enabled();

    DIALOG_STATE.with(|cell| {
        *cell.borrow_mut() = Some(DialogState {
            config: initial_cfg,
            on_save,
            font: null_mut(),
        });
    });

    let dpi = system_dpi();
    let client_w = scale_px(320, dpi);
    let client_h = scale_px(210, dpi);
    let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
    let ex_style = WS_EX_DLGMODALFRAME;

    let mut rc = RECT {
        left: 0,
        top: 0,
        right: client_w,
        bottom: client_h,
    };
    unsafe { AdjustWindowRectEx(&mut rc, style, 0, ex_style) };
    let win_w = rc.right - rc.left;
    let win_h = rc.bottom - rc.top;

    let mut work: RECT = unsafe { std::mem::zeroed() };
    let (x, y) = if unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, (&raw mut work).cast(), 0)
    } != 0
    {
        (
            work.left + ((work.right - work.left) - win_w) / 2,
            work.top + ((work.bottom - work.top) - win_h) / 2,
        )
    } else {
        (100, 100)
    };

    let title = wide("Cài đặt — Caffein");
    let dlg = unsafe {
        CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            title.as_ptr(),
            style,
            x,
            y,
            win_w,
            win_h,
            owner,
            null_mut(),
            hinstance,
            null(),
        )
    };

    if dlg.is_null() {
        DIALOG_STATE.with(|cell| {
            *cell.borrow_mut() = None;
        });
        return;
    }

    SETTINGS_HWND.with(|c| c.set(dlg));
    unsafe {
        ShowWindow(dlg, SW_SHOW);
        SetForegroundWindow(dlg);
    }
}
