use std::{collections::HashMap, str::FromStr, sync::Arc};

use duplicate::duplicate_item;

use crate::{
    context::ObsContext,
    data::{
        output::{ObsOutputRef, ObsOutputTrait},
        properties::{
            ObsProperty, ObsPropertyObject, ObsPropertyObjectPrivate, _ObsPropertiesDropGuard,
            property_ptr_to_struct,
        },
        ObsData,
    },
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObjectInfo, ObsError, ObsString},
};

use super::{
    audio::ObsAudioEncoder, video::ObsVideoEncoder, ObsAudioEncoderType, ObsVideoEncoderType,
};

#[duplicate_item(
    StructName EncoderType;
    [ObsAudioEncoderBuilder] [ObsAudioEncoderType];
    [ObsVideoEncoderBuilder] [ObsVideoEncoderType]
)]
#[derive(Debug)]
pub struct StructName {
    encoder_id: EncoderType,
    runtime: ObsRuntime,
    //TODO: keeping this for now, maybe it'll be useful later
    _context: ObsContext,
    settings: Option<ObsData>,
    hotkey_data: Option<ObsData>,
}

#[duplicate_item(
    StructName EncoderType;
    [ObsAudioEncoderBuilder] [ObsAudioEncoderType];
    [ObsVideoEncoderBuilder] [ObsVideoEncoderType]
)]
impl StructName {
    pub fn new(context: ObsContext, encoder_id: &str) -> Self {
        Self {
            encoder_id: EncoderType::from_str(encoder_id).unwrap(),
            runtime: context.runtime().clone(),
            _context: context,
            settings: None,
            hotkey_data: None,
        }
    }

    pub fn get_encoder_id(&self) -> &EncoderType {
        &self.encoder_id
    }

    pub fn set_settings(&mut self, settings: ObsData) -> &mut Self {
        self.settings = Some(settings);
        self
    }

    pub fn set_hotkey_data(&mut self, hotkey_data: ObsData) -> &mut Self {
        self.hotkey_data = Some(hotkey_data);
        self
    }

    pub fn get_settings(&self) -> Option<&ObsData> {
        self.settings.as_ref()
    }

    pub fn get_hotkey_data(&self) -> Option<&ObsData> {
        self.hotkey_data.as_ref()
    }

    pub fn get_settings_mut(&mut self) -> Option<&mut ObsData> {
        self.settings.as_mut()
    }

    pub fn get_hotkey_data_mut(&mut self) -> Option<&mut ObsData> {
        self.hotkey_data.as_mut()
    }
}

impl ObsAudioEncoderBuilder {
    pub fn apply_to_context(
        self,
        output: &mut dyn ObsOutputTrait,
        name: &str,
        settings: Option<ObsData>,
        hotkey_data: Option<ObsData>,
        mixer_idx: usize,
    ) -> Result<Arc<ObsAudioEncoder>, ObsError> {
        let e_id: ObsString = self.encoder_id.into();
        let info = ObjectInfo::new(e_id, ObsString::new(name), settings, hotkey_data);

        output.create_and_set_audio_encoder(info, mixer_idx)
    }
}

impl ObsVideoEncoderBuilder {
    pub fn set_to_output(
        self,
        output: &mut ObsOutputRef,
        name: &str,
    ) -> Result<Arc<ObsVideoEncoder>, ObsError> {
        let e_id: ObsString = self.encoder_id.into();
        let info = ObjectInfo::new(e_id, ObsString::new(name), self.settings, self.hotkey_data);

        output.create_and_set_video_encoder(info)
    }
}

#[duplicate_item(
    StructName;
    [ObsAudioEncoderBuilder];
    [ObsVideoEncoderBuilder]
)]
impl ObsPropertyObject for StructName {
    fn get_properties(&self) -> Result<HashMap<String, ObsProperty>, ObsError> {
        let properties_raw = self.get_properties_raw()?;
        property_ptr_to_struct(properties_raw, self.runtime.clone())
    }
}

#[duplicate_item(
    StructName;
    [ObsAudioEncoderBuilder];
    [ObsVideoEncoderBuilder]
)]
impl ObsPropertyObjectPrivate for StructName {
    fn get_properties_raw(
        &self,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let encoder_name: ObsString = self.encoder_id.clone().into();

        let property_ptr = run_with_obs!(self.runtime, (encoder_name), move || {
            let encoder_name_ptr = encoder_name.as_ptr().0;

            let property_ptr = unsafe {
                // Safety: encoder_name_ptr is valid because it comes from ObsString
                libobs::obs_get_encoder_properties(encoder_name_ptr)
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = Arc::new(_ObsPropertiesDropGuard::new(
            property_ptr.clone(),
            self.runtime.clone(),
        ));

        Ok(SmartPointerSendable::new(property_ptr.0, drop_guard))
    }

    fn get_properties_by_id_raw<T: Into<ObsString> + Sync + Send>(
        id: T,
        runtime: ObsRuntime,
    ) -> Result<SmartPointerSendable<*mut libobs::obs_properties_t>, ObsError> {
        let id: ObsString = id.into();
        let ptr = run_with_obs!(runtime, (id), move || {
            let id_ptr = id.as_ptr();

            let property_ptr = unsafe {
                // Safety: id_ptr is valid because it comes from ObsString
                libobs::obs_get_encoder_properties(id_ptr.0)
            };

            if property_ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(property_ptr))
            }
        })??;

        let drop_guard = Arc::new(_ObsPropertiesDropGuard::new(ptr.clone(), runtime.clone()));

        Ok(SmartPointerSendable::new(ptr.0, drop_guard))
    }
}
