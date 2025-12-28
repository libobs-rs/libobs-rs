use crate::{
    data::ObsDataPointers,
    run_with_obs,
    utils::{ObsError, ObsString},
};

pub trait ObsDataSetters: ObsDataPointers {
    /// Sets a string in `obs_data` and stores it so
    /// it in `ObsData` does not get freed.
    fn set_string<T: Into<ObsString> + Send + Sync, K: Into<ObsString> + Send + Sync>(
        &mut self,
        key: T,
        value: K,
    ) -> Result<&mut Self, ObsError> {
        let key = key.into();
        let value = value.into();

        let key_ptr = key.as_ptr();
        let value_ptr = value.as_ptr();
        let data_ptr = self.as_ptr();

        run_with_obs!(
            self.runtime(),
            (data_ptr, key_ptr, value_ptr),
            move || unsafe { libobs::obs_data_set_string(data_ptr, key_ptr, value_ptr) }
        )?;

        Ok(self)
    }

    /// Sets an int in `obs_data` and stores the key
    /// in `ObsData` so it does not get freed.
    fn set_int<T: Into<ObsString> + Sync + Send>(
        &mut self,
        key: T,
        value: i64,
    ) -> Result<&mut Self, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (key_ptr, data_ptr), move || unsafe {
            libobs::obs_data_set_int(data_ptr, key_ptr, value);
        })?;

        Ok(self)
    }

    /// Sets a bool in `obs_data` and stores the key
    /// in `ObsData` so it does not get freed.
    fn set_bool<T: Into<ObsString> + Sync + Send>(
        &mut self,
        key: T,
        value: bool,
    ) -> Result<&mut Self, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();
        run_with_obs!(self.runtime(), (key_ptr, data_ptr), move || unsafe {
            libobs::obs_data_set_bool(data_ptr, key_ptr, value);
        })?;

        Ok(self)
    }

    /// Sets a double in `obs_data` and stores the key
    /// in `ObsData` so it does not get freed.
    fn set_double<T: Into<ObsString> + Sync + Send>(
        &mut self,
        key: T,
        value: f64,
    ) -> Result<&mut Self, ObsError> {
        let key = key.into();

        let key_ptr = key.as_ptr();
        let data_ptr = self.as_ptr();

        run_with_obs!(self.runtime(), (key_ptr, data_ptr), move || unsafe {
            libobs::obs_data_set_double(data_ptr, key_ptr, value);
        })?;

        Ok(self)
    }
}
