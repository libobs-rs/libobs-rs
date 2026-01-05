/// Generates builder and updater structs for OBS objects.
///
/// This macro creates two structures for managing OBS objects:
/// - A `{StructName}Builder` struct for constructing new OBS objects
/// - A `{StructName}Updater` struct for updating existing OBS objects
///
/// # Arguments
///
/// - `struct $struct_name`: The base name for the generated builder and updater structs
/// - `$obs_id`: A string literal representing the OBS object type ID
/// - `$underlying_ptr_type`: The underlying pointer type (e.g., `*mut libobs::obs_source`)
/// - `$updatable_name`: The name of the updatable trait/type to implement
/// - Field definitions: Custom fields with their types and optional doc attributes
///
/// # Example
///
/// ```ignore
/// define_object_manager!(
///     struct MySource("underlying_obs_source_id", *mut libobs::obs_source) for MySourceUpdatable {
///         /// ALSA device ID (e.g., "default", "hw:0,0", or custom PCM device)
///        #[obs_property(type_t = "string")]
///        device_id: String,
///
///        /// Custom PCM device name (used when device_id is "__custom__")
///        #[obs_property(type_t = "string")]
///        custom_pcm: String,
///     }
/// );
/// ```
#[allow(unused)]
#[macro_export]
macro_rules! define_object_manager {
    ($(#[$parent_meta:meta])* struct $struct_name:ident($obs_id:literal, $underlying_ptr_type: ty) for $updatable_name:ident {
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

            #[libobs_simple_macro::obs_object_updater($obs_id, $updatable_name, $underlying_ptr_type)]
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

/// Implements custom source functionality with optional signal management.
///
/// This macro provides multiple overloads:
/// 1. Simple version: Takes only the custom source struct name
/// 2. Advanced version: Takes the struct name and signal definitions
///
/// The macro generates:
/// - A custom source struct wrapping `ObsSourceRef` with signal management
/// - Methods for creating updaters and accessing source-specific signals
/// - Automatic forwarding of OBS object and source trait implementations
///
/// # Arguments
///
/// - `$new_source_struct`: The name of the custom source struct to implement
/// - `[$signal_name: { ... }]`: Optional signal definitions with attributes and implementations
///
/// # Example
///
/// ```ignore
/// impl_custom_source!(MyCustomSource);
///
/// // Or with signals:
/// impl_custom_source!(MyCustomSource, [
///     "signal_name": { /* signal definition */ },
/// ]);
///
/// // Or with a custom signal struct name:
/// impl_custom_source!(MyCustomSource, MyCustomSourceSignals);
/// ```
#[allow(unused)]
macro_rules! impl_custom_source {
    ($new_source_struct: ident) => {
        impl_custom_source!($new_source_struct, []);
    };
    ($new_source_struct: ident, [
        $($(#[$attr:meta])* $signal_name: literal: { $($inner_def:tt)* }),* $(,)*
    ]) => {
        paste::paste! {
            libobs_wrapper::impl_signal_manager!(|ptr: libobs_wrapper::unsafe_send::SmartPointerSendable<*mut libobs::obs_source>| unsafe {
                    // Safety: This is a smart pointer, so it is fine
                    libobs::obs_source_get_signal_handler(ptr.get_ptr())
                }, [<$new_source_struct Signals>] for *mut libobs::obs_source, [
            $($(#[$attr])* $signal_name: { $($inner_def)* }),*
            ]);

            impl_custom_source!($new_source_struct, [<$new_source_struct Signals>]);
        }
    };
    ($new_source_struct: ident, $signal_struct_name: ident) => {
        impl_custom_source!($new_source_struct, $signal_struct_name, NO_SPECIFIC_SIGNALS_FUNCTION);

        impl $new_source_struct {
            pub fn source_specific_signals(&self) -> std::sync::Arc<$signal_struct_name> {
                self.source_specific_signals.clone()
            }
        }
    };
    ($new_source_struct: ident, $signal_struct_name: ident, NO_SPECIFIC_SIGNALS_FUNCTION) => {
        paste::paste!{
            #[derive(Debug, Clone)]
            /// This struct is essentially a wrapper around an OBS source with
            /// additional functionality specific to the custom source.
            ///
            /// It provides methods to create an updater and access source-specific signals.
            pub struct $new_source_struct {
                source: ObsSourceRef,
                source_specific_signals: std::sync::Arc<$signal_struct_name>,
            }

            impl $new_source_struct {
                fn new(source: ObsSourceRef) -> Result<Self, libobs_wrapper::utils::ObsError> {
                    use libobs_wrapper::data::object::ObsObjectTrait;
                    let source_specific_signals =
                        $signal_struct_name::new(&source.as_ptr(), source.runtime().clone())?;

                    Ok(Self {
                        source,
                        source_specific_signals: std::sync::Arc::new(source_specific_signals),
                    })
                }

                pub fn create_updater<'a>(&'a mut self) -> Result<[<$new_source_struct Updater>]<'a>, libobs_wrapper::utils::ObsError> {
                    use libobs_wrapper::data::ObsObjectUpdater;
                    use libobs_wrapper::data::object::ObsObjectTrait;
                    [<$new_source_struct Updater>]::create_update(
                        self.runtime().clone(),
                        self.inner_source_mut()
                    )
                }
            }

            libobs_wrapper::forward_obs_object_impl!($new_source_struct, source, *mut libobs::obs_source);
            libobs_wrapper::forward_obs_source_impl!($new_source_struct, source);
        }
    };
}

/// Implements the `ObsSourceBuilder` trait for a builder struct.
///
/// This macro provides a default implementation of `ObsSourceBuilder` that:
/// - Calls the struct's `object_build()` method to construct the underlying OBS object
/// - Wraps the result in an `ObsSourceRef` for the final return type
///
/// # Arguments
///
/// - `$name`: The name of the builder struct implementing this trait
///
/// # Example
///
/// ```ignore
/// impl_default_builder!(MySourceBuilder);
/// ```
///
/// This assumes the struct has a `runtime` field and an `object_build()` method.
#[allow(unused)]
macro_rules! impl_default_builder {
    ($name: ident) => {
        impl libobs_wrapper::sources::ObsSourceBuilder for $name {
            type T = libobs_wrapper::sources::ObsSourceRef;

            fn build(self) -> Result<Self::T, libobs_wrapper::utils::ObsError>
            where
                Self: Sized,
            {
                use libobs_wrapper::data::ObsObjectBuilder;
                let runtime = self.runtime.clone();
                libobs_wrapper::sources::ObsSourceRef::new_from_info(self.object_build()?, runtime)
            }
        }
    };
}

#[allow(unused)]
pub(crate) use {define_object_manager, impl_custom_source, impl_default_builder};
