macro_rules! inner_fn_update_settings {
    ($self:expr, $update_fn:path, $settings:expr) => {{
        let settings = $settings.into_immutable();
        let settings_ptr = settings.as_ptr();
        let obs_ptr = $self.as_ptr();
        let runtime = $self.runtime().clone();

        run_with_obs!(runtime, (obs_ptr, settings_ptr), move || unsafe {
            $update_fn(obs_ptr, settings_ptr)
        })?;

        $self.__internal_replace_settings(settings)?;
        Ok(())
    }};
}

/// Implements every functionality of the ObsObjectTrait and ObsObjectTraitSealed
/// by forwarding the calls to the inner object stored in $var_name.
#[doc(hidden)]
#[macro_export]
macro_rules! forward_obs_object_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl $crate::data::object::ObsObjectTraitSealed for $struct_name {
            fn __internal_replace_settings(
                &self,
                settings: $crate::data::ImmutableObsData,
            ) -> Result<(), $crate::utils::ObsError> {
                self.$var_name.__internal_replace_settings(settings)
            }

            fn __internal_replace_hotkey_data(
                &self,
                hotkey_data: $crate::data::ImmutableObsData,
            ) -> Result<(), $crate::utils::ObsError> {
                self.$var_name.__internal_replace_hotkey_data(hotkey_data)
            }
        }

        impl $crate::data::object::ObsObjectTrait for $struct_name {
            fn name(&self) -> $crate::utils::ObsString {
                self.$var_name.name()
            }

            fn id(&self) -> $crate::utils::ObsString {
                self.$var_name.id()
            }

            fn runtime(&self) -> &$crate::runtime::ObsRuntime {
                self.$var_name.runtime()
            }

            fn settings(&self) -> Result<$crate::data::ImmutableObsData, $crate::utils::ObsError> {
                self.$var_name.settings()
            }

            fn hotkey_data(
                &self,
            ) -> Result<$crate::data::ImmutableObsData, $crate::utils::ObsError> {
                self.$var_name.hotkey_data()
            }

            fn update_settings(
                &self,
                settings: $crate::data::ObsData,
            ) -> Result<(), $crate::utils::ObsError> {
                self.$var_name.update_settings(settings)
            }
        }
    };
}

pub(crate) use inner_fn_update_settings;
