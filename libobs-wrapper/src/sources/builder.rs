use crate::{
    data::ObsObjectBuilder,
    scenes::{ObsSceneItemRef, ObsSceneRef, SceneItemExtSceneTrait},
    sources::ObsSourceTrait,
    utils::ObsError,
};

pub trait ObsSourceBuilder: ObsObjectBuilder {
    type T: ObsSourceTrait + Clone + 'static;

    fn build(self) -> Result<Self::T, ObsError>
    where
        Self: Sized;

    /// Both items are returned: the source and the scene item it was added as.
    /// You can safely drop these items, they are stored within the scene if you don't need them.
    fn add_to_scene(self, scene: &mut ObsSceneRef) -> Result<ObsSceneItemRef<Self::T>, ObsError>
    where
        Self: Sized,
    {
        let source = self.build()?;

        scene.add_source(source.clone())
    }
}
