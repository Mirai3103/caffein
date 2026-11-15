use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};

use crate::win::wide;

const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const NAME: &str = "Caffein";

fn open_run_key(sam: u32) -> Option<HKEY> {
    let subkey = wide(RUN_SUBKEY);
    let mut hkey: HKEY = null_mut();
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, sam, &mut hkey) };
    if status == ERROR_SUCCESS {
        Some(hkey)
    } else {
        None
    }
}

pub fn is_enabled() -> bool {
    is_enabled_as(NAME)
}

pub fn set(enable: bool) -> bool {
    set_as(NAME, enable)
}

pub fn is_enabled_as(name: &str) -> bool {
    let Some(hkey) = open_run_key(KEY_READ) else {
        return false;
    };
    let val_name = wide(name);
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            val_name.as_ptr(),
            null_mut(),
            null_mut(),
            null_mut(),
            null_mut(),
        )
    };
    unsafe { RegCloseKey(hkey) };
    status == ERROR_SUCCESS
}

pub fn set_as(name: &str, enable: bool) -> bool {
    let Some(hkey) = open_run_key(KEY_SET_VALUE) else {
        return false;
    };
    let val_name = wide(name);
    let ok = if enable {
        if let Ok(exe) = std::env::current_exe() {
            let data: Vec<u16> = std::iter::once(b'"' as u16)
                .chain(exe.as_os_str().encode_wide())
                .chain([b'"' as u16, 0])
                .collect();
            let byte_len = (data.len() * std::mem::size_of::<u16>()) as u32;
            let status = unsafe {
                RegSetValueExW(
                    hkey,
                    val_name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr().cast::<u8>(),
                    byte_len,
                )
            };
            status == ERROR_SUCCESS
        } else {
            false
        }
    } else {
        let status = unsafe { RegDeleteValueW(hkey, val_name.as_ptr()) };
        status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND
    };
    unsafe { RegCloseKey(hkey) };
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_remove() {
        const N: &str = "CaffeinTest_DeleteMe";
        assert!(set_as(N, true));
        assert!(is_enabled_as(N));
        assert!(set_as(N, false));
        assert!(!is_enabled_as(N));
        assert!(set_as(N, false)); // xóa khi value không tồn tại vẫn tính là thành công
    }
}
