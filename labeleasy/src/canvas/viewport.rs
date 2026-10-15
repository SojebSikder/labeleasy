//! Mapping between image pixels and canvas (screen) coordinates.

use iced::{Point, Size, Vector};
use labeleasy_core::geometry::Point as ImgPoint;

pub const MIN_SCALE: f32 = 0.01;
pub const MAX_SCALE: f32 = 64.0;
/// margin kept around the image when fitting.
const FIT_MARGIN: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    FitWindow,
    FitWidth,
    Manual { scale: f32, center: ImgPoint },
}

/// `screen = offset + image * scale`, in canvas-local coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub scale: f32,
    pub offset: Vector,
}

impl Transform {
    pub fn to_screen(self, p: ImgPoint) -> Point {
        Point::new(
            self.offset.x + p.x * self.scale,
            self.offset.y + p.y * self.scale,
        )
    }

    pub fn to_image(self, p: Point) -> ImgPoint {
        ImgPoint::new(
            (p.x - self.offset.x) / self.scale,
            (p.y - self.offset.y) / self.scale,
        )
    }

    /// a key for cache invalidation (exact float bits).
    pub fn key(self) -> (u32, u32, u32) {
        (
            self.scale.to_bits(),
            self.offset.x.to_bits(),
            self.offset.y.to_bits(),
        )
    }
}

fn fit_scale(canvas: Size, image: Size, width_only: bool) -> f32 {
    if image.width <= 0.0 || image.height <= 0.0 {
        return 1.0;
    }
    let avail_w = (canvas.width - 2.0 * FIT_MARGIN).max(1.0);
    let avail_h = (canvas.height - 2.0 * FIT_MARGIN).max(1.0);
    let sx = avail_w / image.width;
    let s = if width_only {
        sx
    } else {
        sx.min(avail_h / image.height)
    };
    s.clamp(MIN_SCALE, MAX_SCALE)
}

impl ViewMode {
    pub fn resolve(&self, canvas: Size, image: Size) -> Transform {
        let (scale, center) = match *self {
            ViewMode::FitWindow => (
                fit_scale(canvas, image, false),
                ImgPoint::new(image.width / 2.0, image.height / 2.0),
            ),
            ViewMode::FitWidth => {
                let s = fit_scale(canvas, image, true);
                // pin to the top when the image is taller than the canvas
                let half_visible = canvas.height / 2.0 / s;
                let cy = if image.height * s > canvas.height {
                    half_visible - FIT_MARGIN / s
                } else {
                    image.height / 2.0
                };
                (s, ImgPoint::new(image.width / 2.0, cy))
            }
            ViewMode::Manual { scale, center } => (scale, center),
        };
        Transform {
            scale,
            offset: Vector::new(
                canvas.width / 2.0 - center.x * scale,
                canvas.height / 2.0 - center.y * scale,
            ),
        }
    }

    pub fn scale(&self, canvas: Size, image: Size) -> f32 {
        self.resolve(canvas, image).scale
    }

    /// zooms by `factor`, keeping the image point under `anchor` (canvas
    /// coordinates; the center if `None`) where it is.
    pub fn zoom(&self, canvas: Size, image: Size, factor: f32, anchor: Option<Point>) -> ViewMode {
        let t = self.resolve(canvas, image);
        self.zoom_to(canvas, image, t.scale * factor, anchor)
    }

    pub fn zoom_to(
        &self,
        canvas: Size,
        image: Size,
        scale: f32,
        anchor: Option<Point>,
    ) -> ViewMode {
        let t = self.resolve(canvas, image);
        let scale = scale.clamp(MIN_SCALE, MAX_SCALE);
        let anchor = anchor.unwrap_or(Point::new(canvas.width / 2.0, canvas.height / 2.0));
        let fixed = t.to_image(anchor);
        // choose center so `fixed` maps back to `anchor` at the new scale
        let center = ImgPoint::new(
            fixed.x - (anchor.x - canvas.width / 2.0) / scale,
            fixed.y - (anchor.y - canvas.height / 2.0) / scale,
        );
        ViewMode::Manual { scale, center }
    }

    /// moves the view by `delta` screen pixels.
    pub fn pan(&self, canvas: Size, image: Size, delta: Vector) -> ViewMode {
        let t = self.resolve(canvas, image);
        let center = t.to_image(Point::new(canvas.width / 2.0, canvas.height / 2.0));
        ViewMode::Manual {
            scale: t.scale,
            center: ImgPoint::new(center.x - delta.x / t.scale, center.y - delta.y / t.scale),
        }
    }
}
