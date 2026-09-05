//! Capability-aware media-source controls.

use std::time::Duration;

use crate::{run_with_obs, sources::ObsSourceTrait, utils::ObsError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaState {
    None,
    Playing,
    Opening,
    Buffering,
    Paused,
    Stopped,
    Ended,
    Error,
    Unknown(u32),
}

impl MediaState {
    fn from_native(value: libobs::obs_media_state) -> Self {
        match value {
            libobs::obs_media_state_OBS_MEDIA_STATE_NONE => Self::None,
            libobs::obs_media_state_OBS_MEDIA_STATE_PLAYING => Self::Playing,
            libobs::obs_media_state_OBS_MEDIA_STATE_OPENING => Self::Opening,
            libobs::obs_media_state_OBS_MEDIA_STATE_BUFFERING => Self::Buffering,
            libobs::obs_media_state_OBS_MEDIA_STATE_PAUSED => Self::Paused,
            libobs::obs_media_state_OBS_MEDIA_STATE_STOPPED => Self::Stopped,
            libobs::obs_media_state_OBS_MEDIA_STATE_ENDED => Self::Ended,
            libobs::obs_media_state_OBS_MEDIA_STATE_ERROR => Self::Error,
            other => Self::Unknown(other),
        }
    }
}

fn ensure_media<T: ObsSourceTrait + ?Sized>(source: &T) -> Result<(), ObsError> {
    let source_ptr = source.__native_handle();
    let runtime = source.runtime().clone();
    let flags = run_with_obs!(runtime, (source_ptr), move || unsafe {
        // Safety: source is retained for the actor call.
        libobs::obs_source_get_output_flags(source_ptr.get_ptr())
    })?;
    if flags & libobs::OBS_SOURCE_CONTROLLABLE_MEDIA == 0 {
        return Err(ObsError::InvalidOperation(format!(
            "source '{}' does not expose OBS_SOURCE_CONTROLLABLE_MEDIA",
            source.name()
        )));
    }
    Ok(())
}

pub trait ObsMediaSourceExt: ObsSourceTrait {
    fn media_play(&self) -> Result<(), ObsError> {
        self.media_set_paused(false)
    }

    fn media_pause(&self) -> Result<(), ObsError> {
        self.media_set_paused(true)
    }

    fn media_set_paused(&self, paused: bool) -> Result<(), ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_play_pause(source.get_ptr(), paused);
        })
    }

    fn media_restart(&self) -> Result<(), ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_restart(source.get_ptr());
        })
    }

    fn media_stop(&self) -> Result<(), ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_stop(source.get_ptr());
        })
    }

    fn media_next(&self) -> Result<(), ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_next(source.get_ptr());
        })
    }

    fn media_previous(&self) -> Result<(), ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_previous(source.get_ptr());
        })
    }

    /// Requests a media seek to `position`.
    ///
    /// The native operation is asynchronous and source/plugin-specific. A successful return means
    /// libobs accepted the request, not that [`Self::media_position`] already reflects it. Some
    /// plugins may also defer a seek made while paused until playback resumes.
    fn media_seek(&self, position: Duration) -> Result<(), ObsError> {
        ensure_media(self)?;
        let milliseconds = i64::try_from(position.as_millis()).map_err(|_| {
            ObsError::InvalidOperation("media seek position exceeds libobs i64 milliseconds".into())
        })?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_set_time(source.get_ptr(), milliseconds);
        })
    }

    fn media_position(&self) -> Result<Option<Duration>, ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let ms = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_get_time(source.get_ptr())
        })?;
        Ok((ms >= 0).then(|| Duration::from_millis(ms as u64)))
    }

    fn media_duration(&self) -> Result<Option<Duration>, ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let ms = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_get_duration(source.get_ptr())
        })?;
        Ok((ms >= 0).then(|| Duration::from_millis(ms as u64)))
    }

    fn media_state(&self) -> Result<MediaState, ObsError> {
        ensure_media(self)?;
        let source = self.__native_handle();
        let runtime = self.runtime().clone();
        let state = run_with_obs!(runtime, (source), move || unsafe {
            // Safety: source is retained for the actor call.
            libobs::obs_source_media_get_state(source.get_ptr())
        })?;
        Ok(MediaState::from_native(state))
    }
}

impl<T: ObsSourceTrait + ?Sized> ObsMediaSourceExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_media_states_are_preserved() {
        assert_eq!(
            MediaState::from_native(u32::MAX),
            MediaState::Unknown(u32::MAX)
        );
    }
}
