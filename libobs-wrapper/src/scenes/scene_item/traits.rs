use crate::scenes::{ObsSceneItemRef, ObsSceneRef, SceneItemTrait};
use crate::sources::{ObsSourceRef, ObsSourceTrait};
use crate::utils::{ObsError, SourceInfo};
use std::sync::Arc;

pub trait SceneItemExtSceneTrait {
    /// Adds the specified source to this scene. Returns a reference to the created scene item.
    /// You can use that SceneItemPtr to manipulate the source within this scene (position, scale, rotation, etc).
    fn add_source<T: ObsSourceTrait + Clone + 'static>(
        &mut self,
        source: T,
    ) -> Result<ObsSceneItemRef<T>, ObsError>;

    /// Creates and adds a source to this scene based on the given `SourceInfo`.
    /// Returns a reference to the created scene item, which internally holds the created source.
    fn add_and_create_source(
        &mut self,
        info: SourceInfo,
    ) -> Result<ObsSceneItemRef<ObsSourceRef>, ObsError>;

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
    ) -> Result<ObsSceneItemRef<T>, ObsError> {
        let scene_item = ObsSceneItemRef::new(self, source.clone(), self.runtime.clone())?;

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
    ) -> Result<ObsSceneItemRef<ObsSourceRef>, ObsError> {
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

        let removed = {
            let mut guard = self
                .attached_scene_items
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            let keys = guard
                .keys()
                .filter(|s| s.as_ptr().get_ptr() == source_ptr)
                .cloned()
                .collect::<Vec<_>>();
            keys.into_iter()
                .filter_map(|key| guard.remove_entry(&key))
                .collect::<Vec<_>>()
        };

        // Dropping scene-item guards can synchronously call into OBS and emit
        // item_remove signals. Never hold the map lock across that re-entrant work.
        drop(removed);
        Ok(())
    }

    fn remove_scene_item<K: SceneItemTrait>(&mut self, scene_item: K) -> Result<(), ObsError> {
        let scene_item_ptr = scene_item.as_ptr().get_ptr();
        let (removed_items, removed_entries) = {
            let mut guard = self
                .attached_scene_items
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            let mut removed_items = Vec::new();
            for items in guard.values_mut() {
                let mut index = 0;
                while index < items.len() {
                    if items[index].as_ptr().get_ptr() == scene_item_ptr {
                        removed_items.push(items.swap_remove(index));
                    } else {
                        index += 1;
                    }
                }
            }
            let empty_keys = guard
                .iter()
                .filter(|(_, items)| items.is_empty())
                .map(|(source, _)| source.clone())
                .collect::<Vec<_>>();
            let removed_entries = empty_keys
                .into_iter()
                .filter_map(|key| guard.remove_entry(&key))
                .collect::<Vec<_>>();
            (removed_items, removed_entries)
        };

        // See remove_every_item_of_source: OBS-backed values must be destroyed
        // after releasing the bookkeeping lock.
        drop(removed_items);
        drop(removed_entries);
        Ok(())
    }

    fn remove_all_sources(&mut self) -> Result<(), ObsError> {
        // Move the values out under the lock, then let their OBS drop guards run
        // only after the lock has been released.
        let removed = {
            let mut guard = self
                .attached_scene_items
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
            std::mem::take(&mut *guard)
        };
        drop(removed);

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
