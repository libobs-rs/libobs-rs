use std::fmt::Debug;

use libobs::obs_transform_info;

use crate::{
    enums::{ObsBoundsType, OsEnumType},
    graphics::Vec2,
    macros::enum_from_number,
    scenes::ObsSceneRef,
    sources::ObsSourceTrait,
    utils::ObsError,
};

/// Use `ObsTransformInfoBuilder` to create an instance of this struct.
pub struct ObsTransformInfo(pub(crate) obs_transform_info);
impl Debug for ObsTransformInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsTransformInfo")
            .field("pos", &Vec2::from(self.0.pos))
            .field("scale", &Vec2::from(self.0.scale))
            .field("alignment", &self.0.alignment)
            .field("rot", &self.0.rot)
            .field("bounds", &Vec2::from(self.0.bounds))
            .field("bounds_type", &self.0.bounds_type)
            .field("bounds_alignment", &self.0.bounds_alignment)
            .field("crop_to_bounds", &self.0.crop_to_bounds)
            .finish()
    }
}

impl Clone for ObsTransformInfo {
    fn clone(&self) -> Self {
        ObsTransformInfo(obs_transform_info {
            pos: self.0.pos,
            scale: self.0.scale,
            alignment: self.0.alignment,
            rot: self.0.rot,
            bounds: self.0.bounds,
            bounds_type: self.0.bounds_type,
            bounds_alignment: self.0.bounds_alignment,
            crop_to_bounds: self.0.crop_to_bounds,
        })
    }
}

impl ObsTransformInfo {
    pub fn get_pos(&self) -> Vec2 {
        Vec2::from(self.0.pos)
    }

    pub fn get_scale(&self) -> Vec2 {
        Vec2::from(self.0.scale)
    }

    pub fn get_alignment(&self) -> u32 {
        self.0.alignment
    }

    pub fn get_rot(&self) -> f32 {
        self.0.rot
    }

    pub fn get_bounds(&self) -> Vec2 {
        Vec2::from(self.0.bounds)
    }

    pub fn get_bounds_type(&self) -> ObsBoundsType {
        enum_from_number!(ObsBoundsType, self.0.bounds_type).unwrap()
    }

    pub fn get_bounds_alignment(&self) -> u32 {
        self.0.bounds_alignment
    }

    pub fn get_crop_to_bounds(&self) -> bool {
        self.0.crop_to_bounds
    }
}

pub struct ObsTransformInfoBuilder {
    pos: Option<Vec2>,
    scale: Option<Vec2>,
    alignment: Option<u32>,
    rot: Option<f32>,
    bounds: Option<Vec2>,
    bounds_type: Option<ObsBoundsType>,
    bounds_alignment: Option<u32>,
    crop_to_bounds: Option<bool>,
}

impl Default for ObsTransformInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ObsTransformInfoBuilder {
    pub fn new() -> Self {
        Self {
            pos: None,
            scale: None,
            alignment: None,
            rot: None,
            bounds: None,
            bounds_type: None,
            bounds_alignment: None,
            crop_to_bounds: None,
        }
    }

    pub fn set_pos(mut self, pos: Vec2) -> Self {
        self.pos = Some(pos);
        self
    }

    pub fn set_scale(mut self, scale: Vec2) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Use alignment constants like so: `obs_alignment::LEFT | obs_alignment::TOP`
    pub fn set_alignment(mut self, alignment: u32) -> Self {
        self.alignment = Some(alignment);
        self
    }

    pub fn set_rot(mut self, rot: f32) -> Self {
        self.rot = Some(rot);
        self
    }

    pub fn set_bounds(mut self, bounds: Vec2) -> Self {
        self.bounds = Some(bounds);
        self
    }

    pub fn set_bounds_type(mut self, bounds_type: ObsBoundsType) -> Self {
        self.bounds_type = Some(bounds_type);
        self
    }

    /// Use alignment constants like so: `obs_alignment::LEFT | obs_alignment::TOP`
    pub fn set_bounds_alignment(mut self, bounds_alignment: u32) -> Self {
        self.bounds_alignment = Some(bounds_alignment);
        self
    }

    pub fn set_crop_to_bounds(mut self, crop_to_bounds: bool) -> Self {
        self.crop_to_bounds = Some(crop_to_bounds);
        self
    }

    /// Builds the `ObsTransformInfo` instance and keeps values that have not been set the same.
    pub fn build_with_fallback<T: ObsSourceTrait>(
        self,
        scene: &ObsSceneRef,
        source: &T,
    ) -> Result<ObsTransformInfo, ObsError> {
        let current = scene.get_transform_info(source)?;
        let bounds_type = self
            .bounds_type
            .unwrap_or_else(|| current.get_bounds_type());

        let bounds_type = bounds_type as OsEnumType;
        Ok(ObsTransformInfo(obs_transform_info {
            pos: self.pos.unwrap_or_else(|| current.get_pos()).into(),
            scale: self.scale.unwrap_or_else(|| current.get_scale()).into(),
            alignment: self.alignment.unwrap_or_else(|| current.get_alignment()),
            rot: self.rot.unwrap_or_else(|| current.get_rot()),
            bounds: self.bounds.unwrap_or_else(|| current.get_bounds()).into(),
            bounds_type,
            bounds_alignment: self
                .bounds_alignment
                .unwrap_or_else(|| current.get_bounds_alignment()),
            crop_to_bounds: self
                .crop_to_bounds
                .unwrap_or_else(|| current.get_crop_to_bounds()),
        }))
    }

    /// Builds the transform info with only the values set in the builder. Unset values will be defaulted.
    pub fn build(self, base_width: u32, base_height: u32) -> ObsTransformInfo {
        let bounds_type = self.bounds_type.unwrap_or(ObsBoundsType::ScaleInner) as OsEnumType;

        ObsTransformInfo(obs_transform_info {
            pos: self.pos.unwrap_or_else(|| Vec2::new(0.0, 0.0)).into(),
            scale: self.scale.unwrap_or_else(|| Vec2::new(1.0, 1.0)).into(),
            alignment: self
                .alignment
                .unwrap_or(libobs::OBS_ALIGN_LEFT | libobs::OBS_ALIGN_TOP),
            rot: self.rot.unwrap_or(0.0),
            bounds: self
                .bounds
                .unwrap_or_else(|| Vec2::new(base_width as f32, base_height as f32))
                .into(),
            bounds_type,
            bounds_alignment: self.bounds_alignment.unwrap_or(libobs::OBS_ALIGN_CENTER),
            crop_to_bounds: self.crop_to_bounds.unwrap_or(false),
        })
    }
}
