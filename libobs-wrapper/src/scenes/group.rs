//! Managed OBS scene groups and native-order enumeration.

use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{Arc, Mutex, RwLock, Weak},
};

use libobs::obs_scene_item;

use crate::{
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{NativeObjectId, Sendable, SmartPointerSendable},
    utils::{ObsError, ObsString},
};

use super::{
    scene_item::{
        mark_scene_item_removed, scene_item_native_handle, wrap_owned_scene_item_ref,
        ObsSceneItemHandle, SceneItemTraitSealed,
    },
    ObsSceneRef, SceneItemTrait, SceneItemsBySource,
};

pub(super) type SceneGroups = Arc<RwLock<Vec<Arc<ObsSceneGroupRef>>>>;

/// Replacement mapping returned when native libobs ungrouping replaces a child scene item.
#[derive(Debug, Clone)]
pub struct ObsUngroupedItem {
    previous_object_id: NativeObjectId,
    item: Arc<dyn SceneItemTrait>,
}

impl ObsUngroupedItem {
    pub fn previous_object_id(&self) -> NativeObjectId {
        self.previous_object_id
    }

    pub fn item(&self) -> &Arc<dyn SceneItemTrait> {
        &self.item
    }

    pub fn into_item(self) -> Arc<dyn SceneItemTrait> {
        self.item
    }
}

/// A managed libobs group scene-item.
///
/// Groups behave like ordinary scene items for transforms/order, while their child items live in
/// libobs's dedicated group scene. Reparenting uses libobs's native group operations rather than
/// reconstructing transforms in Rust.
#[derive(Debug, Clone)]
pub struct ObsSceneGroupRef {
    name: ObsString,
    scene_item_ptr: SmartPointerSendable<*mut obs_scene_item>,
    removed: Arc<Mutex<bool>>,
    operation_lock: Arc<Mutex<()>>,
    runtime: ObsRuntime,
    _scene_ptr: SmartPointerSendable<*mut libobs::obs_scene_t>,
    items_by_source: SceneItemsBySource,
    groups: Weak<RwLock<Vec<Arc<ObsSceneGroupRef>>>>,
}

impl ObsSceneGroupRef {
    pub fn name(&self) -> &ObsString {
        &self.name
    }

