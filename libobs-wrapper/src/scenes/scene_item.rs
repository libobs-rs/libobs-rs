use std::sync::Arc;

use libobs::obs_scene_item;

use crate::{
    impl_obs_drop, run_with_obs,
    runtime::ObsRuntime,
    sources::ObsSourceTrait,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::ObsDropGuard,
};

#[derive(Debug)]
pub(super) struct _ObsSceneItemDropGuard {
    scene_item: Sendable<*mut obs_scene_item>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsSceneItemDropGuard {}
impl_obs_drop!(_ObsSceneItemDropGuard, (scene_item), move || unsafe {
    libobs::obs_sceneitem_remove(scene_item.0);
    libobs::obs_sceneitem_release(scene_item.0);
});

#[derive(Debug, Clone)]
pub struct SceneItemRef {
    underlying_source: Arc<Box<dyn ObsSourceTrait>>,
    scene_item_ptr: SmartPointerSendable<*mut obs_scene_item>,
    runtime: ObsRuntime,
}

impl SceneItemRef {
    pub(crate) fn new<T: ObsSourceTrait>(
        scene: &ObsSceneRef,
        source: T,
        runtime: ObsRuntime,
    ) -> Self {
        run_with_obs!(runtime, (scene_ptr, source_ptr), move || unsafe {
            let scene_item_ptr =
                libobs::obs_scene_add_source(scene_ptr.0, source_ptr, std::ptr::null_mut());
        });

        let drop_guard = _ObsSceneItemDropGuard {
            scene_item: Sendable(scene_item_ptr),
            runtime: runtime.clone(),
        };

        let scene_item_ptr = SmartPointerSendable::new(scene_item_ptr, Arc::new(drop_guard));

        Self {
            scene_item_ptr,
            runtime,
        }
    }
}
