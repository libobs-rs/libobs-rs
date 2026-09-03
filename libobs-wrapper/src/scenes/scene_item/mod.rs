//! Scene items essentially hold the transform information and the source in a scene itself.
//! They are specific to the scene which they were created in.
mod traits;
pub use traits::SceneItemExtSceneTrait;

use std::{
    fmt::Debug,
    hash::Hash,
    sync::{Arc, Mutex},
};

use libobs::{obs_scene_item, obs_transform_info, obs_video_info};
use num_traits::FromPrimitive;

use crate::{
    enums::{
        ObsAlignment, ObsBlendMethod, ObsBlendMode, ObsBoundsType, ObsOrderMovement, ObsScaleType,
    },
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

/// Pixel crop applied to a scene item before its transform is evaluated.
///
/// Values map directly to libobs's left/top/right/bottom scene-item crop. Positive values
/// remove pixels from the corresponding edge of the underlying source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ObsSceneItemCrop {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ObsSceneItemCrop {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl From<ObsSceneItemCrop> for libobs::obs_sceneitem_crop {
    fn from(value: ObsSceneItemCrop) -> Self {
        Self {
            left: value.left,
            top: value.top,
            right: value.right,
            bottom: value.bottom,
        }
    }
}

impl From<libobs::obs_sceneitem_crop> for ObsSceneItemCrop {
    fn from(value: libobs::obs_sceneitem_crop) -> Self {
        Self::new(value.left, value.top, value.right, value.bottom)
    }
}

/// Complete render-transform state for one scene item. Unlike [`ObsTransformInfo`], this also
/// includes edge crop, scaling filter, and blending state stored outside `obs_transform_info`.
#[derive(Debug, Clone, PartialEq)]
pub struct ObsSceneItemTransformSnapshot {
    pub position: Vec2,
    pub scale: Vec2,
    pub alignment: ObsAlignment,
    pub rotation: f32,
    pub bounds: Vec2,
    pub bounds_type: ObsBoundsType,
    pub bounds_alignment: ObsAlignment,
    pub crop_to_bounds: bool,
    pub crop: ObsSceneItemCrop,
    pub scale_filter: ObsScaleType,
    pub blend_method: ObsBlendMethod,
    pub blend_mode: ObsBlendMode,
}

/// Restorable scene-item state including transform, visibility, locking and native z-order.
#[derive(Debug, Clone, PartialEq)]
pub struct ObsSceneItemStateSnapshot {
    pub transform: ObsSceneItemTransformSnapshot,
    pub visible: bool,
    pub locked: bool,
    pub order_position: i32,
}

#[derive(Debug)]
pub(super) struct _ObsSceneItemDropGuard {
    scene_item: Sendable<*mut obs_scene_item>,
    removed: Arc<Mutex<bool>>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsSceneItemDropGuard {}
impl_obs_drop!(
    _ObsSceneItemDropGuard,
    (scene_item, removed),
    move || unsafe {
        // Safety: construction takes an explicit native scene-item reference. Remove the item from
        // its scene at most once, then release the reference owned by this managed handle.
        let mut removed = removed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !*removed {
            libobs::obs_sceneitem_remove(scene_item.0);
            *removed = true;
        }
        libobs::obs_sceneitem_release(scene_item.0);
    }
);

/// Wraps one explicit native scene-item reference in the managed lifetime lease used by all
/// scene-item handle types. The caller must already own one `obs_sceneitem_addref` reference.
pub(super) fn wrap_owned_scene_item_ref(
    scene_item: Sendable<*mut obs_scene_item>,
    runtime: ObsRuntime,
) -> (SmartPointerSendable<*mut obs_scene_item>, Arc<Mutex<bool>>) {
    let removed = Arc::new(Mutex::new(false));
    let drop_guard = _ObsSceneItemDropGuard {
        scene_item: scene_item.clone(),
        removed: removed.clone(),
        runtime: runtime.clone(),
    };
    let handle = SmartPointerSendable::new(
        scene_item.0,
        Arc::new(drop_guard),
        runtime.native_registry(),
    );
    (handle, removed)
}

pub(super) fn scene_item_native_handle<T: SceneItemTrait + ?Sized>(
    item: &T,
) -> SmartPointerSendable<*mut obs_scene_item> {
    item.__native_handle().clone()
}

pub(super) fn mark_scene_item_removed<T: SceneItemTrait + ?Sized>(item: &T) {
    let mut removed = item
        .__removed_flag()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *removed = true;
}

#[derive(Debug, Clone)]
/// Holds the specific source that was added to the scene and its scene item.
/// If this struct is attached to the scene, it'll not be dropped as the scene
/// internally stores this struct, thus the source will also not be dropped.
pub struct ObsSceneItemRef<T: ObsSourceTrait + Clone> {
    // Drop the scene item first...
    scene_item_ptr: SmartPointerSendable<*mut obs_scene_item>,
    removed: Arc<Mutex<bool>>,
    runtime: ObsRuntime,
    // Then the scene
    // Note: Ideally, we'd want to keep the whole ObsScene struct, however
    // that would lead to a circular dependency, meaning that this SceneItem / the scene
    // would never be dropped. Because the only argument the scene takes is its name
    // and there are no settings attached to it, it's safe to only have a SmartPointer
    // and not the full scene
    _scene_ptr: SmartPointerSendable<*mut libobs::obs_scene>,

