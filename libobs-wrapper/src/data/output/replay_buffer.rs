//! Provides functionality for working with OBS replay buffers.
//!
//! This module extends the ObsOutputRef to provide replay buffer capabilities.
//! A replay buffer is a special type of output that continuously records
//! the last N seconds of content, allowing the user to save this buffer on demand. This must be configured. More documentation soon.
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    data::{
        object::ObsObjectTrait,
        output::{
            traits::{lock_until, wait_for_completion},
            ObsOutputRef, ObsOutputTrait, ObsOutputTraitSealed,
        },
    },
    forward_obs_object_impl, forward_obs_output_impl, impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsCalldataExt, ObsError, ObsString, OutputInfo},
};

#[derive(Debug, Clone)]
/// A reference to an OBS output.
///
/// This struct is used specifically for the replay buffer to manage saving the buffer to a file
/// and configuring special settings, which are specific to the replay buffer
///
/// The output is associated with video and audio encoders that convert
/// raw media to the required format before sending/storing.
pub struct ObsReplayBufferOutputRef {
    /// Disconnect signals first
    replay_signal_manager: Arc<ObsReplayOutputSignals>,

    /// libobs exposes one replay `saved` signal and one `get_last_replay` value with no request
    /// identity. Serializing saves is therefore required to correlate a completion/path with the
    /// request that initiated it.
    save_lock: Arc<Mutex<()>>,

    output: ObsOutputRef,
}

/// Default maximum time [`ObsReplayBufferOutputRef::save_buffer`] waits for native completion.
pub const DEFAULT_REPLAY_SAVE_TIMEOUT: Duration = Duration::from_secs(30);

impl ObsOutputTraitSealed for ObsReplayBufferOutputRef {
    fn new(mut output: OutputInfo, runtime: ObsRuntime) -> Result<Self, ObsError> {
        output.id = ObsString::new("replay_buffer");
        let output = ObsOutputRef::new(output, runtime.clone())?;

        let replay_signal_manager =
            ObsReplayOutputSignals::new(&output.__native_handle(), runtime)?;
        Ok(Self {
            replay_signal_manager: Arc::new(replay_signal_manager),
            save_lock: Arc::new(Mutex::new(())),
            output,
        })
    }

    fn video_encoder_slot(
        &self,
    ) -> &std::sync::Arc<
        std::sync::RwLock<Option<std::sync::Arc<crate::encoders::video::ObsVideoEncoder>>>,
    > {
        self.output.video_encoder_slot()
    }

    fn audio_encoder_slots(
        &self,
    ) -> &std::sync::Arc<
        std::sync::RwLock<
            std::collections::HashMap<
                usize,
                std::sync::Arc<crate::encoders::audio::ObsAudioEncoder>,
            >,
        >,
    > {
        self.output.audio_encoder_slots()
    }

    fn service_slot(
        &self,
    ) -> &std::sync::Arc<std::sync::RwLock<Option<std::sync::Arc<crate::services::ObsServiceRef>>>>
    {
        self.output.service_slot()
    }

    fn configuration_lock(&self) -> &std::sync::Arc<std::sync::Mutex<()>> {
        self.output.configuration_lock()
    }

    fn lifecycle_lock(&self) -> &std::sync::Arc<std::sync::Mutex<()>> {
        self.output.lifecycle_lock()
    }
}

forward_obs_object_impl!(ObsReplayBufferOutputRef, output, *mut libobs::obs_output);
forward_obs_output_impl!(ObsReplayBufferOutputRef, output);

impl_signal_manager!(|ptr: SmartPointerSendable<*mut libobs::obs_output>| {
    unsafe {
        // Safety: Again, it carries a reference of the drop guard so we must have a valid pointer
        libobs::obs_output_get_signal_handler(ptr.get_ptr())
    }
}, ObsReplayOutputSignals for *mut libobs::obs_output, [
    "saved": {}
]);

impl ObsReplayBufferOutputRef {
    pub fn replay_signals(&self) -> &Arc<ObsReplayOutputSignals> {
        &self.replay_signal_manager
    }
    /// Saves the current replay buffer content to disk using a bounded completion wait.
    ///
    /// Save requests are serialized because the OBS replay-buffer procedure exposes only a single
    /// `saved` signal and global `get_last_replay` path. The subscription is established before
    /// invoking `save`, so even a completion emitted during the initiating actor call is observed.
    ///
    /// If the deadline expires, this returns [`ObsError::OutputOperationTimeout`]. The native save
    /// is not cancelled and may still finish later; wait for/re-query replay state before assuming
    /// that no file was produced.
    pub fn save_buffer(&self) -> Result<Box<Path>, ObsError> {
        self.save_buffer_with_timeout(DEFAULT_REPLAY_SAVE_TIMEOUT)
    }

    /// Saves the replay buffer while waiting at most one overall `timeout`, including time spent
    /// behind another save request.
    pub fn save_buffer_with_timeout(&self, timeout: Duration) -> Result<Box<Path>, ObsError> {
        if self.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }

