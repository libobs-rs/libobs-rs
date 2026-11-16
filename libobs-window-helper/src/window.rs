use std::{
    ffi::OsString,
    os::windows::ffi::OsStrExt,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{error::WindowHelperError, get_thread_proc_id, string_conv::ToUtf8String, ProcessInfo};
use windows::core::Error;
use windows::{
    core::HSTRING,
    Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation},
    Win32::{
        Foundation::{HANDLE, HWND, MAX_PATH, UNICODE_STRING},
        Globalization::GetSystemDefaultLangID,
        Graphics::Gdi::{
            MonitorFromWindow, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL,
        },
        Storage::FileSystem::{
            GetFileVersionInfoExW, GetFileVersionInfoSizeExW, VerQueryValueW, FILE_VER_GET_NEUTRAL,
        },
        System::{
            ProcessStatus::GetModuleFileNameExW,
            Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
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
    // Safety: Opening a process handle requires a valid PID retrieved from `get_thread_proc_id`; the
    // desired access flags match read/query semantics used below.
    let h_proc =
        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, proc_id)? };
    // SAFETY: OpenProcess returned a newly owned handle. Transfer its sole close obligation
    // to OwnedHandle so every error path below also closes it.
    let h_proc = unsafe { OwnedHandle::from_raw_handle(h_proc.0) };

    // Safety: `h_proc` is a live process handle; `path` is a sufficiently sized buffer, and the call
    // does not outlive the buffer.
    let exe = unsafe {
        let mut path = [0_u16; MAX_PATH as usize];
        // HMODULE should be null, not default
        let res = GetModuleFileNameExW(Some(HANDLE(h_proc.as_raw_handle())), None, &mut path);
        if res > 0 {
            Ok::<String, WindowHelperError>(path.as_ref().to_utf8())
        } else {
            Err(Error::from_thread().into())
        }
    }?;

    Ok((proc_id, PathBuf::from_str(&exe)?))
}

pub fn get_title(handle: HWND) -> Result<String, WindowHelperError> {
    // Safety: `handle` is a valid HWND; querying its title length does not require extra invariants.
    let len = unsafe { GetWindowTextLengthW(handle) };
    if len == 0 {
        return Err(Error::from_thread().into());
    }

    let len = TryInto::<usize>::try_into(len)?;

    let mut title = vec![0_u16; len + 1];
    // Safety: `title` is a writable buffer sized from the length query; `handle` is valid.
    let get_title_res = unsafe { GetWindowTextW(handle, &mut title) };
    if get_title_res == 0 {
        return Err(Error::from_thread().into());
    }

    Ok(title.to_utf8())
}

pub fn get_window_class(handle: HWND) -> Result<String, WindowHelperError> {
    let mut class = [0_u16; MAX_PATH as usize + 1];

    // Safety: `class` is a writable buffer and `handle` is a valid HWND; Win32 writes at most
    // `class.len()` characters.
    let len = unsafe { GetClassNameW(handle, &mut class) };
    if len == 0 {
        return Err(Error::from_thread().into());
    }

    Ok(class.as_ref().to_utf8())
}

pub fn get_product_name(full_exe: &Path) -> Result<String, WindowHelperError> {
    let exe_wide = HSTRING::from(full_exe.as_os_str());

    let mut dummy = 0;
    // Safety: `exe_wide` points to a valid string buffer; `dummy` is a valid out pointer for the size.
    let required_buffer_size =
        unsafe { GetFileVersionInfoSizeExW(FILE_VER_GET_NEUTRAL, &exe_wide, &mut dummy) };
    if required_buffer_size == 0 {
        return Err(Error::from_thread().into());
    }

    let mut buffer = vec![0_u16; (required_buffer_size as usize).div_ceil(2)];
    // Safety: `buffer` is allocated with the required size, and `exe_wide`/`dummy` remain valid for the
    // duration of the call.
    unsafe {
        GetFileVersionInfoExW(
            FILE_VER_GET_NEUTRAL,
            &exe_wide,
            None,
            required_buffer_size,
            buffer.as_mut_ptr() as *mut _,
        )?;
    }

    // Safety: This query reads process-global locale information and requires no additional invariants.
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

    // Safety: `buffer` lives for the duration of the call; `pages_ptr` and `pages_length` are valid out
    // parameters for the version query.
    unsafe {
        VerQueryValueW(
            buffer.as_mut_ptr() as _,
            &query_key,
            &mut pages_ptr as *mut _ as _,
            &mut pages_length,
        )
        .ok()?
    };

    product_name_from_query(
        &buffer,
        required_buffer_size as usize,
        pages_ptr,
        pages_length,
    )
}