    // And at last the source
    underlying_source: T,
}

/// Type-erased managed scene-item handle used when libobs itself replaces a native item, such as
/// during group ungrouping. Ordinary source insertion still returns [`ObsSceneItemRef<T>`].
#[derive(Debug, Clone)]
pub struct ObsSceneItemHandle {
    scene_item_ptr: SmartPointerSendable<*mut obs_scene_item>,
    removed: Arc<Mutex<bool>>,
    runtime: ObsRuntime,
    _scene_ptr: SmartPointerSendable<*mut libobs::obs_scene_t>,
}

impl ObsSceneItemHandle {
    pub(super) fn from_owned_native_ref(
        item: Sendable<*mut obs_scene_item>,
        scene_ptr: SmartPointerSendable<*mut libobs::obs_scene_t>,
        runtime: ObsRuntime,
    ) -> Self {
        let (scene_item_ptr, removed) = wrap_owned_scene_item_ref(item, runtime.clone());
        Self {
            scene_item_ptr,
            removed,
            runtime,
            _scene_ptr: scene_ptr,
        }
    }
}

impl<T: ObsSourceTrait + Clone> ObsSceneItemRef<T> {
    pub(crate) fn new(
        scene: &ObsSceneRef,
        source: T,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        runtime.ensure_same_runtime(source.runtime())?;
        runtime.ensure_same_runtime(&scene.runtime)?;
        let scene_ptr = scene.as_ptr();
        let source_ptr = source.__native_handle();

        let scene_item_ptr = run_with_obs!(runtime, (scene_ptr, source_ptr), move || {
            let ptr = unsafe {
                // Safety: The pointers are valid as they are safe pointers
                libobs::obs_scene_add(scene_ptr.get_ptr(), source_ptr.get_ptr())
            };

            if ptr.is_null() {
                Err(ObsError::NullPointer(None))
            } else {
                unsafe {
                    // Safety: `ptr` is a live item returned by obs_scene_add. The managed handle
                    // owns this added reference until its drop guard releases it.
                    libobs::obs_sceneitem_addref(ptr);
                }
                Ok(Sendable(ptr))
            }
        })??;

        let removed = Arc::new(Mutex::new(false));
        let drop_guard = _ObsSceneItemDropGuard {
            scene_item: scene_item_ptr.clone(),
            removed: removed.clone(),
            runtime: runtime.clone(),
        };

        let scene_item_ptr = SmartPointerSendable::new(
            scene_item_ptr.0,
            Arc::new(drop_guard),
            runtime.native_registry(),
        );

        Ok(Self {
            underlying_source: source,
            _scene_ptr: scene.as_ptr().clone(),
            scene_item_ptr,
            removed,
            runtime,
        })
    }
}

trait_with_optional_send_sync! {
    pub(crate) trait SceneItemTraitSealed: Debug {
        fn __native_handle(&self) -> &SmartPointerSendable<*mut obs_scene_item>;
        fn __removed_flag(&self) -> &Arc<Mutex<bool>>;
    }
}

trait_with_optional_send_sync! {
    #[allow(private_bounds)]
    pub trait SceneItemTrait: SceneItemTraitSealed {
        fn runtime(&self) -> ObsRuntime;

        fn object_id(&self) -> crate::unsafe_send::NativeObjectId {
            self.__native_handle().native_id()
        }

        /// Removes this item from its scene immediately while keeping the typed handle valid
        /// until its final Rust reference is dropped. Repeated calls are harmless.
        fn remove_from_scene(&self) -> Result<(), ObsError> {
            let mut removed = self
                .__removed_flag()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if *removed {
                return Ok(());
            }
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item owns a native reference, so removal cannot invalidate
                // the pointer until the drop guard later releases that reference.
                libobs::obs_sceneitem_remove(self_ptr.get_ptr());
            })?;
            *removed = true;
            Ok(())
        }

