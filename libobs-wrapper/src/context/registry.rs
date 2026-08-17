use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::{
    data::{object::ObsObjectTrait, output::ObsOutputTrait},
    display::ObsDisplayRef,
    scenes::ObsSceneRef,
    sources::ObsFilterRef,
    utils::ObsError,
};

/// Owns all context-level native handles behind one internal seam.
///
/// Every stored handle also owns an `ObsRuntime` clone, so native lifetime no longer
/// depends on field declaration order inside `ObsContext`.
#[derive(Debug, Default)]
pub(super) struct ObjectRegistry {
    displays: RwLock<HashMap<usize, ObsDisplayRef>>,
    outputs: RwLock<Vec<Arc<dyn ObsOutputTrait>>>,
    scenes: RwLock<Vec<ObsSceneRef>>,
    filters: RwLock<Vec<ObsFilterRef>>,
}

impl ObjectRegistry {
    pub(super) fn add_display(&self, display: ObsDisplayRef) -> Result<(), ObsError> {
        self.displays
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire display registry".into()))?
            .insert(display.id(), display);
        Ok(())
    }

    pub(super) fn remove_display(&self, id: usize) -> Result<(), ObsError> {
        self.displays
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire display registry".into()))?
            .remove(&id);
        Ok(())
    }

    pub(super) fn display(&self, id: usize) -> Result<Option<ObsDisplayRef>, ObsError> {
        Ok(self
            .displays
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire display registry".into()))?
            .get(&id)
            .cloned())
    }

    pub(super) fn add_output<T>(&self, output: T) -> Result<(), ObsError>
    where
        T: ObsOutputTrait + 'static,
    {
        self.outputs
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire output registry".into()))?
            .push(Arc::new(output));
        Ok(())
    }

    pub(super) fn output(&self, name: &str) -> Result<Option<Arc<dyn ObsOutputTrait>>, ObsError> {
        Ok(self
            .outputs
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire output registry".into()))?
            .iter()
            .find(|output| output.name() == name)
            .cloned())
    }

    pub(super) fn any_output_active(&self) -> Result<bool, ObsError> {
        for output in self
            .outputs
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire output registry".into()))?
            .iter()
        {
            if output.is_active()? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn add_filter(&self, filter: ObsFilterRef) -> Result<(), ObsError> {
        self.filters
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire filter registry".into()))?
            .push(filter);
        Ok(())
    }

    pub(super) fn filter(&self, name: &str) -> Result<Option<ObsFilterRef>, ObsError> {
        Ok(self
            .filters
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire filter registry".into()))?
            .iter()
            .find(|filter| filter.name() == name)
            .cloned())
    }

    pub(super) fn add_scene(&self, scene: ObsSceneRef) -> Result<(), ObsError> {
        self.scenes
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire scene registry".into()))?
            .push(scene);
        Ok(())
    }

    pub(super) fn scene(&self, name: &str) -> Result<Option<ObsSceneRef>, ObsError> {
        Ok(self
            .scenes
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire scene registry".into()))?
            .iter()
            .find(|scene| scene.name() == name)
            .cloned())
    }
}
