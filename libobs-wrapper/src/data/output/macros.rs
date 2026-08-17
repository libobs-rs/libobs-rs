#[doc(hidden)]
#[macro_export]
/// Implements every method of the ObsOutputTrait and forwards it to an internal variable.
macro_rules! forward_obs_output_impl {
    ($struct_name: ident, $var_name: ident) => {
        impl $crate::data::output::ObsOutputTrait for $struct_name {
            fn signals(&self) -> &std::sync::Arc<$crate::data::output::ObsOutputSignals> {
                self.$var_name.signals()
            }
        }

        impl $struct_name {
            pub fn inner_output(&self) -> &$crate::data::output::ObsOutputRef {
                &self.$var_name
            }

            pub fn inner_output_mut(&mut self) -> &mut $crate::data::output::ObsOutputRef {
                &mut self.$var_name
            }

            pub fn into_inner_output(self) -> $crate::data::output::ObsOutputRef {
                self.$var_name
            }
        }
    };
}
