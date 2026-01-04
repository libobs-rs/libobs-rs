mod transform_info;
pub use transform_info::*;

mod scene_drop_guards;
mod scene_item;

mod filter_traits;
pub use filter_traits::*;

pub use scene_item::*;

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use libobs::{obs_scene_t, obs_source_t};

use crate::macros::impl_eq_of_ptr;
use crate::scenes::scene_drop_guards::_SceneDropGuard;
use crate::sources::{ObsFilterGuardPair, ObsSourceTrait};
use crate::unsafe_send::SmartPointerSendable;
use crate::utils::{GeneralTraitHashMap, ObsDropGuard};
use crate::{
    impl_signal_manager, run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::Sendable,
    utils::{ObsError, ObsString},
};

#[derive(Debug)]
struct _NoOpDropGuard;
impl ObsDropGuard for _NoOpDropGuard {}

#[derive(Debug, Clone)]
pub struct ObsSceneRef {
    name: ObsString,
    attached_scene_items:
        GeneralTraitHashMap<dyn ObsSourceTrait, Vec<Arc<Box<dyn SceneItemTrait + 'static>>>>,
    attached_filters: Arc<RwLock<Vec<ObsFilterGuardPair>>>,
    runtime: ObsRuntime,
    signals: Arc<ObsSceneSignals>,
    scene: SmartPointerSendable<*mut obs_scene_t>,
}

impl_eq_of_ptr!(ObsSceneRef);

impl ObsSceneRef {
    pub(crate) fn new(name: ObsString, runtime: ObsRuntime) -> Result<Self, ObsError> {
        let scene = run_with_obs!(runtime, (name), move || {
            let name_ptr = name.as_ptr();

            let scene_ptr = unsafe {
                // Safety: name_ptr is valid because we have the name variable in scope.
                libobs::obs_scene_create(name_ptr.0)
            };
            if scene_ptr.is_null() {
                return Err(ObsError::NullPointer(None));
            }

            let source_ptr = unsafe {
                // Safety: scene_ptr is valid because we just created it and its not null.
                libobs::obs_scene_get_source(scene_ptr)
            };

            if source_ptr.is_null() {
                unsafe {
                    // Safety: scene_ptr is valid because we just created it and its not null.
                    libobs::obs_scene_release(scene_ptr);
                }
                return Err(ObsError::NullPointer(None));
            }

            Ok(Sendable(scene_ptr))
        })??;

        let drop_guard = Arc::new(_SceneDropGuard::new(scene.clone(), runtime.clone()));
        let scene = SmartPointerSendable::new(scene.0, drop_guard);

        let signals = Arc::new(ObsSceneSignals::new(&scene, runtime.clone())?);
        Ok(Self {
            name,
            scene,
            attached_scene_items: Arc::new(RwLock::new(HashMap::new())),
            attached_filters: Arc::new(RwLock::new(Vec::new())),
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

        let scene_source_ptr = self.get_scene_source_ptr()?;
        run_with_obs!(self.runtime, (scene_source_ptr), move || unsafe {
            // Safety: We are in the runtime and the struct hasn't been dropped yet, therefore the scene source must be valid.
            // Also we are removing that pointer from the output source if this scene is dropped in the Drop guard
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

        run_with_obs!(self.runtime, (), move || unsafe {
            // Safety: We are in the runtime
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

    pub fn as_ptr(&self) -> SmartPointerSendable<*mut obs_scene_t> {
        self.scene.clone()
    }

    pub fn name(&self) -> ObsString {
        self.name.clone()
    }

    pub fn signals(&self) -> Arc<ObsSceneSignals> {
        self.signals.clone()
    }
}

impl_signal_manager!(|scene_ptr: SmartPointerSendable<*mut obs_scene_t>| unsafe {
    // Safety: This is a smart pointer, so it is fine
    let source_ptr = libobs::obs_scene_get_source(scene_ptr.get_ptr());

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