    /// Moves an already-managed scene item into this group. libobs preserves its apparent
    /// canvas transform while changing parentage.
    pub fn add_item<T: SceneItemTrait + ?Sized>(&self, item: &T) -> Result<(), ObsError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        self.runtime.ensure_same_runtime(&item.runtime())?;
        if self.is_removed() || item.is_removed() {
            return Err(ObsError::InvalidOperation(
                "cannot group a removed scene item".into(),
            ));
        }
        let group = self.scene_item_ptr.clone();
        let item = scene_item_native_handle(item);
        run_with_obs!(self.runtime, (group, item), move || unsafe {
            // Safety: both managed item references remain alive for the actor call. Validate the
            // native parent relationship before asking libobs to reparent the item.
            if libobs::obs_sceneitem_is_group(item.get_ptr()) {
                return Err(ObsError::InvalidOperation(
                    "nested OBS scene groups are not supported by this managed group API".into(),
                ));
            }
            let parent = libobs::obs_sceneitem_get_scene(group.get_ptr());
            let item_parent = libobs::obs_sceneitem_get_scene(item.get_ptr());
            if parent.is_null() || item_parent != parent {
                return Err(ObsError::InvalidOperation(
                    "scene item must be a top-level item in the same scene as the group".into(),
                ));
            }
            libobs::obs_sceneitem_group_add_item(group.get_ptr(), item.get_ptr());
            Ok(())
        })?
    }

    /// Removes a child from this group and reparents it to the containing scene.
    pub fn remove_item<T: SceneItemTrait + ?Sized>(&self, item: &T) -> Result<(), ObsError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        self.runtime.ensure_same_runtime(&item.runtime())?;
        if self.is_removed() || item.is_removed() {
            return Err(ObsError::InvalidOperation(
                "cannot ungroup a removed scene item".into(),
            ));
        }
        let group = self.scene_item_ptr.clone();
        let item = scene_item_native_handle(item);
        run_with_obs!(self.runtime, (group, item), move || unsafe {
            // Safety: both handles remain alive. Validate membership because the underlying libobs
            // function trusts the caller and otherwise reparents an item from any scene/group.
            let group_scene = libobs::obs_sceneitem_group_get_scene(group.get_ptr());
            let item_parent = libobs::obs_sceneitem_get_scene(item.get_ptr());
            if group_scene.is_null() || item_parent != group_scene {
                return Err(ObsError::InvalidOperation(
                    "scene item is not a child of this group".into(),
                ));
            }
            libobs::obs_sceneitem_group_remove_item(group.get_ptr(), item.get_ptr());
            Ok(())
        })?
    }

    /// Returns group children in libobs's actual bottom-to-top native order.
    pub fn items_in_order(&self) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        self.items_in_order_unlocked()
    }

    fn items_in_order_unlocked(&self) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
        if self.is_removed() {
            return Ok(Vec::new());
        }
        let group = self.scene_item_ptr.clone();
        let addresses = run_with_obs!(self.runtime, (group), move || unsafe {
            let mut result = Vec::<usize>::new();
            // Safety: enumeration is synchronous and `result` remains live for all callbacks.
            libobs::obs_sceneitem_group_enum_items(
                group.get_ptr(),
                Some(collect_item_address),
                (&mut result as *mut Vec<usize>).cast::<c_void>(),
            );
            result
        })?;
        map_native_order(
            addresses,
            &self.items_by_source,
            self.groups.upgrade().as_ref(),
        )
    }

    /// Dissolves this group and returns replacement handles for every child.
    ///
    /// libobs implements ungrouping by creating new parent-scene items and copying each child into
    /// them. Consequently, existing child handles cannot remain the identity of the visible item.
    /// This method marks those old handles removed, rebinds the scene registry, and returns a
    /// mapping from each previous object ID to its replacement managed handle.
    pub fn ungroup(&self) -> Result<Vec<ObsUngroupedItem>, ObsError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        if self.is_removed() {
            return Ok(Vec::new());
        }

        let old_children = self.items_in_order_unlocked()?;
        let old_ids = old_children
            .iter()
            .map(|item| item.object_id())
            .collect::<Vec<_>>();

        // Acquire every fallible managed-state lock before mutating native state. Once libobs
        // dissolves the group there is no rollback operation that restores the old item identities.
        let mut registry = self
            .items_by_source
            .write()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        for old_id in &old_ids {
            let occurrences = registry
                .values()
                .flat_map(|(_, items)| items.iter())
                .filter(|item| item.object_id() == *old_id)
                .count();
            if occurrences != 1 {
                return Err(ObsError::InvalidOperation(format!(
                    "group child {old_id:?} has {occurrences} managed registry entries"
                )));
            }
        }

        let groups = self.groups.upgrade();
        let mut groups_guard = match groups.as_ref() {
            Some(groups) => Some(
                groups
                    .write()
                    .map_err(|error| ObsError::LockError(error.to_string()))?,
            ),
            None => None,
        };
        if let Some(groups) = groups_guard.as_ref() {
            if !groups
                .iter()
                .any(|group| group.object_id() == self.object_id())
            {
                return Err(ObsError::InvalidOperation(
                    "managed group is missing from its parent scene registry".into(),
                ));
            }
        }

        let before = native_scene_item_addresses(&self.runtime, self._scene_ptr.clone())?
            .into_iter()
            .collect::<HashSet<_>>();
        let group = self.scene_item_ptr.clone();
        let scene = self._scene_ptr.clone();
        let new_native = run_with_obs!(self.runtime, (group, scene, before), move || unsafe {
            // Safety: group and parent scene are held by managed leases. libobs creates replacement
            // parent items synchronously before returning. Take one explicit reference for each new
            // item before it leaves the actor call.
            libobs::obs_sceneitem_group_ungroup2(group.get_ptr(), true);

            let mut after = Vec::<usize>::new();
            libobs::obs_scene_enum_items(
                scene.get_ptr(),
                Some(collect_item_address),
                (&mut after as *mut Vec<usize>).cast::<c_void>(),
            );
            let mut replacements = Vec::new();
            for address in after {
                if before.contains(&address) {
                    continue;
                }
                let item = address as *mut libobs::obs_sceneitem_t;
                libobs::obs_sceneitem_addref(item);
                replacements.push(Sendable(item));
            }
            replacements
        })?;

        let replacement_count = new_native.len();
        let mut replacements = Vec::with_capacity(old_children.len().min(replacement_count));
        let mut new_native = new_native.into_iter();

        for old in &old_children {
            let previous_object_id = old.object_id();
            let Some(raw) = new_native.next() else {
                // Native ungroup already removed the old child. Make the managed graph reflect that
                // even if libobs unexpectedly failed to create a replacement.
                mark_scene_item_removed(old.as_ref());
                for (_, items) in registry.values_mut() {
                    items.retain(|item| item.object_id() != previous_object_id);
                }
                continue;
            };

            let replacement = Arc::new(ObsSceneItemHandle::from_owned_native_ref(
                raw,
                self._scene_ptr.clone(),
                self.runtime.clone(),
            ));
            let replacement_dyn: Arc<dyn SceneItemTrait> = replacement;
            for (_, items) in registry.values_mut() {
                for slot in items.iter_mut() {
                    if slot.object_id() == previous_object_id {
                        *slot = replacement_dyn.clone();
                    }
                }
            }
            mark_scene_item_removed(old.as_ref());
            replacements.push(ObsUngroupedItem {
                previous_object_id,
                item: replacement_dyn,
            });
        }
        registry.retain(|_, (_, items)| !items.is_empty());

        // More replacements than original children would indicate a broken libobs invariant. Do not
        // leak the explicit references we took above; the parent scene still owns the native items.
        let extras = new_native.collect::<Vec<_>>();
        if !extras.is_empty() {
            run_with_obs!(self.runtime, (extras), move || unsafe {
                // Safety: each pointer carries exactly one explicit reference acquired above.
                for item in extras {
                    libobs::obs_sceneitem_release(item.0);
                }
            })?;
        }

        {
            let mut removed = self
                .removed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *removed = true;
        }
        if let Some(groups) = groups_guard.as_mut() {
            groups.retain(|group| group.object_id() != self.object_id());
        }

        if replacement_count != old_children.len() {
            return Err(ObsError::Unexpected(format!(
                "OBS ungroup replaced {} children with {replacement_count} native items",
                old_children.len()
            )));
        }

        Ok(replacements)
    }
}

