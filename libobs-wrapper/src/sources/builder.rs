use crate::{
    data::ObsObjectBuilder, scenes::ObsSceneRef, sources::ObsSourceTrait, utils::ObsError,
};

pub trait ObsSourceBuilder: ObsObjectBuilder {
    type T: ObsSourceTrait;

    fn add_to_scene(self, scene: &mut ObsSceneRef) -> Result<Self::T, ObsError>
    where
        Self: Sized;
}
