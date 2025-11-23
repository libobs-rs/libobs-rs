use std::ffi::CStr;

use libobs::obs_data;

use crate::{
    data::ObsData,
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString},
};

impl ObsDataGetters for ObsData {
    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn as_ptr(&self) -> Sendable<*mut obs_data> {
        self.obs_data.clone()
    }
}

pub trait ObsDataGetters {
    fn runtime(&self) -> &ObsRuntime;
    fn as_ptr(&self) -> Sendable<*mut obs_data>;
    fn get_string<T: Into<ObsString> + Send + Sync>(
        &self,
        key: T,
    ) -> Result<Option<String>, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        let result = run_with_obs!(self.runtime(), (data_ptr, key_ptr), move || unsafe {
            if libobs::obs_data_has_user_value(data_ptr, key_ptr)
                || libobs::obs_data_has_default_value(data_ptr, key_ptr)
            {
                Some(Sendable(libobs::obs_data_get_string(data_ptr, key_ptr)))
            } else {
                None
            }
        })?;

        if result.is_none() {
            return Ok(None);
        }

        let result = result.unwrap();
        if result.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let result = unsafe { CStr::from_ptr(result.0) };
        let result = result
            .to_str()
            .map_err(|_| ObsError::StringConversionError)?;

        Ok(Some(result.to_string()))
    }
    fn get_int<T: Into<ObsString> + Sync + Send>(&self, key: T) -> Result<Option<i64>, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        let result = run_with_obs!(self.runtime(), (data_ptr, key_ptr), move || unsafe {
            if libobs::obs_data_has_user_value(data_ptr, key_ptr)
                || libobs::obs_data_has_default_value(data_ptr, key_ptr)
            {
                Some(libobs::obs_data_get_int(data_ptr, key_ptr))
            } else {
                None
            }
        })?;

        Ok(result)
    }
    fn get_bool<T: Into<ObsString> + Sync + Send>(&self, key: T) -> Result<Option<bool>, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        let result = run_with_obs!(self.runtime(), (data_ptr, key_ptr), move || unsafe {
            if libobs::obs_data_has_user_value(data_ptr, key_ptr)
                || libobs::obs_data_has_default_value(data_ptr, key_ptr)
            {
                Some(libobs::obs_data_get_bool(data_ptr, key_ptr))
            } else {
                None
            }
        })?;

        Ok(result)
    }
    fn get_double<T: Into<ObsString> + Sync + Send>(
        &self,
        key: T,
    ) -> Result<Option<f64>, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        let result = run_with_obs!(self.runtime(), (key_ptr, data_ptr), move || unsafe {
            if libobs::obs_data_has_user_value(data_ptr, key_ptr)
                || libobs::obs_data_has_default_value(data_ptr, key_ptr)
            {
                Some(libobs::obs_data_get_double(data_ptr, key_ptr))
            } else {
                None
            }
        })?;

        Ok(result)
    }

    fn get_json(&self) -> Result<String, ObsError> {
        let data_ptr = self.as_ptr();
        let ptr = run_with_obs!(self.runtime(), (data_ptr), move || unsafe {
            Sendable(libobs::obs_data_get_json(data_ptr))
        })?;

        if ptr.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        let ptr = unsafe { CStr::from_ptr(ptr.0) };
        let ptr = ptr.to_str().map_err(|_| ObsError::JsonParseError)?;

        Ok(ptr.to_string())
    }
}
