//! Per-object OBS signal subscriptions.
//!
//! Signal state used to live in process-global maps keyed by native pointer addresses.
//! Besides unnecessary global locking, that let callback-only raw pointers escape
//! through async channels.  A signal manager now owns its own hubs and passes stable
//! hub addresses directly to libobs as callback data.

mod handler;
mod traits;

pub use traits::*;

use crossbeam_channel::{
    bounded, Receiver, RecvError, RecvTimeoutError, Sender, TryRecvError, TrySendError,
};
use std::{sync::Mutex, time::Duration};

const SIGNAL_QUEUE_CAPACITY: usize = 32;

/// Opaque identity of an object mentioned by an OBS callback.
///
/// This is deliberately not a raw pointer and cannot be dereferenced. Callback data is
/// often borrowed only for the duration of the C callback; exposing that pointer as a
/// sendable Rust value allowed use-after-free in otherwise safe code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalObjectId(usize);

impl SignalObjectId {
    /// Construct an opaque callback identity from a callback-borrowed native pointer.
    ///
    /// # Safety
    /// `ptr` must be a pointer supplied by libobs for the current callback. The returned
    /// identity is never dereferenceable and does not extend the native object's lifetime.
    #[doc(hidden)]
    pub unsafe fn from_raw<T>(ptr: *mut T) -> Self {
        Self(ptr as usize)
    }

    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

/// Synchronous receiver for an OBS signal subscription.
///
/// Each subscriber has a bounded queue. If it falls behind, new callback events are
/// dropped for that subscriber rather than blocking the OBS callback thread.
#[derive(Debug)]
pub struct SignalReceiver<T> {
    receiver: Receiver<T>,
}

impl<T> SignalReceiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        self.receiver.recv()
    }

    /// Compatibility alias for the old Tokio broadcast receiver call sites.
    pub fn blocking_recv(&self) -> Result<T, RecvError> {
        self.recv()
    }

    /// Waits for a signal for at most `timeout`.
    ///
    /// Native OBS callbacks are not cancelled when this deadline expires. Callers that use a
    /// signal as completion for a native operation must therefore treat a timeout as an unknown
    /// completion state and re-query native state before retrying or tearing resources down.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.receiver.try_recv()
    }
}

/// A small per-object fan-out hub used by generated signal managers.
#[doc(hidden)]
#[derive(Debug)]
pub struct SignalHub<T> {
    subscribers: Mutex<Vec<Sender<T>>>,
}

impl<T> Default for SignalHub<T> {
    fn default() -> Self {
        Self {
            subscribers: Mutex::new(Vec::new()),
        }
    }
}

impl<T> SignalHub<T> {
    #[doc(hidden)]
    pub fn new() -> Self {
        Self::default()
    }

    #[doc(hidden)]
    pub fn subscribe(&self) -> SignalReceiver<T> {
        let (tx, rx) = bounded(SIGNAL_QUEUE_CAPACITY);
        match self.subscribers.lock() {
            Ok(mut subscribers) => subscribers.push(tx),
            Err(poisoned) => poisoned.into_inner().push(tx),
        }
        SignalReceiver { receiver: rx }
    }
}

impl<T: Clone> SignalHub<T> {
    #[doc(hidden)]
    pub fn publish(&self, value: T) {
        let mut subscribers = match self.subscribers.lock() {
            Ok(subscribers) => subscribers,
            Err(poisoned) => poisoned.into_inner(),
        };
        subscribers.retain(|subscriber| match subscriber.try_send(value.clone()) {
            Ok(()) | Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        });
    }
}

