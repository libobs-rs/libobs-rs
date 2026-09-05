//! Managed transition control with retained source lifetimes.

use std::{sync::Mutex, time::Duration};

use crate::{
    data::object::ObsObjectTrait,
    run_with_obs,
    signals::SignalReceiver,
    sources::{ObsSourceRef, ObsSourceTrait},
    utils::ObsError,
};

pub const DEFAULT_TRANSITION_COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct TransitionState {
    current: Option<ObsSourceRef>,
    in_flight: Option<ObsSourceRef>,
    completion: Option<SignalReceiver<()>>,
}

/// Managed controller around an OBS transition source.
///
/// The destination is retained while native transition work is active. [`Self::begin_to`] is the
/// non-blocking form for editor/event-loop integrations: initiate a transition, optionally interrupt
/// it with [`Self::force_stop`], and call [`Self::wait_for_completion`] when a bounded synchronous
/// wait is appropriate. [`Self::start_to_with_timeout`] combines begin + wait for simpler workflows.
///
/// If a bounded wait times out, the destination remains retained in `in_flight` until a later wait,
/// [`Self::reconcile`], or [`Self::force_stop`] resolves native state. Timeout therefore never claims
/// to cancel libobs work or releases the destination prematurely.
#[derive(Debug)]
pub struct ManagedTransition {
    transition: ObsSourceRef,
    state: Mutex<TransitionState>,
    operation: Mutex<()>,
}

impl ManagedTransition {
    pub fn new(transition: ObsSourceRef) -> Result<Self, ObsError> {
        let ptr = transition.__native_handle();
        let runtime = transition.runtime().clone();
        let source_type = run_with_obs!(runtime, (ptr), move || unsafe {
            // Safety: transition is retained for the actor call.
            libobs::obs_source_get_type(ptr.get_ptr())
        })?;
        if source_type != libobs::obs_source_type_OBS_SOURCE_TYPE_TRANSITION {
            return Err(ObsError::InvalidOperation(format!(
                "source '{}' is not an OBS transition",
                transition.name()
            )));
        }
        Ok(Self {
            transition,
            state: Mutex::new(TransitionState {
                current: None,
                in_flight: None,
                completion: None,
            }),
            operation: Mutex::new(()),
        })
    }

    pub fn source(&self) -> &ObsSourceRef {
        &self.transition
    }

    pub fn current_source(&self) -> Result<Option<ObsSourceRef>, ObsError> {
        self.state
            .lock()
            .map(|state| state.current.clone())
            .map_err(|e| ObsError::LockError(e.to_string()))
    }

    pub fn set_current_source(&self, source: ObsSourceRef) -> Result<(), ObsError> {
        self.transition
            .runtime()
            .ensure_same_runtime(source.runtime())?;
        let _operation = self
            .operation
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if self.is_active()? {
            return Err(ObsError::InvalidOperation(
                "cannot replace transition source while a transition is active".into(),
            ));
        }
        let transition = self.transition.__native_handle();
        let source_ptr = source.__native_handle();
        let runtime = self.transition.runtime().clone();
        run_with_obs!(runtime, (transition, source_ptr), move || unsafe {
            // Safety: both managed source leases are retained for the actor call.
            libobs::obs_transition_set(transition.get_ptr(), source_ptr.get_ptr());
        })?;
        let mut state = self
            .state
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        state.current = Some(source);
        state.in_flight = None;
        state.completion = None;
        Ok(())
    }

    pub fn is_active(&self) -> Result<bool, ObsError> {
        let transition = self.transition.__native_handle();
        let runtime = self.transition.runtime().clone();
        run_with_obs!(runtime, (transition), move || unsafe {
            // Safety: transition is retained for the actor call.
            libobs::obs_transition_is_active(transition.get_ptr())
        })
    }

    pub fn start_to(&self, destination: ObsSourceRef, duration: Duration) -> Result<(), ObsError> {
        self.start_to_with_timeout(destination, duration, DEFAULT_TRANSITION_COMPLETION_TIMEOUT)
    }