        let operation = "replay-save";
        let started = Instant::now();
        let deadline = started.checked_add(timeout).unwrap_or(started);
        let _save = lock_until(&self.save_lock, deadline, operation, timeout)?;

        if !self.is_active()? {
            return Err(ObsError::OutputSaveBufferFailure(
                "Replay buffer output is not active.".to_string(),
            ));
        }

        log::trace!("Saving replay buffer...");
        let output_ptr = self.__native_handle();

        log::trace!("Getting procedure handler for replay buffer output...");
        let proc_handler = run_with_obs!(self.runtime().clone(), (output_ptr), move || {
            // Safety: output_ptr is retained by the managed output handle.
            let ph = unsafe { libobs::obs_output_get_proc_handler(output_ptr.get_ptr()) };
            if ph.is_null() {
                return Err(ObsError::OutputSaveBufferFailure(
                    "Failed to get replay-buffer procedure handler.".to_string(),
                ));
            }
            Ok(Sendable(ph))
        })??;

        // Critical ordering: observe completion before calling `save`.
        let saved = self.replay_signals().on_saved()?;

        log::trace!("Calling 'save' procedure on replay buffer output...");
        // Safety: proc_handler belongs to this still-live output.
        unsafe {
            self.runtime().call_proc_handler(&proc_handler, "save")?;
        }

        log::trace!("Waiting for 'saved' signal from replay buffer output...");
        wait_for_completion(&saved, deadline, operation, timeout)?;

        log::trace!("Retrieving last replay path from replay buffer output...");
        // Safety: proc_handler belongs to this still-live output.
        let mut calldata = unsafe {
            self.runtime()
                .call_proc_handler(&proc_handler, "get_last_replay")?
        };

        log::trace!("Extracting path from calldata...");
        let path = PathBuf::from(calldata.get_string("path")?);
        if path.as_os_str().is_empty() {
            return Err(ObsError::OutputSaveBufferFailure(
                "Replay save completed but OBS returned an empty last-replay path.".to_string(),
            ));
        }

        Ok(path.into_boxed_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::SignalHub;
    use std::sync::{mpsc, Barrier};
    use std::thread;

    #[test]
    fn completion_published_during_initiation_is_not_lost() {
        let hub = SignalHub::new();
        let receiver = hub.subscribe();
        let timeout = Duration::from_millis(100);
        let started = Instant::now();
        let deadline = started.checked_add(timeout).unwrap();

        // Models a native `save` procedure that emits `saved` before returning.
        hub.publish(());

        wait_for_completion(&receiver, deadline, "replay-save", timeout).unwrap();
    }

    #[test]
    fn overlapping_save_lock_is_bounded() {
        let save_lock = Arc::new(Mutex::new(()));
        let held = save_lock.lock().unwrap();
        let lock_for_thread = save_lock.clone();
        let barrier = Arc::new(Barrier::new(2));
        let thread_barrier = barrier.clone();

        let waiter = thread::spawn(move || {
            thread_barrier.wait();
            let timeout = Duration::from_millis(20);
            let started = Instant::now();
            let deadline = started.checked_add(timeout).unwrap();
            lock_until(&lock_for_thread, deadline, "replay-save", timeout).map(|_| ())
        });

        barrier.wait();
        let result = waiter.join().unwrap();
        drop(held);
        assert!(matches!(
            result,
            Err(ObsError::OutputOperationTimeout { operation, .. }) if operation == "replay-save"
        ));
    }

    #[test]
    fn overlapping_save_requests_serialize_when_deadline_allows() {
        let save_lock = Arc::new(Mutex::new(()));
        let (first_acquired_tx, first_acquired_rx) = mpsc::channel();
        let (release_first_tx, release_first_rx) = mpsc::channel();

        let first_lock = save_lock.clone();
        let first = thread::spawn(move || {
            let timeout = Duration::from_secs(1);
            let started = Instant::now();
            let deadline = started.checked_add(timeout).unwrap();
            let _guard = lock_until(&first_lock, deadline, "replay-save", timeout).unwrap();
            first_acquired_tx.send(()).unwrap();
            release_first_rx.recv().unwrap();
        });

        first_acquired_rx.recv().unwrap();
        let second_lock = save_lock.clone();
        let second = thread::spawn(move || {
            let timeout = Duration::from_secs(1);
            let started = Instant::now();
            let deadline = started.checked_add(timeout).unwrap();
            lock_until(&second_lock, deadline, "replay-save", timeout).map(|_| ())
        });

        release_first_tx.send(()).unwrap();
        first.join().unwrap();
        second.join().unwrap().unwrap();
    }

    #[test]
    fn missing_replay_completion_times_out() {
        let hub = SignalHub::<()>::new();
        let receiver = hub.subscribe();
        let timeout = Duration::from_millis(1);
        let started = Instant::now();
        let deadline = started.checked_add(timeout).unwrap();

        assert!(matches!(
            wait_for_completion(&receiver, deadline, "replay-save", timeout),
            Err(ObsError::OutputOperationTimeout { operation, .. }) if operation == "replay-save"
        ));
    }
}
