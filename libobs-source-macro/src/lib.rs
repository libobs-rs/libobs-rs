use obs_properties::obs_properties_to_functions;
use parse::UpdaterInput;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Fields, ItemImpl, LitStr, Type, TypePath};

mod docs;
mod fields;
mod obs_properties;
mod parse;

#[proc_macro_attribute]
//TODO more documents here
/// This macro is used to generate an updater pattern for an obs object (for example a source).
/// For more examples look at libobs-sources
pub fn obs_object_updater(attr: TokenStream, item: TokenStream) -> TokenStream {
    let u_input = parse_macro_input!(attr as UpdaterInput);
    let id_value = u_input.name.value();
    let updatable_type = u_input.updatable_type;

    let input = parse_macro_input!(item as DeriveInput);

    let i_ident = input.ident;
    let updater_name = format_ident!("{}", i_ident);

    let visibility = input.vis;
    let attributes = input.attrs;

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    let (struct_fields, struct_initializers) = fields::generate_struct_fields(&fields);
    let functions = obs_properties_to_functions(
        &fields,
        quote! {
            use libobs_wrapper::data::ObsObjectUpdater;
            self.get_settings_updater()
        },
    );

    let updatable_type2 = updatable_type.clone();
    let expanded = quote! {
        #(#attributes)*
        #[allow(dead_code)]
        #visibility struct #updater_name<'a> {
            #(#struct_fields,)*
            settings: libobs_wrapper::data::ObsData,
            settings_updater: libobs_wrapper::data::ObsDataUpdater,
            updatable: &'a mut #updatable_type2
        }

        impl <'a> libobs_wrapper::data::ObsObjectUpdater<'a> for #updater_name<'a> {
            type ToUpdate = #updatable_type;

            fn create_update(runtime: libobs_wrapper::runtime::ObsRuntime, updatable: &'a mut Self::ToUpdate) -> Result<Self, libobs_wrapper::utils::ObsError> {
                let source_id = Self::get_id();
                let flags = unsafe {
                    libobs::obs_get_source_output_flags(source_id.as_ptr().0)
                };

                if flags == 0 {
                    return Err(libobs_wrapper::utils::ObsError::SourceNotAvailable(source_id.to_string()))
                }

                let mut settings = libobs_wrapper::data::ObsData::new(runtime.clone())?;

                Ok(Self {
                    #(#struct_initializers,)*
                    settings_updater: settings.bulk_update(),
                    settings,
                    updatable,
                })
            }

            fn get_settings(&self) -> &libobs_wrapper::data::ObsData {
                &self.settings
            }

            fn get_settings_updater(&mut self) -> &mut libobs_wrapper::data::ObsDataUpdater {
                &mut self.settings_updater
            }

            fn get_id() -> libobs_wrapper::utils::ObsString {
                #id_value.into()
            }

            fn update(self) -> Result<(), libobs_wrapper::utils::ObsError> {
                use libobs_wrapper::utils::traits::ObsUpdatable;
                let #updater_name {
                    settings_updater,
                    updatable,
                    settings,
                    ..
                } = self;

                log::trace!("Updating settings for {:?}", Self::get_id());
                settings_updater.update()?;

                log::trace!("Updating raw settings for {:?}", Self::get_id());
                let e = updatable.update_raw(settings);
                log::trace!("Update done for {:?}", Self::get_id());

                e
            }
        }

        impl <'a> #updater_name <'a> {
            #(#functions)*
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
/// This macro is used to generate a builder pattern for an obs source. <br>
/// The attribute should be the id of the source.<br>
/// The struct should have named fields, each field should have an attribute `#[obs_property(type_t="your_type")]`. <br>
/// `type_t` can be `enum`, `enum_string`, `string`, `bool` or `int`. <br>
/// - `enum`: the field should be an enum with `num_derive::{FromPrimitive, ToPrimitive}`.
/// - `enum_string`: the field should be an enum which implements `StringEnum`.
/// - `string`: the field should be a string.
/// - `bool`: the field should be a bool.
/// - `type_t`: `int`, the field should be an i64.
///   The attribute can also have a `settings_key` which is the key used in the settings, if this attribute is not given, the macro defaults to the field name.
///
/// Documentation is inherited from the field to the setter function.
///
/// Example:
///
/// ```
/// use libobs_wrapper::data::StringEnum;
/// use libobs_source_macro::obs_object_builder;
/// use num_derive::{FromPrimitive, ToPrimitive};
///
/// #[repr(i32)]
/// #[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// pub enum ObsWindowCaptureMethod {
///        MethodAuto = libobs::window_capture_method_METHOD_AUTO,
///        MethodBitBlt = libobs::window_capture_method_METHOD_BITBLT,
///        MethodWgc = libobs::window_capture_method_METHOD_WGC,
/// }
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// pub enum ObsGameCaptureRgbaSpace {
///     SRgb,
///     RGBA2100pq
/// }
///
/// impl StringEnum for ObsGameCaptureRgbaSpace {
///     fn to_str(&self) -> &str {
///         match self {
///             ObsGameCaptureRgbaSpace::SRgb => "sRGB",
///             ObsGameCaptureRgbaSpace::RGBA2100pq => "Rec. 2100 (PQ)"
///         }
///     }
/// }
///
/// /// Provides an easy-to-use builder for the window capture source.
/// #[derive(Debug)]
/// #[obs_object_builder("window_capture")]
/// pub struct WindowCaptureSourceBuilder {
/// #[obs_property(type_t="enum")]
///     /// Sets the capture method for the window capture
///     capture_method: ObsWindowCaptureMethod,
///
///     /// Sets the window to capture.
///     #[obs_property(type_t = "string", settings_key = "window")]
///     window_raw: String,
///
///     #[obs_property(type_t = "bool")]
///     /// Sets whether the cursor should be captured
///     cursor: bool,
///
///     /// Sets the capture mode for the game capture source. Look at doc for `ObsGameCaptureMode`
///     #[obs_property(type_t = "enum_string")]
///     capture_mode: ObsGameCaptureMode,
/// }
/// ```
pub fn obs_object_builder(attr: TokenStream, item: TokenStream) -> TokenStream {
    let id = parse_macro_input!(attr as LitStr);

    let input = parse_macro_input!(item as DeriveInput);

    let i_ident = input.ident;
    let builder_name = format_ident!("{}", i_ident);

    let generics = input.generics;
    let visibility = input.vis;
    let attributes = input.attrs;

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    let id_value = id.value();
    let (struct_fields, struct_initializers) = fields::generate_struct_fields(&fields);

    let functions = obs_properties_to_functions(
        &fields,
        quote! {
            use libobs_wrapper::data::ObsObjectBuilder;
            self.get_settings_updater()
        },
    );

    let expanded = quote! {
        #(#attributes)*
        #[allow(dead_code)]
        #visibility struct #builder_name #generics {
            #(#struct_fields,)*
            settings: libobs_wrapper::data::ObsData,
            settings_updater: libobs_wrapper::data::ObsDataUpdater,
            hotkeys: libobs_wrapper::data::ObsData,
            hotkeys_updater: libobs_wrapper::data::ObsDataUpdater,
            name: libobs_wrapper::utils::ObsString,
            runtime: libobs_wrapper::runtime::ObsRuntime
        }

        impl libobs_wrapper::data::ObsObjectBuilder for #builder_name {
            fn new<T: Into<libobs_wrapper::utils::ObsString> + Send + Sync>(name: T, runtime: libobs_wrapper::runtime::ObsRuntime) -> Result<Self, libobs_wrapper::utils::ObsError> {
                let name = name.into();
                let source_id = Self::get_id();
                let flags = unsafe {
                    libobs::obs_get_source_output_flags(source_id.as_ptr().0)
                };

                if flags == 0 {
                    return Err(libobs_wrapper::utils::ObsError::SourceNotAvailable(source_id.to_string()))
                }

                let mut hotkeys = libobs_wrapper::data::ObsData::new(runtime.clone())?;
                let mut settings = libobs_wrapper::data::ObsData::new(runtime.clone())?;

                Ok(Self {
                    #(#struct_initializers,)*
                    name,
                    settings_updater: settings.bulk_update(),
                    settings,
                    hotkeys_updater: hotkeys.bulk_update(),
                    hotkeys,
                    runtime
                })
            }

            fn get_settings(&self) -> &libobs_wrapper::data::ObsData {
                &self.settings
            }

            fn get_settings_updater(&mut self) -> &mut libobs_wrapper::data::ObsDataUpdater {
                &mut self.settings_updater
            }

            fn get_hotkeys(&self) -> &libobs_wrapper::data::ObsData {
                &self.hotkeys
            }

            fn get_hotkeys_updater(&mut self) -> &mut libobs_wrapper::data::ObsDataUpdater {
                &mut self.hotkeys_updater
            }

            fn get_name(&self) -> libobs_wrapper::utils::ObsString {
                self.name.clone()
            }

            fn get_id() -> libobs_wrapper::utils::ObsString {
                #id_value.into()
            }

            fn build(self) -> Result<libobs_wrapper::utils::ObjectInfo, libobs_wrapper::utils::ObsError> {
                let name = self.get_name();
                let #builder_name {
                    settings_updater,
                    hotkeys_updater,
                    settings,
                    hotkeys,
                    ..
                } = self;

                settings_updater.update()?;
                hotkeys_updater.update()?;

                Ok(libobs_wrapper::utils::ObjectInfo::new(
                    Self::get_id(),
                    name,
                    Some(settings),
                    Some(hotkeys),
                ))
            }
        }

        impl #builder_name {
            #(#functions)*
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn obs_object_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    // Extract the function from the implementation
    let impl_item = input.items;
    let impl_item2 = impl_item.clone();

    // Create the builder and updater struct names
    let base_name = if let Type::Path(TypePath { path, .. }) = &*input.self_ty {
        path.segments.last().unwrap().ident.to_string()
    } else {
        panic!("Only path types are supported in self_ty")
    };

    let builder_name = format_ident!("{}Builder", base_name);
    let updater_name = format_ident!("{}Updater", base_name);

    let expanded = quote! {
        // Builder implementation
        impl #builder_name {
            #(#impl_item)*
        }

        // Updater implementation with lifetime
        impl<'a> #updater_name<'a> {
            #(#impl_item2)*
        }
    };

    TokenStream::from(expanded)
}
