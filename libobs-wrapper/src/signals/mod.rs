mod handler;
mod traits;

pub use traits::*;

/// Generates a signal manager for OBS objects that can emit signals.
///
/// This macro creates a complete signal management system including:
/// - Signal handler functions that interface with OBS's C API
/// - A manager struct that maintains signal subscriptions
/// - Methods to subscribe to signals via `tokio::sync::broadcast` channels
/// - Automatic cleanup on drop
///
/// # Parameters
///
/// * `$handler_getter` - A closure that takes a `SmartPointerSendable<$ptr>` and returns the raw signal handler pointer.
///   The closure should have an explicit type annotation for the parameter and typically contains an unsafe block.
///   Example: `|scene_ptr: SmartPointerSendable<*mut obs_scene_t>| unsafe { libobs::obs_scene_get_signal_handler(scene_ptr.get_ptr()) }`
///
/// * `$name` - The identifier for the generated signal manager struct.
///
/// * `$ptr` - The raw pointer type for the OBS object (e.g., `*mut obs_scene_t`).
///
/// * Signal definitions - A list of signal definitions with the following syntax:
///   - `"signal_name": {}` - For signals with no data
///   - `"signal_name": { field: Type }` - For signals with a single field
///   - `"signal_name": { struct StructName { field1: Type1, field2: Type2 } }` - For signals with multiple fields
///   - `"signal_name": { struct StructName { field1: Type1; POINTERS { ptr_field: *mut Type } } }` - For signals with both regular and pointer fields
///
/// # Generated Code
///
/// The macro generates:
/// - A `$name` struct that manages all signal subscriptions for a single object instance
/// - `on_<signal_name>()` methods that return `broadcast::Receiver` for each signal
/// - Automatic signal handler registration and cleanup
/// - Thread-safe signal dispatching using `tokio::sync::broadcast`
///
/// # Signal Data Types
///
/// Signals can carry different types of data:
/// - **Empty signals**: Use `"signal_name": {}`
/// - **Single value**: Use `"signal_name": { value: Type }` where Type can be primitives, String, or enums
/// - **Struct**: Use `"signal_name": { struct Name { field1: Type1, field2: Type2 } }`
/// - **Pointers**: Use the `POINTERS` section to mark fields as raw pointers that need special handling
///
/// # Safety
///
/// The generated code is safe to use, but relies on:
/// - The OBS runtime being properly initialized
/// - Smart pointers remaining valid for the lifetime of the signal manager
/// - Signal handlers being called on the OBS thread
///
/// # Examples
///
/// ```ignore
/// impl_signal_manager!(
///     |scene_ptr: SmartPointerSendable<*mut obs_scene_t>| unsafe {
///         let source_ptr = libobs::obs_scene_get_source(scene_ptr.get_ptr());
///         libobs::obs_source_get_signal_handler(source_ptr)
///     },
///     ObsSceneSignals for *mut obs_scene_t,
///     [
///         // Simple signal with no data
///         "refresh": {},
///         
///         // Signal with a single pointer field
///         "item_add": {
///             struct ItemAddSignal {
///                 POINTERS {
///                     item: *mut libobs::obs_sceneitem_t,
///                 }
///             }
///         },
///         
///         // Signal with both regular and pointer fields
///         "item_visible": {
///             struct ItemVisibleSignal {
///                 visible: bool;
///                 POINTERS {
///                     item: *mut libobs::obs_sceneitem_t,
///                 }
///             }
///         }
///     ]
/// );
/// ```
///
/// # Usage
///
/// The generated signal manager is typically stored in an `Arc` within your main struct:
///
/// ```ignore
/// pub struct ObsSceneRef {
///     signals: Arc<ObsSceneSignals>,
///     // ... other fields
/// }
///
/// impl ObsSceneRef {
///     pub fn signals(&self) -> Arc<ObsSceneSignals> {
///         self.signals.clone()
///     }
/// }
///
/// // Subscribe to signals
/// let scene = ObsSceneRef::new(name, runtime)?;
/// let signals = scene.signals();
/// let mut rx = signals.on_refresh()?;
///
/// tokio::spawn(async move {
///     while let Ok(_) = rx.recv().await {
///         println!("Scene refreshed!");
///     }
/// });
/// ```
#[macro_export]
macro_rules! impl_signal_manager {
    ($handler_getter: expr, $name: ident for $ptr: ty, [
        $($(#[$attr:meta])* $signal_name: literal: { $($inner_def:tt)* }),* $(,)*
    ]) => {
        paste::paste! {
            $($crate::__signals_impl_signal!($ptr, $signal_name, $($inner_def)*);)*

            $(
            extern "C" fn [< $signal_name:snake _handler>](obj_ptr_key: *mut std::ffi::c_void, __internal_calldata: *mut libobs::calldata_t) {
                let obj_ptr_key = obj_ptr_key as usize;

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
                let senders = senders.get(&obj_ptr_key);
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
                fn smart_ptr_to_key(ptr: &$crate::unsafe_send::SmartPointerSendable<$ptr>) -> usize {
                    ptr.get_ptr() as usize
                }

                pub(crate) fn new(smart_ptr: &$crate::unsafe_send::SmartPointerSendable<$ptr>, runtime: $crate::runtime::ObsRuntime) -> Result<Self, $crate::utils::ObsError> {
                    use $crate::utils::ObsString;
                    let smart_ptr = smart_ptr.clone();
                    let smart_ptr_as_key = Self::smart_ptr_to_key(&smart_ptr);

                    $(
                        let senders = [<$signal_name:snake:upper _SENDERS>].clone();
                        let senders = senders.write();
                        if senders.is_err() {
                            return Err($crate::utils::ObsError::LockError("Failed to acquire write lock for signal senders".to_string()));
                        }

                        let (tx, [<_ $signal_name:snake _rx>]) = tokio::sync::broadcast::channel(16);
                        let mut senders = senders.unwrap();
                        // Its fine since we are just using the pointer as key
                        senders.insert(smart_ptr_as_key.clone(), tx);
                    )*

                    $crate::run_with_obs!(runtime, (smart_ptr_as_key, smart_ptr), move || {
                            let handler = ($handler_getter)(smart_ptr);
                            $(
                                let signal = ObsString::new($signal_name);
                                unsafe {
                                    // Safety: We know that the handler must exist, the signal is still in scope, so the ptr to that is valid as well and we are just using the raw_ptr as key in the handler function.
                                    libobs::signal_handler_connect(
                                        handler,
                                        signal.as_ptr().0,
                                        Some([< $signal_name:snake _handler>]),
                                        // We are just casting it back to a usize in the handler function
                                        smart_ptr_as_key as *mut std::ffi::c_void,
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
