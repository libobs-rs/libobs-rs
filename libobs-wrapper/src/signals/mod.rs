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
            /// This signal manager must be within an `Arc` if you want to clone it.
            pub struct $name {
                pointer: $crate::unsafe_send::SendableComp<$ptr>,
                runtime: $crate::runtime::ObsRuntime
            }

            impl $name {
                pub(crate) fn new(ptr: &$crate::unsafe_send::Sendable<$ptr>, runtime: $crate::runtime::ObsRuntime) -> Result<Self, $crate::utils::ObsError> {
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
