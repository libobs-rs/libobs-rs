mod transform_info;
pub use transform_info::*;

mod scene_drop_guards;
mod scene_item;

pub use scene_item::SceneItemRef;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use libobs::{obs_scene_t, obs_source_t, obs_transform_info, obs_video_info};

use crate::data::object::ObsObjectTrait;
use crate::enums::ObsBoundsType;
use crate::macros::impl_eq_of_ptr;
use crate::scenes::scene_drop_guards::_SceneDropGuard;
use crate::sources::{ObsFilterGuardPair, ObsSourceTrait, _ObsRemoveFilterOnDrop};
use crate::unsafe_send::SmartPointerSendable;
use crate::utils::GeneralTraitHashMap;
use crate::{
    graphics::Vec2,
    impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    sources::{ObsFilterRef, ObsSourceRef},
    unsafe_send::Sendable,
    utils::{ObsError, ObsString, SourceInfo},
};

struct _NoOpDropGuard;
impl crate::utils::ObsDropGuard for _NoOpDropGuard {}

#[derive(Debug, Clone)]
pub struct ObsSceneRef {
    name: ObsString,
    global_active_scenes: Arc<RwLock<HashMap<u32, ObsSceneRef>>>,
    attached_scene_items: GeneralTraitHashMap<dyn ObsSourceTrait, Vec<SceneItemRef>>,
    attached_filters: Arc<RwLock<Vec<ObsFilterGuardPair>>>,
    runtime: ObsRuntime,
    signals: Arc<ObsSceneSignals>,
    scene: SmartPointerSendable<*mut obs_scene_t>,
}

impl_eq_of_ptr!(ObsSceneRef);

impl ObsSceneRef {
    pub(crate) fn new(
        name: ObsString,
        active_scenes: Arc<RwLock<HashMap<u32, ObsSceneRef>>>,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let scene = run_with_obs!(runtime, (name), move || unsafe {
            let name_ptr = name.as_ptr();

            let scene_ptr = libobs::obs_scene_create(name_ptr.0);
            if scene_ptr.is_null() {
                return Err(ObsError::NullPointer(None));
            }

            let source_ptr = libobs::obs_scene_get_source(scene_ptr);
            if source_ptr.is_null() {
                libobs::obs_scene_release(scene_ptr);
                return Err(ObsError::NullPointer(None));
            }

            Ok(Sendable(scene_ptr))
        })??;

        let drop_guard = Arc::new(_SceneDropGuard {
            scene: scene.clone(),
            runtime: runtime.clone(),
        });

        let scene = SmartPointerSendable::new(scene.0, drop_guard);
        let signals = Arc::new(ObsSceneSignals::new(&scene, runtime.clone())?);

        Ok(Self {
            name,
            scene,
            attached_scene_items: Arc::new(RwLock::new(HashMap::new())),
            attached_filters: Arc::new(RwLock::new(Vec::new())),
            global_active_scenes: active_scenes,
            runtime,
            signals,
        })
    }

    #[deprecated = "Use ObsSceneRef::set_to_channel instead"]
    pub fn add_and_set(&self, channel: u32) -> Result<(), ObsError> {
        self.set_to_channel(channel)
    }

    /// Sets this scene to a given output channel.
    /// There are 64
    /// channels that you can assign scenes to, which will draw on top of each
    /// other in ascending index order.
    pub fn set_to_channel(&self, channel: u32) -> Result<(), ObsError> {
        if channel >= libobs::MAX_CHANNELS {
            return Err(ObsError::InvalidOperation(format!(
                "Channel {} is out of bounds (max {})",
                channel,
                libobs::MAX_CHANNELS - 1
            )));
        }

        // let mut s = self
        //     .active_scenes
        //     .write()
        //     .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        // s.insert(channel, self.clone());

        let scene_source_ptr = self.get_scene_source_ptr()?;
        run_with_obs!(self.runtime, (scene_source_ptr), move || unsafe {
            libobs::obs_set_output_source(channel, scene_source_ptr.0);
        })
    }

    /// Removes a scene from a given output channel, for more info about channels see `set_to_channel`.
    pub fn remove_from_channel(&self, channel: u32) -> Result<(), ObsError> {
        if channel >= libobs::MAX_CHANNELS {
            return Err(ObsError::InvalidOperation(format!(
                "Channel {} is out of bounds (max {})",
                channel,
                libobs::MAX_CHANNELS - 1
            )));
        }

        let mut s = self
            .global_active_scenes
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        s.remove(&channel);

        run_with_obs!(self.runtime, (), move || unsafe {
            libobs::obs_set_output_source(channel, std::ptr::null_mut());
        })
    }

