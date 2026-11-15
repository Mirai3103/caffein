use windows_sys::Win32::System::Power::SetThreadExecutionState;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn set_execution_state(flags: u32) {
    unsafe {
        SetThreadExecutionState(flags);
    }
}

pub fn now_ms() -> u64 {
    unsafe { GetTickCount64() }
}