impl SceneItemTraitSealed for ObsSceneGroupRef {
    fn __native_handle(&self) -> &SmartPointerSendable<*mut obs_scene_item> {
        &self.scene_item_ptr
    }

    fn __removed_flag(&self) -> &Arc<Mutex<bool>> {
        &self.removed
    }
}

impl SceneItemTrait for ObsSceneGroupRef {
    fn runtime(&self) -> ObsRuntime {
        self.runtime.clone()
    }

    fn remove_from_scene(&self) -> Result<(), ObsError> {
        let _operation = self
            .operation_lock
            .lock()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        if self.is_removed() {
            return Ok(());
        }
        let children = self.items_in_order_unlocked()?;
        let removed_ids = children
            .iter()
            .map(|child| child.object_id())
            .collect::<HashSet<_>>();

        // Pre-acquire all fallible registry locks before native removal. Removing a group also
        // removes its group-scene children, so the managed registry must be updated atomically with
        // that irreversible native transition.
        let mut registry = self
            .items_by_source
            .write()
            .map_err(|error| ObsError::LockError(error.to_string()))?;
        for child_id in &removed_ids {
            if !registry
                .values()
                .flat_map(|(_, items)| items.iter())
                .any(|item| item.object_id() == *child_id)
            {
                return Err(ObsError::InvalidOperation(format!(
                    "group child {child_id:?} is missing from its parent scene registry"
                )));
            }
        }
        let groups = self.groups.upgrade();
        let mut groups_guard = match groups.as_ref() {
            Some(groups) => Some(
                groups
                    .write()
                    .map_err(|error| ObsError::LockError(error.to_string()))?,
            ),
            None => None,
        };

        let group = self.scene_item_ptr.clone();
        run_with_obs!(self.runtime, (group), move || unsafe {
            // Safety: the managed group owns an explicit native reference. Removing the parent
            // scene's ownership destroys its private group scene and detaches every child.
            libobs::obs_sceneitem_remove(group.get_ptr());
        })?;

        {
            let mut removed = self
                .removed
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *removed = true;
        }
        for child in &children {
            mark_scene_item_removed(child.as_ref());
        }
        registry.retain(|_, (_, items)| {
            items.retain(|item| !removed_ids.contains(&item.object_id()));
            !items.is_empty()
        });
        if let Some(groups) = groups_guard.as_mut() {
            groups.retain(|group| group.object_id() != self.object_id());
        }
        Ok(())
    }
}

impl ObsSceneRef {
    /// Creates an empty managed group at the top of this scene.
    pub fn create_group(
        &mut self,
        name: impl Into<ObsString>,
    ) -> Result<ObsSceneGroupRef, ObsError> {
        let name = name.into();
        let scene = self.as_ptr();
        let raw = run_with_obs!(self.runtime, (scene, name), move || unsafe {
            // Safety: scene/name remain valid for the actor call. A successful group belongs to the
            // scene; take one additional reference for the managed Rust handle.
            let item = libobs::obs_scene_add_group(scene.get_ptr(), name.as_ptr().0);
            if item.is_null() {
                return Err(ObsError::NullPointer(Some(
                    "OBS scene group creation".into(),
                )));
            }
            libobs::obs_sceneitem_addref(item);
            Ok(Sendable(item))
        })??;
        let (scene_item_ptr, removed) = wrap_owned_scene_item_ref(raw, self.runtime.clone());
        let group = ObsSceneGroupRef {
            name: name.clone(),
            scene_item_ptr,
            removed,
            operation_lock: Arc::new(Mutex::new(())),
            runtime: self.runtime.clone(),
            _scene_ptr: self.as_ptr(),
            items_by_source: self.attached_scene_items.clone(),
            groups: Arc::downgrade(&self.attached_groups),
        };
        self.attached_groups
            .write()
            .map_err(|error| ObsError::LockError(error.to_string()))?
            .push(Arc::new(group.clone()));
        Ok(group)
    }

