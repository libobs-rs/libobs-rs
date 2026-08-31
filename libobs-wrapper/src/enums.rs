//! Contains all important enums for this crate
use core::fmt;
use std::fmt::Display;

use bitflags::bitflags;
use num_derive::{FromPrimitive, ToPrimitive};

#[cfg(target_os = "windows")]
pub(crate) type OsEnumType = std::os::raw::c_int;
#[cfg(not(target_os = "windows"))]
pub(crate) type OsEnumType = std::os::raw::c_uint;

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Describes the video output format used by the
/// OBS video context. Used in `ObsVideoInfo`.
pub enum ObsVideoFormat {
    AYUV = libobs::video_format_VIDEO_FORMAT_AYUV,
    BGR3 = libobs::video_format_VIDEO_FORMAT_BGR3,
    BGRA = libobs::video_format_VIDEO_FORMAT_BGRA,
    BGRX = libobs::video_format_VIDEO_FORMAT_BGRX,
    I010 = libobs::video_format_VIDEO_FORMAT_I010,
    I210 = libobs::video_format_VIDEO_FORMAT_I210,
    I40A = libobs::video_format_VIDEO_FORMAT_I40A,
    I412 = libobs::video_format_VIDEO_FORMAT_I412,
    I420 = libobs::video_format_VIDEO_FORMAT_I420,
    I422 = libobs::video_format_VIDEO_FORMAT_I422,
    I42A = libobs::video_format_VIDEO_FORMAT_I42A,
    I444 = libobs::video_format_VIDEO_FORMAT_I444,
    NONE = libobs::video_format_VIDEO_FORMAT_NONE,
    NV12 = libobs::video_format_VIDEO_FORMAT_NV12,
    P010 = libobs::video_format_VIDEO_FORMAT_P010,
    P216 = libobs::video_format_VIDEO_FORMAT_P216,
    P416 = libobs::video_format_VIDEO_FORMAT_P416,
    R10L = libobs::video_format_VIDEO_FORMAT_R10L,
    RGBA = libobs::video_format_VIDEO_FORMAT_RGBA,
    UYVY = libobs::video_format_VIDEO_FORMAT_UYVY,
    V210 = libobs::video_format_VIDEO_FORMAT_V210,
    Y800 = libobs::video_format_VIDEO_FORMAT_Y800,
    YA2L = libobs::video_format_VIDEO_FORMAT_YA2L,
    YUVA = libobs::video_format_VIDEO_FORMAT_YUVA,
    YUY2 = libobs::video_format_VIDEO_FORMAT_YUY2,
    YVYU = libobs::video_format_VIDEO_FORMAT_YVYU,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Describes the colorspace that an OBS video context
/// uses. Used in `ObsVideoInfo`.
pub enum ObsColorspace {
    CS2100HLG = libobs::video_colorspace_VIDEO_CS_2100_HLG,
    CS2100PQ = libobs::video_colorspace_VIDEO_CS_2100_PQ,
    CS601 = libobs::video_colorspace_VIDEO_CS_601,
    CS709 = libobs::video_colorspace_VIDEO_CS_709,
    Default = libobs::video_colorspace_VIDEO_CS_DEFAULT,
    CSRGB = libobs::video_colorspace_VIDEO_CS_SRGB,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Describes the minimum and maximum color levels that
/// an OBS video context is allowed to encode. Used in
/// `ObsVideoInfo.`
pub enum ObsVideoRange {
    Default = libobs::video_range_type_VIDEO_RANGE_DEFAULT,
    Partial = libobs::video_range_type_VIDEO_RANGE_PARTIAL,
    Full = libobs::video_range_type_VIDEO_RANGE_FULL,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Describes how libobs should reconcile non-matching
/// base and output resolutions when creating a video
/// context.
pub enum ObsScaleType {
    Area = libobs::obs_scale_type_OBS_SCALE_AREA,
    Bicubic = libobs::obs_scale_type_OBS_SCALE_BICUBIC,
    Bilinear = libobs::obs_scale_type_OBS_SCALE_BILINEAR,
    Disable = libobs::obs_scale_type_OBS_SCALE_DISABLE,
    Lanczos = libobs::obs_scale_type_OBS_SCALE_LANCZOS,
    Point = libobs::obs_scale_type_OBS_SCALE_POINT,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Describes which graphics backend should be used
/// in the OBS video context. Used in `ObsVideoInfo`.
pub enum ObsGraphicsModule {
    OpenGL,
    DirectX11,
    /// Native Metal renderer used by OBS on macOS.
    Metal,
}

#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Status types returned after attempting to
/// reset the OBS video context using the
/// function `obs_reset_video`.
pub enum ObsResetVideoStatus {
    /// `obs_reset_video` was successful.
    Success = libobs::OBS_VIDEO_SUCCESS as i32,
    /// The adapter is not supported as it
    /// lacks capabilities.
    NotSupported = libobs::OBS_VIDEO_NOT_SUPPORTED,
    /// A parameter is invalid.
    InvalidParameter = libobs::OBS_VIDEO_INVALID_PARAM,
    /// An output is currently running, preventing
    /// resetting the video context.
    CurrentlyActive = libobs::OBS_VIDEO_CURRENTLY_ACTIVE,
    /// Generic error occured when attempting to
    /// reset the OBS video context.
    Failure = libobs::OBS_VIDEO_FAIL,
}

/// Audio samples per second options that are
/// supported by libobs.
#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsSamplesPerSecond {
    /// 44.1 kHz
    F44100 = 44100,
    /// 48.0 kHz
    F48000 = 48000,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ObsSpeakerLayout {
    S2Point1 = libobs::speaker_layout_SPEAKERS_2POINT1,
    S4Point0 = libobs::speaker_layout_SPEAKERS_4POINT0,
    S4Point1 = libobs::speaker_layout_SPEAKERS_4POINT1,
    S5Point1 = libobs::speaker_layout_SPEAKERS_5POINT1,
    S7Point1 = libobs::speaker_layout_SPEAKERS_7POINT1,
    Mono = libobs::speaker_layout_SPEAKERS_MONO,
    Stereo = libobs::speaker_layout_SPEAKERS_STEREO,
    Unknown = libobs::speaker_layout_SPEAKERS_UNKNOWN,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObsOutputStopSignal {
    /// Successfully stopped
    Success,
    /// The specified path was invalid
    BadPath,
    /// Failed to connect to a server
    ConnectFailed,
    /// Invalid stream path
    InvalidStream,
    /// Generic error
    Error,
    /// Unexpectedly disconnected
    Disconnected,
    /// The settings, video/audio format, or codecs are unsupported by this output
    Unsupported,
    /// Ran out of disk space
    NoSpace,
    /// Encoder error
    EncodeError,
}

impl fmt::Display for ObsOutputStopSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ObsOutputStopSignal::Success => "Success",
            ObsOutputStopSignal::BadPath => "Bad Path",
            ObsOutputStopSignal::ConnectFailed => "Connect Failed",
            ObsOutputStopSignal::InvalidStream => "Invalid Stream",
            ObsOutputStopSignal::Error => "Error",
            ObsOutputStopSignal::Disconnected => "Disconnected",
            ObsOutputStopSignal::Unsupported => "Unsupported",
            ObsOutputStopSignal::NoSpace => "No Space",
            ObsOutputStopSignal::EncodeError => "Encode Error",
        };
        write!(f, "{}", s)
    }
}

impl From<ObsOutputStopSignal> for i32 {
    fn from(val: ObsOutputStopSignal) -> Self {
        match val {
            ObsOutputStopSignal::Success => libobs::OBS_OUTPUT_SUCCESS as i32,
            ObsOutputStopSignal::BadPath => libobs::OBS_OUTPUT_BAD_PATH,
            ObsOutputStopSignal::ConnectFailed => libobs::OBS_OUTPUT_CONNECT_FAILED,
            ObsOutputStopSignal::InvalidStream => libobs::OBS_OUTPUT_INVALID_STREAM,
            ObsOutputStopSignal::Error => libobs::OBS_OUTPUT_ERROR,
            ObsOutputStopSignal::Disconnected => libobs::OBS_OUTPUT_DISCONNECTED,
            ObsOutputStopSignal::Unsupported => libobs::OBS_OUTPUT_UNSUPPORTED,
            ObsOutputStopSignal::NoSpace => libobs::OBS_OUTPUT_NO_SPACE,
            ObsOutputStopSignal::EncodeError => libobs::OBS_OUTPUT_ENCODE_ERROR,
        }
    }
}

impl TryFrom<i32> for ObsOutputStopSignal {
    type Error = &'static str;

