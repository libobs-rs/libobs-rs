use std::{
    ffi::{c_char, CStr},
    mem::MaybeUninit,
    pin::Pin,
    sync::Arc,
};

use libobs::{calldata_t, proc_handler_t};

use crate::{
    context::ObsContext,
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{calldata_free, ObsError, ObsString},
};

/// RAII wrapper for libobs `calldata_t` ensuring stable address and proper free.
pub struct CalldataWrapper {
    data: Sendable<Pin<Box<calldata_t>>>, // stable address
    runtime: ObsRuntime,
    _drop_guard: Arc<_CalldataWrapperDropGuard>,
}

impl CalldataWrapper {
    /// Returns a mutable pointer to the inner `calldata_t`.
    pub fn as_mut_ptr(&mut self) -> Sendable<*mut calldata_t> {
        // Safe: pinned box guarantees stable location; we only get a mutable reference.
        let r: &mut calldata_t = unsafe { Pin::as_mut(&mut self.data.0).get_unchecked_mut() };
        Sendable(r as *mut _)
    }

    /// Extracts a C string pointer for the given key from the calldata.
    pub fn get_string<T: Into<ObsString>>(&mut self, key: T) -> Result<String, ObsError> {
        let key: ObsString = key.into();
        let key = Sendable(key.clone());
        let self_ptr = self.as_mut_ptr();

        let value = run_with_obs!(self.runtime.clone(), (self_ptr, key), move || unsafe {
            let mut data = MaybeUninit::<*const c_char>::uninit();

            let ok = libobs::calldata_get_string(self_ptr, key.as_ptr().0, data.as_mut_ptr());
            if !ok {
                return Err(ObsError::Unexpected(format!(
                    "Calldata String {key} not found."
                )));
            }

            let data_ptr = data.assume_init();
            if data_ptr.is_null() {
                return Err(ObsError::Unexpected(format!(
                    "Calldata String {key} is null."
                )));
            }

            let data = CStr::from_ptr(data_ptr);
            let data = data.to_str();
            if let Err(_e) = data {
                return Err(ObsError::Unexpected(format!(
                    "Calldata String {key} is not valid UTF-8."
                )));
            }

            let data = data.unwrap();
            Ok(data.to_string())
        })??;

        Ok(value)
    }

    //TODO implement calldata get_data type but I think this is hard to safely do this
}

struct _CalldataWrapperDropGuard {
    calldata_ptr: Sendable<*mut calldata_t>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_CalldataWrapperDropGuard, (calldata_ptr), move || unsafe {
    calldata_free(calldata_ptr);
});

/// Extension trait on `ObsRuntime` to call a proc handler and return a RAII calldata wrapper.
pub trait ObsCalldataExt {
    fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError>;
}

impl ObsCalldataExt for ObsRuntime {
    fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError> {
        if proc_handler.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let proc_handler = proc_handler.clone();
        let name: ObsString = name.into();
        let name = Sendable(name);
        let mut calldata = run_with_obs!(self.clone(), (proc_handler, name), move || unsafe {
            let data: calldata_t = std::mem::zeroed();
            let mut data = Box::pin(data);
            let raw_ptr = Pin::as_mut(&mut data).get_unchecked_mut();

            let ok = libobs::proc_handler_call(proc_handler, name.as_ptr().0, raw_ptr);
            if !ok {
                return Err(ObsError::Unexpected(
                    "Couldn't call proc handler".to_string(),
                ));
            }

            Ok(Sendable(data))
        })??;

        // Safety: Data will never get moved out of the pinned box, as this pointer will only be used on drop and then freed.
        let calldata_ptr = unsafe { Pin::as_mut(&mut calldata.0).get_unchecked_mut() };

        // Pin the calldata to a stable heap location and create a drop guard.
        let guard = Arc::new(_CalldataWrapperDropGuard {
            calldata_ptr: Sendable(calldata_ptr),
            runtime: self.clone(),
        });

        Ok(CalldataWrapper {
            data: calldata,
            runtime: self.clone(),
            _drop_guard: guard,
        })
    }
}

impl ObsCalldataExt for ObsContext {
    fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError> {
        self.runtime.call_proc_handler(proc_handler, name)
    }
}