    /// Gets the underlying source pointer of this scene, which is used internally when setting it to a channel.
    pub fn get_scene_source_ptr(&self) -> Result<Sendable<*mut obs_source_t>, ObsError> {
        let scene_ptr = self.scene.clone();
        run_with_obs!(self.runtime, (scene_ptr), move || {
            unsafe {
                // Safety: We are in the runtime and the scene ptr must be valid because we are using a smart pointer
                Sendable(libobs::obs_scene_get_source(scene_ptr.get_ptr()))
            }
        })
    }

    /// Adds the specified source to this scene. Returns a reference to the created scene item.
    /// You can use that SceneItemPtr to manipulate the source within this scene (position, scale, rotation, etc).
    pub fn add_source<T: ObsSourceTrait + 'static>(
        &mut self,
        source: T,
    ) -> Result<SceneItemRef, ObsError> {
        let scene_ptr = self.scene.clone();
        let source_ptr = source.as_ptr();

        let ptr = run_with_obs!(self.runtime, (scene_ptr, source_ptr), move || {
            let ptr = unsafe {
                // Safety: Because we are using smart pointers for both scene and source, they are valid in this scope
                libobs::obs_scene_add(scene_ptr.get_ptr(), source_ptr.get_ptr())
            };

            if ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(ptr))
            }
        })??;

        let scene_item_ptr = SmartPointerSendable::new(
            ptr.0,
            Arc::new(_ObsSceneItemDropGuard {
                scene_item: ptr.clone(),
                runtime: self.runtime.clone(),
            }),
        );

        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .entry(Arc::new(Box::new(source)))
            .or_insert_with(Vec::new)
            .push(scene_item_ptr.clone());

        Ok(scene_item_ptr)
    }

    /// Adds and creates the specified source to this scene. Returns a reference to the created source. The source is also stored internally in this scene.
    ///
    /// If you need to remove the source later, use `remove_source` or if you addded multiple of the same source ,call `remove_scene_item` by using the corresponding SceneItemPtr.
    pub fn add_and_create_source(
        &mut self,
        info: SourceInfo,
    ) -> Result<(ObsSourceRef, SceneItemPtr), ObsError> {
        let source = ObsSourceRef::new(
            info.id,
            info.name,
            info.settings,
            info.hotkey_data,
            self.runtime.clone(),
        )?;

        let scene_item = self.add_source(source.clone())?;

        Ok((source, scene_item))
    }

    /// Gets a source by name from this scene. Returns None if no source with the given name exists in this scene.
    pub fn get_source_mut(
        &self,
        name: &str,
    ) -> Result<Option<Arc<Box<dyn ObsSourceTrait>>>, ObsError> {
        let r = self
            .attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .keys()
            .find(|s| s.name() == name)
            .cloned();

        Ok(r)
    }

    /// Removes the given source from this scene. Removes the corresponding scene item as well. It may be possible that this source is still added to another scene.
    pub fn remove_source<T: ObsSourceTrait>(&mut self, source: T) -> Result<(), ObsError> {
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

    pub fn remove_scene_item(&mut self, scene_item: SceneItemPtr) -> Result<(), ObsError> {
        let mut guard = self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        guard.iter_mut().for_each(|(_, items)| {
            items.retain(|item| item.get_ptr() != scene_item.get_ptr());
        });
        Ok(())
    }

    pub fn remove_all_sources(&mut self) -> Result<(), ObsError> {
        // Dropping the scene items is handled by the smart pointer drop guards
        self.attached_scene_items
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .clear();

        Ok(())
    }

    /// Adds a filter to the given source in this scene.
    pub fn add_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError> {
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

        guard.push(ObsFilterGuardPair {
            filter: filter_ref.clone(),
            guard: Arc::new(_ObsRemoveFilterOnDrop::new(
                // We are using a no-op drop guard, because we are keeping the actual scene alive in the additional variable field
                SmartPointerSendable::new(source_ptr.0, Arc::new(_NoOpDropGuard)),
                filter_ref.as_ptr(),
                self.as_ptr(),
                self.runtime.clone(),
            )),
        });

        Ok(())
    }

    /// Removes a filter from the this scene (internally removes the filter to the scene's source).
    pub fn remove_scene_filter(&self, filter_ref: &ObsFilterRef) -> Result<(), ObsError> {
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

    /// Gets the underlying scene item pointer for the given source in this scene.
    ///
    /// A scene item is basically the representation of a source within this scene. It holds information about the position, scale, rotation, etc.
    pub fn get_scene_item_ptr<T: ObsSourceTrait>(
        &self,
        source: &T,
    ) -> Result<Vec<SceneItemPtr>, ObsError> {
        let guard = self.attached_scene_items
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

        let res = guard    .iter()
            .find_map(|(s, scene_item_ptr)| {
                if s.as_ptr().get_ptr() == source.as_ptr().get_ptr() {
                    Some(scene_item_ptr.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(Vec::new);

        Ok(res)
    }

    /// Gets the transform info of the given source in this scene.
    pub fn get_transform_info(
        &self,
        scene_item: SceneItemPtr,
    ) -> Result<ObsTransformInfo, ObsError> {
        let item_info = run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            let mut item_info: obs_transform_info = std::mem::zeroed();
            libobs::obs_sceneitem_get_info2(scene_item_ptr, &mut item_info);
            ObsTransformInfo(item_info)
        })?;

        Ok(item_info)
    }

    /// Gets the position of the given source in this scene.
    pub fn get_source_position<T: ObsSourceTrait>(&self, source: &T) -> Result<Vec2, ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        let position = run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            let mut main_pos: libobs::vec2 = std::mem::zeroed();
            libobs::obs_sceneitem_get_pos(scene_item_ptr, &mut main_pos);
            Vec2::from(main_pos)
        })?;

        Ok(position)
    }

    /// Gets the scale of the given source in this scene.
    pub fn get_source_scale<T: ObsSourceTrait>(&self, source: &T) -> Result<Vec2, ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        let scale = run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            let mut main_pos: libobs::vec2 = std::mem::zeroed();
            libobs::obs_sceneitem_get_scale(scene_item_ptr, &mut main_pos);
            Vec2::from(main_pos)
        })?;

        Ok(scale)
    }

    /// Sets the position of the given source in this scene.
    pub fn set_source_position<T: ObsSourceTrait>(
        &self,
        source: &T,
        position: Vec2,
    ) -> Result<(), ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            libobs::obs_sceneitem_set_pos(scene_item_ptr, &position.into());
        })?;

        Ok(())
    }

    /// Sets the scale of the given source in this scene.
    pub fn set_source_scale<T: ObsSourceTrait>(
        &self,
        source: &T,
        scale: Vec2,
    ) -> Result<(), ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            libobs::obs_sceneitem_set_scale(scene_item_ptr, &scale.into());
        })?;

        Ok(())
    }

    /// Sets the transform info of the given source in this scene.
    /// The `ObsTransformInfo` can be built by using the `ObsTransformInfoBuilder`.
    pub fn set_transform_info<T: ObsSourceTrait>(
        &self,
        source: &T,
        info: &ObsTransformInfo,
    ) -> Result<(), ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        let item_info = Sendable(info.clone());
        run_with_obs!(self.runtime, (scene_item_ptr, item_info), move || unsafe {
            libobs::obs_sceneitem_set_info2(scene_item_ptr, &item_info.0);
        })?;

        Ok(())
    }

    /// Fits the given source to the screen size.
    /// If the source is locked, no action is taken.
    ///
    /// Returns `Ok(true)` if the source was resized, `Ok(false)` if the source was locked and not resized.
    pub fn fit_source_to_screen<T: ObsSourceTrait>(&self, source: &T) -> Result<bool, ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

        let is_locked = {
            run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
                libobs::obs_sceneitem_locked(scene_item_ptr)
            })?
        };

        if is_locked {
            return Ok(false);
        }

        let ovi = run_with_obs!(self.runtime, (), move || unsafe {
            let mut ovi = std::mem::MaybeUninit::<obs_video_info>::uninit();
            libobs::obs_get_video_info(ovi.as_mut_ptr());

            Sendable(ovi.assume_init())
        })?;

        let bounds_crop = run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
            libobs::obs_sceneitem_get_bounds_crop(scene_item_ptr)
        })?;

        // We are not constructing it from the source here because we want to reset full transform (so we use build instead of build_with_fallback)
        let item_info = ObsTransformInfoBuilder::new()
            .set_bounds_type(ObsBoundsType::ScaleInner)
            .set_crop_to_bounds(bounds_crop)
            .build(ovi.0.base_width, ovi.0.base_height);

        self.set_transform_info(source, &item_info)?;
        Ok(true)
    }

    pub fn as_ptr(&self) -> SmartPointerSendable<*mut obs_scene_t> {
        self.scene.clone()
    }
}

impl_signal_manager!(|scene_ptr| unsafe {
    let source_ptr = libobs::obs_scene_get_source(scene_ptr);

    libobs::obs_source_get_signal_handler(source_ptr)
}, ObsSceneSignals for ObsSceneRef<*mut obs_scene_t>, [
    "item_add": {
        struct ItemAddSignal {
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "item_remove": {
        struct ItemRemoveSignal {
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "reorder": {},
    "refresh": {},
    "item_visible": {
        struct ItemVisibleSignal {
            visible: bool;
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "item_locked": {
        struct ItemLockedSignal {
            locked: bool;
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "item_select": {
        struct ItemSelectSignal {
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "item_deselect": {
        struct ItemDeselectSignal {
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    },
    "item_transform": {
        struct ItemTransformSignal {
            POINTERS {
                item: *mut libobs::obs_sceneitem_t,
            }
        }
    }
]);