/// Generate a signal manager whose subscription state is owned by that manager.
#[macro_export]
macro_rules! impl_signal_manager {
    ($handler_getter: expr, $name: ident for $ptr: ty, [
        $($(#[$attr:meta])* $signal_name: literal: { $($inner_def:tt)* }),* $(,)*
    ]) => {
        paste::paste! {
            $($crate::__signals_impl_signal!($ptr, $signal_name, $($inner_def)*);)*

            $(
                extern "C" fn [<$signal_name:snake _handler>](
                    hub_ptr: *mut std::ffi::c_void,
                    __internal_calldata: *mut libobs::calldata_t,
                ) {
                    let Some(hub) = (unsafe {
                        // Safety: libobs receives this address from Arc::as_ptr during
                        // connect and the manager keeps the Arc alive until disconnect.
                        (hub_ptr as *const $crate::signals::SignalHub<
                            [<__Private $signal_name:camel Type>]
                        >).as_ref()
                    }) else {
                        log::warn!("Null signal hub for {}", $signal_name);
                        return;
                    };

                    let value = unsafe {
                        // Safety: libobs invokes the callback with valid calldata for
                        // the duration of this call.
                        [<$signal_name:snake _handler_inner>](__internal_calldata)
                    };
                    match value {
                        Ok(value) => hub.publish(value),
                        Err(err) => log::warn!("Error processing signal {}: {:?}", $signal_name, err),
                    }
                }
            )*

            #[derive(Debug)]
            pub struct $name {
                runtime: $crate::runtime::ObsRuntime,
                pointer: $crate::unsafe_send::SmartPointerSendable<$ptr>,
                $([<$signal_name:snake _hub>]: std::sync::Arc<
                    $crate::signals::SignalHub<[<__Private $signal_name:camel Type>]>
                >,)*
            }

            impl $name {
                pub(crate) fn new(
                    smart_ptr: &$crate::unsafe_send::SmartPointerSendable<$ptr>,
                    runtime: $crate::runtime::ObsRuntime,
                ) -> Result<Self, $crate::utils::ObsError> {
                    use $crate::utils::ObsString;
                    let smart_ptr = smart_ptr.clone();
                    $(
                        let [<$signal_name:snake _hub>] = std::sync::Arc::new(
                            $crate::signals::SignalHub::<[<__Private $signal_name:camel Type>]>::new()
                        );
                    )*

                    // Connect the complete manager in one actor command. A queue/shutdown
                    // failure therefore happens before any callback userdata is registered,
                    // eliminating partially-constructed managers with dangling C userdata.
                    let connect_ptr = smart_ptr.clone();
                    $(let [<$signal_name:snake _connect_hub>] = [<$signal_name:snake _hub>].clone();)*
                    $crate::run_with_obs!(
                        runtime,
                        (connect_ptr, $([<$signal_name:snake _connect_hub>],)*),
                        move || {
                            $(
                                let handler = ($handler_getter)(connect_ptr.clone());
                                let signal = ObsString::new($signal_name);
                                unsafe {
                                    // SAFETY: handler and signal are valid on the OBS actor;
                                    // each Arc allocation has a stable address until disconnect.
                                    libobs::signal_handler_connect(
                                        handler,
                                        signal.as_c_str().as_ptr(),
                                        Some([<$signal_name:snake _handler>]),
                                        std::sync::Arc::as_ptr(&[<$signal_name:snake _connect_hub>])
                                            as *mut std::ffi::c_void,
                                    );
                                }
                            )*
                        }
                    )?;

                    Ok(Self {
                        pointer: smart_ptr,
                        runtime,
                        $([<$signal_name:snake _hub>],)*
                    })
                }

                $(
                    $(#[$attr])*
                    pub fn [<on_ $signal_name:snake>](&self) -> Result<
                        $crate::signals::SignalReceiver<[<__Private $signal_name:camel Type>]>,
                        $crate::utils::ObsError,
                    > {
                        Ok(self.[<$signal_name:snake _hub>].subscribe())
                    }
                )*
            }

            impl Drop for $name {
                fn drop(&mut self) {
                    log::trace!("Dropping signal manager {}...", stringify!($name));
                    let ptr = self.pointer.clone();
                    $(let [<$signal_name:snake _hub>] = self.[<$signal_name:snake _hub>].clone();)*

                    // Deferred cleanup runs on the OBS owner thread; capturing a runtime clone
                    // would let the final runtime handle drop on the actor during shutdown.
                    self.runtime.defer_obs_cleanup(move || {
                        let handler = ($handler_getter)(ptr.clone());
                        $(
                            let signal = $crate::utils::ObsString::new($signal_name);
                            unsafe {
                                // SAFETY: Deferred cleanup executes on the OBS owner thread;
                                // the signal string and captured hub Arc remain alive for disconnect.
                                libobs::signal_handler_disconnect(
                                    handler,
                                    signal.as_c_str().as_ptr(),
                                    Some([<$signal_name:snake _handler>]),
                                    std::sync::Arc::as_ptr(&[<$signal_name:snake _hub>])
                                        as *mut std::ffi::c_void,
                                );
                            }
                        )*
                    });
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_signal_subscriber_drops_new_events_without_blocking() {
        let hub = SignalHub::new();
        let receiver = hub.subscribe();
        for value in 0..(SIGNAL_QUEUE_CAPACITY + 5) {
            hub.publish(value);
        }

        let drained: Vec<_> = (0..SIGNAL_QUEUE_CAPACITY)
            .map(|_| receiver.try_recv().expect("queued signal"))
            .collect();
        assert_eq!(drained, (0..SIGNAL_QUEUE_CAPACITY).collect::<Vec<_>>());
        assert!(matches!(receiver.try_recv(), Err(TryRecvError::Empty)));
    }

    #[test]
    fn disconnected_signal_subscribers_are_removed() {
        let hub = SignalHub::new();
        let receiver = hub.subscribe();
        drop(receiver);
        hub.publish(1_u8);
        let subscribers = hub
            .subscribers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        assert!(subscribers.is_empty());
    }

    #[test]
    fn timeout_receiver_observes_completion_published_before_wait() {
        let hub = SignalHub::new();
        let receiver = hub.subscribe();
        hub.publish(42_u8);

        assert_eq!(receiver.recv_timeout(Duration::from_millis(1)).unwrap(), 42);
    }

    #[test]
    fn timeout_receiver_reports_missing_completion() {
        let hub = SignalHub::<u8>::new();
        let receiver = hub.subscribe();

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(1)),
            Err(RecvTimeoutError::Timeout)
        ));
    }
}
