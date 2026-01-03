use crate::data::object::ObsObjectTrait;
use crate::run_with_obs;
use crate::scenes::ObsSceneRef;
use crate::sources::ObsFilterGuardPair;
use crate::sources::ObsFilterRef;
use crate::sources::_ObsRemoveFilterOnDrop;
use crate::unsafe_send::SmartPointerSendable;
use crate::utils::ObsError;
use std::sync::Arc;

pub trait ObsSceneExtFilter {
    /// Adds a filter to the given source in this scene.
    fn add_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError>;

    /// Removes a filter from the this scene (internally removes the filter to the scene's source).
    fn remove_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError>;
}

impl ObsSceneExtFilter for ObsSceneRef {
    fn add_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError> {
        let source_ptr = self.get_scene_source_ptr()?;
        let filter_ptr = filter_ref.as_ptr();

        let mut guard = self.attached_filters.write().map_err(|_| {
            ObsError::LockError("Failed to acquire write lock on attached filters".into())
        })?;

        run_with_obs!(self.runtime, (source_ptr, filter_ptr), move || {
            unsafe {
                // Safety: Both source_ptr and filter_ptr are valid because of SmartPointers
                libobs::obs_source_filter_add(source_ptr.0, filter_ptr.get_ptr());
            };
        })?;

        let drop_guard = _ObsRemoveFilterOnDrop::new(
            // We are using a no-op drop guard, because we are keeping the actual scene alive in the additional variable field
            SmartPointerSendable::new(source_ptr.0, Arc::new(super::_NoOpDropGuard)),
            filter_ref.as_ptr(),
            Some(self.as_ptr()),
            self.runtime.clone(),
        );

        guard.push(ObsFilterGuardPair::new(
            filter_ref.clone(),
            Arc::new(drop_guard),
        ));

        Ok(())
    }

    fn remove_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError> {
        self.attached_filters
            .write()
            .map_err(|_| {
                ObsError::LockError("Failed to acquire write lock on attached filters".into())
            })?
            .retain(|f| {
                // Keep everything except this one filter
                f.get_inner().as_ptr().get_ptr() != filter_ref.as_ptr().get_ptr()
            });
        Ok(())
    }
}
