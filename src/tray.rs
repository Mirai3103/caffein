use std::ptr::null;

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NOTIFYICONDATAW, NOTIFYICONDATAW_0, NOTIFYICON_VERSION_4, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, InsertMenuItemW,
    MENUITEMINFOW, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MFS_CHECKED,
    MFS_UNCHECKED, MFT_RADIOCHECK, MFT_STRING, MIIM_FTYPE, MIIM_ID, MIIM_STATE, MIIM_STRING,
    PostMessageW, SetForegroundWindow, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP, WM_NULL,
};

use crate::config::ALL;
use crate::state::State;
use crate::win::wide;

pub const WM_TRAY: u32 = WM_APP + 1;
pub const TRAY_ID: u32 = 1;

pub const ID_TOGGLE: u32 = 100;
pub const ID_DUR_BASE: u32 = 110;
pub const ID_DISPLAY: u32 = 120;
pub const ID_SETTINGS: u32 = 130;
pub const ID_EXIT: u32 = 140;

pub fn tooltip(state: &State, now_ms: u64) -> String {
    if !state.active {
        return "Caffein — Tắt".to_string();
    }
    match state.remaining_secs(now_ms) {
        None => "Caffein — Đang giữ thức".to_string(),
        Some(secs) => {
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            if h > 0 {
                format!("Caffein — Còn {h}:{m:02}:{s:02}")
            } else {
                format!("Caffein — Còn {m:02}:{s:02}")
            }
        }
    }
}

fn fill_tip(dst: &mut [u16; 128], tip: &str) {
    dst.fill(0);
    let mut i = 0;
    for ch in tip.encode_utf16() {
        if i + 1 >= dst.len() {
            break;
        }
        dst[i] = ch;
        i += 1;
    }
}

fn make_nid(hwnd: HWND, icon: HICON, tip: &str) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    fill_tip(&mut nid.szTip, tip);
    nid
}

pub fn add(hwnd: HWND, icon: HICON, tip: &str) {
    let mut nid = make_nid(hwnd, icon, tip);
    unsafe {
        Shell_NotifyIconW(NIM_ADD, &nid);
        nid.Anonymous = NOTIFYICONDATAW_0 {
            uVersion: NOTIFYICON_VERSION_4,
        };
        Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
}

pub fn update(hwnd: HWND, icon: HICON, tip: &str) {
    let nid = make_nid(hwnd, icon, tip);
    unsafe {
        Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn remove(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

pub fn show_menu(hwnd: HWND, state: &State) {
    unsafe {
        let menu = CreatePopupMenu();
        let dur_menu = CreatePopupMenu();

        for (i, &dur) in ALL.iter().enumerate() {
            let mut label = wide(dur.label());
            let mut mii: MENUITEMINFOW = std::mem::zeroed();
            mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
            mii.fMask = MIIM_ID | MIIM_FTYPE | MIIM_STATE | MIIM_STRING;
            mii.fType = MFT_STRING | MFT_RADIOCHECK;
            mii.fState = if state.duration == dur {
                MFS_CHECKED
            } else {
                MFS_UNCHECKED
            };
            mii.wID = ID_DUR_BASE + i as u32;
            mii.dwTypeData = label.as_mut_ptr();
            InsertMenuItemW(dur_menu, i as u32, 1, &mii);
        }

        let toggle_label = wide(if state.active { "Tắt" } else { "Bật" });
        AppendMenuW(menu, MF_STRING, ID_TOGGLE as usize, toggle_label.as_ptr());

        let dur_label = wide("Thời lượng");
        AppendMenuW(menu, MF_POPUP, dur_menu as usize, dur_label.as_ptr());

        let display_label = wide("Giữ màn hình sáng");
        let display_flags = MF_STRING | if state.keep_display { MF_CHECKED } else { MF_UNCHECKED };
        AppendMenuW(menu, display_flags, ID_DISPLAY as usize, display_label.as_ptr());

        AppendMenuW(menu, MF_SEPARATOR, 0, null());

        let settings_label = wide("Cài đặt…");
        AppendMenuW(menu, MF_STRING, ID_SETTINGS as usize, settings_label.as_ptr());

        let exit_label = wide("Thoát");
        AppendMenuW(menu, MF_STRING, ID_EXIT as usize, exit_label.as_ptr());

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Duration;
    use crate::state::State;

    #[test]
    fn tooltip_text() {
        let mut s = State::new(true, Duration::H2);
        assert_eq!(tooltip(&s, 0), "Caffein — Tắt");
        s.activate(0);
        assert_eq!(tooltip(&s, 0), "Caffein — Còn 2:00:00");
        assert_eq!(tooltip(&s, 7_200_000 - 754_000), "Caffein — Còn 12:34");
        let mut i = State::new(true, Duration::Infinite);
        i.activate(0);
        assert_eq!(tooltip(&i, 0), "Caffein — Đang giữ thức");
    }
}
