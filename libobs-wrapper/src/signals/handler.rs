#[macro_export]
#[doc(hidden)]
macro_rules! __signals_impl_primitive_handler {
    () => {move || {
        Ok(())
    }};

    // Match against all primitive types
    ($field_name: ident, i8) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, i8) };
    ($field_name: ident, i16) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, i16) };
    ($field_name: ident, i32) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, i32) };
    ($field_name: ident, i64) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, i64) };
    ($field_name: ident, i128) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, i128) };
    ($field_name: ident, isize) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, isize) };

    ($field_name: ident, u8) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, u8) };
    ($field_name: ident, u16) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, u16) };
    ($field_name: ident, u32) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, u32) };
    ($field_name: ident, u64) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, u64) };
    ($field_name: ident, u128) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, u128) };
    ($field_name: ident, usize) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, usize) };

    ($field_name: ident, f32) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, f32) };
    ($field_name: ident, f64) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, f64) };

    ($field_name: ident, bool) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, bool) };
    ($field_name: ident, char) => { $crate::__signals_impl_primitive_handler!(__inner, $field_name, char) };

    ($field_name: ident, String) => {
        move |__internal_calldata|  {
            let mut $field_name = std::ptr::null_mut();
            let obs_str = $crate::utils::ObsString::new(stringify!($field_name));
            let success = libobs::calldata_get_string(
                __internal_calldata,
                obs_str.as_ptr().0,
                &mut $field_name as *const _ as _,
            );

            if !success {
                return Err($crate::utils::ObsError::SignalDataError(
                    format!("Failed to get {} from calldata", stringify!($field_name))
                ));
            }

            let $field_name = std::ffi::CStr::from_ptr($field_name).to_str()
                .map_err(|_| $crate::utils::ObsError::StringConversionError)?;

            Result::<_, $crate::utils::ObsError>::Ok($field_name.to_owned())
        }
    };

    // For any other type, return false
    ($field_name: ident, $other:ty) => { $crate::__signals_impl_primitive_handler!(__enum $field_name, $other) };

    (__inner, $field_name: ident, $field_type: ty) => {
        move |__internal_calldata| {
            let mut $field_name = std::mem::zeroed::<$field_type>();
            let obs_str = $crate::utils::ObsString::new(stringify!($field_name));
            let success = libobs::calldata_get_data(
                __internal_calldata,
                obs_str.as_ptr().0,
                &mut $field_name as *const _ as *mut std::ffi::c_void,
                std::mem::size_of::<$field_type>(),
            );

            if !success {
                return Err($crate::utils::ObsError::SignalDataError(
                    format!("Failed to get {} from calldata", stringify!($field_name))
                ));
            }

            Result::<_, $crate::utils::ObsError>::Ok($field_name)
        }
    };
    (__ptr, $field_name: ident, $field_type: ty) => {
        move |__internal_calldata| {
            let mut $field_name = std::mem::zeroed::<$field_type>();
            let obs_str = $crate::utils::ObsString::new(stringify!($field_name));
            let success = libobs::calldata_get_data(
                __internal_calldata,
                obs_str.as_ptr().0,
                &mut $field_name as *const _ as *mut std::ffi::c_void,
                std::mem::size_of::<$field_type>(),
            );

            if !success {
                return Err($crate::utils::ObsError::SignalDataError(
                    format!("Failed to get {} from calldata", stringify!($field_name))
                ));
            }

            Result::<_, $crate::utils::ObsError>::Ok($crate::unsafe_send::Sendable($field_name))
        }
    };
    (__enum $field_name: ident, $enum_type: ty) => {
        move |__internal_calldata| {
            let code = $crate::__signals_impl_primitive_handler!(__inner, $field_name, i64)(__internal_calldata)?;
            let en = <$enum_type>::try_from(code as i32);
            if let Err(e) = en {
                return Err($crate::utils::ObsError::EnumConversionError(
                    format!("Failed to convert code to {}: {}", stringify!($field_name), e)
                ));
            }

            Result::<_, $crate::utils::ObsError>::Ok(en.unwrap())
        }
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! __signals_impl_signal {
    ($ptr: ty, $signal_name: literal, $field_name: ident: $gen_type:ty) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type >] = $gen_type;
            lazy_static::lazy_static! {
                static ref [<$signal_name:snake:upper _SENDERS>]: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<$crate::unsafe_send::SendableComp<$ptr>, tokio::sync::broadcast::Sender<$gen_type>>>> = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
            }

            unsafe fn [< $signal_name:snake _handler_inner>](cd: *mut libobs::calldata_t) -> Result<$gen_type, $crate::utils::ObsError> {
                let e = $crate::__signals_impl_primitive_handler!($field_name, $gen_type)(cd);

                e
            }
        }

    };
    ($ptr: ty, $signal_name: literal, ) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type >] = ();
            lazy_static::lazy_static! {
                static ref [<$signal_name:snake:upper _SENDERS>]: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<$crate::unsafe_send::SendableComp<$ptr>, tokio::sync::broadcast::Sender<()>>>> = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
            }

            unsafe fn [< $signal_name:snake _handler_inner>](_cd: *mut libobs::calldata_t) -> Result<(), $crate::utils::ObsError> {
                Ok(())
            }
        }

    };
    ($ptr: ty, $signal_name: literal, struct $name: ident {
        $($field_name: ident: $field_type: ty),* $(,)*
    }) => {
        $crate::__signals_impl_signal!($ptr, $signal_name, struct $name {
            $($field_name: $field_type),*;
            POINTERS {}
        });
    };
    ($ptr: ty, $signal_name: literal, struct $name: ident {
        POINTERS
        {$($ptr_field_name: ident: $ptr_field_type: ty),* $(,)*}
    }) => {
        $crate::__signals_impl_signal!($ptr, $signal_name, struct $name {
            ;POINTERS { $($ptr_field_name: $ptr_field_type),* }
        });
    };
    ($ptr: ty, $signal_name: literal, struct $name: ident {
        $($field_name: ident: $field_type: ty),* $(,)*;
        POINTERS
        {$($ptr_field_name: ident: $ptr_field_type: ty),* $(,)*}
    }) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type >] = $name;
            lazy_static::lazy_static! {
                static ref [<$signal_name:snake:upper _SENDERS>]: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<$crate::unsafe_send::SendableComp<$ptr>, tokio::sync::broadcast::Sender<$name>>>> = std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
            }

            #[derive(Debug, Clone)]
            pub struct $name {
                $(pub $field_name: $field_type,)*
                $(pub $ptr_field_name: $crate::unsafe_send::Sendable<$ptr_field_type>,)*
            }

            unsafe fn [< $signal_name:snake _handler_inner>](cd: *mut libobs::calldata_t) -> Result<$name, $crate::utils::ObsError> {
                $(
                    let $field_name = $crate::__signals_impl_primitive_handler!($field_name, $field_type)(cd)?;
                )*
                $(
                    let $ptr_field_name = $crate::__signals_impl_primitive_handler!(__ptr, $ptr_field_name, $ptr_field_type)(cd)?;
                )*

                Ok($name {
                    $($field_name,)*
                    $($ptr_field_name,)*
                })
            }
        }
    }
}
