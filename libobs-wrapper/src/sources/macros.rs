#[doc(hidden)]
#[macro_export]
macro_rules! forward_obs_source_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl $crate::sources::ObsSourceTrait for $struct_name {
            fn signals(&self) -> &std::sync::Arc<$crate::sources::ObsSourceSignals> {
                self.$var_name.signals()
            }

            fn get_active_filters(
                &self,
            ) -> Result<Vec<$crate::sources::ObsFilterGuardPair>, $crate::utils::ObsError> {
                self.$var_name.get_active_filters()
            }
            fn apply_filter(
                &self,
                filter: &$crate::sources::ObsFilterRef,
            ) -> Result<(), $crate::utils::ObsError> {
                self.$var_name.apply_filter(filter)
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
