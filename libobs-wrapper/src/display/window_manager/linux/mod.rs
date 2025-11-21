use crate::{
    display::{GeneralDisplayWindowManager, MiscDisplayTrait, ObsWindowHandle},
    unsafe_send::Sendable,
    utils::ObsError,
};

#[derive(Debug)]
pub(crate) struct DisplayWindowManager {
    window_handle: ObsWindowHandle,
    display: Option<Sendable<*mut libobs::obs_display>>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl DisplayWindowManager {
    pub fn new(
        window_handle: ObsWindowHandle,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<Self, ObsError> {
        // Implementation for Linux
        Ok(Self {})
    }
}

impl MiscDisplayTrait for DisplayWindowManager {
    fn update_color_space(&self) -> Result<(), ObsError> {
        Ok(())
    }
}

impl GeneralDisplayWindowManager for DisplayWindowManager {}
