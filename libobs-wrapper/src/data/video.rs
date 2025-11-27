use std::{boxed::Box, fmt::Debug, pin::Pin};

use display_info::DisplayInfo;
use libobs::obs_video_info;

#[cfg(target_os = "linux")]
use crate::utils::linux::get_linux_opengl_lib_name;
use crate::{
    enums::{
        ObsColorspace, ObsGraphicsModule, ObsScaleType, ObsVideoFormat, ObsVideoRange, OsEnumType,
    },
    unsafe_send::Sendable,
    utils::ObsString,
};

#[derive(Clone, Debug)]
pub struct ObsSdrVideoInfo {
    /// The white level in nits
    pub sdr_white_level: f32,
    /// The nominal peak level in nits
    pub hdr_nominal_peak_level: f32,
}

impl Default for ObsSdrVideoInfo {
    fn default() -> Self {
        Self {
            sdr_white_level: 300.0,
            hdr_nominal_peak_level: 1000.0,
        }
    }
}

/// A wrapper for `obs_video_info`, which is used
/// to pass information to libobs for the new OBS
/// video context after resetting the old OBS
/// video context.
/// A wrapper for `obs_video_info`, which is used
/// to pass information to libobs for the new OBS
/// video context after resetting the old OBS
/// video context. The obs_video_info is pinned in memory
/// to ensure its address never changes, as required by libobs.
pub struct ObsVideoInfo {
    ovi: Sendable<Pin<Box<obs_video_info>>>,
    // False positive. This is necessary to ensure
    // that the graphics module string in the
    // `obs_video_info` struct does not free.
    #[allow(dead_code)]
    graphics_module: ObsString,

    sdr_info: ObsSdrVideoInfo,
}

impl Debug for ObsVideoInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObsVideoInfo")
            .field("fps_num", &self.get_fps_num())
            .field("fps_den", &self.get_fps_den())
            .field("base_width", &self.get_base_width())
            .field("base_height", &self.get_base_height())
            .field("output_width", &self.get_output_width())
            .field("output_height", &self.get_output_height())
            .field("sdr_info", &self.get_sdr_info())
            .finish()
    }
}

impl ObsVideoInfo {
    /// Creates a new `ObsVideoInfo`.
    ///
    /// Note that this function is not meant to
    /// be used externally. The recommended,
    /// supported way to build new `ObsVideoInfo`
    /// structs is through `ObsVideoInfoBuilder`.
    #[deprecated = "Use new_with_sdr_info or the ObsVideoInfoBuilder instead"]
    pub fn new(ovi: obs_video_info, graphics_module: ObsString) -> Self {
        Self {
            ovi: Sendable(Box::pin(ovi)),
            graphics_module,
            sdr_info: ObsSdrVideoInfo::default(),
        }
    }

    /// Creates a new `ObsVideoInfo`.
    ///
    /// Note that this function is not meant to
    /// be used externally. The recommended,
    /// supported way to build new `ObsVideoInfo`
    /// structs is through `ObsVideoInfoBuilder`.
    pub fn new_with_sdr_info(
        ovi: obs_video_info,
        graphics_module: ObsString,
        sdr_info: ObsSdrVideoInfo,
    ) -> Self {
        Self {
            ovi: Sendable(Box::pin(ovi)),
            graphics_module,
            sdr_info,
        }
    }

    /// Returns a pointer to the pinned `obs_video_info`.
    pub fn as_ptr(&self) -> *mut obs_video_info {
        // Safe because ovi is pinned for the lifetime of this struct
        let ptr: *const obs_video_info = &*Pin::as_ref(&self.ovi.0);
        ptr as *mut obs_video_info
    }

    pub fn graphics_module(&self) -> &ObsString {
        &self.graphics_module
    }

    pub fn get_fps_num(&self) -> u32 {
        self.ovi.0.fps_num
    }

    pub fn get_fps_den(&self) -> u32 {
        self.ovi.0.fps_den
    }

    pub fn get_base_width(&self) -> u32 {
        self.ovi.0.base_width
    }

    pub fn get_base_height(&self) -> u32 {
        self.ovi.0.base_height
    }

    pub fn get_output_width(&self) -> u32 {
        self.ovi.0.output_width
    }

    pub fn get_output_height(&self) -> u32 {
        self.ovi.0.output_height
    }

    pub fn get_sdr_info(&self) -> &ObsSdrVideoInfo {
        &self.sdr_info
    }
}

impl Default for ObsVideoInfo {
    fn default() -> Self {
        ObsVideoInfoBuilder::new().build()
    }
}

/// A structure intended to help make
/// creating new `ObsVideoInfo` structs
/// easier for resetting the OBS video
/// context.
#[derive(Clone, Debug)]
pub struct ObsVideoInfoBuilder {
    adapter: u32,
    graphics_module: ObsGraphicsModule,
    fps_num: u32,
    fps_den: u32,
    base_width: u32,
    base_height: u32,
    output_width: u32,
    output_height: u32,
    output_format: ObsVideoFormat,
    gpu_conversion: bool,
    colorspace: ObsColorspace,
    range: ObsVideoRange,
    scale_type: ObsScaleType,
    sdr_info: ObsSdrVideoInfo,
}

