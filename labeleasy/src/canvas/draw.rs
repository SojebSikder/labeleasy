//! Rendering of the image and shapes. Geometry is transformed to screen space
//! before building paths so stroke widths and handles stay constant at any zoom.

use super::{AnnotationCanvas, Transform};
use iced::widget::canvas::{Frame, Path, Stroke, Text, path::Builder};
use iced::{Color, Pixels, Point, Size};
use labeleasy_core::geometry::Point as ImgPoint;
use labeleasy_core::shape::Hit;
use labeleasy_core::{LabelSet, Shape, ShapeType};

const STROKE: f32 = 2.0;
const HANDLE: f32 = 7.0;
const POINT_RADIUS: f32 = 4.0;

pub fn label_color(labels: &LabelSet, label: &str) -> Color {
    let c = labels.color(label);
    Color::from_rgb8(c.r, c.g, c.b)
}

/// the color of a shape that has no label yet.
pub fn drawing_color() -> Color {
    Color::from_rgb8(0, 220, 120)
}

fn alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

pub struct ShapeStyle {
    pub color: Color,
    pub selected: bool,
    pub fill_opacity: f32,
    pub show_label: bool,
    pub active_handle: Option<usize>,
}

impl ShapeStyle {
    pub fn new(canvas: &AnnotationCanvas<'_>, index: usize, force_selected: bool) -> Self {
        let shape = &canvas.shapes[index];
        Self {
            color: label_color(canvas.labels, &shape.label),
            selected: force_selected || canvas.selected.contains(&index),
            fill_opacity: canvas.options.fill_opacity,
            show_label: canvas.options.show_labels,
            active_handle: None,
        }
    }
}

/// the path of `points` in screen space.
fn polyline(points: &[ImgPoint], t: Transform, closed: bool) -> Path {
    let mut b = Builder::new();
    if let Some((first, rest)) = points.split_first() {
        b.move_to(t.to_screen(*first));
        for p in rest {
            b.line_to(t.to_screen(*p));
        }
        if closed {
            b.close();
        }
    }
    b.build()
}

fn outline_path(shape: &Shape, t: Transform) -> Option<Path> {
    match shape.shape_type {
        ShapeType::Circle => {
            let center = *shape.points.first()?;
            Some(Path::circle(
                t.to_screen(center),
                shape.circle_radius() * t.scale,
            ))
        }
        ShapeType::Point | ShapeType::Points => None,
        _ => Some(polyline(&shape.outline(), t, shape.shape_type.is_closed())),
    }
}

pub fn shape(frame: &mut Frame, shape: &Shape, t: Transform, style: &ShapeStyle) {
    let stroke_color = style.color;
    if let Some(path) = outline_path(shape, t) {
        let filled = shape.shape_type.is_closed() || shape.shape_type == ShapeType::Circle;
        if filled {
            let fill = if style.selected {
                (style.fill_opacity + 0.25).min(0.8)
            } else {
                style.fill_opacity
            };
            if fill > 0.0 {
                frame.fill(&path, alpha(style.color, fill));
            }
        }
        let width = if style.selected { STROKE + 0.5 } else { STROKE };
        frame.stroke(
            &path,
            Stroke::default().with_color(stroke_color).with_width(width),
        );
    }

    // point shapes are their handles; others show handles when selected
    let is_point = matches!(shape.shape_type, ShapeType::Point | ShapeType::Points);
    if is_point || style.selected {
        for (i, h) in shape.handles().iter().enumerate() {
            let active = style.active_handle == Some(i);
            handle(frame, t.to_screen(*h), style.color, style.selected, active);
        }
    }

    if style.show_label
        && !shape.label.is_empty()
        && let Some(b) = shape.bounds()
    {
        label_tag(frame, t.to_screen(b.min), &shape.label, style.color);
    }
}

fn handle(frame: &mut Frame, at: Point, color: Color, selected: bool, active: bool) {
    let size = if active { HANDLE + 3.0 } else { HANDLE };
    if selected {
        let half = size / 2.0;
        let rect = Path::rectangle(Point::new(at.x - half, at.y - half), Size::new(size, size));
        frame.fill(&rect, if active { color } else { Color::WHITE });
        frame.stroke(&rect, Stroke::default().with_color(color).with_width(1.5));
    } else {
        let dot = Path::circle(at, POINT_RADIUS);
        frame.fill(&dot, color);
        frame.stroke(
            &dot,
            Stroke::default().with_color(Color::WHITE).with_width(1.0),
        );
    }
}

