use std::sync::Arc;

use crate::{
    data::ObsObjectBuilder, scenes::ObsSceneRef, sources::ObsSourceTrait, utils::ObsError,
};

pub trait ObsSourceBuilder: ObsObjectBuilder {
    fn add_to_scene(
        self,
        scene: &mut ObsSceneRef,
    ) -> Result<Arc<Box<dyn ObsSourceTrait>>, ObsError>
    where
        Self: Sized;
}
