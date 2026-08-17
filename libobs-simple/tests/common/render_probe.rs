#![allow(dead_code)]

use std::{thread, time::Duration};

use libobs_wrapper::{scenes::ObsSceneRef, utils::ObsError};

#[derive(Clone, Debug)]
pub struct RgbaFrame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelBounds {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl PixelBounds {
    pub fn width(self) -> u32 {
        self.right - self.left + 1
    }

    pub fn height(self) -> u32 {
        self.bottom - self.top + 1
    }
}

impl RgbaFrame {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(x < self.width && y < self.height, "pixel outside frame");
        let offset = ((y * self.width + x) * 4) as usize;
        self.pixels[offset..offset + 4]
            .try_into()
            .expect("RGBA pixel is four bytes")
    }

    pub fn color_bounds(&self, expected: [u8; 4], tolerance: u8) -> Option<PixelBounds> {
        let mut bounds = None::<PixelBounds>;
        for y in 0..self.height {
            for x in 0..self.width {
                if color_close(self.pixel(x, y), expected, tolerance) {
                    match bounds.as_mut() {
                        Some(bounds) => {
                            bounds.left = bounds.left.min(x);
                            bounds.top = bounds.top.min(y);
                            bounds.right = bounds.right.max(x);
                            bounds.bottom = bounds.bottom.max(y);
                        }
                        None => {
                            bounds = Some(PixelBounds {
                                left: x,
                                top: y,
                                right: x,
                                bottom: y,
                            });
                        }
                    }
                }
            }
        }
        bounds
    }

    pub fn count_color(&self, expected: [u8; 4], tolerance: u8) -> usize {
        (0..self.height)
            .flat_map(|y| (0..self.width).map(move |x| (x, y)))
            .filter(|&(x, y)| color_close(self.pixel(x, y), expected, tolerance))
            .count()
    }

    pub fn assert_pixel_close(&self, x: u32, y: u32, expected: [u8; 4], tolerance: u8) {
        let actual = self.pixel(x, y);
        assert!(
            color_close(actual, expected, tolerance),
            "pixel ({x}, {y}) mismatch: expected {expected:?} ± {tolerance}, got {actual:?}"
        );
    }

    pub fn assert_color_bounds(
        &self,
        color: [u8; 4],
        tolerance: u8,
        expected: PixelBounds,
        label: &str,
    ) {
        assert_eq!(
            self.color_bounds(color, tolerance),
            Some(expected),
            "unexpected {label} rendered bounds"
        );
    }

    pub fn assert_color_count(&self, color: [u8; 4], tolerance: u8, expected: usize, label: &str) {
        assert_eq!(
            self.count_color(color, tolerance),
            expected,
            "unexpected {label} rendered pixel count"
        );
    }

    pub fn assert_color_absent(&self, color: [u8; 4], tolerance: u8, label: &str) {
        self.assert_color_count(color, tolerance, 0, label);
    }
}

pub fn color_close(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
    actual
        .into_iter()
        .zip(expected)
        .all(|(actual, expected)| actual.abs_diff(expected) <= tolerance)
}

/// Captures bounded successive real frames until `predicate` accepts one. This models libobs
/// property/scene updates that intentionally take effect on the next video tick.
pub fn capture_until(
    scene: &ObsSceneRef,
    width: u32,
    height: u32,
    attempts: usize,
    delay: Duration,
    mut predicate: impl FnMut(&RgbaFrame) -> bool,
) -> Result<RgbaFrame, ObsError> {
    let attempts = attempts.max(1);
    let mut last = capture_program(scene, width, height)?;
    if predicate(&last) {
        return Ok(last);
    }
    for _ in 1..attempts {
        thread::sleep(delay);
        last = capture_program(scene, width, height)?;
        if predicate(&last) {
            return Ok(last);
        }
    }
    Ok(last)
}