fn product_name_from_query(
    buffer: &[u16],
    valid_bytes: usize,
    pages_ptr: *const u16,
    pages_length: u32,
) -> Result<String, WindowHelperError> {
    if pages_length == 0 {
        return Err(WindowHelperError::InvalidState(
            "Invalid buffer state".to_string(),
        ));
    }

    let product_name = utf16_range(
        buffer,
        valid_bytes,
        pages_ptr,
        pages_length as usize * size_of::<u16>(),
    )?;
    let Some((&0, product_name)) = product_name.split_last() else {
        return Err(WindowHelperError::InvalidState(
            "Unterminated product name".into(),
        ));
    };
    let product_name = String::from_utf16_lossy(product_name);

    Ok(product_name)
}

pub fn hwnd_to_monitor(handle: HWND) -> Result<HMONITOR, WindowHelperError> {
    // Safety: `handle` is a valid HWND; querying its nearest monitor returns either a valid handle or an
    // error which we propagate.
    unsafe {
        let res = MonitorFromWindow(handle, MONITOR_DEFAULTTONEAREST);
        if res.is_invalid() {
            return Err(Error::from_thread().into());
        }

        Ok(res)
    }
}

pub fn intersects_with_multiple_monitors(handle: HWND) -> Result<bool, WindowHelperError> {
    // Safety: `handle` is a valid HWND; querying for a monitor handle is safe and the result is checked
    // for validity before use.
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

    // Safety: `proc_id` originates from `get_thread_proc_id`; requested access is limited to read/query
    // operations needed for command line extraction.
    let handle = unsafe {
        OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, //
            false,                                       //
            proc_id,                                     //
        )?
    };

    // SAFETY: OpenProcess returned a newly owned handle, closed by OwnedHandle on every exit.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    // SAFETY: OwnedHandle retains the open query-capable handle throughout the query.
    unsafe { get_command_line_args_priv(HANDLE(handle.as_raw_handle())) }
}

