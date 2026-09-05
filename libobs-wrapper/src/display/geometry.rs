//! Preview viewport geometry and editor coordinate conversion.

/// How preview content is mapped into the display viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewScaleMode {
    /// Preserve aspect ratio and show all content, leaving letterbox/pillarbox regions as needed.
    #[default]
    Fit,
    /// Preserve aspect ratio and cover the viewport, cropping content at the edges as needed.
    Fill,
    /// Independently scale X/Y to fill the viewport.
    Stretch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreviewPoint {
    pub x: f32,
    pub y: f32,
}

impl PreviewPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PreviewRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Geometry uses physical pixels for the viewport/render rectangle and content-space pixels for
/// the OBS canvas/source. `dpi_scale` converts UI logical pointer coordinates to physical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewGeometry {
    pub content_width: f32,
    pub content_height: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub dpi_scale: f32,
    pub render_rect: PreviewRect,
    pub mode: PreviewScaleMode,
}

impl PreviewGeometry {
    pub fn new(
        content_width: u32,
        content_height: u32,
        viewport_width: u32,
        viewport_height: u32,
        mode: PreviewScaleMode,
        dpi_scale: f32,
    ) -> Option<Self> {
        if content_width == 0
            || content_height == 0
            || viewport_width == 0
            || viewport_height == 0
            || !dpi_scale.is_finite()
            || dpi_scale <= 0.0
        {
            return None;
        }
        let cw = content_width as f32;
        let ch = content_height as f32;
        let vw = viewport_width as f32;
        let vh = viewport_height as f32;
        let render_rect = match mode {
            PreviewScaleMode::Stretch => PreviewRect {
                x: 0.0,
                y: 0.0,
                width: vw,
                height: vh,
            },
            PreviewScaleMode::Fit | PreviewScaleMode::Fill => {
                let sx = vw / cw;
                let sy = vh / ch;
                let scale = if mode == PreviewScaleMode::Fit {
                    sx.min(sy)
                } else {
                    sx.max(sy)
                };
                let width = cw * scale;
                let height = ch * scale;
                PreviewRect {
                    x: (vw - width) * 0.5,
                    y: (vh - height) * 0.5,
                    width,
                    height,
                }
            }
        };
        Some(Self {
            content_width: cw,
            content_height: ch,
            viewport_width: vw,
            viewport_height: vh,
            dpi_scale,
            render_rect,
            mode,
        })
    }

    /// Converts a physical viewport point to content coordinates. Fit-mode letterbox areas return
    /// `None`; fill-mode cropped viewport points remain valid and map into the visible crop.
    pub fn viewport_to_content(&self, point: PreviewPoint) -> Option<PreviewPoint> {
        if point.x < 0.0
            || point.y < 0.0
            || point.x > self.viewport_width
            || point.y > self.viewport_height
        {
            return None;
        }
        if self.mode == PreviewScaleMode::Fit
            && (point.x < self.render_rect.x
                || point.y < self.render_rect.y
                || point.x > self.render_rect.x + self.render_rect.width
                || point.y > self.render_rect.y + self.render_rect.height)
        {
            return None;
        }
        Some(PreviewPoint {
            x: (point.x - self.render_rect.x) * self.content_width / self.render_rect.width,
            y: (point.y - self.render_rect.y) * self.content_height / self.render_rect.height,
        })
    }

    /// Converts a UI logical point using this geometry's DPI scale before mapping to content.
    pub fn logical_viewport_to_content(&self, point: PreviewPoint) -> Option<PreviewPoint> {
        self.viewport_to_content(PreviewPoint {
            x: point.x * self.dpi_scale,
            y: point.y * self.dpi_scale,
        })
    }

    pub fn content_to_viewport(&self, point: PreviewPoint) -> PreviewPoint {
        PreviewPoint {
            x: self.render_rect.x + point.x * self.render_rect.width / self.content_width,
            y: self.render_rect.y + point.y * self.render_rect.height / self.content_height,
        }
    }

    pub fn content_to_logical_viewport(&self, point: PreviewPoint) -> PreviewPoint {
        let physical = self.content_to_viewport(point);
        PreviewPoint {
            x: physical.x / self.dpi_scale,
            y: physical.y / self.dpi_scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.001, "{a} != {b}");
    }

    #[test]
    fn fit_letterboxes_and_rejects_pointer_in_bars() {
        let geometry =
            PreviewGeometry::new(1920, 1080, 1000, 1000, PreviewScaleMode::Fit, 1.0).unwrap();
        close(geometry.render_rect.width, 1000.0);
        close(geometry.render_rect.height, 562.5);
        close(geometry.render_rect.y, 218.75);
        assert!(geometry
            .viewport_to_content(PreviewPoint::new(500.0, 50.0))
            .is_none());
        let center = geometry
            .viewport_to_content(PreviewPoint::new(500.0, 500.0))
            .unwrap();
        close(center.x, 960.0);
        close(center.y, 540.0);
    }

    #[test]
    fn fill_crops_and_maps_visible_center() {
        let geometry =
            PreviewGeometry::new(1920, 1080, 1000, 1000, PreviewScaleMode::Fill, 1.0).unwrap();
        assert!(geometry.render_rect.x < 0.0);
        let center = geometry
            .viewport_to_content(PreviewPoint::new(500.0, 500.0))
            .unwrap();
        close(center.x, 960.0);
        close(center.y, 540.0);
    }

    #[test]
    fn stretch_and_dpi_round_trip() {
        let geometry =
            PreviewGeometry::new(1280, 720, 1600, 900, PreviewScaleMode::Stretch, 2.0).unwrap();
        let content = PreviewPoint::new(320.0, 180.0);
        let logical = geometry.content_to_logical_viewport(content);
        let round_trip = geometry.logical_viewport_to_content(logical).unwrap();
        close(round_trip.x, content.x);
        close(round_trip.y, content.y);
    }

    #[test]
    fn invalid_zero_or_dpi_geometry_is_rejected() {
        assert!(PreviewGeometry::new(0, 1080, 100, 100, PreviewScaleMode::Fit, 1.0).is_none());
        assert!(PreviewGeometry::new(1920, 1080, 100, 100, PreviewScaleMode::Fit, 0.0).is_none());
    }
}
