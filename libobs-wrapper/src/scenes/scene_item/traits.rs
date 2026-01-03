use crate::scenes::{ObsSceneRef, SceneItemRef, SceneItemTrait};
use crate::sources::{ObsSourceRef, ObsSourceTrait};
use crate::utils::{ObsError, SourceInfo};
use std::sync::Arc;

pub trait SceneItemExtSceneTrait {
    /// Adds the specified source to this scene. Returns a reference to the created scene item.
    /// You can use that SceneItemPtr to manipulate the source within this scene (position, scale, rotation, etc).
    fn add_source<T: ObsSourceTrait + Clone + 'static>(
        &mut self,
        source: T,
    ) -> Result<SceneItemRef<T>, ObsError>;

    /// Creates and adds a source to this scene based on the given `SourceInfo`.
    /// Returns a reference to the created scene item, which internally holds the created source.
    fn add_and_create_source(
        &mut self,
        info: SourceInfo,
    ) -> Result<SceneItemRef<ObsSourceRef>, ObsError>;

    /// Gets a source by name from this scene. Returns None if no source with the given name exists in this scene.
    fn get_source_mut(&self, name: &str) -> Result<Option<Arc<Box<dyn ObsSourceTrait>>>, ObsError>;

    /// Removes the given source from this scene. Removes the corresponding scene item as well. It may be possible that this source is still added to another scene.
    fn remove_every_item_of_source<T: ObsSourceTrait>(&mut self, source: T)
        -> Result<(), ObsError>;

    /// Removes a specific scene item from this scene.
    fn remove_scene_item<K: SceneItemTrait>(&mut self, scene_item: K) -> Result<(), ObsError>;

    /// Removes all sources from this scene.
    fn remove_all_sources(&mut self) -> Result<(), ObsError>;

    /// Gets the underlying scene item pointers for the given source in this scene.
    ///
    /// A scene item is basically the representation of a source within this scene. It holds information about the position, scale, rotation, etc.
    fn get_scene_item_ptr<T: ObsSourceTrait + Clone>(
        &self,
        source: &T,
    ) -> Result<Vec<Arc<Box<dyn SceneItemTrait>>>, ObsError>;
}

impl SceneItemExtSceneTrait for ObsSceneRef {
    fn add_source<T: ObsSourceTrait + Clone + 'static>(
        &mut self,
        source: T,
    ) -> Result<SceneItemRef<T>, ObsError> {
        let scene_item = SceneItemRef::new(self, source.clone(), self.runtime.clone())?;

        let scene_clone = scene_item.clone();
        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .entry(Arc::new(Box::new(source)))
            .or_insert_with(Vec::new)
            .push(Arc::new(Box::new(scene_clone)));

        Ok(scene_item)
    }

    fn add_and_create_source(
        &mut self,
        info: SourceInfo,
    ) -> Result<SceneItemRef<ObsSourceRef>, ObsError> {
        let source = crate::sources::ObsSourceRef::new(
            info.id,
            info.name,
            info.settings,
            info.hotkey_data,
            self.runtime.clone(),
        )?;

        let scene_item = self.add_source(source.clone())?;
        Ok(scene_item)
    }

    fn get_source_mut(&self, name: &str) -> Result<Option<Arc<Box<dyn ObsSourceTrait>>>, ObsError> {
        let r = self
            .attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .keys()
            .find(|s| s.name() == name)
            .cloned();

        Ok(r)
    }

    fn remove_every_item_of_source<T: ObsSourceTrait>(
        &mut self,
        source: T,
    ) -> Result<(), ObsError> {
        let source_ptr = source.as_ptr().get_ptr();

        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .retain(|s, _| {
                //TODO: Maybe find a better way to utilize the HashMap's capabilities here
                s.as_ptr().get_ptr() != source_ptr
            });

        Ok(())
    }

    fn remove_scene_item<K: SceneItemTrait>(&mut self, scene_item: K) -> Result<(), ObsError> {
        let mut guard = self
            .attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        guard.retain(|_, items| {
            items.retain(|item| {
                // Keep everything except this one scene item
                item.as_ptr().get_ptr() != scene_item.as_ptr().get_ptr()
            });
            // Remove the entry if no items remain
            !items.is_empty()
        });
        Ok(())
    }

    fn remove_all_sources(&mut self) -> Result<(), ObsError> {
        // Dropping the scene items is handled by the smart pointer drop guards
        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .clear();

        Ok(())
    }

    fn get_scene_item_ptr<T: ObsSourceTrait + Clone>(
        &self,
        source: &T,
    ) -> Result<Vec<Arc<Box<dyn SceneItemTrait>>>, ObsError> {
        let guard = self
            .attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        let res = guard
            .iter()
            .find_map(|(s, scene_item_pointers)| {
                if s.as_ptr().get_ptr() == source.as_ptr().get_ptr() {
                    Some(scene_item_pointers.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(Vec::new);

        Ok(res)
    }
}
