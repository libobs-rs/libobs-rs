use std::{
    ffi::{c_char, CStr},
    mem::MaybeUninit,
    pin::Pin,
};

use libobs::{calldata_t, proc_handler_t};

use crate::{
    context::ObsContext,
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{calldata_free, ObsError, ObsString},
};

/// RAII wrapper for libobs `calldata_t` ensuring stable address and actor-thread cleanup.
///
/// The pinned allocation itself is retained until `calldata_free` has run on the OBS actor. This
/// is important: retaining only a raw pointer in a deferred drop guard would allow the Rust `Box`
/// containing the `calldata_t` to be freed before the asynchronous native cleanup dereferences it.
pub(crate) struct CalldataWrapper {
    data: Option<Sendable<Pin<Box<calldata_t>>>>,
    runtime: ObsRuntime,
}

impl CalldataWrapper {
    /// Returns a mutable pointer to the inner `calldata_t`.
    ///
    /// # Safety
    /// The caller must not move the pointee out of the pinned allocation.
    pub(crate) unsafe fn as_mut_ptr(&mut self) -> Sendable<*mut calldata_t> {
        let data = self
            .data
            .as_mut()
            .expect("calldata storage is present until Drop");
        // Safety: the `Box` is pinned for the complete lifetime of this wrapper and cleanup.
        let r: &mut calldata_t = unsafe { Pin::as_mut(&mut data.0).get_unchecked_mut() };
        Sendable(r as *mut _)
    }

    /// Extracts a C string pointer for the given key from the calldata and copies it into Rust.
    pub fn get_string<T: Into<ObsString>>(&mut self, key: T) -> Result<String, ObsError> {
        let key: ObsString = key.into();
        let self_ptr = unsafe {
            // Safety: the pointer is borrowed only for this actor call and the pinned allocation
            // stays inside `self` for at least that long.
            self.as_mut_ptr()
        };

        let value = run_with_obs!(self.runtime.clone(), (self_ptr, key), move || {
            let key_ptr = key.as_ptr().0;

            let mut data = MaybeUninit::<*const c_char>::uninit();
            let ok = unsafe {
                // Safety: self_ptr and key_ptr remain valid for the actor call.
                libobs::calldata_get_string(self_ptr.0, key_ptr, data.as_mut_ptr())
            };
            if !ok {
                return Err(ObsError::Unexpected(format!(
                    "Calldata String {key} not found."
                )));
            }

            let data_ptr = unsafe {
                // Safety: a successful `calldata_get_string` initialized the out pointer.
                data.assume_init()
            };
            if data_ptr.is_null() {
                return Err(ObsError::Unexpected(format!(
                    "Calldata String {key} is null."
                )));
            }

            let data = unsafe {
                // Safety: libobs returns a NUL-terminated string borrowed from the calldata.
                CStr::from_ptr(data_ptr)
            };
            let data = data.to_str().map_err(|_| {
                ObsError::Unexpected(format!("Calldata String {key} is not valid UTF-8."))
            })?;
            Ok(data.to_string())
        })??;

        Ok(value)
    }
}

impl Drop for CalldataWrapper {
    fn drop(&mut self) {
        let Some(data) = self.data.take() else {
            return;
        };
        let runtime = self.runtime.clone();
        runtime.defer_obs_cleanup(move || {
            // Pass the transport wrapper as a whole. Calling a helper also prevents RFC 2229
            // precise closure capture from capturing only `data.0` (whose raw pointer fields do
            // not themselves implement Send).
            free_owned_calldata(data);
        });
    }
}

/// Frees one owned pinned calldata allocation. Must run on the OBS actor.
fn free_owned_calldata(mut data: Sendable<Pin<Box<calldata_t>>>) {
    // SAFETY: the pinned allocation is owned by this function and remains valid through cleanup.
    let ptr = unsafe { Pin::as_mut(&mut data.0).get_unchecked_mut() as *mut calldata_t };
    // SAFETY: `ptr` identifies the owned live calldata allocation and cleanup runs on the actor.
    unsafe { calldata_free(ptr) };
    // `data` drops here, after the internal libobs stack was released.
}

/// Extension trait on `ObsRuntime` to call a proc handler and return a RAII calldata wrapper.
pub(crate) trait ObsCalldataExt {
    /// # Safety
    /// `proc_handler` must be valid for the complete call.
    unsafe fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError>;
}

impl ObsCalldataExt for ObsRuntime {
    unsafe fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError> {
        if proc_handler.0.is_null() {
            return Err(ObsError::NullPointer(None));
        }

        let proc_handler = proc_handler.clone();
        let name: ObsString = name.into();
        let calldata = run_with_obs!(self.clone(), (proc_handler, name), move || {
            // `calldata_init` is an inline C helper equivalent to zero-initializing this POD. Keep
            // the value pinned because proc callbacks receive its address and may allocate its
            // internal stack while processing out parameters.
            // SAFETY: `calldata_t` is the C POD initialized by libobs's inline `calldata_init`,
            // whose implementation is exactly a zero fill of the complete structure.
            let data: calldata_t = unsafe { std::mem::zeroed() };
            let mut data = Sendable(Box::pin(data));
            let raw_ptr = unsafe {
                // Safety: the allocation is pinned before its address is exposed to libobs.
                Pin::as_mut(&mut data.0).get_unchecked_mut() as *mut calldata_t
            };

            let ok = unsafe {
                // Safety: the caller validated the handler lifetime; name/data live through call.
                libobs::proc_handler_call(proc_handler.0, name.as_ptr().0, raw_ptr)
            };
            if !ok {
                unsafe {
                    // Safety: even a failed proc lookup/call may have touched calldata. It is still
                    // owned and currently on the OBS actor, so release any allocated stack now.
                    calldata_free(raw_ptr);
                }
                return Err(ObsError::Unexpected(format!(
                    "Couldn't call OBS procedure '{name}'"
                )));
            }

            Ok(data)
        })??;

        Ok(CalldataWrapper {
            data: Some(calldata),
            runtime: self.clone(),
        })
    }
}

impl ObsCalldataExt for ObsContext {
    unsafe fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError> {
        self.runtime().call_proc_handler(proc_handler, name)
    }
}
