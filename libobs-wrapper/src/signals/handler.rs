#[macro_export]
#[doc(hidden)]
macro_rules! __signals_impl_primitive_handler {
    () => {
        move || Ok(())
    };

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
        move |__internal_calldata| {
            let mut $field_name: *const std::os::raw::c_char = std::ptr::null();
            let obs_str = $crate::utils::ObsString::new(stringify!($field_name));
            let success = libobs::calldata_get_string(
                __internal_calldata,
                obs_str.as_c_str().as_ptr(),
                &mut $field_name as *const _ as _,
            );
            if !success || $field_name.is_null() {
                return Err($crate::utils::ObsError::SignalDataError(format!(
                    "Failed to get {} from calldata",
                    stringify!($field_name)
                )));
            }
            let value = std::ffi::CStr::from_ptr($field_name)
                .to_str()
                .map_err(|_| $crate::utils::ObsError::StringConversionError)?;
            Result::<_, $crate::utils::ObsError>::Ok(value.to_owned())
        }
    };

    ($field_name: ident, $other:ty) => {
        $crate::__signals_impl_primitive_handler!(__enum $field_name, $other)
    };

    (__inner, $field_name: ident, $field_type: ty) => {
        move |__internal_calldata| {
            let mut $field_name = std::mem::zeroed::<$field_type>();
            let obs_str = $crate::utils::ObsString::new(stringify!($field_name));
            let success = libobs::calldata_get_data(
                __internal_calldata,
                obs_str.as_c_str().as_ptr(),
                &mut $field_name as *const _ as *mut std::ffi::c_void,
                std::mem::size_of::<$field_type>(),
            );
            if !success {
                return Err($crate::utils::ObsError::SignalDataError(format!(
                    "Failed to get {} from calldata",
                    stringify!($field_name)
                )));
            }
            Result::<_, $crate::utils::ObsError>::Ok($field_name)
        }
    };

    (__ptr, $field_name: ident, $field_type: ty) => {
        move |__internal_calldata| {
            let raw = $crate::__signals_impl_primitive_handler!(__inner, $field_name, $field_type)(
                __internal_calldata,
            )?;
            // SAFETY: `raw` was copied from libobs callback calldata for this
            // callback invocation. SignalObjectId stores only opaque identity.
            Result::<_, $crate::utils::ObsError>::Ok(unsafe {
                $crate::signals::SignalObjectId::from_raw(raw)
            })
        }
    };

    (__enum $field_name: ident, $enum_type: ty) => {
        move |__internal_calldata| {
            let code = $crate::__signals_impl_primitive_handler!(__inner, $field_name, i64)(
                __internal_calldata,
            )?;
            <$enum_type>::try_from(code as i32).map_err(|e| {
                $crate::utils::ObsError::EnumConversionError(format!(
                    "Failed to convert {}: {}",
                    stringify!($field_name),
                    e
                ))
            })
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __signals_impl_signal {
    ($ptr: ty, $signal_name: literal, $field_name: ident: $gen_type:ty) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type>] = $gen_type;

            #[allow(unknown_lints)]
            #[allow(ensure_obs_call_in_runtime)]
            /// # Safety
            /// Called only by the generated extern-C signal callback while OBS
            /// guarantees that the calldata pointer remains valid for this invocation.
            unsafe fn [<$signal_name:snake _handler_inner>](
                cd: *mut libobs::calldata_t,
            ) -> Result<$gen_type, $crate::utils::ObsError> {
                $crate::__signals_impl_primitive_handler!($field_name, $gen_type)(cd)
            }
        }
    };

    ($ptr: ty, $signal_name: literal, ) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type>] = ();

            #[allow(unknown_lints)]
            #[allow(ensure_obs_call_in_runtime)]
            /// # Safety
            /// Called only by the generated extern-C signal callback while OBS
            /// guarantees that the calldata pointer remains valid for this invocation.
            unsafe fn [<$signal_name:snake _handler_inner>](
                _cd: *mut libobs::calldata_t,
            ) -> Result<(), $crate::utils::ObsError> {
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
        POINTERS {$($ptr_field_name: ident: $ptr_field_type: ty),* $(,)*}
    }) => {
        $crate::__signals_impl_signal!($ptr, $signal_name, struct $name {
            ; POINTERS {$($ptr_field_name: $ptr_field_type),*}
        });
    };

    ($ptr: ty, $signal_name: literal, struct $name: ident {
        $($field_name: ident: $field_type: ty),* $(,)*;
        POINTERS {$($ptr_field_name: ident: $ptr_field_type: ty),* $(,)*}
    }) => {
        paste::paste! {
            type [<__Private $signal_name:camel Type>] = $name;

            #[derive(Debug, Clone)]
            pub struct $name {
                $(pub $field_name: $field_type,)*
                $(pub $ptr_field_name: $crate::signals::SignalObjectId,)*
            }

            #[allow(unknown_lints)]
            #[allow(ensure_obs_call_in_runtime)]
            /// # Safety
            /// Called only by the generated extern-C signal callback while OBS
            /// guarantees that the calldata pointer remains valid for this invocation.
            unsafe fn [<$signal_name:snake _handler_inner>](
                cd: *mut libobs::calldata_t,
            ) -> Result<$name, $crate::utils::ObsError> {
                $(
                    let $field_name =
                        $crate::__signals_impl_primitive_handler!($field_name, $field_type)(cd)?;
                )*
                $(
                    let $ptr_field_name =
                        $crate::__signals_impl_primitive_handler!(__ptr, $ptr_field_name, $ptr_field_type)(cd)?;
                )*

                Ok($name {
                    $($field_name,)*
                    $($ptr_field_name,)*
                })
            }
        }
    };
}
