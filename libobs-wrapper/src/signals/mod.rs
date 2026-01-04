mod handler;
mod traits;

pub use traits::*;

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
                let res = unsafe {
                    // Safety: We are in the runtime and the calldata pointer is valid because OBS is calling this function
                    [< $signal_name:snake _handler_inner>](__internal_calldata)
                };
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

            /// This signal manager must be within an `Arc` if you want to clone it.
            #[derive(Debug)]
            pub struct $name {
                runtime: $crate::runtime::ObsRuntime,
                pointer: $crate::unsafe_send::SmartPointerSendable<$ptr>,
            }

            impl $name {
                fn smart_ptr_to_key(ptr: &$crate::unsafe_send::SmartPointerSendable<$ptr>) -> $crate::unsafe_send::SendableComp<$ptr> {
                    $crate::unsafe_send::SendableComp(ptr.get_ptr())
                }

                pub(crate) fn new(smart_ptr: &$crate::unsafe_send::SmartPointerSendable<$ptr>, runtime: $crate::runtime::ObsRuntime) -> Result<Self, $crate::utils::ObsError> {
                    use $crate::utils::ObsString;
                    let smart_ptr = smart_ptr.clone();
                    let raw_ptr = Self::smart_ptr_to_key(&smart_ptr);

                    $(
                        let senders = [<$signal_name:snake:upper _SENDERS>].clone();
                        let senders = senders.write();
                        if senders.is_err() {
                            return Err($crate::utils::ObsError::LockError("Failed to acquire write lock for signal senders".to_string()));
                        }

                        let (tx, [<_ $signal_name:snake _rx>]) = tokio::sync::broadcast::channel(16);
                        let mut senders = senders.unwrap();
                        // Its fine since we are just using the pointer as key
                        senders.insert(raw_ptr.clone(), tx);
                    )*

                    $crate::run_with_obs!(runtime, (raw_ptr, smart_ptr), move || {
                            let handler = ($handler_getter)(smart_ptr);
                            $(
                                let signal = ObsString::new($signal_name);
                                unsafe {
                                    // Safety: We know that the handler must exist, the signal is still in scope, so the ptr to that is valid as well and we are just using the raw_ptr as key in the handler function.
                                    libobs::signal_handler_connect(
                                        handler,
                                        signal.as_ptr().0,
                                        Some([< $signal_name:snake _handler>]),
                                        raw_ptr.0 as *mut std::ffi::c_void,
                                    );
                                };
                            )*
                    })?;

                    Ok(Self {
                        pointer: smart_ptr,
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
                        let handler_key = Self::smart_ptr_to_key(&self.pointer);
                        let rx = handlers.get(&handler_key)
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
                        let handler = ($handler_getter)(ptr.clone());
                        $(
                            let signal = $crate::utils::ObsString::new($signal_name);
                            unsafe {
                                // Safety: We are in the runtime, the signal string is allocated, we still have the drop guard as ptr in this scope so the handler is valid.
                                libobs::signal_handler_disconnect(
                                    handler,
                                    signal.as_ptr().0,
                                    Some([< $signal_name:snake _handler>]),
                                    ptr.get_ptr() as *mut std::ffi::c_void,
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
                            handlers.remove(&Self::smart_ptr_to_key(&self.pointer));
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
