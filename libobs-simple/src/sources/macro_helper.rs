#[allow(unused)]
#[macro_export]
macro_rules! define_object_manager {
    ($(#[$parent_meta:meta])* struct $struct_name:ident($obs_id:literal) updates $updatable_name:ident {
        $(
            $(#[$meta:meta])*
            $field:ident: $ty:ty,
        )*
    }) => {
        paste::paste! {
            #[libobs_simple_macro::obs_object_builder($obs_id)]
            $(#[$parent_meta])*
            pub struct [<$struct_name Builder>] {
                $(
                    $(#[$meta])*
                    $field: $ty,
                )*
            }

            #[libobs_simple_macro::obs_object_updater($obs_id, $updatable_name)]
            /// Used to update the source this updater was created from. For more details look
            /// at docs for the corresponding builder.
            pub struct [<$struct_name Updater>] {
                $(
                    $(#[$meta])*
                    $field: $ty,
                )*
            }
        }
    };
}

#[allow(unused)]
macro_rules! add_source_specific_signals {
    ($new_source_struct: ident, [
        $($(#[$attr:meta])* $signal_name: literal: { $($inner_def:tt)* }),* $(,)*
    ]) => {
        paste::paste! {
                libobs_wrapper::impl_signal_manager!(|ptr| unsafe { libobs::obs_source_get_signal_handler(ptr) }, [<$new_source_struct Signals>] for $new_source_struct<*mut libobs::obs_source>, [
            $($(#[$attr])* $signal_name: { $($inner_def)* }),*
            ]);

    #[derive(Debug, Clone)]
    pub struct $new_source_struct {
        source: ObsSourceRef,
        source_specific_signals: std::sync::Arc<[<$new_source_struct Signals>]>,
    }

    impl $new_source_struct {
        fn new(source: ObsSourceRef) -> Result<Self, libobs_wrapper::utils::ObsError> {
            use libobs_wrapper::data::object::ObsObjectTrait;
            use libobs_wrapper::sources::ObsSourceTrait;
            let source_specific_signals =
                [<$new_source_struct Signals>]::new(&source.as_ptr(), source.runtime().clone())?;

            Ok(Self {
                source,
                source_specific_signals: std::sync::Arc::new(source_specific_signals),
            })
        }

        pub fn source_specific_signals(&self) -> std::sync::Arc<[<$new_source_struct Signals>]> {
            self.source_specific_signals.clone()
        }
    }

    libobs_wrapper::forward_obs_object_impl!($new_source_struct, source);
    libobs_wrapper::forward_obs_source_impl!($new_source_struct, source);

        }
    };
}

#[allow(unused)]
macro_rules! impl_default_builder {
    ($name: ident) => {
        impl libobs_wrapper::sources::ObsSourceBuilder for $name {
            type T = libobs_wrapper::sources::ObsSourceRef;

            fn add_to_scene(
                self,
                scene: &mut libobs_wrapper::scenes::ObsSceneRef,
            ) -> Result<Self::T, libobs_wrapper::utils::ObsError>
            where
                Self: Sized,
            {
                use libobs_wrapper::data::ObsObjectBuilder;
                scene.add_source(self.build()?)
            }
        }
    };
}

#[allow(unused)]
pub(crate) use {add_source_specific_signals, define_object_manager, impl_default_builder};
