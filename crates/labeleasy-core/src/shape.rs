//! One annotation. Field names and `shape_type` strings match labelme's JSON.

use crate::geometry::{self, Point, Rect};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeType {
    /// labelme treats a missing `shape_type` as a polygon; see [`Shape`]'s
    /// deserializer for the 1- and 2-point exceptions.
    #[default]
    Polygon,
    Rectangle,
    /// `points` = `[center, a point on the circumference]`.
    Circle,
    Line,
    Point,
    #[serde(rename = "linestrip")]
    LineStrip,
    /// a set of unconnected points.
    Points,
    /// `points` = bbox corners, pixels in the base64 PNG `mask`.
    Mask,
}

impl ShapeType {
    /// the types a user can draw, in toolbar order.
    pub const DRAWABLE: [ShapeType; 6] = [
        ShapeType::Polygon,
        ShapeType::Rectangle,
        ShapeType::Circle,
        ShapeType::Line,
        ShapeType::Point,
        ShapeType::LineStrip,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ShapeType::Polygon => "Polygon",
            ShapeType::Rectangle => "Rectangle",
            ShapeType::Circle => "Circle",
            ShapeType::Line => "Line",
            ShapeType::Point => "Point",
            ShapeType::LineStrip => "Line Strip",
            ShapeType::Points => "Points",
            ShapeType::Mask => "Mask",
        }
    }

    /// whether the outline joins the last point back to the first.
    pub fn is_closed(self) -> bool {
        matches!(
            self,
            ShapeType::Polygon | ShapeType::Rectangle | ShapeType::Mask
        )
    }

    /// shapes made of exactly this many clicks finish themselves.
    pub fn fixed_point_count(self) -> Option<usize> {
        match self {
            ShapeType::Rectangle | ShapeType::Circle | ShapeType::Line | ShapeType::Mask => Some(2),
            ShapeType::Point => Some(1),
            ShapeType::Polygon | ShapeType::LineStrip | ShapeType::Points => None,
        }
    }

    /// fewest points a finished shape may have.
    pub fn min_points(self) -> usize {
        match self {
            ShapeType::Polygon => 3,
            ShapeType::LineStrip => 2,
            ShapeType::Points => 1,
            other => other.fixed_point_count().unwrap_or(1),
        }
    }

    /// whether vertices can be inserted on / removed from edges.
    pub fn is_freeform(self) -> bool {
        matches!(self, ShapeType::Polygon | ShapeType::LineStrip)
    }
}

impl std::fmt::Display for ShapeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RawShape")]
pub struct Shape {
    pub label: String,
    pub points: Vec<Point>,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default, deserialize_with = "null_as_default")]
    pub description: String,
    pub shape_type: ShapeType,
    #[serde(default, deserialize_with = "null_as_default")]
    pub flags: BTreeMap<String, bool>,
    #[serde(default)]
    pub mask: Option<String>,
    /// keys written by other tools, preserved on save.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// [`Shape`] as read from disk, before a missing `shape_type` is inferred.
#[derive(Deserialize)]
struct RawShape {
    label: String,
    points: Vec<Point>,
    #[serde(default)]
    group_id: Option<i64>,
    #[serde(default, deserialize_with = "null_as_default")]
    description: String,
    #[serde(default)]
    shape_type: Option<ShapeType>,
    #[serde(default, deserialize_with = "null_as_default")]
    flags: BTreeMap<String, bool>,
    #[serde(default)]
    mask: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

impl From<RawShape> for Shape {
    fn from(raw: RawShape) -> Self {
        // labelme defaults to a polygon, but a polygon needs 3+ points: files
        // that omit the type with 2 points mean a bbox.
        let shape_type = raw.shape_type.unwrap_or(match raw.points.len() {
            1 => ShapeType::Point,
            2 => ShapeType::Rectangle,
            _ => ShapeType::Polygon,
        });
        Self {
            label: raw.label,
            points: raw.points,
            group_id: raw.group_id,
            description: raw.description,
            shape_type,
            flags: raw.flags,
            mask: raw.mask,
            extra: raw.extra,
        }
    }
}

pub(crate) fn null_as_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// what part of a shape a cursor is over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Hit {
    /// an editable handle, see [`Shape::handles`].
    Vertex(usize),
    /// an edge: inserting at `index` would put a new vertex at `at`.
    Edge {
        index: usize,
        at: Point,
    },
    Body,
}

