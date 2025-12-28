macro_rules! forward_obs_output_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl ObsOutputTrait for $struct_name {
            fn signal_manager(&self) -> &Arc<ObsOutputSignals> {
                self.$var_name.signal_manager()
            }

            fn video_encoder(&self) -> &Arc<RwLock<Option<Arc<ObsVideoEncoder>>>> {
                self.$var_name.video_encoder()
            }

            fn audio_encoders(&self) -> &Arc<RwLock<HashMap<usize, Arc<ObsAudioEncoder>>>> {
                self.$var_name.audio_encoders()
            }

            fn as_ptr(&self) -> Sendable<*mut libobs::obs_output> {
                self.$var_name.as_ptr()
            }
        }
    };
}

pub(crate) use forward_obs_output_impl;
