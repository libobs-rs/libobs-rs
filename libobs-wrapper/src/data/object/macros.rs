macro_rules! inner_fn_update_settings {
    ($self:expr, $update_fn:path, $settings:expr) => {{
        let settings = $settings.into_immutable();
        let settings_ptr = settings.as_ptr();
        let obs_ptr = $self.as_ptr();
        let runtime = $self.runtime().clone();

        run_with_obs!(runtime, (obs_ptr, settings_ptr), move || unsafe {
            $update_fn(obs_ptr, settings_ptr)
        })?;

        $self.replace_settings(settings)?;
        Ok(())
    }};
}

/// Implements every functionality of the ObsObjectTrait and ObsObjectTraitSealed
/// by forwarding the calls to the inner object stored in $var_name.
macro_rules! forward_obs_object_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl ObsObjectTraitSealed for $struct_name {
            fn replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
                self.$var_name.replace_settings(settings)
            }

            fn replace_hotkey_data(&self, hotkey_data: ImmutableObsData) -> Result<(), ObsError> {
                self.$var_name.replace_hotkey_data(hotkey_data)
            }
        }

        impl ObsObjectTrait for $struct_name {
            fn name(&self) -> ObsString {
                self.$var_name.name()
            }

            fn id(&self) -> ObsString {
                self.$var_name.id()
            }

            fn runtime(&self) -> &ObsRuntime {
                self.$var_name.runtime()
            }

            fn settings(&self) -> Result<ImmutableObsData, ObsError> {
                self.$var_name.settings()
            }

            fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
                self.$var_name.hotkey_data()
            }

            fn update_settings(&self, settings: crate::data::ObsData) -> Result<(), ObsError> {
                self.$var_name.update_settings(settings)
            }
        }
    };
}

pub(crate) use {forward_obs_object_impl, inner_fn_update_settings};