    /// Starts an automatic transition without blocking the caller.
    ///
    /// Active transitions are rejected rather than implicitly interrupted. This form is intended
    /// for GUI/event-loop code that needs to remain responsive and possibly call [`Self::force_stop`]
    /// before later waiting or reconciling.
    pub fn begin_to(&self, destination: ObsSourceRef, duration: Duration) -> Result<(), ObsError> {
        self.transition
            .runtime()
            .ensure_same_runtime(destination.runtime())?;
        let _operation = self
            .operation
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        self.reconcile_locked()?;
        if self.is_active()? {
            return Err(ObsError::InvalidOperation(
                "transition already active; force-stop it explicitly before interrupting".into(),
            ));
        }

        let duration_ms = u32::try_from(duration.as_millis()).map_err(|_| {
            ObsError::InvalidOperation("transition duration exceeds u32 milliseconds".into())
        })?;
        let completion = self.transition.signals().on_transition_stop()?;
        {
            let mut state = self
                .state
                .lock()
                .map_err(|e| ObsError::LockError(e.to_string()))?;
            state.in_flight = Some(destination.clone());
            state.completion = Some(completion);
        }

        let transition = self.transition.__native_handle();
        let destination_ptr = destination.__native_handle();
        let runtime = self.transition.runtime().clone();
        let started = run_with_obs!(runtime, (transition, destination_ptr), move || unsafe {
            // Safety: both source handles are retained and state retains destination beyond call.
            libobs::obs_transition_start(
                transition.get_ptr(),
                libobs::obs_transition_mode_OBS_TRANSITION_MODE_AUTO,
                duration_ms,
                destination_ptr.get_ptr(),
            )
        })?;
        if !started {
            let mut state = self
                .state
                .lock()
                .map_err(|e| ObsError::LockError(e.to_string()))?;
            state.in_flight = None;
            state.completion = None;
            return Err(ObsError::InvalidOperation(
                "libobs rejected transition start".into(),
            ));
        }
        Ok(())
    }

    /// Waits for the transition initiated by [`Self::begin_to`].
    ///
    /// A timeout leaves both destination and completion subscription retained so callers can retry
    /// the wait or reconcile native state later. Calling this on the OBS actor thread is rejected
    /// because it would prevent libobs from making progress.
    pub fn wait_for_completion(&self, timeout: Duration) -> Result<(), ObsError> {
        if self.transition.runtime().is_actor_thread() {
            return Err(ObsError::RuntimeReentrantBlocking);
        }
        let completion = {
            let mut state = self
                .state
                .lock()
                .map_err(|e| ObsError::LockError(e.to_string()))?;
            state.completion.take().ok_or_else(|| {
                ObsError::InvalidOperation("no managed transition completion is pending".into())
            })?
        };

        match completion.recv_timeout(timeout) {
            Ok(()) => {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|e| ObsError::LockError(e.to_string()))?;
                if let Some(completed) = state.in_flight.take() {
                    state.current = Some(completed);
                }
                Ok(())
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                self.state
                    .lock()
                    .map_err(|e| ObsError::LockError(e.to_string()))?
                    .completion = Some(completion);
                Err(ObsError::OutputOperationTimeout {
                    operation: "transition".into(),
                    timeout_ms: timeout.as_millis().min(u64::MAX as u128) as u64,
                })
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                Err(ObsError::OutputCompletionDisconnected {
                    operation: "transition".into(),
                })
            }
        }
    }

    /// Starts an automatic transition and waits for `transition_stop`.
    pub fn start_to_with_timeout(
        &self,
        destination: ObsSourceRef,
        duration: Duration,
        timeout: Duration,
    ) -> Result<(), ObsError> {
        self.begin_to(destination, duration)?;
        self.wait_for_completion(timeout)
    }

    /// Interrupts a managed transition and deterministically snaps to its requested destination.
    ///
    /// libobs's `obs_transition_force_stop` invokes plugin-specific stop hooks/signals but does not
    /// by itself reset generic transition source state. We therefore follow it with
    /// `obs_transition_set`, retaining the destination through the actor call and making the native
    /// transition inactive before returning.
    pub fn force_stop(&self) -> Result<(), ObsError> {
        let _operation = self
            .operation
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        let destination = {
            let state = self
                .state
                .lock()
                .map_err(|e| ObsError::LockError(e.to_string()))?;
            state.in_flight.clone().or_else(|| state.current.clone())
        };
        let transition = self.transition.__native_handle();
        let destination_ptr = destination.as_ref().map(|source| source.__native_handle());
        let runtime = self.transition.runtime().clone();
        run_with_obs!(runtime, (transition, destination_ptr), move || unsafe {
            // Safety: transition and optional destination are retained for the actor call.
            libobs::obs_transition_force_stop(transition.get_ptr());
            libobs::obs_transition_set(
                transition.get_ptr(),
                destination_ptr
                    .as_ref()
                    .map_or(std::ptr::null_mut(), |source| source.get_ptr()),
            );
        })?;

        let mut state = self
            .state
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        state.current = destination;
        state.in_flight = None;
        state.completion = None;
        Ok(())
    }

    /// Reconciles retained in-flight state after a timeout/late completion.
    pub fn reconcile(&self) -> Result<(), ObsError> {
        let _operation = self
            .operation
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        self.reconcile_locked()
    }

    fn reconcile_locked(&self) -> Result<(), ObsError> {
        if self.is_active()? {
            return Ok(());
        }
        let mut state = self
            .state
            .lock()
            .map_err(|e| ObsError::LockError(e.to_string()))?;
        if let Some(completed) = state.in_flight.take() {
            state.current = Some(completed);
        }
        state.completion = None;
        Ok(())
    }
}