        /// Whether this item has been explicitly removed from its scene.
        fn is_removed(&self) -> bool {
            *self
                .__removed_flag()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        /// Gets the transform info of the given source in this scene.
        fn get_transform_info(&self) -> Result<ObsTransformInfo, ObsError> {
            let self_ptr = self.__native_handle().clone();
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
            let self_ptr = self.__native_handle().clone();
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
            let self_ptr = self.__native_handle().clone();
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
            let self_ptr = self.__native_handle().clone();

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
            let self_ptr = self.__native_handle().clone();

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
            let self_ptr = self.__native_handle().clone();
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

        /// Returns whether the item is visible.
        fn is_visible(&self) -> Result<bool, ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_visible(self_ptr.get_ptr())
            })
        }

        /// Returns the explicit pixel crop applied to this scene item.
        fn crop(&self) -> Result<ObsSceneItemCrop, ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call. The
                // output structure is initialized even if libobs unexpectedly declines to write.
                let mut crop = libobs::obs_sceneitem_crop {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                libobs::obs_sceneitem_get_crop(self_ptr.get_ptr(), &mut crop);
                ObsSceneItemCrop::from(crop)
            })
        }

        /// Replaces the explicit left/top/right/bottom pixel crop for this scene item.
        /// Negative edge values are clamped to zero by libobs.
        fn set_crop(&self, crop: ObsSceneItemCrop) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call and the
                // converted crop value lives for the complete native setter invocation.
                let crop: libobs::obs_sceneitem_crop = crop.into();
                libobs::obs_sceneitem_set_crop(self_ptr.get_ptr(), &crop);
            })
        }

        /// Shows or hides the item.
        fn set_visible(&self, visible: bool) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                let _ = libobs::obs_sceneitem_set_visible(self_ptr.get_ptr(), visible);
            })
        }

        /// Returns whether the item transform is locked.
        fn is_locked(&self) -> Result<bool, ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_locked(self_ptr.get_ptr())
            })
        }

        /// Locks or unlocks the item transform.
        fn set_locked(&self, locked: bool) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                let _ = libobs::obs_sceneitem_set_locked(self_ptr.get_ptr(), locked);
            })
        }

        /// Returns the clockwise rotation in degrees.
        fn rotation(&self) -> Result<f32, ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_get_rot(self_ptr.get_ptr())
            })
        }

        /// Sets the clockwise rotation in degrees.
        fn set_rotation(&self, degrees: f32) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_set_rot(self_ptr.get_ptr(), degrees);
            })
        }

        /// Returns the item's zero-based order position within its scene.
        fn order_position(&self) -> Result<i32, ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_get_order_position(self_ptr.get_ptr())
            })
        }

        /// Moves the item to an absolute order position within its scene.
        fn set_order_position(&self, position: i32) -> Result<(), ObsError> {
            if position < 0 {
                return Err(ObsError::InvalidOperation(
                    "Scene item order position cannot be negative".to_string(),
                ));
            }
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_set_order_position(self_ptr.get_ptr(), position);
            })
        }

        /// Moves the item relative to the other items in its scene.
        fn move_order(&self, movement: ObsOrderMovement) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item retains a native reference for the actor call.
                libobs::obs_sceneitem_set_order(self_ptr.get_ptr(), movement as libobs::obs_order_movement);
            })
        }

        /// Returns the scaling filter used when this item is resized.
        fn scale_filter(&self) -> Result<ObsScaleType, ObsError> {
            let self_ptr = self.__native_handle().clone();
            let raw = run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed scene-item reference remains alive for the actor call.
                libobs::obs_sceneitem_get_scale_filter(self_ptr.get_ptr())
            })?;
            ObsScaleType::from_i64(raw as i64).ok_or_else(|| {
                ObsError::EnumConversionError(format!("unknown scene-item scale filter {raw}"))
            })
        }

        fn set_scale_filter(&self, filter: ObsScaleType) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item remains alive and `filter` is a libobs enum value.
                libobs::obs_sceneitem_set_scale_filter(
                    self_ptr.get_ptr(),
                    filter as libobs::obs_scale_type,
                );
            })
        }

        fn blend_method(&self) -> Result<ObsBlendMethod, ObsError> {
            let self_ptr = self.__native_handle().clone();
            let raw = run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed scene-item reference remains alive for the actor call.
                libobs::obs_sceneitem_get_blending_method(self_ptr.get_ptr())
            })?;
            ObsBlendMethod::from_i64(raw as i64).ok_or_else(|| {
                ObsError::EnumConversionError(format!("unknown scene-item blend method {raw}"))
            })
        }

        fn set_blend_method(&self, method: ObsBlendMethod) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item remains alive and `method` is a libobs enum value.
                libobs::obs_sceneitem_set_blending_method(
                    self_ptr.get_ptr(),
                    method as libobs::obs_blending_method,
                );
            })
        }

        fn blend_mode(&self) -> Result<ObsBlendMode, ObsError> {
            let self_ptr = self.__native_handle().clone();
            let raw = run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed scene-item reference remains alive for the actor call.
                libobs::obs_sceneitem_get_blending_mode(self_ptr.get_ptr())
            })?;
            ObsBlendMode::from_i64(raw as i64).ok_or_else(|| {
                ObsError::EnumConversionError(format!("unknown scene-item blend mode {raw}"))
            })
        }

        fn set_blend_mode(&self, mode: ObsBlendMode) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();
            run_with_obs!(self.runtime(), (self_ptr), move || unsafe {
                // Safety: the managed item remains alive and `mode` is a libobs enum value.
                libobs::obs_sceneitem_set_blending_mode(
                    self_ptr.get_ptr(),
                    mode as libobs::obs_blending_type,
                );
            })
        }

        /// Captures every transform-affecting scene-item property exposed by libobs.
        fn transform_snapshot(&self) -> Result<ObsSceneItemTransformSnapshot, ObsError> {
            let info = self.get_transform_info()?;
            let bounds_type = info.get_bounds_type().ok_or_else(|| {
                ObsError::EnumConversionError("unknown scene-item bounds type".into())
            })?;
            Ok(ObsSceneItemTransformSnapshot {
                position: info.get_pos(),
                scale: info.get_scale(),
                alignment: ObsAlignment::from_bits_retain(info.get_alignment()),
                rotation: info.get_rot(),
                bounds: info.get_bounds(),
                bounds_type,
                bounds_alignment: ObsAlignment::from_bits_retain(info.get_bounds_alignment()),
                crop_to_bounds: info.get_crop_to_bounds(),
                crop: self.crop()?,
                scale_filter: self.scale_filter()?,
                blend_method: self.blend_method()?,
                blend_mode: self.blend_mode()?,
            })
        }

        /// Restores a transform snapshot through the same managed actor seam as individual setters.
        fn apply_transform_snapshot(
            &self,
            snapshot: &ObsSceneItemTransformSnapshot,
        ) -> Result<(), ObsError> {
            let info = ObsTransformInfoBuilder::new()
                .set_pos(snapshot.position)
                .set_scale(snapshot.scale)
                .set_alignment(snapshot.alignment)
                .set_rot(snapshot.rotation)
                .set_bounds(snapshot.bounds)
                .set_bounds_type(snapshot.bounds_type)
                .set_bounds_alignment(snapshot.bounds_alignment)
                .set_crop_to_bounds(snapshot.crop_to_bounds)
                .build_with_fallback(self)?;
            self.set_transform_info(&info)?;
            self.set_crop(snapshot.crop)?;
            self.set_scale_filter(snapshot.scale_filter)?;
            self.set_blend_method(snapshot.blend_method)?;
            self.set_blend_mode(snapshot.blend_mode)
        }

        fn state_snapshot(&self) -> Result<ObsSceneItemStateSnapshot, ObsError> {
            Ok(ObsSceneItemStateSnapshot {
                transform: self.transform_snapshot()?,
                visible: self.is_visible()?,
                locked: self.is_locked()?,
                order_position: self.order_position()?,
            })
        }

        fn apply_state_snapshot(
            &self,
            snapshot: &ObsSceneItemStateSnapshot,
        ) -> Result<(), ObsError> {
            self.apply_transform_snapshot(&snapshot.transform)?;
            self.set_visible(snapshot.visible)?;
            self.set_locked(snapshot.locked)?;
            self.set_order_position(snapshot.order_position)
        }

        /// Preferred shorthand for [`SceneItemTrait::get_source_position`].
        fn position(&self) -> Result<Vec2, ObsError> {
            self.get_source_position()
        }

        /// Preferred shorthand for [`SceneItemTrait::set_source_position`].
        fn set_position(&self, position: Vec2) -> Result<(), ObsError> {
            self.set_source_position(position)
        }

        /// Preferred shorthand for [`SceneItemTrait::get_source_scale`].
        fn scale(&self) -> Result<Vec2, ObsError> {
            self.get_source_scale()
        }

        /// Preferred shorthand for [`SceneItemTrait::set_source_scale`].
        fn set_scale(&self, scale: Vec2) -> Result<(), ObsError> {
            self.set_source_scale(scale)
        }

        /// Sets the scale of the given source in this scene.
        fn set_source_scale(&self, scale: Vec2) -> Result<(), ObsError> {
            let self_ptr = self.__native_handle().clone();

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

impl SceneItemTraitSealed for ObsSceneItemHandle {
    fn __native_handle(&self) -> &SmartPointerSendable<*mut obs_scene_item> {
        &self.scene_item_ptr
    }

    fn __removed_flag(&self) -> &Arc<Mutex<bool>> {
        &self.removed
    }
}

impl SceneItemTrait for ObsSceneItemHandle {
    fn runtime(&self) -> ObsRuntime {
        self.runtime.clone()
    }
}

impl<T: ObsSourceTrait + Clone> SceneItemTraitSealed for ObsSceneItemRef<T> {
    fn __native_handle(&self) -> &SmartPointerSendable<*mut obs_scene_item> {
        &self.scene_item_ptr
    }

    fn __removed_flag(&self) -> &Arc<Mutex<bool>> {
        &self.removed
    }
}

impl<T: ObsSourceTrait + Clone> SceneItemTrait for ObsSceneItemRef<T> {
    fn runtime(&self) -> ObsRuntime {
        self.runtime.clone()
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
        self.scene_item_ptr.native_id() == other.scene_item_ptr.native_id()
    }
}

impl<T: ObsSourceTrait + Clone> Eq for ObsSceneItemRef<T> {}

impl<T: ObsSourceTrait + Clone> Hash for ObsSceneItemRef<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.scene_item_ptr.native_id().hash(state);
    }
}

#[cfg(test)]
mod crop_tests {
    use super::ObsSceneItemCrop;

    #[test]
    fn obs_scene_item_crop_round_trips_native_layout() {
        let crop = ObsSceneItemCrop::new(1, 2, 3, 4);
        let native: libobs::obs_sceneitem_crop = crop.into();
        assert_eq!(native.left, 1);
        assert_eq!(native.top, 2);
        assert_eq!(native.right, 3);
        assert_eq!(native.bottom, 4);
        assert_eq!(ObsSceneItemCrop::from(native), crop);
    }

    #[test]
    fn obs_scene_item_crop_default_is_uncropped() {
        assert_eq!(
            ObsSceneItemCrop::default(),
            ObsSceneItemCrop::new(0, 0, 0, 0)
        );
    }
}
