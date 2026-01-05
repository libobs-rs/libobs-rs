mod traits;
pub use traits::SceneItemExtSceneTrait;

use std::{fmt::Debug, hash::Hash, sync::Arc};

use libobs::{obs_scene_item, obs_transform_info, obs_video_info};

use crate::{
    enums::ObsBoundsType,
    graphics::Vec2,
    impl_obs_drop,
    macros::trait_with_optional_send_sync,
    run_with_obs,
    runtime::ObsRuntime,
    scenes::{ObsSceneRef, ObsTransformInfo, ObsTransformInfoBuilder},
    sources::ObsSourceTrait,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObsDropGuard, ObsError},
};

#[derive(Debug)]
pub(super) struct _ObsSceneItemDropGuard {
    scene_item: Sendable<*mut obs_scene_item>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsSceneItemDropGuard {}
impl_obs_drop!(_ObsSceneItemDropGuard, (scene_item), move || unsafe {
    // Safety: The pointer is valid as long as we are in the runtime and the guard is alive.
    // Because scene item is attached to a scene, we first remove it from the scene and then release it.
    libobs::obs_sceneitem_remove(scene_item.0);
    libobs::obs_sceneitem_release(scene_item.0);
});

#[derive(Debug, Clone)]
pub struct ObsSceneItemRef<T: ObsSourceTrait + Clone> {
    underlying_source: T,
    scene_item_ptr: SmartPointerSendable<*mut obs_scene_item>,
    runtime: ObsRuntime,
}

impl<T: ObsSourceTrait + Clone> ObsSceneItemRef<T> {
    pub(crate) fn new(
        scene: &ObsSceneRef,
        source: T,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let scene_ptr = scene.as_ptr();
        let source_ptr = source.as_ptr();

        let scene_item_ptr = run_with_obs!(runtime, (scene_ptr, source_ptr), move || {
            let ptr = unsafe {
                // Safety: The pointers are valid as they are safe pointers
                libobs::obs_scene_add(scene_ptr.get_ptr(), source_ptr.get_ptr())
            };

            if ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                Ok(Sendable(ptr))
            }
        })??;

        let drop_guard = _ObsSceneItemDropGuard {
            scene_item: scene_item_ptr.clone(),
            runtime: runtime.clone(),
        };

        let scene_item_ptr = SmartPointerSendable::new(scene_item_ptr.0, Arc::new(drop_guard));

        Ok(Self {
            underlying_source: source,
            scene_item_ptr,
            runtime,
        })
    }
}

