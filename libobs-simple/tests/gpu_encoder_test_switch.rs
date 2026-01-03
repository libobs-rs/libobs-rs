#![cfg(target_family = "windows")]

/// Standalone reproduction for OBS crash when switching encoders
///
/// Expected result: Crash eventually occurs on NVENC
/// It's not very consistent, if it doesn't crash, just run it again until it does :)
use std::{collections::HashMap, sync::Arc, thread, time::Duration};

use libobs_simple::sources::{
    windows::{monitor_capture::MonitorCaptureSource, MonitorCaptureSourceBuilder},
    ObsSourceBuilder,
};
use libobs_wrapper::{
    context::ObsContext,
    data::{
        object::ObsObjectTrait, output::ObsOutputTrait, video::ObsVideoInfoBuilder, ObsDataSetters,
    },
    encoders::{
        audio::ObsAudioEncoder, video::ObsVideoEncoder, ObsContextEncoders, ObsVideoEncoderType,
    },
    enums::ObsScaleType,
    scenes::SceneItemRef,
    utils::{AudioEncoderInfo, ObsPath, OutputInfo, VideoEncoderInfo},
};

const ROUNDS: usize = 6;
const RECORDING_DURATION: u64 = 2;

// Simplified encoder type enum for this repro
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EncoderType {
    X264,
    NvEnc,
    AmdGpu,
}

impl EncoderType {
    // Chill clippy, this is just a test repro
    #[allow(clippy::wrong_self_convention)]
    fn to_obs_encoder_type(&self) -> ObsVideoEncoderType {
        match self {
            EncoderType::X264 => ObsVideoEncoderType::OBS_X264,
            EncoderType::NvEnc => ObsVideoEncoderType::OBS_NVENC_AV1_TEX,
            EncoderType::AmdGpu => ObsVideoEncoderType::H264_TEXTURE_AMF,
        }
    }

    fn id(&self) -> &str {
        match self {
            EncoderType::X264 => "x264",
            EncoderType::NvEnc => "nv_enc",
            EncoderType::AmdGpu => "texture_amf",
        }
    }
}

struct ReproState {
    obs_context: ObsContext,
    output: libobs_wrapper::data::output::ObsOutputRef,
    audio_encoder: Arc<ObsAudioEncoder>,

    // Key point: storing encoders by type to reuse them (like production code)
    video_encoders: HashMap<EncoderType, Arc<ObsVideoEncoder>>,
    _scene: libobs_wrapper::scenes::ObsSceneRef,
    _monitor_capture: SceneItemRef<MonitorCaptureSource>,
}

impl ReproState {
    fn new() -> Self {
        println!("\n=== Creating OBS Context ===");

        let video_info = ObsVideoInfoBuilder::new()
            .adapter(0)
            .fps_num(60)
            .fps_den(1)
            .base_width(1920)
            .base_height(1080)
            .output_width(1920)
            .output_height(1080)
            .scale_type(ObsScaleType::Bicubic)
            .build();

        let mut obs_context =
            ObsContext::new(ObsContext::builder().set_video_info(video_info)).unwrap();

        println!("=== Creating Output (reused for all recordings) ===");
        let output_settings = obs_context.data().unwrap();
        let output_info = OutputInfo::new("ffmpeg_muxer", "output", Some(output_settings), None);
        let output = obs_context.output(output_info).unwrap();

        println!("=== Creating Audio Encoder (reused for all recordings) ===");
        let mut audio_settings = obs_context.data().unwrap();
        audio_settings.set_int("bitrate", 160).unwrap();
        let audio_info =
            AudioEncoderInfo::new("ffmpeg_aac", "audio_encoder", Some(audio_settings), None);
        let audio_encoder =
            ObsAudioEncoder::new_from_info(audio_info, 0, obs_context.runtime().clone()).unwrap();

        let mut scene = obs_context.scene("main", Some(0)).unwrap();
        let monitors = MonitorCaptureSourceBuilder::get_monitors().unwrap();

        let monitor_capture = obs_context
            .source_builder::<MonitorCaptureSourceBuilder, _>("Monitor Capture")
            .unwrap()
            .set_monitor(&monitors[0])
            .add_to_scene(&mut scene)
            .unwrap();

        Self {
            obs_context,
            output,
            audio_encoder,
            video_encoders: HashMap::new(),
            _scene: scene,
            _monitor_capture: monitor_capture,
        }
    }

