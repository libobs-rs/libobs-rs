use crate::error::WindowHelperError;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFOEXW};

use crate::string_conv::ToUtf8String;

pub fn get_monitor_id(monitor: HMONITOR) -> Result<String, WindowHelperError> {
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;

    unsafe {
        GetMonitorInfoW(monitor, &mut monitor_info as *mut _ as _).ok()?;
    }

    Ok(monitor_info.szDevice.to_utf8())
}