trait_with_optional_send_sync! {
    pub trait SceneItemTrait: Debug {
        fn as_ptr(&self) -> &SmartPointerSendable<*mut obs_scene_item>;
        fn runtime(&self) -> ObsRuntime;
        fn inner_source_dyn(&self) -> &dyn ObsSourceTrait;
        fn inner_source_dyn_mut(&mut self) -> &mut dyn ObsSourceTrait;

        /// Gets the transform info of the given source in this scene.
        fn get_transform_info(&self) -> Result<ObsTransformInfo, ObsError> {
            let self_ptr = self.as_ptr();
            let item_info = run_with_obs!(self.runtime(), (self_ptr), move || {
                let mut item_info: obs_transform_info = unsafe {
                    // Safety: this is safe to call because we are filling a struct with zeros
                    std::mem::zeroed()
                };
                unsafe {
                    // Safety: Fill the transform info struct with the data
                    libobs::obs_sceneitem_get_info2(self_ptr.get_ptr(), &mut item_info)
                };

                ObsTransformInfo(item_info)
            })?;

            Ok(item_info)
        }

        /// Gets the position of the given source in this scene.
        fn get_source_position(&self) -> Result<Vec2, ObsError> {
            let self_ptr = self.as_ptr();
            let position = run_with_obs!(self.runtime(), (self_ptr), move || {
                let main_pos = unsafe {
                    // Safety: this is safe to call because we a filling a struct with zeros
                    let mut main_pos: libobs::vec2 = std::mem::zeroed();

                    // Safety: Fill the vec2 struct with the position data
                    libobs::obs_sceneitem_get_pos(self_ptr.get_ptr(), &mut main_pos);

                    main_pos
                };

                Vec2::from(main_pos)
            })?;

            Ok(position)
        }

        /// Gets the scale of the given source in this scene.
        fn get_source_scale(&self) -> Result<Vec2, ObsError> {
            let self_ptr = self.as_ptr();
            let scale = run_with_obs!(self.runtime(), (self_ptr), move || {
                let main_pos = unsafe {
                    // Safety: this is safe to call because we a filling a struct with zeros
                    let mut main_pos: libobs::vec2 = std::mem::zeroed();

                    // Safety: Fill the vec2 struct with the scale data, this using the correct size
                    libobs::obs_sceneitem_get_scale(self_ptr.get_ptr(), &mut main_pos);

                    main_pos
                };

                Vec2::from(main_pos)
            })?;

            Ok(scale)
        }

        /// Sets the position of the given source in this scene.
        fn set_source_position(&self, position: Vec2) -> Result<(), ObsError> {
            let self_ptr = self.as_ptr();

            run_with_obs!(self.runtime(), (self_ptr), move || {
                let position: libobs::vec2 = position.into();

                unsafe {
                    // Safety: The pointer is valid as it is a safe pointer
                    libobs::obs_sceneitem_set_pos(self_ptr.get_ptr(), &position);
                }
            })?;

            Ok(())
        }

        /// Sets the transform info of the given source in this scene.
        /// The `ObsTransformInfo` can be built by using the `ObsTransformInfoBuilder`.
        fn set_transform_info(&self, info: &ObsTransformInfo) -> Result<(), ObsError> {
            let item_info = Sendable(info.clone());
            let self_ptr = self.as_ptr();

            run_with_obs!(self.runtime(), (self_ptr, item_info), move || {
                let item_info = item_info.0 .0;

                unsafe {
                    // Safety: The pointers are valid as they are safe pointers
                    libobs::obs_sceneitem_set_info2(self_ptr.get_ptr(), &item_info);
                }
            })?;

            Ok(())
        }

        /// Fits the given source to the screen size.
        /// If the source is locked, no action is taken.
        ///
        /// Returns `Ok(true)` if the source was resized, `Ok(false)` if the source was locked and not resized.
        fn fit_source_to_screen(&self) -> Result<bool, ObsError> {
            let self_ptr = self.as_ptr();
            let is_locked = {
                run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                    // Safety: The pointer is valid as it is a safe pointer
                    libobs::obs_sceneitem_locked(self_ptr.get_ptr())
                })?
            };

            if is_locked {
                return Ok(false);
            }

            let ovi = run_with_obs!(self.runtime(), (), move || {
                let mut ovi = std::mem::MaybeUninit::<obs_video_info>::uninit();
                let success = unsafe {
                    // Safety: This is safe because we are providing a valid pointer to be filled
                    libobs::obs_get_video_info(ovi.as_mut_ptr())
                };

                if success {
                    let res = unsafe {
                        // Safety: This is safe because libobs filled the pointer and returned success
                        ovi.assume_init()
                    };

                    Ok(Sendable(res))
                } else {
                    Err(ObsError::NullPointer(Some(
                        "Failed to get video info".to_string(),
                    )))
                }
            })??;

            let bounds_crop = run_with_obs!(self.runtime(), (self_ptr), move || {
                unsafe {
                    // Safety: The pointer is valid as it is a safe pointer
                    libobs::obs_sceneitem_get_bounds_crop(self_ptr.get_ptr())
                }
            })?;

            // We are not constructing it from the source here because we want to reset full transform (so we use build instead of build_with_fallback)
            let item_info = ObsTransformInfoBuilder::new()
                .set_bounds_type(ObsBoundsType::ScaleInner)
                .set_crop_to_bounds(bounds_crop)
                .build(ovi.0.base_width, ovi.0.base_height);

            self.set_transform_info(&item_info)?;
            Ok(true)
        }

        /// Sets the scale of the given source in this scene.
        fn set_source_scale(&self, scale: Vec2) -> Result<(), ObsError> {
            let self_ptr = self.as_ptr();

            run_with_obs!(self.runtime(), (self_ptr), move || {
                let scale: libobs::vec2 = scale.into();

                unsafe {
                    // Safety: The pointer is valid as it is a safe pointer
                    libobs::obs_sceneitem_set_scale(self_ptr.get_ptr(), &scale);
                }
            })?;

            Ok(())
        }
    }
}

impl<T: ObsSourceTrait + Clone> SceneItemTrait for ObsSceneItemRef<T> {
    fn as_ptr(&self) -> &SmartPointerSendable<*mut obs_scene_item> {
        &self.scene_item_ptr
    }

    fn runtime(&self) -> ObsRuntime {
        self.runtime.clone()
    }

    fn inner_source_dyn(&self) -> &dyn ObsSourceTrait {
        &self.underlying_source
    }

    fn inner_source_dyn_mut(&mut self) -> &mut dyn ObsSourceTrait {
        &mut self.underlying_source
    }
}

impl<T> ObsSceneItemRef<T>
where
    T: ObsSourceTrait + Clone,
{
    /// Returns a reference to the specific source type.
    pub fn inner_source(&self) -> &T {
        &self.underlying_source
    }

    /// Returns a reference to the specific source type.
    pub fn inner_source_mut(&mut self) -> &mut T {
        &mut self.underlying_source
    }
}

// The macro doesn't support generics yet, so we implement it manually
//impl_eq_of_ptr!(SceneItemRef<T>, scene_item_ptr);

impl<T: ObsSourceTrait + Clone> PartialEq for ObsSceneItemRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.scene_item_ptr.get_ptr() == other.scene_item_ptr.get_ptr()
    }
}

impl<T: ObsSourceTrait + Clone> Eq for ObsSceneItemRef<T> {}

impl<T: ObsSourceTrait + Clone> Hash for ObsSceneItemRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.scene_item_ptr.get_ptr().hash(state);
    }
}