/// Renders OBS's current program texture into an off-screen RGBA texture and reads it back.
///
/// This deliberately uses the same texrender/staging path that OBS's screenshot implementation
/// uses, so visual assertions exercise the real libobs scene renderer rather than duplicating its
/// transform math in Rust.
pub fn capture_program(
    scene: &ObsSceneRef,
    width: u32,
    height: u32,
) -> Result<RgbaFrame, ObsError> {
    let runtime = scene.runtime().clone();
    let captured = runtime.run_with_obs_result(move || unsafe {
        // Safety: the complete graphics operation runs on the OBS actor and is enclosed by
        // obs_enter_graphics/obs_leave_graphics. Every created graphics object is destroyed before
        // leaving the graphics context and no raw pointer escapes this closure.
        libobs::obs_enter_graphics();

        let texrender = libobs::gs_texrender_create(
            libobs::gs_color_format_GS_RGBA,
            libobs::gs_zstencil_format_GS_ZS_NONE,
        );
        let stagesurface =
            libobs::gs_stagesurface_create(width, height, libobs::gs_color_format_GS_RGBA);

        let result = (|| -> Result<RgbaFrame, String> {
            if texrender.is_null() || stagesurface.is_null() {
                return Err("failed to allocate off-screen OBS graphics resources".to_string());
            }

            libobs::gs_texrender_reset(texrender);
            if !libobs::gs_texrender_begin(texrender, width, height) {
                return Err("gs_texrender_begin failed".to_string());
            }

            let clear = std::mem::zeroed::<libobs::vec4>();
            libobs::gs_clear(libobs::GS_CLEAR_COLOR, &clear, 0.0, 0);
            libobs::gs_ortho(0.0, width as f32, 0.0, height as f32, -100.0, 100.0);
            libobs::gs_blend_state_push();
            libobs::gs_blend_function(
                libobs::gs_blend_type_GS_BLEND_ONE,
                libobs::gs_blend_type_GS_BLEND_ZERO,
            );

            // obs_render_main_texture() depends on the asynchronous video thread having already
            // produced a main texture. Fetch and render the program source directly instead, which
            // is deterministic immediately after scene mutations and matches OBS's source-screenshot
            // rendering path. obs_get_output_source returns a strong reference that we release here.
            let program_source = libobs::obs_get_output_source(0);
            if program_source.is_null() {
                libobs::gs_blend_state_pop();
                libobs::gs_texrender_end(texrender);
                return Err("program channel 0 has no source".to_string());
            }
            libobs::obs_source_inc_showing(program_source);
            libobs::obs_source_video_render(program_source);
            libobs::obs_source_dec_showing(program_source);
            libobs::obs_source_release(program_source);

            libobs::gs_blend_state_pop();
            libobs::gs_texrender_end(texrender);

            let texture = libobs::gs_texrender_get_texture(texrender);
            if texture.is_null() {
                return Err("texrender produced no texture".to_string());
            }
            libobs::gs_stage_texture(stagesurface, texture);

            let mut mapped = std::ptr::null_mut::<u8>();
            let mut linesize = 0_u32;
            if !libobs::gs_stagesurface_map(stagesurface, &mut mapped, &mut linesize) {
                return Err("gs_stagesurface_map failed".to_string());
            }
            if mapped.is_null() || linesize < width * 4 {
                libobs::gs_stagesurface_unmap(stagesurface);
                return Err(format!(
                    "invalid staged frame buffer: ptr={mapped:p}, linesize={linesize}"
                ));
            }

            let mut pixels = vec![0_u8; (width * height * 4) as usize];
            let row_bytes = (width * 4) as usize;
            for y in 0..height as usize {
                let src = std::slice::from_raw_parts(mapped.add(y * linesize as usize), row_bytes);
                let dst = &mut pixels[y * row_bytes..(y + 1) * row_bytes];
                dst.copy_from_slice(src);
            }
            libobs::gs_stagesurface_unmap(stagesurface);

            Ok(RgbaFrame {
                width,
                height,
                pixels,
            })
        })();

        if !stagesurface.is_null() {
            libobs::gs_stagesurface_destroy(stagesurface);
        }
        if !texrender.is_null() {
            libobs::gs_texrender_destroy(texrender);
        }
        libobs::obs_leave_graphics();
        result
    })?;

    captured.map_err(ObsError::Unexpected)
}
