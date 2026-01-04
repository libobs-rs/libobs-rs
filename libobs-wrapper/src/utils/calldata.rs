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
    // Not using a SmartPointerSendable here because its just way too complicated if we want to get the inner mut pointer
    //TODO ?
    data: Sendable<Pin<Box<calldata_t>>>, // stable address
    runtime: ObsRuntime,
    _drop_guard: Arc<_CalldataWrapperDropGuard>,
}

impl CalldataWrapper {
    /// Returns a mutable pointer to the inner `calldata_t`.
    /// # Safety
    ///
    /// This function is unsafe. You must guarantee that you will never move
    /// the data out of the mutable reference you receive when you call this
    /// function, so that the invariants on the `Pin` type can be upheld.
    pub unsafe fn as_mut_ptr(&mut self) -> Sendable<*mut calldata_t> {
        // Safe: pinned box guarantees stable location; we only get a mutable reference.
        let r: &mut calldata_t = Pin::as_mut(&mut self.data.0).get_unchecked_mut();
        Sendable(r as *mut _)
    }

    /// Extracts a C string pointer for the given key from the calldata.
    pub fn get_string<T: Into<ObsString>>(&mut self, key: T) -> Result<String, ObsError> {
        let key: ObsString = key.into();
        let self_ptr = unsafe {
            // Safety: We won't modify the calldata, so its safe to get a mutable pointer here.
            self.as_mut_ptr()
        };

        let _drop_guard = self._drop_guard.clone(); // Ensure runtime is valid during the call
        let value = run_with_obs!(
            self.runtime.clone(),
            (_drop_guard, self_ptr, key),
            move || {
                let key_ptr = key.as_ptr().0;

                let mut data = MaybeUninit::<*const c_char>::uninit();
                let ok = unsafe {
                    // Safety: self_ptr and key_ptr are valid pointers.
                    libobs::calldata_get_string(self_ptr.0, key_ptr, data.as_mut_ptr())
                };
                if !ok {
                    return Err(ObsError::Unexpected(format!(
                        "Calldata String {key} not found."
                    )));
                }

                let data_ptr = unsafe {
                    // Safety: data was initialized by calldata_get_string and we made sure the call was ok.
                    data.assume_init()
                };
                if data_ptr.is_null() {
                    return Err(ObsError::Unexpected(format!(
                        "Calldata String {key} is null."
                    )));
                }

                let data = unsafe {
                    // Safety: data_ptr is a valid C string pointer because it is not null.
                    CStr::from_ptr(data_ptr)
                };
                let data = data.to_str();
                if let Err(_e) = data {
                    return Err(ObsError::Unexpected(format!(
                        "Calldata String {key} is not valid UTF-8."
                    )));
                }

                let data = data.unwrap();
                Ok(data.to_string())
            }
        )??;

        Ok(value)
    }

    //TODO implement calldata get_data type but I think this is hard to safely do this
}

struct _CalldataWrapperDropGuard {
    calldata_ptr: Sendable<*mut calldata_t>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_CalldataWrapperDropGuard, (calldata_ptr), move || unsafe {
    // Safety: We are in the runtime and drop guards are always constructed frm valid calldata pointers.
    calldata_free(calldata_ptr.0);
});

/// Extension trait on `ObsRuntime` to call a proc handler and return a RAII calldata wrapper.
pub trait ObsCalldataExt {
    /// # Safety
    /// Make sure that the proc_handler pointer is valid.
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
        let mut calldata = run_with_obs!(self.clone(), (proc_handler, name), move || {
            // Safety: calldata will be properly freed by the drop guard and we are using a struct for the `zeroed` call.
            let data: calldata_t = unsafe { std::mem::zeroed() };
            let mut data = Box::pin(data);
            // Safety: Data will not get moved out of the pinned box, only the proc handler call will use the pointer and not move it.
            let raw_ptr = unsafe { Pin::as_mut(&mut data).get_unchecked_mut() };

            // Safety: the caller must have made sure that the proc handler is valid, the name pointer and the raw_ptr of the calldata is valid.
            let ok = unsafe { libobs::proc_handler_call(proc_handler.0, name.as_ptr().0, raw_ptr) };
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
    unsafe fn call_proc_handler<T: Into<ObsString>>(
        &self,
        proc_handler: &Sendable<*mut proc_handler_t>,
        name: T,
    ) -> Result<CalldataWrapper, ObsError> {
        self.runtime.call_proc_handler(proc_handler, name)
    }
}
