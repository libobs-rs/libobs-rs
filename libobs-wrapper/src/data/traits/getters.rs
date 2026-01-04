use std::ffi::CStr;

use crate::{
    data::ObsDataPointers,
    run_with_obs,
    unsafe_send::SmartPointerSendable,
    utils::{ObsError, ObsString},
};

/// # Safety
/// This function must be called on the OBS runtime.
#[allow(unknown_lints)]
#[allow(ensure_obs_call_in_runtime)]
unsafe fn has_value(
    data_ptr: SmartPointerSendable<*mut libobs::obs_data_t>,
    key: &ObsString,
) -> bool {
    libobs::obs_data_has_user_value(data_ptr.get_ptr(), key.as_ptr().0)
        || libobs::obs_data_has_default_value(data_ptr.get_ptr(), key.as_ptr().0)
}

pub trait ObsDataGetters: ObsDataPointers {
    fn get_string<T: Into<ObsString> + Send + Sync>(
        &self,
        key: T,
    ) -> Result<Option<String>, ObsError> {
        let key = key.into();
        let data_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (data_ptr, key), move || {
            let has_value = unsafe {
                // Safety: We are running on the OBS runtime.
                has_value(data_ptr.clone(), &key)
            };

            if has_value {
                let result = unsafe {
                    // Safety: The pointer is valid because we are using a smart pointer
                    libobs::obs_data_get_string(data_ptr.get_ptr(), key.as_ptr().0)
                };

                if result.is_null() {
                    Err(ObsError::NullPointer(None))
                } else {
                    let result = unsafe {
                        // Safety: The pointer is valid because OBS returned it and we are still in runtime.
                        CStr::from_ptr(result)
                    };
                    let result = result
                        .to_str()
                        .map_err(|_| ObsError::StringConversionError)?
                        .to_string();

                    Ok(Some(result))
                }
            } else {
                Ok(None)
            }
        })?
    }
    fn get_int<T: Into<ObsString> + Sync + Send>(&self, key: T) -> Result<Option<i64>, ObsError> {
        let key = key.into();
        let data_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (data_ptr, key), move || {
            let has_value = unsafe {
                // Safety: We are running on the OBS runtime.
                has_value(data_ptr.clone(), &key)
            };

            if has_value {
                Some(unsafe {
                    // Safety: The pointer is valid because we are using a smart pointer
                    libobs::obs_data_get_int(data_ptr.get_ptr(), key.as_ptr().0)
                })
            } else {
                None
            }
        })
    }
    fn get_bool<T: Into<ObsString> + Sync + Send>(&self, key: T) -> Result<Option<bool>, ObsError> {
        let key = key.into();

        let data_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (data_ptr, key), move || {
            let has_value = unsafe {
                // Safety: We are running on the OBS runtime.
                has_value(data_ptr.clone(), &key)
            };

            if has_value {
                Some(unsafe {
                    // Safety: The pointer is valid because we are using a smart pointer
                    libobs::obs_data_get_bool(data_ptr.get_ptr(), key.as_ptr().0)
                })
            } else {
                None
            }
        })
    }
    fn get_double<T: Into<ObsString> + Sync + Send>(
        &self,
        key: T,
    ) -> Result<Option<f64>, ObsError> {
        let key = key.into();
        let data_ptr = self.as_ptr();

        let result = run_with_obs!(self.runtime(), (key, data_ptr), move || {
            let has_value = unsafe {
                // Safety: We are running on the OBS runtime.
                has_value(data_ptr.clone(), &key)
            };

            if has_value {
                Some(unsafe {
                    // Safety: The pointer is valid because we are using a smart pointer
                    libobs::obs_data_get_double(data_ptr.get_ptr(), key.as_ptr().0)
                })
            } else {
                None
            }
        })?;

        Ok(result)
    }

    fn get_json(&self) -> Result<String, ObsError> {
        let data_ptr = self.as_ptr();
        run_with_obs!(self.runtime(), (data_ptr), move || {
            let json_ptr = unsafe {
                // Safety: The pointer is valid because we are using a smart pointer
                libobs::obs_data_get_json(data_ptr.get_ptr())
            };

            if json_ptr.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "Couldn't get json representation of OBS data".into(),
                )));
            }

            let json = unsafe {
                // Safety: The pointer is valid because OBS returned it and we are still in runtime.
                CStr::from_ptr(json_ptr)
            }
            .to_str()
            .map_err(|_| ObsError::JsonParseError)?
            .to_string();

            Ok(json)
        })?
    }
}