fn label_tag(frame: &mut Frame, at: Point, label: &str, color: Color) {
    const SIZE: f32 = 12.0;
    let width = label.chars().count() as f32 * SIZE * 0.6 + 8.0;
    let top = (at.y - SIZE - 6.0).max(0.0);
    frame.fill_rectangle(
        Point::new(at.x, top),
        Size::new(width, SIZE + 6.0),
        alpha(color, 0.85),
    );
    let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
    frame.fill_text(Text {
        content: label.to_string(),
        position: Point::new(at.x + 4.0, top + 3.0),
        color: if luminance > 0.6 {
            Color::BLACK
        } else {
            Color::WHITE
        },
        size: Pixels(SIZE),
        ..Text::default()
    });
}

/// highlight for whatever the cursor is over in edit mode.
pub fn hover(frame: &mut Frame, shape: &Shape, hit: Hit, t: Transform, color: Color) {
    match hit {
        Hit::Vertex(i) => {
            if let Some(p) = shape.handles().get(i) {
                let ring = Path::circle(t.to_screen(*p), HANDLE);
                frame.fill(&ring, alpha(Color::WHITE, 0.6));
                frame.stroke(&ring, Stroke::default().with_color(color).with_width(2.0));
            }
        }
        Hit::Edge { at, .. } => {
            let dot = Path::circle(t.to_screen(at), POINT_RADIUS);
            frame.fill(&dot, Color::WHITE);
            frame.stroke(&dot, Stroke::default().with_color(color).with_width(1.5));
        }
        Hit::Body => {
            if let Some(path) = outline_path(shape, t) {
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(alpha(Color::WHITE, 0.8))
                        .with_width(STROKE + 2.0),
                );
                frame.stroke(
                    &path,
                    Stroke::default().with_color(color).with_width(STROKE),
                );
            }
        }
    }
}

/// a shape being drawn, with a rubber band to the cursor.
pub fn in_progress(
    frame: &mut Frame,
    shape_type: ShapeType,
    points: &[ImgPoint],
    cursor: Option<ImgPoint>,
    t: Transform,
    color: Color,
) {
    let mut preview_points = points.to_vec();
    if let Some(c) = cursor {
        preview_points.push(c);
    }
    let preview = match shape_type {
        ShapeType::Rectangle | ShapeType::Circle | ShapeType::Line if preview_points.len() >= 2 => {
            Some(Shape::new(shape_type, preview_points[..2].to_vec()))
        }
        ShapeType::Polygon | ShapeType::LineStrip => {
            Some(Shape::new(ShapeType::LineStrip, preview_points))
        }
        _ => None,
    };
    if let Some(preview) = preview
        && let Some(path) = outline_path(&preview, t)
    {
        if matches!(shape_type, ShapeType::Rectangle | ShapeType::Circle) {
            frame.fill(&path, alpha(color, 0.15));
        }
        frame.stroke(
            &path,
            Stroke::default().with_color(color).with_width(STROKE),
        );
    }
    for p in points {
        handle(frame, t.to_screen(*p), color, false, false);
    }
}

pub fn close_marker(frame: &mut Frame, at: Point, color: Color) {
    let ring = Path::circle(at, CLOSE_RING);
    frame.fill(&ring, alpha(color, 0.35));
    frame.stroke(
        &ring,
        Stroke::default().with_color(Color::WHITE).with_width(1.5),
    );
}
const CLOSE_RING: f32 = 8.0;

pub fn crosshair(frame: &mut Frame, at: Point) {
    let size = frame.size();
    let stroke = Stroke::default()
        .with_color(alpha(Color::WHITE, 0.55))
        .with_width(1.0);
    frame.stroke(
        &Path::line(Point::new(0.0, at.y), Point::new(size.width, at.y)),
        stroke,
    );
    frame.stroke(
        &Path::line(Point::new(at.x, 0.0), Point::new(at.x, size.height)),
        stroke,
    );
}

/// image coordinates under the cursor and the zoom level, bottom-left.
pub fn cursor_readout(frame: &mut Frame, p: ImgPoint, scale: f32) {
    let text = format!("x {:.0}  y {:.0}   {:.0}%", p.x, p.y, scale * 100.0);
    let y = frame.height() - 22.0;
    frame.fill_rectangle(
        Point::new(6.0, y - 2.0),
        Size::new(text.len() as f32 * 6.6 + 10.0, 18.0),
        Color::from_rgba(0.0, 0.0, 0.0, 0.55),
    );
    frame.fill_text(Text {
        content: text,
        position: Point::new(11.0, y + 1.0),
        color: Color::WHITE,
        size: Pixels(11.0),
        font: iced::Font::MONOSPACE,
        ..Text::default()
    });
}