    /// Returns all currently managed groups owned by this scene.
    pub fn groups(&self) -> Result<Vec<ObsSceneGroupRef>, ObsError> {
        Ok(self
            .attached_groups
            .read()
            .map_err(|error| ObsError::LockError(error.to_string()))?
            .iter()
            .filter(|group| !group.is_removed())
            .map(|group| (**group).clone())
            .collect())
    }

    /// Returns top-level scene items in libobs's actual bottom-to-top native order. Items inside a
    /// group are enumerated by [`ObsSceneGroupRef::items_in_order`] instead.
    pub fn items_in_order(&self) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
        let addresses = native_scene_item_addresses(&self.runtime, self.as_ptr())?;
        map_native_order(
            addresses,
            &self.attached_scene_items,
            Some(&self.attached_groups),
        )
    }

    pub(super) fn clear_groups(&mut self) -> Result<(), ObsError> {
        let groups = self
            .attached_groups
            .read()
            .map_err(|error| ObsError::LockError(error.to_string()))?
            .clone();
        for group in groups {
            group.remove_from_scene()?;
        }
        self.attached_groups
            .write()
            .map_err(|error| ObsError::LockError(error.to_string()))?
            .clear();
        Ok(())
    }
}

fn native_scene_item_addresses(
    runtime: &ObsRuntime,
    scene: SmartPointerSendable<*mut libobs::obs_scene_t>,
) -> Result<Vec<usize>, ObsError> {
    run_with_obs!(runtime, (scene), move || unsafe {
        let mut result = Vec::<usize>::new();
        // Safety: enumeration is synchronous and the vector outlives every callback.
        libobs::obs_scene_enum_items(
            scene.get_ptr(),
            Some(collect_item_address),
            (&mut result as *mut Vec<usize>).cast::<c_void>(),
        );
        result
    })
}

/// # Safety
/// `param` must point to a live `Vec<usize>` for the full synchronous libobs enumeration.
/// `item` must be the callback-borrowed scene item supplied by libobs for that invocation.
unsafe extern "C" fn collect_item_address(
    _scene: *mut libobs::obs_scene_t,
    item: *mut libobs::obs_sceneitem_t,
    param: *mut c_void,
) -> bool {
    // Safety: callers pass a live Vec<usize> for the complete synchronous enumeration.
    if let Some(result) = unsafe { param.cast::<Vec<usize>>().as_mut() } {
        result.push(item as usize);
        true
    } else {
        false
    }
}

fn map_native_order(
    addresses: Vec<usize>,
    items_by_source: &SceneItemsBySource,
    groups: Option<&SceneGroups>,
) -> Result<Vec<Arc<dyn SceneItemTrait>>, ObsError> {
    let mut managed = HashMap::<usize, Arc<dyn SceneItemTrait>>::new();
    for (_, items) in items_by_source
        .read()
        .map_err(|error| ObsError::LockError(error.to_string()))?
        .values()
    {
        for item in items {
            if item.is_removed() {
                continue;
            }
            let handle = scene_item_native_handle(item.as_ref());
            // SAFETY: the address is used only as an ephemeral identity key while the managed
            // handle remains alive; it is never dereferenced outside the actor.
            managed.insert(unsafe { handle.raw_ptr_unchecked() } as usize, item.clone());
        }
    }
    if let Some(groups) = groups {
        for group in groups
            .read()
            .map_err(|error| ObsError::LockError(error.to_string()))?
            .iter()
        {
            if group.is_removed() {
                continue;
            }
            let handle = scene_item_native_handle(group.as_ref());
            // SAFETY: the address is only an ephemeral lookup key; the managed group handle keeps
            // the native item alive for the complete mapping operation and the pointer is not dereferenced.
            managed.insert(
                unsafe { handle.raw_ptr_unchecked() } as usize,
                group.clone() as Arc<dyn SceneItemTrait>,
            );
        }
    }

    let mut ordered = Vec::with_capacity(addresses.len());
    for address in addresses {
        let Some(item) = managed.get(&address) else {
            return Err(ObsError::InvalidOperation(format!(
                "scene contains unmanaged native item at 0x{address:x}"
            )));
        };
        ordered.push(item.clone());
    }
    Ok(ordered)
}