    fn simulate_recording(&mut self, encoder_type: EncoderType, recording_num: usize) {
        println!(
            "\n=== Recording {} with {} ===",
            recording_num,
            encoder_type.id()
        );

        // Create a dummy output path (replace with your own)
        let output_path = format!("encoder_test_switch_{}.mp4", encoder_type.id());
        let output_path = ObsPath::from_relative(&output_path).build();

        // KEY FIX: Reset video settings between recordings (like production code does)
        // This simulates changing game resolution and may trigger the bug
        let video_info = ObsVideoInfoBuilder::new()
            .adapter(0)
            .fps_num(60)
            .fps_den(1)
            .base_width(1920)
            .base_height(1080)
            .output_width(1920)
            .output_height(1080)
            .scale_type(ObsScaleType::Bicubic)
            .build();
        println!("  → Resetting video settings (simulating resolution change)");
        self.obs_context.reset_video(video_info).unwrap();

        // Update output path
        let mut output_settings = self.obs_context.data().unwrap();
        output_settings.set_string("path", output_path).unwrap();
        self.output.update_settings(output_settings).unwrap();

        // Create encoder settings
        let mut encoder_settings = self.obs_context.data().unwrap();
        encoder_settings.set_int("bitrate", 2500).unwrap();
        encoder_settings.set_string("rate_control", "CBR").unwrap();
        encoder_settings.set_string("profile", "high").unwrap();
        encoder_settings.set_int("bf", 2).unwrap();
        encoder_settings.set_bool("psycho_aq", true).unwrap();
        encoder_settings.set_bool("lookahead", true).unwrap();

        // Add encoder-specific settings
        match encoder_type {
            EncoderType::X264 => {
                encoder_settings.set_string("preset", "veryfast").unwrap();
            }
            EncoderType::NvEnc => {
                encoder_settings.set_string("preset2", "p4").unwrap();
                encoder_settings.set_string("tune", "hq").unwrap();
            }
            EncoderType::AmdGpu => {
                encoder_settings.set_string("usage", "balanced").unwrap();
                encoder_settings.set_string("quality", "balanced").unwrap();
            }
        }

        // Use unique encoder names per type (more realistic)
        let encoder_name = format!("video_encoder_{}", encoder_type.id());

        let video_encoder = if let Some(existing) = self.video_encoders.get(&encoder_type) {
            println!("  → Reusing existing {} encoder", encoder_type.id());
            existing.clone()
        } else {
            println!("  → Creating new {} encoder", encoder_type.id());
            let encoder = ObsVideoEncoder::new_from_info(
                VideoEncoderInfo::new(
                    encoder_type.to_obs_encoder_type(),
                    encoder_name.as_str(),
                    Some(encoder_settings.clone()),
                    None,
                ),
                self.obs_context.runtime().clone(),
            )
            .unwrap();
            self.video_encoders.insert(encoder_type, encoder.clone());
            encoder
        };

        // Set encoders on output
        println!("  → Setting video encoder on output");
        self.output.set_video_encoder(video_encoder).unwrap();

        println!("  → Setting audio encoder on output");
        self.output
            .set_audio_encoder(self.audio_encoder.clone(), 0)
            .unwrap();

        // Start output
        println!("  → Starting output...");
        self.output.start().unwrap();

        // Simulate recording
        println!("  → Recording for {RECORDING_DURATION} seconds...");
        thread::sleep(Duration::from_secs(RECORDING_DURATION));

        // Stop output
        println!("  → Stopping output...");
        self.output.stop().unwrap();

        println!("  → Recording {} completed successfully", recording_num);

        // Small delay between recordings
        thread::sleep(Duration::from_millis(500));
    }
}

#[test]
fn test_encoder_switch() {
    let mut state = ReproState::new();
    let has_nvidia = state
        .obs_context
        .available_video_encoders()
        .unwrap()
        .iter()
        .any(|info| info.get_encoder_id() == &EncoderType::NvEnc.to_obs_encoder_type());

    for i in 0..ROUNDS {
        state.simulate_recording(
            if i % 2 == 0 {
                if has_nvidia {
                    EncoderType::NvEnc
                } else {
                    EncoderType::AmdGpu
                }
            } else {
                EncoderType::X264
            },
            i,
        );
    }
}
