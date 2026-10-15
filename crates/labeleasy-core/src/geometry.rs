//! Plain 2D geometry in image pixel space (origin top-left, y down).

use serde::{Deserialize, Serialize};

/// a point in image coordinates. Serializes as `[x, y]`, the way labelme stores `points`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(from = "[f64; 2]", into = "[f64; 2]")]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl From<[f64; 2]> for Point {
    fn from([x, y]: [f64; 2]) -> Self {
        Point::new(x as f32, y as f32)
    }
}

impl From<Point> for [f64; 2] {
    fn from(p: Point) -> Self {
        // round-trip through the shortest decimal so files don't fill up with
        // f32 noise like 12.300000190734863
        let clean = |v: f32| v.to_string().parse::<f64>().unwrap_or(v as f64);
        [clean(p.x), clean(p.y)]
    }
}

impl Point {
    pub const ORIGIN: Point = Point { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Point) -> f32 {
        (self - other).length()
    }

    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// clamps into `[0, width] x [0, height]`.
    pub fn clamp_to(self, width: f32, height: f32) -> Point {
        Point::new(self.x.clamp(0.0, width), self.y.clamp(0.0, height))
    }
}

impl std::ops::Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::Mul<f32> for Point {
    type Output = Point;
    fn mul(self, rhs: f32) -> Point {
        Point::new(self.x * rhs, self.y * rhs)
    }
}

/// axis-aligned rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    /// the rectangle spanned by two opposite corners, in any order.
    pub fn from_corners(a: Point, b: Point) -> Self {
        Rect {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// bounding box of `points`, `None` if empty.
    pub fn bounding(points: &[Point]) -> Option<Self> {
        let first = *points.first()?;
        Some(
            points
                .iter()
                .fold(Rect::from_corners(first, first), |r, p| Rect {
                    min: Point::new(r.min.x.min(p.x), r.min.y.min(p.y)),
                    max: Point::new(r.max.x.max(p.x), r.max.y.max(p.y)),
                }),
        )
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

/// shortest distance from `p` to the segment `a`-`b`.
pub fn distance_to_segment(p: Point, a: Point, b: Point) -> f32 {
    p.distance(project_on_segment(p, a, b))
}

/// the point on segment `a`-`b` closest to `p`.
pub fn project_on_segment(p: Point, a: Point, b: Point) -> Point {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 <= f32::EPSILON {
        return a;
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len2).clamp(0.0, 1.0);
    a + ab * t
}

/// even-odd point-in-polygon test.
pub fn polygon_contains(polygon: &[Point], p: Point) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let (pi, pj) = (polygon[i], polygon[j]);
        if (pi.y > p.y) != (pj.y > p.y) && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// signed area (shoelace); positive for counter-clockwise in y-up space.
pub fn polygon_area(polygon: &[Point]) -> f32 {
    if polygon.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        sum += a.x * b.y - b.x * a.y;
    }
    sum / 2.0
}
