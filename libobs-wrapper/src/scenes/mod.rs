mod transform_info;
pub use transform_info::*;

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::hash::Hash;
use std::ptr;
use std::sync::{Arc, RwLock};

use getters0::Getters;
use libobs::{obs_scene_item, obs_scene_t, obs_source_t, obs_transform_info, obs_video_info};

use crate::enums::ObsBoundsType;
use crate::macros::impl_eq_of_ptr;
use crate::sources::{ObsSourceTrait, ObsSourceTraitSealed};
use crate::unsafe_send::SendableComp;
use crate::{
    graphics::Vec2,
    impl_obs_drop, impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    sources::{ObsFilterRef, ObsSourceRef},
    unsafe_send::Sendable,
    utils::{ObsError, ObsString, SourceInfo},
};

#[derive(Debug)]
struct _SceneDropGuard {
    scene: Sendable<*mut obs_scene_t>,
    runtime: ObsRuntime,
}

impl_obs_drop!(_SceneDropGuard, (scene), move || unsafe {
    let scene_source = libobs::obs_scene_get_source(scene);

    for i in 0..libobs::MAX_CHANNELS {
        let current_source = libobs::obs_get_output_source(i);
        if current_source == scene_source {
            libobs::obs_set_output_source(i, ptr::null_mut());
        }

        libobs::obs_source_release(current_source);
    }

    libobs::obs_source_release(scene_source);
    libobs::obs_scene_release(scene);
});

type GeneralSetStorage<T> = Arc<RwLock<HashSet<Arc<Box<T>>>>>;

#[derive(Debug, Clone, Getters)]
#[skip_new]
pub struct ObsSceneRef {
    #[skip_getter]
    pub(crate) scene: Arc<Sendable<*mut obs_scene_t>>,
    name: ObsString,
    #[get_mut]
    pub(crate) sources: GeneralSetStorage<dyn ObsSourceTrait>,
    #[skip_getter]
    /// Maps the currently current active scenes by their channel (this is a shared reference between all scenes)
    pub(crate) active_scenes: Arc<RwLock<HashMap<u32, ObsSceneRef>>>,

    #[skip_getter]
    _guard: Arc<_SceneDropGuard>,

    #[skip_getter]
    runtime: ObsRuntime,

    pub(crate) signals: Arc<ObsSceneSignals>,
}

impl_eq_of_ptr!(ObsSceneRef, scene);

impl ObsSceneRef {
    pub(crate) fn new(
        name: ObsString,
        active_scenes: Arc<RwLock<HashMap<u32, ObsSceneRef>>>,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let name_ptr = name.as_ptr();
        let scene = run_with_obs!(runtime, (name_ptr), move || unsafe {
            Sendable(libobs::obs_scene_create(name_ptr))
        })?;

        let signals = Arc::new(ObsSceneSignals::new(&scene, runtime.clone())?);
        Ok(Self {
            name,
            scene: Arc::new(scene.clone()),
            sources: Arc::new(RwLock::new(HashSet::new())),
            active_scenes,
            _guard: Arc::new(_SceneDropGuard {
                scene,
                runtime: runtime.clone(),
            }),
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
            libobs::obs_set_output_source(channel, scene_source_ptr);
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
            .active_scenes
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
        run_with_obs!(self.runtime, (scene_ptr), move || unsafe {
            Sendable(libobs::obs_scene_get_source(scene_ptr))
        })
    }

    /// Adds and creates the specified source to this scene. Returns a reference to the created source. The source is also stored internally in this scene.
    ///
    /// If you need to remove the source later, use `remove_source`.
    pub fn add_source(&mut self, info: SourceInfo) -> Result<ObsSourceRef, ObsError> {
        let source = ObsSourceRef::new(
            info.id,
            info.name,
            info.settings,
            info.hotkey_data,
            self.runtime.clone(),
        )?;

        let scene_ptr = self.scene.clone();
        let source_ptr = source.source.clone();

        let ptr = run_with_obs!(self.runtime, (scene_ptr, source_ptr), move || unsafe {
            Sendable(libobs::obs_scene_add(scene_ptr, source_ptr))
        })?;

        if ptr.0.is_null() {
            return Err(ObsError::NullPointer);
        }

        //TODO We should clear one reference because with this obs doesn't clean up properly
        {
            // Keep this as is, we want to make sure that IF the lock is poisoned, we don't add the source
            let mut sources = self
                .sources
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            source.add_scene_item_ptr(SendableComp(self.scene.0), ptr.clone())?;
            sources.insert(Arc::new(Box::new(source.clone())));
        }
        Ok(source)
    }

    /// Gets a source by name from this scene. Returns None if no source with the given name exists in this scene.
    pub fn get_source_mut(
        &self,
        name: &str,
    ) -> Result<Option<Arc<Box<dyn ObsSourceTrait>>>, ObsError> {
        let r = self
            .sources
            .read()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .iter()
            .find(|s| s.name() == name)
            .cloned();

        Ok(r)
    }

    /// Removes the given source from this scene. Removes the corresponding scene item as well. It may be possible that this source is still added to another scene.
    pub fn remove_source<T: ObsSourceTrait>(&mut self, source: T) -> Result<(), ObsError> {
        let sendable_comp = SendableComp(self.scene.0);
        {
            let scene_item_ptr = source
                .get_scene_item_ptr(&sendable_comp)?
                .ok_or(ObsError::SourceNotFound)?;

            run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
                // Remove the scene item
                libobs::obs_sceneitem_remove(scene_item_ptr);
                // Release the scene item reference
                libobs::obs_sceneitem_release(scene_item_ptr);
            })?;
        }

        // We need to make sure to remove references from both the scene and the source
        self.sources
            .write()
            .map_err(|e| ObsError::LockError(format!("{:?}", e)))?
            .retain(|s| s.as_ptr().0 != source.as_ptr().0);

        source.remove_scene_item_ptr(sendable_comp)?;

        Ok(())
    }

