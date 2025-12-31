//! String handling utilities for OBS API integration
//!
//! This module provides safe string handling between Rust and the OBS C API.
//! The core type `ObsString` wraps C-compatible strings in a memory-safe way,
//! ensuring proper lifetime management and UTF-8 validation.

use std::ffi::CString;
use std::fmt;
use std::os::raw::c_char;

use crate::unsafe_send::Sendable;

/// String wrapper for OBS function calls.
///
/// `ObsString` provides safe interaction with OBS C API functions that require
/// C-style strings. It wraps `CString` internally with convenient helper functions
/// for converting between Rust strings and C-compatible strings.
///
/// # Safety
///
/// - Any NUL byte in input strings is stripped during conversion to prevent panicking
/// - Memory is properly managed to prevent use-after-free and memory leaks
/// - Automatically handles conversion between Rust's UTF-8 strings and C's NUL-terminated strings
///
/// # Examples
///
/// ```
/// use libobs_wrapper::utils::ObsString;
///
/// // Create an ObsString from a Rust string
/// let obs_string = ObsString::new("Hello, OBS!");
///
/// // Use in OBS API calls
/// unsafe {
///     let ptr = obs_string.as_ptr();
///     // Pass ptr.0 to OBS functions
/// }
/// ```
#[derive(Clone, Default, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObsString {
    /// The underlying C string representation
    c_string: CString,
}

impl ObsString {
    /// Creates a new `ObsString` from a string slice.
    ///
    /// Any NUL bytes in the input are automatically stripped to prevent
    /// panicking when converting to a C string.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let obs_string = ObsString::new("source_name");
    /// ```
    pub fn new<S: AsRef<str>>(s: S) -> Self {
        let s = s.as_ref().replace("\0", "");
        Self {
            c_string: CString::new(s).unwrap(),
        }
    }

    /// Returns a pointer to the underlying C string along with sendable wrapper.
    ///
    /// The returned pointer is suitable for passing to OBS C API functions.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let obs_string = ObsString::new("source_name");
    /// let ptr = obs_string.as_ptr();
    ///
    /// // Use ptr.0 in OBS API calls
    /// ```
    pub fn as_ptr(&self) -> Sendable<*const c_char> {
        Sendable(self.c_string.as_ptr())
    }
}
impl fmt::Display for ObsString {
    /// Converts the `ObsString` back to a Rust `String` for display.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let obs_string = ObsString::new("Hello");
    /// assert_eq!(format!("{}", obs_string), "Hello");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.c_string.to_string_lossy())
    }
}

impl From<&str> for ObsString {
    /// Creates an `ObsString` from a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let obs_string: ObsString = "Hello".into();
    /// ```
    fn from(value: &str) -> Self {
        let value = value.replace("\0", "");
        Self {
            c_string: CString::new(value).unwrap(),
        }
    }
}

impl From<Vec<u8>> for ObsString {
    /// Creates an `ObsString` from a vector of bytes.
    ///
    /// Any NUL bytes in the input are automatically filtered out.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let bytes = b"Hello".to_vec();
    /// let obs_string: ObsString = bytes.into();
    /// ```
    fn from(mut value: Vec<u8>) -> Self {
        value.retain(|&c| c != 0);
        Self {
            c_string: CString::new(value).unwrap(),
        }
    }
}

impl From<String> for ObsString {
    /// Creates an `ObsString` from a `String`.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let s = String::from("Hello");
    /// let obs_string: ObsString = s.into();
    /// ```
    fn from(value: String) -> Self {
        let value = value.replace("\0", "");
        Self {
            c_string: CString::new(value).unwrap(),
        }
    }
}

impl PartialEq<&str> for ObsString {
    /// Compares an `ObsString` with a string slice for equality.
    ///
    /// # Examples
    ///
    /// ```
    /// use libobs_wrapper::utils::ObsString;
    ///
    /// let obs_string = ObsString::new("Hello");
    /// assert_eq!(obs_string, "Hello");
    /// ```
    fn eq(&self, other: &&str) -> bool {
        self.c_string.to_str() == Ok(*other)
    }
}
