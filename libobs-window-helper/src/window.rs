use std::{
    ffi::OsString,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{error::WindowHelperError, get_thread_proc_id, string_conv::ToUtf8String, ProcessInfo};
use windows::core::Error;
use windows::{
    core::HSTRING,
    Wdk::System::Threading::{NtQueryInformationProcess, ProcessBasicInformation},
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, MAX_PATH, UNICODE_STRING},
        Globalization::GetSystemDefaultLangID,
        Graphics::Gdi::{
            MonitorFromWindow, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL,
        },
        Storage::FileSystem::{
            GetFileVersionInfoExW, GetFileVersionInfoSizeExW, VerQueryValueW, FILE_VER_GET_NEUTRAL,
        },
        System::{
            Diagnostics::Debug::ReadProcessMemory,
            ProcessStatus::GetModuleFileNameExW,
            Threading::{
                OpenProcess, PROCESS_BASIC_INFORMATION, PROCESS_QUERY_INFORMATION,
                PROCESS_TERMINATE, PROCESS_VM_READ,
            },
        },
        UI::WindowsAndMessaging::{GetClassNameW, GetWindowTextLengthW, GetWindowTextW},
    },
};

const SZ_STRING_FILE_INFO: &str = "StringFileInfo";
const SZ_PRODUCT_NAME: &str = "ProductName";
const SZ_HEX_CODE_PAGE_ID_UNICODE: &str = "04B0";

/// Retrieves the executable path and process ID associated with the given window handle.
///
/// # Arguments
///
/// * `handle` - The handle to the window.
///
/// # Returns
///
/// Returns a tuple containing the process ID and the path to the executable.
///
/// # Errors
///
/// Returns an error if there was a problem retrieving the executable path or process ID.
pub fn get_exe(handle: HWND) -> Result<(u32, PathBuf), WindowHelperError> {
    let ProcessInfo {
        process_id: proc_id,
        ..
    } = get_thread_proc_id(handle)?;
    let h_proc = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_TERMINATE,
            false,
            proc_id,
        )?
    };

    let exe = unsafe {
        let mut path = [0_u16; MAX_PATH as usize];
        // HMODULE should be null, not default
        let res = GetModuleFileNameExW(Some(h_proc), None, &mut path);
        if res > 0 {
            Ok::<String, WindowHelperError>(path.as_ref().to_utf8())
        } else {
            Err(Error::from_thread().into())
        }
    }?;

    unsafe {
        CloseHandle(h_proc)?;
    }

    Ok((proc_id, PathBuf::from_str(&exe)?))
}

pub fn get_title(handle: HWND) -> Result<String, WindowHelperError> {
    let len = unsafe { GetWindowTextLengthW(handle) };
    if len == 0 {
        return Err(Error::from_thread().into());
    }

    let len = TryInto::<usize>::try_into(len)?;

    let mut title = vec![0_u16; len + 1];
    let get_title_res = unsafe { GetWindowTextW(handle, &mut title) };
    if get_title_res == 0 {
        return Err(Error::from_thread().into());
    }

    Ok(title.to_utf8())
}

pub fn get_window_class(handle: HWND) -> Result<String, WindowHelperError> {
    let mut class = [0_u16; MAX_PATH as usize + 1];

    let len = unsafe { GetClassNameW(handle, &mut class) };
    if len == 0 {
        return Err(Error::from_thread().into());
    }

    Ok(class.as_ref().to_utf8())
}