    fn try_from(value: i32) -> Result<Self, <ObsOutputStopSignal as TryFrom<i32>>::Error> {
        match value {
            x if x == libobs::OBS_OUTPUT_SUCCESS as i32 => Ok(ObsOutputStopSignal::Success),
            x if x == libobs::OBS_OUTPUT_BAD_PATH => Ok(ObsOutputStopSignal::BadPath),
            x if x == libobs::OBS_OUTPUT_CONNECT_FAILED => Ok(ObsOutputStopSignal::ConnectFailed),
            x if x == libobs::OBS_OUTPUT_INVALID_STREAM => Ok(ObsOutputStopSignal::InvalidStream),
            x if x == libobs::OBS_OUTPUT_ERROR => Ok(ObsOutputStopSignal::Error),
            x if x == libobs::OBS_OUTPUT_DISCONNECTED => Ok(ObsOutputStopSignal::Disconnected),
            x if x == libobs::OBS_OUTPUT_UNSUPPORTED => Ok(ObsOutputStopSignal::Unsupported),
            x if x == libobs::OBS_OUTPUT_NO_SPACE => Ok(ObsOutputStopSignal::NoSpace),
            x if x == libobs::OBS_OUTPUT_ENCODE_ERROR => Ok(ObsOutputStopSignal::EncodeError),
            _ => Err("Invalid value"),
        }
    }
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ObsEncoderType {
    Video = libobs::obs_encoder_type_OBS_ENCODER_VIDEO,
    Audio = libobs::obs_encoder_type_OBS_ENCODER_AUDIO,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ObsLogLevel {
    Error = libobs::LOG_ERROR,
    Warning = libobs::LOG_WARNING,
    Info = libobs::LOG_INFO,
    Debug = libobs::LOG_DEBUG,
}

impl Display for ObsLogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[cfg(feature = "color-logger")]
impl ObsLogLevel {
    pub fn colorize(&self, s: &str) -> String {
        use colored::Colorize;

        match self {
            ObsLogLevel::Error => s.on_red().to_string(),
            ObsLogLevel::Warning => s.yellow().to_string(),
            ObsLogLevel::Info => s.green().bold().to_string(),
            ObsLogLevel::Debug => s.blue().to_string(),
        }
    }
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
/// Used with scene items to indicate the type of bounds to use for scene items.
/// Mostly determines how the image will be scaled within those bounds, or
/// whether to use bounds at all.
pub enum ObsBounds {
    /// No bounds
    None = libobs::obs_bounds_type_OBS_BOUNDS_NONE,
    /// stretch (ignores base scale)
    Stretch = libobs::obs_bounds_type_OBS_BOUNDS_STRETCH,
    /// scales to inner rectangle
    ScaleInner = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_INNER,
    /// scales to outer rectangle
    ScaleOuter = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_OUTER,
    /// scales to the width
    ScaleToWidth = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_TO_WIDTH,
    /// scales to the height
    ScaleToHeight = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_TO_HEIGHT,
    /// no scaling, maximum size only
    MaxOnly = libobs::obs_bounds_type_OBS_BOUNDS_MAX_ONLY,
}

#[cfg_attr(target_os = "windows", repr(i32))]
#[cfg_attr(not(target_os = "windows"), repr(u32))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
pub enum ObsBoundsType {
    None = libobs::obs_bounds_type_OBS_BOUNDS_NONE,
    Stretch = libobs::obs_bounds_type_OBS_BOUNDS_STRETCH,
    ScaleInner = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_INNER,
    ScaleOuter = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_OUTER,
    ScaleToWidth = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_TO_WIDTH,
    ScaleToHeight = libobs::obs_bounds_type_OBS_BOUNDS_SCALE_TO_HEIGHT,
    MaxOnly = libobs::obs_bounds_type_OBS_BOUNDS_MAX_ONLY,
}

bitflags! {
    /// Represents a set of flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ObsAlignment: u32 {
        const LEFT = libobs::OBS_ALIGN_LEFT;
        const TOP = libobs::OBS_ALIGN_TOP;
        const RIGHT = libobs::OBS_ALIGN_RIGHT;
        const BOTTOM = libobs::OBS_ALIGN_BOTTOM;
        const CENTER = libobs::OBS_ALIGN_CENTER;
    }
}
