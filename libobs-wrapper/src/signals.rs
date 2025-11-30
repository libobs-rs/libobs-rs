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

#[macro_export]
macro_rules! impl_signal_manager {
    ($handler_getter: expr, $name: ident for $ref: ident<$ptr: ty>, [
        $($(#[$attr:meta])* $signal_name: literal: { $($inner_def:tt)* }),* $(,)*
    ]) => {
        paste::paste! {
            $($crate::__signals_impl_signal!($ptr, $signal_name, $($inner_def)*);)*

            $(
            extern "C" fn [< $signal_name:snake _handler>](obj_ptr: *mut std::ffi::c_void, __internal_calldata: *mut libobs::calldata_t) {
                #[allow(unused_unsafe)]
                let res = unsafe { [< $signal_name:snake _handler_inner>](__internal_calldata) };
                if res.is_err() {
                    log::warn!("Error processing signal {}: {:?}", stringify!($signal_name), res.err());
                    return;
                }

                let res = res.unwrap();
                let senders = [<$signal_name:snake:upper _SENDERS>].read();
                if let Err(e) = senders {
                    log::warn!("Failed to acquire read lock for signal {}: {}", stringify!($signal_name), e);
                    return;
                }

                let senders = senders.unwrap();
                let senders = senders.get(&$crate::unsafe_send::SendableComp(obj_ptr as $ptr));
                if senders.is_none() {
                    log::warn!("No sender found for signal {}", stringify!($signal_name));
                    return;
                }

                let senders = senders.unwrap();
                let _ = senders.send(res);
            })*

            #[derive(Debug)]
            pub struct $name {
                pointer: $crate::unsafe_send::SendableComp<$ptr>,
                runtime: $crate::runtime::ObsRuntime
            }

            impl $name {
                pub(crate) fn new(ptr: &Sendable<$ptr>, runtime: $crate::runtime::ObsRuntime) -> Result<Self, $crate::utils::ObsError> {
                    use $crate::{utils::ObsString, unsafe_send::SendableComp};
                    let pointer =  SendableComp(ptr.0);

                    $(
                        let senders = [<$signal_name:snake:upper _SENDERS>].clone();
                        let senders = senders.write();
                        if senders.is_err() {
                            return Err($crate::utils::ObsError::LockError("Failed to acquire write lock for signal senders".to_string()));
                        }

                        let (tx, [<_ $signal_name:snake _rx>]) = tokio::sync::broadcast::channel(16);
                        let mut senders = senders.unwrap();
                        senders.insert(pointer.clone(), tx);
                    )*

                    $crate::run_with_obs!(runtime, (pointer), move || {
                            let handler = ($handler_getter)(pointer);
                            $(
                                let signal = ObsString::new($signal_name);
                                unsafe {
                                    libobs::signal_handler_connect(
                                        handler,
                                        signal.as_ptr().0,
                                        Some([< $signal_name:snake _handler>]),
                                        pointer as *mut std::ffi::c_void,
                                    );
                                };
                            )*
                    })?;

                    Ok(Self {
                        pointer,
                        runtime
                    })
                }

                $(
                    $(#[$attr])*
                    pub fn [<on_ $signal_name:snake>](&self) -> Result<tokio::sync::broadcast::Receiver<[<__Private $signal_name:camel Type >]>, $crate::utils::ObsError> {
                        let handlers = [<$signal_name:snake:upper _SENDERS>].read();
                        if handlers.is_err() {
                            return Err($crate::utils::ObsError::LockError("Failed to acquire read lock for signal senders".to_string()));
                        }

                        let handlers = handlers.unwrap();
                        let rx = handlers.get(&self.pointer)
                            .ok_or_else(|| $crate::utils::ObsError::NoSenderError)?
                            .subscribe();

                        Ok(rx)
                    }
                )*
            }

            impl Drop for $name {
                fn drop(&mut self) {
                    log::trace!("Dropping signal manager {}...", stringify!($name));

                    #[allow(unused_variables)]
                    let ptr = self.pointer.clone();
                    #[allow(unused_variables)]
                    let runtime = self.runtime.clone();

                    //TODO make this non blocking
                    let future = $crate::run_with_obs!(runtime, (ptr), move || {
                        #[allow(unused_variables)]
                        let handler = ($handler_getter)(ptr);
                        $(
                            let signal = $crate::utils::ObsString::new($signal_name);
                            unsafe {
                                libobs::signal_handler_disconnect(
                                    handler,
                                    signal.as_ptr().0,
                                    Some([< $signal_name:snake _handler>]),
                                    ptr as *mut std::ffi::c_void,
                                );
                            }
                        )*
                    });

                    let r = {
                        $(
                            let handlers = [<$signal_name:snake:upper _SENDERS>].write();
                            if handlers.is_err() {
                                log::warn!("Failed to acquire write lock for signal {} senders during drop", stringify!($signal_name));
                                return;
                            }

                            let mut handlers = handlers.unwrap();
                            handlers.remove(&self.pointer);
                        )*

                        future
                    };

                    if std::thread::panicking() {
                        return;
                    }

                    r.unwrap();
                }
            }
        }
    };
}