impl Shape {
    pub fn new(shape_type: ShapeType, points: Vec<Point>) -> Self {
        Self {
            label: String::new(),
            points,
            group_id: None,
            description: String::new(),
            shape_type,
            flags: BTreeMap::new(),
            mask: None,
            extra: Default::default(),
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// whether the shape has enough (distinct) points to be kept.
    pub fn is_valid(&self) -> bool {
        if self.points.len() < self.shape_type.min_points() {
            return false;
        }
        let (a, b) = (
            self.points[0],
            *self.points.get(1).unwrap_or(&self.points[0]),
        );
        match self.shape_type {
            // a rectangle needs area, not just two distinct corners
            ShapeType::Rectangle | ShapeType::Mask => {
                (a.x - b.x).abs() > f32::EPSILON && (a.y - b.y).abs() > f32::EPSILON
            }
            ShapeType::Circle | ShapeType::Line => a.distance(b) > f32::EPSILON,
            ShapeType::Polygon => geometry::polygon_area(&self.points).abs() > f32::EPSILON,
            _ => true,
        }
    }

    pub fn circle_radius(&self) -> f32 {
        match self.points.as_slice() {
            [c, r, ..] => c.distance(*r),
            _ => 0.0,
        }
    }

    /// the corner points that make up the outline as drawn: a rectangle's
    /// four corners (it only stores two), every point otherwise.
    pub fn outline(&self) -> Vec<Point> {
        match (self.shape_type, self.points.as_slice()) {
            (ShapeType::Rectangle | ShapeType::Mask, [a, b, ..]) => {
                let r = Rect::from_corners(*a, *b);
                vec![
                    r.min,
                    Point::new(r.max.x, r.min.y),
                    r.max,
                    Point::new(r.min.x, r.max.y),
                ]
            }
            _ => self.points.clone(),
        }
    }

    /// draggable handles. Rectangles get all four corners so any can be grabbed.
    /// everything else exposes its stored points.
    pub fn handles(&self) -> Vec<Point> {
        match self.shape_type {
            ShapeType::Rectangle => self.outline(),
            // a mask's bbox must keep the size of its bitmap: move only
            ShapeType::Mask => Vec::new(),
            _ => self.points.clone(),
        }
    }

    pub fn bounds(&self) -> Option<Rect> {
        match self.shape_type {
            ShapeType::Circle => {
                let c = *self.points.first()?;
                let r = self.circle_radius();
                Some(Rect::from_corners(
                    Point::new(c.x - r, c.y - r),
                    Point::new(c.x + r, c.y + r),
                ))
            }
            _ => Rect::bounding(&self.outline()),
        }
    }

    /// segments of the outline as `(index of first point, a, b)`.
    fn edges(&self) -> Vec<(usize, Point, Point)> {
        let outline = self.outline();
        let n = outline.len();
        if n < 2 {
            return Vec::new();
        }
        let count = if self.shape_type.is_closed() {
            n
        } else {
            n - 1
        };
        (0..count)
            .map(|i| (i, outline[i], outline[(i + 1) % n]))
            .collect()
    }

    /// cheap pre-check for hit testing: whether `p` is within `tolerance`
    /// of the shape's bounding box. Allocation-free.
    pub fn near(&self, p: Point, tolerance: f32) -> bool {
        let Some(first) = self.points.first() else {
            return false;
        };
        if self.shape_type == ShapeType::Circle {
            return first.distance(p) <= self.circle_radius() + tolerance;
        }
        let (mut min, mut max) = (*first, *first);
        for q in &self.points[1..] {
            min = Point::new(min.x.min(q.x), min.y.min(q.y));
            max = Point::new(max.x.max(q.x), max.y.max(q.y));
        }
        p.x >= min.x - tolerance
            && p.x <= max.x + tolerance
            && p.y >= min.y - tolerance
            && p.y <= max.y + tolerance
    }

    /// hit test in image space. `tolerance` is in image pixels (callers scale
    /// a screen-space tolerance by the zoom so grabbing feels the same at any zoom).
    pub fn hit_test(&self, p: Point, tolerance: f32) -> Option<Hit> {
        if !self.near(p, tolerance) {
            return None;
        }
        if let Some(i) = self.nearest_handle(p, tolerance) {
            return Some(Hit::Vertex(i));
        }
        if self.shape_type.is_freeform()
            && let Some((index, at)) = self.nearest_edge(p, tolerance)
        {
            return Some(Hit::Edge { index, at });
        }
        self.contains(p, tolerance).then_some(Hit::Body)
    }

    pub fn nearest_handle(&self, p: Point, tolerance: f32) -> Option<usize> {
        self.handles()
            .iter()
            .enumerate()
            .map(|(i, h)| (i, h.distance(p)))
            .filter(|&(_, d)| d <= tolerance)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// the edge nearest `p` within `tolerance`, as `(insert index, projected point)`.
    pub fn nearest_edge(&self, p: Point, tolerance: f32) -> Option<(usize, Point)> {
        self.edges()
            .into_iter()
            .map(|(i, a, b)| (i + 1, geometry::project_on_segment(p, a, b)))
            .map(|(i, at)| (i, at, at.distance(p)))
            .filter(|&(_, _, d)| d <= tolerance)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(i, at, _)| (i, at))
    }

    pub fn contains(&self, p: Point, tolerance: f32) -> bool {
        match self.shape_type {
            ShapeType::Polygon => geometry::polygon_contains(&self.points, p),
            ShapeType::Rectangle | ShapeType::Mask => self.bounds().is_some_and(|r| r.contains(p)),
            ShapeType::Circle => self
                .points
                .first()
                .is_some_and(|c| c.distance(p) <= self.circle_radius()),
            ShapeType::Line | ShapeType::LineStrip => self
                .edges()
                .iter()
                .any(|&(_, a, b)| geometry::distance_to_segment(p, a, b) <= tolerance),
            ShapeType::Point | ShapeType::Points => {
                self.points.iter().any(|q| q.distance(p) <= tolerance)
            }
        }
    }

    pub fn translate(&mut self, delta: Point) {
        for p in &mut self.points {
            *p = *p + delta;
        }
    }

    /// moves handle `index` (see [`Shape::handles`]) to `to`.
    pub fn move_handle(&mut self, index: usize, to: Point) {
        match self.shape_type {
            ShapeType::Mask => {}
            ShapeType::Rectangle => {
                let corners = self.outline();
                if corners.len() == 4 && index < 4 {
                    let opposite = corners[(index + 2) % 4];
                    let r = Rect::from_corners(opposite, to);
                    self.points = vec![r.min, r.max];
                }
            }
            _ => {
                if let Some(p) = self.points.get_mut(index) {
                    *p = to;
                }
            }
        }
    }

    pub fn can_insert_vertex(&self) -> bool {
        self.shape_type.is_freeform()
    }

    pub fn can_remove_vertex(&self) -> bool {
        self.shape_type.is_freeform() && self.points.len() > self.shape_type.min_points()
    }

    /// the offset that keeps every point inside `[0, w] x [0, h]` when moved by `delta`.
    pub fn clamp_delta(&self, delta: Point, width: f32, height: f32) -> Point {
        let Some(b) = Rect::bounding(&self.points) else {
            return delta;
        };
        let clamp = |d: f32, lo: f32, hi: f32, limit: f32| {
            if hi - lo > limit {
                d
            } else {
                d.clamp(-lo, limit - hi)
            }
        };
        Point::new(
            clamp(delta.x, b.min.x, b.max.x, width),
            clamp(delta.y, b.min.y, b.max.y, height),
        )
    }

    /// stores a rectangle's corners as `[top-left, bottom-right]`, the way
    /// most downstream converters expect.
    pub fn normalize(&mut self) {
        if self.shape_type == ShapeType::Rectangle && self.points.len() >= 2 {
            let r = Rect::from_corners(self.points[0], self.points[1]);
            self.points = vec![r.min, r.max];
        }
    }
}
