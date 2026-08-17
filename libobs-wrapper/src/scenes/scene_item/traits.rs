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
    fn get_source(&self, name: &str) -> Result<Option<Arc<dyn ObsSourceTrait>>, ObsError>;

    #[deprecated = "Use get_source; the returned shared handle was never a mutable reference"]
    fn get_source_mut(&self, name: &str) -> Result<Option<Arc<dyn ObsSourceTrait>>, ObsError> {
        self.get_source(name)
    }

    /// Removes the given source from this scene. Removes the corresponding scene item as well. It may be possible that this source is still added to another scene.
    fn remove_every_item_of_source<T: ObsSourceTrait>(&mut self, source: T)
        -> Result<(), ObsError>;

    /// Removes a specific scene item from this scene immediately.
    fn remove_item<K: SceneItemTrait + ?Sized>(&mut self, scene_item: &K) -> Result<(), ObsError>;

    #[deprecated = "Use remove_item; it removes the native scene item immediately without consuming the handle"]
    fn remove_scene_item<K: SceneItemTrait>(&mut self, scene_item: K) -> Result<(), ObsError> {
        self.remove_item(&scene_item)
    }

    /// Removes all sources from this scene.
    fn remove_all_sources(&mut self) -> Result<(), ObsError>;

    /// Gets the underlying scene item pointers for the given source in this scene.
    ///
    /// A scene item is basically the representation of a source within this scene. It holds information about the position, scale, rotation, etc.
    fn items_for_source<T: ObsSourceTrait + Clone>(
        &self,
        source: &T,
    ) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError>;

    #[deprecated = "Use items_for_source; scene item handles are managed objects, not raw pointers"]
    fn get_scene_item_ptr<T: ObsSourceTrait + Clone>(
        &self,
        source: &T,
    ) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
        self.items_for_source(source)
    }
}

impl SceneItemExtSceneTrait for ObsSceneRef {
    fn add_source<T: ObsSourceTrait + Clone + 'static>(
        &mut self,
        source: T,
    ) -> Result<ObsSceneItemRef<T>, ObsError> {
        self.runtime.ensure_same_runtime(source.runtime())?;
        let scene_item = ObsSceneItemRef::new(self, source.clone(), self.runtime.clone())?;

        let scene_clone = scene_item.clone();
        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .entry(source.__native_handle().native_id())
            .or_insert_with(|| (Arc::new(source), Vec::new()))
            .1
            .push(Arc::new(scene_clone));

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

    fn get_source(&self, name: &str) -> Result<Option<Arc<dyn ObsSourceTrait>>, ObsError> {
        let guard = self
            .attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        Ok(guard
            .values()
            .find(|(source, _)| source.name() == name)
            .map(|(source, _)| source.clone()))
    }

    fn remove_every_item_of_source<T: ObsSourceTrait>(
        &mut self,
        source: T,
    ) -> Result<(), ObsError> {
        self.runtime.ensure_same_runtime(source.runtime())?;
        let source_id = source.__native_handle().native_id();

        let mut guard = self
            .attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        let items = guard
            .get(&source_id)
            .map(|(_, items)| items.clone())
            .unwrap_or_default();
        for item in items {
            item.remove_from_scene()?;
        }
        guard.remove(&source_id);

        Ok(())
    }

    fn remove_item<K: SceneItemTrait + ?Sized>(&mut self, scene_item: &K) -> Result<(), ObsError> {
        self.runtime.ensure_same_runtime(&scene_item.runtime())?;
        // Group removal needs to inspect/prune this same registry in order to invalidate child
        // handles. Do not hold the registry lock while dispatching polymorphic removal behavior.
        scene_item.remove_from_scene()?;
        let mut guard = self
            .attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        guard.retain(|_, (_, items)| {
            items.retain(|item| {
                // Keep everything except this one scene item
                item.object_id() != scene_item.object_id()
            });
            // Remove the entry if no items remain
            !items.is_empty()
        });
        drop(guard);
        self.attached_groups
            .write()
            .map_err(|e| ObsError::LockError(format!("{e:?}")))?
            .retain(|group| group.object_id() != scene_item.object_id());
        Ok(())
    }

    fn remove_all_sources(&mut self) -> Result<(), ObsError> {
        let mut guard = self
            .attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;
        let items = guard
            .values()
            .flat_map(|(_, items)| items.iter().cloned())
            .collect::<Vec<_>>();
        for item in items {
            item.remove_from_scene()?;
        }
        guard.clear();

        Ok(())
    }

    fn items_for_source<T: ObsSourceTrait + Clone>(
        &self,
        source: &T,
    ) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
        self.runtime.ensure_same_runtime(source.runtime())?;
        let guard = self
            .attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        let res = guard
            .get(&source.__native_handle().native_id())
            .map(|(_, scene_items)| scene_items.clone())
            .unwrap_or_default();

        Ok(res)
    }
}
