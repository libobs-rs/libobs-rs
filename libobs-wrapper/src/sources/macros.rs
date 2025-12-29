#[doc(hidden)]
#[macro_export]
macro_rules! forward_obs_source_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl $crate::sources::ObsSourceTraitSealed for $struct_name {
            fn add_scene_item_ptr(
                &self,
                scene_ptr: $crate::unsafe_send::SendableComp<*mut libobs::obs_scene_t>,
                item_ptr: $crate::unsafe_send::Sendable<*mut libobs::obs_scene_item>,
            ) -> Result<(), $crate::utils::ObsError> {
                self.$var_name.add_scene_item_ptr(scene_ptr, item_ptr)
            }

            fn remove_scene_item_ptr(
                &self,
                scene_ptr: $crate::unsafe_send::SendableComp<*mut libobs::obs_scene_t>,
            ) -> Result<(), libobs_wrapper::utils::ObsError> {
                self.$var_name.remove_scene_item_ptr(scene_ptr)
            }

            fn get_scene_item_ptr(
                &self,
                scene_ptr: &$crate::unsafe_send::SendableComp<*mut libobs::obs_scene_t>,
            ) -> Result<
                Option<$crate::unsafe_send::Sendable<*mut libobs::obs_scene_item>>,
                $crate::utils::ObsError,
            > {
                self.$var_name.get_scene_item_ptr(scene_ptr)
            }
        }

        impl $crate::sources::ObsSourceTrait for $struct_name {
            fn signals(&self) -> &std::sync::Arc<$crate::sources::ObsSourceSignals> {
                self.$var_name.signals()
            }

            fn as_ptr(&self) -> $crate::unsafe_send::Sendable<*mut libobs::obs_source_t> {
                self.$var_name.as_ptr()
            }
        }

        impl $struct_name {
            pub fn inner_source(&self) -> &$crate::sources::ObsSourceRef {
                &self.$var_name
            }

            pub fn inner_source_mut(&mut self) -> &mut $crate::sources::ObsSourceRef {
                &mut self.$var_name
            }

            /// Consumes self and returns the inner ObsSourceRef
            ///
            /// You can still update this source (if created by libobs-simple) and create an updater like so:
            ///
            /// ```no_run
            /// # This is how you would typically use it
            /// let updater = my_custom_source.create_updater()?;
            ///
            /// # but you can still use the inner source directly (although you'd loose the custom source methods)
            /// let source = my_custom_source.into_inner_source();
            /// let updater = source.create_updater::<MyCustomSourceUpdater>()?;
            /// ````
            pub fn into_inner_source(self) -> $crate::sources::ObsSourceRef {
                self.$var_name
            }
        }
    };
}
