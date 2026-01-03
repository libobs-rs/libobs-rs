use libobs::obs_scene_t;

use crate::{impl_obs_drop, runtime::ObsRuntime, unsafe_send::Sendable, utils::ObsDropGuard};
use std::ptr;

#[derive(Debug)]
pub(super) struct _SceneDropGuard {
    scene: Sendable<*mut obs_scene_t>,
    runtime: ObsRuntime,
}

impl _SceneDropGuard {
    pub(super) fn new(scene: Sendable<*mut obs_scene_t>, runtime: ObsRuntime) -> Self {
        Self { scene, runtime }
    }
}

impl ObsDropGuard for _SceneDropGuard {}

impl_obs_drop!(_SceneDropGuard, (scene), move || unsafe {
    let scene_source = libobs::obs_scene_get_source(scene.0);

    for i in 0..libobs::MAX_CHANNELS {
        let current_source = libobs::obs_get_output_source(i);
        if current_source == scene_source {
            libobs::obs_set_output_source(i, ptr::null_mut());
        }

        libobs::obs_source_release(current_source);
    }

    libobs::obs_source_release(scene_source);
    libobs::obs_scene_release(scene.0);
});
