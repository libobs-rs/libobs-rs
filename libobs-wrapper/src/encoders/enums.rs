use std::{convert::Infallible, str::FromStr};

use crate::utils::ObsString;

macro_rules! encoder_enum {
    ($name:ident, { $($plugin:literal: [ $($(#[$attr:meta])* $variant:ident,)* ],)* }) => { paste::paste! {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
        #[allow(non_camel_case_types)]
        pub enum $name {
            $(
                $(
                    #[doc = "From plugin: `" $plugin "`"]
                    $(#[$attr])*
                    [<$variant:upper>],
                )*
            )*
            Other(String),
        }

        impl FromStr for $name {
            type Err = Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                #[allow(deprecated)]
                return Ok(match s {
                    $( $( stringify!($variant) => Self::[<$variant:upper>], )* )*
                    e => Self::Other(e.to_string()),
                });
            }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                match Self::from_str(value) {
                    Ok(value) => value,
                    Err(never) => match never {},
                }
            }
        }
        impl From<$name> for ObsString {
            fn from(v: $name) -> ObsString {
                #[allow(deprecated)]
                return match v {
                    $( $( $name::[<$variant:upper>] => ObsString::new(stringify!($variant)), )* )*
                    $name::Other(e) => ObsString::new(&e),
                };
            }
        }
    } };
}

// These lists were produced using
//   rg "struct obs_encoder_info [^\*].*=" -A 4
// on the OBS codebase, which lists all of the encoders and their types, as well as their
// capabilities (including whether they're deprecated or not).

encoder_enum!(
    ObsVideoEncoderType,
    {
        "obs-ffmpeg": [
            h264_texture_amf,
            h265_texture_amf,
            av1_texture_amf,
            ffmpeg_vaapi,
            ffmpeg_vaapi_tex,
            av1_ffmpeg_vaapi,
            av1_ffmpeg_vaapi_tex,
            hevc_ffmpeg_vaapi,
            hevc_ffmpeg_vaapi_tex,
            ffmpeg_openh264,
            #[deprecated]
            ffmpeg_nvenc,
            #[deprecated]
            ffmpeg_hevc_nvenc,
            ffmpeg_svt_av1,
            ffmpeg_aom_av1,
        ],
        "obs-nvenc": [
            #[deprecated]
            obs_nvenc_h264_cuda,
            #[deprecated]
            obs_nvenc_hevc_cuda,
            #[deprecated]
            obs_nvenc_av1_cuda,
            obs_nvenc_h264_tex,
            obs_nvenc_hevc_tex,
            obs_nvenc_av1_tex,
            #[deprecated]
            jim_nvenc,
            #[deprecated]
            jim_hevc_nvenc,
            #[deprecated]
            jim_av1_nvenc,
            obs_nvenc_h264_soft,
            obs_nvenc_hevc_soft,
            obs_nvenc_av1_soft,
        ],
        "obs-qsv11": [
            obs_qsv11,
            obs_qsv11_soft,
            obs_qsv11_v2,
            obs_qsv11_soft_v2,
            obs_qsv11_av1,
            obs_qsv11_av1_soft,
            obs_qsv11_hevc,
            obs_qsv11_hevc_soft,
        ],
        "obs-x264": [
            obs_x264,
        ],
    }
);

encoder_enum!(
    ObsAudioEncoderType,
    {
        "obs-ffmpeg": [
            ffmpeg_aac,
            ffmpeg_opus,
            ffmpeg_pcm_s16le,
            ffmpeg_pcm_s24le,
            ffmpeg_pcm_f32le,
            ffmpeg_alac,
            ffmpeg_flac,
        ],
        "obs-libfdk": [
            libfdk_aac,
        ],
    }
);
