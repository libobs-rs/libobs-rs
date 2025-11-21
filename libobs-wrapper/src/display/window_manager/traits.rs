use crate::utils::ObsError;

pub trait MiscDisplayTrait {
    fn is_enabled(&self) -> Result<bool, ObsError>;

    fn set_enabled(&self, enabled: bool) -> Result<(), ObsError>;

    fn set_background_color(&self, r: u8, g: u8, b: u8) -> Result<(), ObsError>;
}

pub trait WindowPositionTrait {
    fn set_render_at_bottom(&self, render_at_bottom: bool) -> Result<(), ObsError>;
    fn get_render_at_bottom(&self) -> Result<bool, ObsError>;
    fn set_pos(&self, x: i32, y: i32) -> Result<(), ObsError>;
    fn set_size(&self, width: u32, height: u32) -> Result<(), ObsError>;
    fn set_scale(&self, scale: f32) -> Result<(), ObsError>;

    fn get_pos(&self) -> Result<(i32, i32), ObsError>;

    fn get_size(&self) -> Result<(u32, u32), ObsError>;

    fn get_scale(&self) -> Result<f32, ObsError>;
}

pub trait ShowHideTrait {
    /// Shows the window.
    fn show(&mut self) -> Result<(), ObsError>;

    /// Hides the window.
    fn hide(&mut self) -> Result<(), ObsError>;

    /// Returns true if the window is visible.
    fn is_visible(&self) -> Result<bool, ObsError>;
}