    pub fn remove_all_sources(&mut self) -> Result<(), ObsError> {
        let sendable_comp = SendableComp(self.scene.0);
        let sources = {
            let mut sources_guard = self
                .sources
                .write()
                .map_err(|e| ObsError::LockError(format!("{:?}", e)))?;

            let sources: Vec<Arc<Box<dyn ObsSourceTrait>>> = sources_guard.drain().collect();

            sources
        };

        for source in sources {
            let scene_item_ptr = source
                .get_scene_item_ptr(&sendable_comp)?
                .ok_or(ObsError::SourceNotFound)?;

            run_with_obs!(self.runtime, (scene_item_ptr), move || unsafe {
                // Remove the scene item
                libobs::obs_sceneitem_remove(scene_item_ptr);
                // Release the scene item reference
                libobs::obs_sceneitem_release(scene_item_ptr);
            })?;

            source.remove_scene_item_ptr(sendable_comp.clone())?;
        }

        Ok(())
    }

    /// Adds a filter to the given source in this scene.
    pub fn add_scene_filter<T: ObsSourceTrait>(
        &self,
        source: &T,
        filter_ref: &ObsFilterRef,
    ) -> Result<(), ObsError> {
        let source_ptr = source.as_ptr();
        let filter_ptr = filter_ref.source.clone();
        run_with_obs!(self.runtime, (source_ptr, filter_ptr), move || unsafe {
            libobs::obs_source_filter_add(source_ptr, filter_ptr);
        })?;
        Ok(())
    }

    /// Removes a filter from the this scene (internally removes the filter to the scene's source).
    pub fn remove_scene_filter<T: ObsSourceTrait>(
        &self,
        source: &T,
        filter_ref: &ObsFilterRef,
    ) -> Result<(), ObsError> {
        let source_ptr = source.as_ptr();
        let filter_ptr = filter_ref.source.clone();
        run_with_obs!(self.runtime, (source_ptr, filter_ptr), move || unsafe {
            libobs::obs_source_filter_remove(source_ptr, filter_ptr);
        })?;
        Ok(())
    }

    /// Gets the underlying scene item pointer for the given source in this scene.
    ///
    /// A scene item is basically the representation of a source within this scene. It holds information about the position, scale, rotation, etc.
    pub fn get_scene_item_ptr<T: ObsSourceTrait>(
        &self,
        source: &T,
    ) -> Result<Sendable<*mut obs_scene_item>, ObsError> {
        let sendable_comp = SendableComp(self.scene.0);
        let scene_item_ptr = source
            .get_scene_item_ptr(&sendable_comp)?
            .ok_or(ObsError::SourceNotFound)?
            .clone();

        Ok(scene_item_ptr)
    }

    /// Gets the transform info of the given source in this scene.
    pub fn get_transform_info<T: ObsSourceTrait>(
        &self,
        source: &T,
    ) -> Result<ObsTransformInfo, ObsError> {
        let scene_item_ptr = self.get_scene_item_ptr(source)?;

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

    pub fn as_ptr(&self) -> Sendable<*mut obs_scene_t> {
        Sendable(self.scene.0)
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