pub fn get_product_name(full_exe: &Path) -> Result<String, WindowHelperError> {
    let exe_wide = HSTRING::from(full_exe.as_os_str());

    let mut dummy = 0;
    let required_buffer_size =
        unsafe { GetFileVersionInfoSizeExW(FILE_VER_GET_NEUTRAL, &exe_wide, &mut dummy) };
    if required_buffer_size == 0 {
        return Err(Error::from_thread().into());
    }

    let mut buffer: Vec<u16> = vec![0; required_buffer_size as usize];
    unsafe {
        GetFileVersionInfoExW(
            FILE_VER_GET_NEUTRAL,
            &exe_wide,
            None,
            required_buffer_size,
            buffer.as_mut_ptr() as *mut _,
        )?;
    }

    let lang_id = unsafe { GetSystemDefaultLangID() };
    let query_key: Vec<u16> = OsString::from(format!(
        "\\{}\\{}{}\\{}",
        SZ_STRING_FILE_INFO, lang_id, SZ_HEX_CODE_PAGE_ID_UNICODE, SZ_PRODUCT_NAME
    ))
    .encode_wide()
    .collect();
    let query_key = HSTRING::from_wide(&query_key);

    let mut pages_ptr: *mut u16 = std::ptr::null_mut();
    let mut pages_length = 0;

    unsafe {
        VerQueryValueW(
            buffer.as_mut_ptr() as _,
            &query_key,
            &mut pages_ptr as *mut _ as _,
            &mut pages_length,
        )
        .ok()?
    };

    let chars_in_buf = required_buffer_size / (std::mem::size_of::<u16>() as u32);
    if pages_ptr.is_null() || chars_in_buf < pages_length {
        return Err(WindowHelperError::InvalidState(
            "Invalid buffer state".to_string(),
        ));
    }

    let product_name = unsafe { std::slice::from_raw_parts(pages_ptr, pages_length as usize - 1) };
    let product_name = String::from_utf16_lossy(product_name);

    Ok(product_name)
}

pub fn hwnd_to_monitor(handle: HWND) -> Result<HMONITOR, WindowHelperError> {
    unsafe {
        let res = MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST);
        if res.is_invalid() {
            return Err(Error::from_thread().into());
        }

        Ok(res)
    }
}

pub fn intersects_with_multiple_monitors(handle: HWND) -> Result<bool, WindowHelperError> {
    unsafe {
        let res = MonitorFromWindow(handle, MONITOR_DEFAULTTONULL);

        Ok(!res.is_invalid())
    }
}

pub fn get_command_line_args(wnd: HWND) -> Result<String, WindowHelperError> {
    let ProcessInfo {
        process_id: proc_id,
        ..
    } = get_thread_proc_id(wnd)?;

    let handle = unsafe {
        OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, //
            false,                                       //
            proc_id,                                     //
        )?
    };

    if handle.is_invalid() {
        return Err(Error::from_thread().into());
    }

    let res = unsafe { get_command_line_args_priv(handle) };
    unsafe {
        CloseHandle(handle)?;
    }

    res
}

unsafe fn get_command_line_args_priv(handle: HANDLE) -> Result<String, WindowHelperError> {
    let mut pbi = PROCESS_BASIC_INFORMATION::default();
    // get process information
    NtQueryInformationProcess(
        handle,
        ProcessBasicInformation,
        &mut pbi as *mut _ as _,
        size_of_val(&pbi) as u32,
        std::ptr::null_mut(),
    )
    .ok()?;

    // use WinDbg "dt ntdll!_PEB" command and search for ProcessParameters offset to find the truth out
    let process_parameter_offset = 0x20;
    let command_line_offset = 0x70;

    // read basic info to get ProcessParameters address, we only need the beginning of PEB
    let peb_size = process_parameter_offset + 8;
    let mut peb = vec![0u8; peb_size];

    // read basic info to get CommandLine address, we only need the beginning of ProcessParameters
    let pp_size = command_line_offset + 16;
    let mut pp = vec![0u8; pp_size];

    // read PEB
    ReadProcessMemory(
        handle,
        pbi.PebBaseAddress as _,
        peb.as_mut_ptr() as _,
        peb_size,
        None,
    )?;

    // read ProcessParameters
    let parameters = *(peb.as_ptr().add(process_parameter_offset) as *const *const u8); // address in remote process adress space

    ReadProcessMemory(
        handle,               //
        parameters as _,      //
        pp.as_mut_ptr() as _, //
        pp_size,              //
        None,                 //
    )?;

    let ptr_cmd_line = pp
        .as_ptr() //
        .add(command_line_offset) as *const UNICODE_STRING;

    let maximum_len = (*ptr_cmd_line).MaximumLength as usize;
    let mut cmd_line = vec![0u16; maximum_len];

    ReadProcessMemory(
        handle,
        (*ptr_cmd_line).Buffer.as_ptr() as _,
        cmd_line.as_mut_ptr() as _,
        maximum_len,
        None,
    )?;

    Ok(cmd_line.to_utf8())
}