impl ObsVideoInfoBuilder {
    /// Creates a new `ObsVideoInfoBuilder`
    /// for creating new `ObsVideoInfo` to
    /// pass to the video context reset
    /// function.
    ///
    /// This function comes with
    /// sensible default values and chooses
    /// the backend depending on which
    /// if the OS supports DX11 (Windows)
    /// or not (OpenGL on MacOS and Unix).
    pub fn new() -> Self {
        let display_infos = DisplayInfo::all().unwrap_or_default();
        let (mut width, mut height) = (1920, 1080);
        for display_info in display_infos {
            if display_info.is_primary {
                width = display_info.width;
                height = display_info.height;
                break;
            }
        }

        Self {
            adapter: 0,
            #[cfg(target_family = "unix")]
            graphics_module: ObsGraphicsModule::OpenGL,
            #[cfg(target_family = "windows")]
            graphics_module: ObsGraphicsModule::DirectX11,
            fps_num: 30,
            fps_den: 1,
            base_width: width,
            base_height: height,
            output_width: width,
            output_height: height,
            output_format: ObsVideoFormat::NV12,
            gpu_conversion: true,
            colorspace: ObsColorspace::CS709,
            range: ObsVideoRange::Default,
            scale_type: ObsScaleType::Lanczos,
            sdr_info: ObsSdrVideoInfo::default(),
        }
    }

    /// Consumes the `ObsVideoInfoBuilder`
    /// to create an `ObsVideoInfo`.
    pub fn build(self) -> ObsVideoInfo {
        let graphics_mod_str = match self.graphics_module {
            #[cfg(not(target_os = "linux"))]
            ObsGraphicsModule::OpenGL => ObsString::new("libobs-opengl"),
            #[cfg(target_os = "linux")]
            ObsGraphicsModule::OpenGL => ObsString::new(get_linux_opengl_lib_name()),
            ObsGraphicsModule::DirectX11 => ObsString::new("libobs-d3d11.dll"),
        };

        let ovi = obs_video_info {
            adapter: self.adapter,
            graphics_module: graphics_mod_str.as_ptr().0,
            fps_num: self.fps_num,
            fps_den: self.fps_den,
            base_width: self.base_width,
            base_height: self.base_height,
            output_width: self.output_width,
            output_height: self.output_height,
            output_format: self.output_format as OsEnumType,
            gpu_conversion: self.gpu_conversion,
            colorspace: self.colorspace as OsEnumType,
            range: self.range as OsEnumType,
            scale_type: self.scale_type as OsEnumType,
        };

        ObsVideoInfo {
            ovi: Sendable(Box::pin(ovi)),
            graphics_module: graphics_mod_str,
            sdr_info: self.sdr_info,
        }
    }

    pub fn set_sdr_info(mut self, sdr_info: ObsSdrVideoInfo) -> Self {
        self.sdr_info = sdr_info;
        self
    }

    /// Sets the GPU adapter device
    /// that the video output is coming
    /// from.
    pub fn adapter(mut self, value: u32) -> Self {
        self.adapter = value;
        self
    }

    /// Sets the graphics backend
    /// that libobs uses to record.
    pub fn graphics_module(mut self, value: ObsGraphicsModule) -> Self {
        self.graphics_module = value;
        self
    }

    /// Sets the framerate of the
    /// output video. Note that this
    /// value may not reflect the
    /// final framerate if `fps_den`
    /// is not equal to 1.
    pub fn fps_num(mut self, value: u32) -> Self {
        self.fps_num = value;
        self
    }

    /// Divides the FPS numerator to
    /// allow for fractional FPS
    /// counts on output.
    pub fn fps_den(mut self, value: u32) -> Self {
        self.fps_den = value;
        self
    }

    /// Sets the width of the screen
    /// being recorded.
    pub fn base_width(mut self, value: u32) -> Self {
        self.base_width = value;
        self
    }

    /// Sets the height of the screen
    /// being recorded.
    pub fn base_height(mut self, value: u32) -> Self {
        self.base_height = value;
        self
    }

    /// Sets the width of the video
    /// output.
    pub fn output_width(mut self, value: u32) -> Self {
        self.output_width = value;
        self
    }

    /// Sets the height of the video
    /// output.
    pub fn output_height(mut self, value: u32) -> Self {
        self.output_height = value;
        self
    }

    /// Sets the format in which the
    /// video will be output.
    pub fn output_format(mut self, value: ObsVideoFormat) -> Self {
        self.output_format = value;
        self
    }

    /// Sets whether the GPU will handle
    /// conversion in the video.
    pub fn gpu_conversion(mut self, value: bool) -> Self {
        self.gpu_conversion = value;
        self
    }

    /// Sets the video colorspace.
    pub fn colorspace(mut self, value: ObsColorspace) -> Self {
        self.colorspace = value;
        self
    }

    /// Sets the video range.
    pub fn range(mut self, value: ObsVideoRange) -> Self {
        self.range = value;
        self
    }

    /// Sets the video scaling type.
    pub fn scale_type(mut self, value: ObsScaleType) -> Self {
        self.scale_type = value;
        self
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for ObsVideoInfoBuilder {
    fn default() -> Self {
        Self::new()
    }
}