/// # Safety
///
/// `handle` must be a valid process handle with read/query permissions that remains open for the
/// duration of this call. Process exit is reported as an API error.
unsafe fn get_command_line_args_priv(handle: HANDLE) -> Result<String, WindowHelperError> {
    let mut required = 0;
    // SAFETY: The null, zero-sized buffer requests only the required size; `required`
    // is a live out parameter. This query copies the string into our process, including
    // for WOW64 targets, without interpreting remote PEB layouts.
    let status = unsafe {
        NtQueryInformationProcess(
            handle,
            ProcessCommandLineInformation,
            std::ptr::null_mut(),
            0,
            &mut required,
        )
    };
    if required == 0 {
        status.ok()?;
    }
    // The command line can grow between sizing and copying. Bound both retries and allocation.
    for _ in 0..3 {
        let byte_len = required as usize;
        if !(size_of::<UNICODE_STRING>()..=size_of::<UNICODE_STRING>() + u16::MAX as usize + 2)
            .contains(&byte_len)
        {
            return Err(WindowHelperError::InvalidState(
                "Invalid command line buffer size".into(),
            ));
        }
        let mut buffer = vec![0_usize; byte_len.div_ceil(size_of::<usize>())];
        // SAFETY: usize storage supplies pointer alignment and at least byte_len writable
        // bytes. Windows writes a UNICODE_STRING followed by its UTF-16 data.
        let status = unsafe {
            NtQueryInformationProcess(
                handle,
                ProcessCommandLineInformation,
                buffer.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        };
        if status.is_err() && required as usize > byte_len {
            continue;
        }
        status.ok()?;
        if (required as usize) < size_of::<UNICODE_STRING>() || required as usize > byte_len {
            return Err(WindowHelperError::InvalidState(
                "Invalid command line result size".into(),
            ));
        }
        // SAFETY: A successful query wrote the header into aligned, sufficiently large storage.
        let header = unsafe { buffer.as_ptr().cast::<UNICODE_STRING>().read() };
        if header.Length > header.MaximumLength {
            return Err(WindowHelperError::InvalidState(
                "Invalid command line length".into(),
            ));
        }
        if header.Length == 0 {
            return Ok(String::new());
        }
        // SAFETY: All storage was initialized, u16 accepts every bit pattern, and usize
        // alignment satisfies u16. The immutable view lasts only while buffer is live.
        let words = unsafe {
            std::slice::from_raw_parts(
                buffer.as_ptr().cast::<u16>(),
                buffer.len() * size_of::<usize>() / 2,
            )
        };
        let text = utf16_range(
            words,
            required as usize,
            header.Buffer.as_ptr(),
            header.Length as usize,
        )?;
        return Ok(String::from_utf16_lossy(text));
    }
    Err(WindowHelperError::InvalidState(
        "Command line changed during query".into(),
    ))
}

/// Validate an API-returned address and byte count, then borrow through the original
/// allocation. Merely checking a non-null address or the length alone is insufficient.
fn utf16_range(
    buffer: &[u16],
    valid_bytes: usize,
    pointer: *const u16,
    byte_len: usize,
) -> Result<&[u16], WindowHelperError> {
    let invalid = || WindowHelperError::InvalidState("UTF-16 range lies outside its buffer".into());
    let offset = (pointer as usize)
        .checked_sub(buffer.as_ptr() as usize)
        .ok_or_else(invalid)?;
    let end = offset.checked_add(byte_len).ok_or_else(invalid)?;
    if pointer.is_null()
        || !offset.is_multiple_of(2)
        || !byte_len.is_multiple_of(2)
        || end > valid_bytes
        || valid_bytes > std::mem::size_of_val(buffer)
    {
        return Err(invalid());
    }
    Ok(&buffer[offset / 2..end / 2])
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    #[test]
    fn utf16_ranges_validate_both_address_and_byte_length() {
        let buffer = [0, 65, 66, 0];
        let ptr = buffer.as_ptr().wrapping_add(1);
        assert_eq!(utf16_range(&buffer, 8, ptr, 6).unwrap(), [65, 66, 0]);
        assert!(utf16_range(&buffer, 8, std::ptr::null(), 0).is_err());
        assert!(utf16_range(&buffer, 8, ptr, 8).is_err());
        assert!(utf16_range(&buffer, 8, ptr, 3).is_err());
        assert!(utf16_range(&buffer, 6, ptr, 6).is_err());
        assert!(utf16_range(&buffer, 10, ptr, 6).is_err());
        let unaligned = buffer.as_ptr().cast::<u8>().wrapping_add(1).cast();
        assert!(utf16_range(&buffer, 8, unaligned, 2).is_err());
    }

    #[test]
    fn product_name_rejects_empty_unterminated_and_out_of_bounds_results() {
        let buffer = [65, 66, 0];
        let ptr = buffer.as_ptr();
        assert_eq!(product_name_from_query(&buffer, 6, ptr, 3).unwrap(), "AB");
        assert!(product_name_from_query(&buffer, 6, ptr, 0).is_err());
        assert!(product_name_from_query(&buffer, 6, ptr, 2).is_err());
        assert!(product_name_from_query(&buffer, 6, ptr.wrapping_add(1), 3).is_err());
        assert!(product_name_from_query(&buffer, 6, std::ptr::null(), 1).is_err());
    }

    #[test]
    fn command_line_query_reads_current_process() {
        // SAFETY: The pseudo-handle remains valid and is borrowed, never closed.
        let result = unsafe {
            get_command_line_args_priv(windows::Win32::System::Threading::GetCurrentProcess())
        }
        .unwrap();
        assert!(result.contains("libobs_window_helper"), "{result}");
    }

    #[test]
    fn command_line_query_reads_long_unicode_args_from_wow64_process() {
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
        };

        let cmd = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("SysWOW64/cmd.exe");
        if !cmd.exists() {
            return; // WOW64 is optional on some Windows installations.
        }
        let marker = "\u{6e2c}\u{8a66}\u{1f600}".repeat(256);
        // set /p blocks on our pipe until it is closed; rem keeps the marker inert.
        let mut child = Command::new(cmd)
            .args(["/D", "/Q", "/C", &format!("set /p x= & rem {marker}")])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .unwrap();
        let result = (|| {
            // SAFETY: Open a query handle for the live child, then transfer ownership once.
            let raw = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, false, child.id()) }?;
            let handle = unsafe { OwnedHandle::from_raw_handle(raw.0) };
            // SAFETY: The owned handle remains open throughout this read-only query.
            unsafe { get_command_line_args_priv(HANDLE(handle.as_raw_handle())) }
        })();
        drop(child.stdin.take());
        child.wait().unwrap();
        assert!(result.unwrap().contains(&marker));
    }
}
