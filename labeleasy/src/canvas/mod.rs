//! The annotation canvas.
//!
//! The image itself is drawn by [`image_layer::ImageLayer`] stacked below
//! this canvas (see there for why). The canvas renders two layers:
//! - a cached layer with every shape, rebuilt only when the document,
//!   the selection/visibility, or the view transform changes;
//! - a per-frame overlay with hover highlights, the shape being drawn and
//!   drag previews.
//!
//! Transient interaction (hover, drags, a half-drawn polygon) lives in the
//! canvas' own state, so moving the mouse never round-trips through the app's
//! `update`/`view`. Only committed edits are published as [`CanvasEvent`]s, one per undo step.

mod draw;
pub mod image_layer;
pub mod viewport;

pub use draw::label_color;
use iced::keyboard::{self, Key, key::Named};
use iced::mouse::{self, Button, ScrollDelta};
use iced::widget::canvas::{self, Action, Cache, Event, Frame, Geometry};
use iced::{Point, Rectangle, Renderer, Size, Theme, Vector};
use labeleasy_core::geometry::Point as ImgPoint;
use labeleasy_core::shape::Hit;
use labeleasy_core::{LabelSet, Shape, ShapeType};
use std::cell::Cell;
use std::collections::{BTreeSet, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::{Duration, Instant};
pub use viewport::{Transform, ViewMode};

/// grab distance for vertices and edges, in screen pixels.
const HIT_TOLERANCE: f32 = 7.0;
/// a polygon closes when clicking this close (screen px) to its first point.
const CLOSE_TOLERANCE: f32 = 9.0;
const DOUBLE_CLICK: Duration = Duration::from_millis(350);
/// a press-drag-release farther than this draws a 2-point shape in one gesture.
const DRAG_THRESHOLD: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Edit,
    Create(ShapeType),
}

#[derive(Debug, Clone)]
pub enum CanvasEvent {
    Resized(Size),
    ViewChanged(ViewMode),
    ShapeDrawn(Shape),
    /// `None` clears; `additive` toggles `index` in the selection.
    Select {
        index: Option<usize>,
        additive: bool,
    },
    MoveShapes {
        indices: Vec<usize>,
        delta: ImgPoint,
    },
    MoveHandle {
        shape: usize,
        handle: usize,
        to: ImgPoint,
    },
    InsertVertex {
        shape: usize,
        index: usize,
        at: ImgPoint,
    },
    RemoveVertex {
        shape: usize,
        index: usize,
    },
    EditShape(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    pub fill_opacity: f32,
    pub show_labels: bool,
    pub crosshair: bool,
    /// plain wheel zooms (ctrl+wheel scrolls) instead of labelme's opposite.
    pub wheel_zooms: bool,
}

pub struct AnnotationCanvas<'a> {
    pub image_size: Size,
    pub shapes: &'a [Shape],
    pub selected: &'a BTreeSet<usize>,
    pub hidden: &'a HashSet<usize>,
    pub hidden_labels: &'a HashSet<String>,
    pub labels: &'a LabelSet,
    pub view: ViewMode,
    pub mode: Mode,
    pub options: Options,
    /// changes whenever another image is opened.
    pub doc_id: u64,
    /// the document's revision.
    pub revision: u64,
    /// bumped by the app for any other change affecting the cached layer
    /// (selection, visibility, label colors, options).
    pub canvas_rev: u64,
}

#[derive(Debug, Default)]
enum Interaction {
    #[default]
    None,
    Drawing {
        shape_type: ShapeType,
        points: Vec<ImgPoint>,
        /// where the first point was pressed (screen), for drag-to-draw.
        press: Point,
    },
    MovingShapes {
        indices: Vec<usize>,
        start: ImgPoint,
        current: ImgPoint,
    },
    MovingHandle {
        shape: usize,
        handle: usize,
        pos: ImgPoint,
        moved: bool,
    },
    /// a vertex grabbed off an edge. committed on release, as a single undo step.
    InsertingVertex {
        shape: usize,
        index: usize,
        pos: ImgPoint,
        /// where the edge was grabbed (screen); a click without dragging
        /// past `DRAG_THRESHOLD` inserts nothing.
        press: Point,
        moved: bool,
    },
}

/// a pan in progress. Each move pans from where the drag started (not from
/// the latest view), so moves batched into one frame - which all see the
/// same, not yet updated, view - don't lose distance.
#[derive(Debug, Clone, Copy)]
struct Pan {
    origin: Point,
    view: ViewMode,
    button: Button,
}

#[derive(Default)]
pub struct State {
    cache: Cache,
    cache_key: Cell<u64>,
    interaction: Interaction,
    /// panning is independent of `interaction`, so a half-drawn polygon
    /// survives panning around to place its next point.
    pan: Option<Pan>,
    /// the last wheel step: `(view it was based on, view published)`. While
    /// the app hasn't applied it yet (batched events), the next step chains
    /// from the published view instead of re-using the stale one.
    wheel: Option<(ViewMode, ViewMode)>,
    hover: Option<(usize, Hit)>,
    cursor: Option<Point>,
    modifiers: keyboard::Modifiers,
    size: Size,
    /// the size last published as `Resized`.
    reported_size: Size,
    space_down: bool,
    last_click: Option<(Instant, Point)>,
    doc_id: u64,
    revision: u64,
}

impl State {
    /// shapes drawn by the overlay instead of the cached layer while dragged.
    fn excluded(&self) -> &[usize] {
        match &self.interaction {
            Interaction::MovingShapes { indices, .. } => indices,
            Interaction::MovingHandle { shape, .. }
            | Interaction::InsertingVertex {
                shape, moved: true, ..
            } => std::slice::from_ref(shape),
            _ => &[],
        }
    }
}

impl AnnotationCanvas<'_> {
    fn transform(&self, size: Size) -> Transform {
        self.view.resolve(size, self.image_size)
    }

    pub fn is_visible(&self, index: usize) -> bool {
        !self.hidden.contains(&index)
            && self
                .shapes
                .get(index)
                .is_some_and(|s| !self.hidden_labels.contains(&s.label))
    }

    fn clamp(&self, p: ImgPoint) -> ImgPoint {
        p.clamp_to(self.image_size.width, self.image_size.height)
    }

    /// what's under `p`, by priority: a vertex (selected shapes first), then
    /// an edge, then a body - each topmost first. So a vertex stays grabbable
    /// even when another shape's body covers it.
    fn hit(&self, p: ImgPoint, tolerance: f32) -> Option<(usize, Hit)> {
        let visible = || {
            (0..self.shapes.len())
                .rev()
                .filter(|&i| self.is_visible(i))
                .filter(move |&i| self.shapes[i].near(p, tolerance))
        };
        let selected_first = self
            .selected
            .iter()
            .rev()
            .copied()
            .filter(|&i| self.is_visible(i))
            .chain(visible().filter(|i| !self.selected.contains(i)));

        let vertex = selected_first
            .filter_map(|i| {
                self.shapes
                    .get(i)?
                    .nearest_handle(p, tolerance)
                    .map(|h| (i, Hit::Vertex(h)))
            })
            .next();
        // edges only on selected shapes: on an unselected (possibly tiny,
        // zoomed-out) shape a click should select/move it, not reshape it
        vertex
            .or_else(|| {
                visible()
                    .filter(|i| self.selected.contains(i))
                    .find_map(|i| {
                        let shape = &self.shapes[i];
                        shape
                            .shape_type
                            .is_freeform()
                            .then(|| shape.nearest_edge(p, tolerance))
                            .flatten()
                            .map(|(index, at)| (i, Hit::Edge { index, at }))
                    })
            })
            .or_else(|| {
                visible()
                    .find(|&i| self.shapes[i].contains(p, tolerance))
                    .map(|i| (i, Hit::Body))
            })
    }

    fn finish_drawing(
        &self,
        state: &mut State,
        shape_type: ShapeType,
        points: Vec<ImgPoint>,
    ) -> Option<Action<CanvasEvent>> {
        state.interaction = Interaction::None;
        let shape = Shape::new(shape_type, points);
        Some(if shape.is_valid() {
            Action::publish(CanvasEvent::ShapeDrawn(shape)).and_capture()
        } else {
            Action::request_redraw().and_capture()
        })
    }

    fn on_left_press(
        &self,
        state: &mut State,
        pos: Point,
        t: Transform,
    ) -> Option<Action<CanvasEvent>> {
        let now = Instant::now();
        let double = state
            .last_click
            .is_some_and(|(at, p)| now.duration_since(at) < DOUBLE_CLICK && p.distance(pos) < 5.0);
        state.last_click = (!double).then_some((now, pos));

        let p = t.to_image(pos);
        match self.mode {
            Mode::Create(shape_type) => self.on_draw_click(state, shape_type, p, pos, double, t),
            Mode::Edit => self.on_edit_click(state, p, double, t),
        }
    }

    fn on_draw_click(
        &self,
        state: &mut State,
        shape_type: ShapeType,
        p: ImgPoint,
        pos: Point,
        double: bool,
        t: Transform,
    ) -> Option<Action<CanvasEvent>> {
        let p = self.clamp(p);
        let Interaction::Drawing { points, .. } = &mut state.interaction else {
            if shape_type.fixed_point_count() == Some(1) {
                return self.finish_drawing(state, shape_type, vec![p]);
            }
            state.interaction = Interaction::Drawing {
                shape_type,
                points: vec![p],
                press: pos,
            };
            return Some(Action::request_redraw().and_capture());
        };

        match shape_type {
            ShapeType::Polygon => {
                let closes =
                    points.len() >= 3 && t.to_screen(points[0]).distance(pos) <= CLOSE_TOLERANCE;
                if closes || (double && points.len() >= 3) {
                    let points = std::mem::take(points);
                    return self.finish_drawing(state, shape_type, points);
                }
                if !double {
                    points.push(p);
                }
            }
            ShapeType::LineStrip | ShapeType::Points => {
                if double && points.len() >= 2 {
                    let points = std::mem::take(points);
                    return self.finish_drawing(state, shape_type, points);
                }
                if !double {
                    points.push(p);
                }
            }
            _ => {
                points.push(p);
                if Some(points.len()) == shape_type.fixed_point_count() {
                    let points = std::mem::take(points);
                    return self.finish_drawing(state, shape_type, points);
                }
            }
        }
        Some(Action::request_redraw().and_capture())
    }

    fn on_edit_click(
        &self,
        state: &mut State,
        p: ImgPoint,
        double: bool,
        t: Transform,
    ) -> Option<Action<CanvasEvent>> {
        let tolerance = HIT_TOLERANCE / t.scale;
        let hit = self.hit(p, tolerance);
        let additive = state.modifiers.command();
        let select = |i: usize| CanvasEvent::Select {
            index: Some(i),
            additive: false,
        };

        if double && let Some((i, _)) = hit {
            state.interaction = Interaction::None;
            return Some(Action::publish(CanvasEvent::EditShape(i)).and_capture());
        }

        match hit {
            Some((i, Hit::Vertex(h)))
                if state.modifiers.alt() && self.shapes[i].can_remove_vertex() =>
            {
                Some(
                    Action::publish(CanvasEvent::RemoveVertex { shape: i, index: h }).and_capture(),
                )
            }
            Some((i, Hit::Vertex(h))) => {
                state.interaction = Interaction::MovingHandle {
                    shape: i,
                    handle: h,
                    pos: p,
                    moved: false,
                };
                Some(if self.selected.contains(&i) {
                    Action::request_redraw().and_capture()
                } else {
                    Action::publish(select(i)).and_capture()
                })
            }
            Some((i, Hit::Edge { index, at })) => {
                state.interaction = Interaction::InsertingVertex {
                    shape: i,
                    index,
                    pos: at,
                    press: t.to_screen(at),
                    moved: false,
                };
                Some(if self.selected.contains(&i) {
                    Action::request_redraw().and_capture()
                } else {
                    Action::publish(select(i)).and_capture()
                })
            }
            Some((i, Hit::Body)) if additive => Some(
                Action::publish(CanvasEvent::Select {
                    index: Some(i),
                    additive: true,
                })
                .and_capture(),
            ),
            Some((i, Hit::Body)) => {
                let already = self.selected.contains(&i);
                let indices = if already {
                    self.selected.iter().copied().collect()
                } else {
                    vec![i]
                };
                state.interaction = Interaction::MovingShapes {
                    indices,
                    start: p,
                    current: p,
                };
                Some(if already {
                    Action::request_redraw().and_capture()
                } else {
                    Action::publish(select(i)).and_capture()
                })
            }
            None if !additive && !self.selected.is_empty() => Some(
                Action::publish(CanvasEvent::Select {
                    index: None,
                    additive: false,
                })
                .and_capture(),
            ),
            None => Some(Action::capture()),
        }
    }

    fn on_release(
        &self,
        state: &mut State,
        pos: Option<Point>,
        t: Transform,
    ) -> Option<Action<CanvasEvent>> {
        match std::mem::take(&mut state.interaction) {
            Interaction::MovingShapes {
                indices,
                start,
                current,
            } => {
                let delta = current - start;
                (delta.length() * t.scale >= 0.5)
                    .then(|| Action::publish(CanvasEvent::MoveShapes { indices, delta }))
                    .or(Some(Action::request_redraw()))
            }
            Interaction::InsertingVertex {
                shape,
                index,
                pos: at,
                moved,
                ..
            } => Some(if moved {
                Action::publish(CanvasEvent::InsertVertex { shape, index, at })
            } else {
                Action::request_redraw()
            }),
            Interaction::MovingHandle {
                shape,
                handle,
                pos: to,
                moved,
            } => Some(if moved {
                Action::publish(CanvasEvent::MoveHandle { shape, handle, to })
            } else {
                Action::request_redraw()
            }),
            Interaction::Drawing {
                shape_type,
                mut points,
                press,
            } => {
                let dragged = pos.is_some_and(|p| p.distance(press) > DRAG_THRESHOLD);
                if shape_type.fixed_point_count() == Some(2) && points.len() == 1 && dragged {
                    points.push(self.clamp(t.to_image(pos.unwrap_or(press))));
                    return self.finish_drawing(state, shape_type, points);
                }
                state.interaction = Interaction::Drawing {
                    shape_type,
                    points,
                    press,
                };
                None
            }
            Interaction::None => None,
        }
    }

    fn on_key(&self, state: &mut State, key: &Key) -> Option<Action<CanvasEvent>> {
        let Interaction::Drawing {
            shape_type, points, ..
        } = &mut state.interaction
        else {
            return None;
        };
        match key {
            Key::Named(Named::Escape) => {
                state.interaction = Interaction::None;
                Some(Action::request_redraw().and_capture())
            }
            Key::Named(Named::Backspace) => {
                points.pop();
                if points.is_empty() {
                    state.interaction = Interaction::None;
                }
                Some(Action::request_redraw().and_capture())
            }
            Key::Named(Named::Enter) if points.len() >= shape_type.min_points() => {
                let (shape_type, points) = (*shape_type, std::mem::take(points));
                self.finish_drawing(state, shape_type, points)
            }
            _ => None,
        }
    }

    fn on_wheel(
        &self,
        state: &mut State,
        delta: &ScrollDelta,
        pos: Point,
    ) -> Option<Action<CanvasEvent>> {
        let (dx, dy) = match *delta {
            ScrollDelta::Lines { x, y } => (x * 40.0, y * 40.0),
            ScrollDelta::Pixels { x, y } => (x, y),
        };
        let base = match state.wheel {
            Some((based_on, published)) if based_on == self.view => published,
            _ => self.view,
        };
        let zoom = state.modifiers.command() != self.options.wheel_zooms;
        let view = if zoom {
            let factor = (dy * 0.0035).exp();
            base.zoom(state.size, self.image_size, factor, Some(pos))
        } else if state.modifiers.shift() {
            base.pan(state.size, self.image_size, Vector::new(dy, dx))
        } else {
            base.pan(state.size, self.image_size, Vector::new(dx, dy))
        };
        state.wheel = Some((self.view, view));
        Some(Action::publish(CanvasEvent::ViewChanged(view)).and_capture())
    }

    fn update_hover(&self, state: &mut State, t: Transform) {
        state.hover = match (&state.interaction, self.mode, state.cursor) {
            (Interaction::None, Mode::Edit, Some(pos)) => {
                self.hit(t.to_image(pos), HIT_TOLERANCE / t.scale)
            }
            _ => None,
        };
    }

    fn handle(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<CanvasEvent>> {
        let t = self.transform(bounds.size());
        let pos = cursor.position_in(bounds);

        match event {
            Event::Window(iced::window::Event::Unfocused) => {
                // key releases while unfocused never arrive
                state.space_down = false;
                state.modifiers = keyboard::Modifiers::default();
                None
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(m)) => {
                state.modifiers = *m;
                None
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if *key == Key::Named(Named::Space) && pos.is_some() {
                    state.space_down = true;
                    return None;
                }
                self.on_key(state, key)
            }
            Event::Keyboard(keyboard::Event::KeyReleased {
                key: Key::Named(Named::Space),
                ..
            }) => {
                state.space_down = false;
                None
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                state.cursor = None;
                state.hover = None;
                Some(Action::request_redraw())
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let previous = state.cursor;
                state.cursor = pos;
                if let Some(pan) = state.pan {
                    let view = pan
                        .view
                        .pan(state.size, self.image_size, *position - pan.origin);
                    return Some(Action::publish(CanvasEvent::ViewChanged(view)));
                }
                match &mut state.interaction {
                    Interaction::MovingShapes { current, .. } => {
                        if let Some(p) = pos {
                            *current = t.to_image(p);
                        }
                    }
                    Interaction::MovingHandle {
                        pos: handle_pos,
                        moved,
                        ..
                    } => {
                        if let Some(p) = pos {
                            *handle_pos = self.clamp(t.to_image(p));
                            *moved = true;
                        }
                    }
                    Interaction::InsertingVertex {
                        pos: vertex,
                        press,
                        moved,
                        ..
                    } => {
                        if let Some(p) = pos {
                            *moved |= p.distance(*press) > DRAG_THRESHOLD;
                            if *moved {
                                *vertex = self.clamp(t.to_image(p));
                            }
                        }
                    }
                    Interaction::None | Interaction::Drawing { .. } => {}
                }
                self.update_hover(state, t);
                (previous != state.cursor || !matches!(state.interaction, Interaction::None))
                    .then(Action::request_redraw)
            }
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                let local = pos?;
                let global = cursor.position()?;
                match button {
                    Button::Left if state.space_down => {
                        state.pan = Some(Pan {
                            origin: global,
                            view: self.view,
                            button: Button::Left,
                        });
                        Some(Action::capture())
                    }
                    Button::Left => self.on_left_press(state, local, t),
                    Button::Middle | Button::Right => {
                        state.pan = Some(Pan {
                            origin: global,
                            view: self.view,
                            button: *button,
                        });
                        Some(Action::capture())
                    }
                    _ => None,
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                if state.pan.is_some_and(|pan| pan.button == *button) {
                    state.pan = None;
                    return Some(Action::request_redraw());
                }
                if *button != Button::Left {
                    return None;
                }
                let action = self.on_release(state, pos, t);
                self.update_hover(state, t);
                action
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                pos.and_then(|p| self.on_wheel(state, delta, p))
            }
            _ => None,
        }
    }
}

impl canvas::Program<CanvasEvent> for AnnotationCanvas<'_> {
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<CanvasEvent>> {
        if state.doc_id != self.doc_id {
            state.doc_id = self.doc_id;
            state.interaction = Interaction::None;
            state.pan = None;
            state.hover = None;
        }
        // the document changed under a drag (e.g. undo mid-drag): its shape
        // indices may no longer mean the same shapes, so abandon the drag
        if state.revision != self.revision {
            state.revision = self.revision;
            state.hover = None;
            if !matches!(state.interaction, Interaction::Drawing { .. }) {
                state.interaction = Interaction::None;
            }
        }
        // switching tools abandons a half-drawn shape
        if let Interaction::Drawing { shape_type, .. } = state.interaction
            && self.mode != Mode::Create(shape_type)
        {
            state.interaction = Interaction::None;
        }
        state.size = bounds.size();
        let action = self.handle(state, event, bounds, cursor);
        // the new size is reported on an event nothing else wanted, so
        // reporting it never swallows a click or key press
        if action.is_none() && state.reported_size != state.size {
            state.reported_size = state.size;
            return Some(Action::publish(CanvasEvent::Resized(state.size)));
        }
        action
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let t = self.transform(bounds.size());
        let excluded = state.excluded();

        let mut hasher = DefaultHasher::new();
        (
            self.doc_id,
            self.revision,
            self.canvas_rev,
            t.key(),
            excluded,
        )
            .hash(&mut hasher);
        self.options.fill_opacity.to_bits().hash(&mut hasher);
        (
            self.options.show_labels,
            bounds.width.to_bits(),
            bounds.height.to_bits(),
        )
            .hash(&mut hasher);
        let key = hasher.finish();
        if state.cache_key.replace(key) != key {
            state.cache.clear();
        }

        let shapes = state.cache.draw(renderer, bounds.size(), |frame| {
            for (i, shape) in self.shapes.iter().enumerate() {
                if self.is_visible(i) && !excluded.contains(&i) {
                    let style = draw::ShapeStyle::new(self, i, false);
                    draw::shape(frame, shape, t, &style);
                }
            }
        });

        let mut overlay = Frame::new(renderer, bounds.size());
        self.draw_overlay(&mut overlay, state, t);
        vec![shapes, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }
        match (&state.interaction, self.mode) {
            _ if state.pan.is_some() => mouse::Interaction::Grabbing,
            _ if state.space_down => mouse::Interaction::Grab,
            (Interaction::MovingShapes { .. }, _) => mouse::Interaction::Move,
            (Interaction::MovingHandle { .. } | Interaction::InsertingVertex { .. }, _) => {
                mouse::Interaction::Crosshair
            }
            (_, Mode::Create(_)) => mouse::Interaction::Crosshair,
            (_, Mode::Edit) => match state.hover {
                Some((_, Hit::Vertex(_))) => mouse::Interaction::Crosshair,
                Some((_, Hit::Edge { .. })) => mouse::Interaction::Copy,
                Some((_, Hit::Body)) => mouse::Interaction::Pointer,
                None => mouse::Interaction::default(),
            },
        }
    }
}

impl AnnotationCanvas<'_> {
    fn draw_overlay(&self, frame: &mut Frame, state: &State, t: Transform) {
        match &state.interaction {
            Interaction::MovingShapes {
                indices,
                start,
                current,
            } => {
                let (w, h) = (self.image_size.width, self.image_size.height);
                let delta = indices
                    .iter()
                    .filter_map(|&i| self.shapes.get(i))
                    .fold(*current - *start, |d, s| s.clamp_delta(d, w, h));
                for &i in indices {
                    if let Some(shape) = self.shapes.get(i) {
                        let mut moved = shape.clone();
                        moved.translate(delta);
                        draw::shape(frame, &moved, t, &draw::ShapeStyle::new(self, i, true));
                    }
                }
            }
            Interaction::MovingHandle {
                shape, handle, pos, ..
            } => {
                if let Some(s) = self.shapes.get(*shape) {
                    let mut edited = s.clone();
                    edited.move_handle(*handle, *pos);
                    let mut style = draw::ShapeStyle::new(self, *shape, true);
                    style.active_handle = Some(*handle);
                    draw::shape(frame, &edited, t, &style);
                }
            }
            Interaction::InsertingVertex {
                shape,
                index,
                pos,
                moved: true,
                ..
            } => {
                if let Some(s) = self.shapes.get(*shape) {
                    let mut edited = s.clone();
                    edited
                        .points
                        .insert((*index).min(edited.points.len()), *pos);
                    let mut style = draw::ShapeStyle::new(self, *shape, true);
                    style.active_handle = Some(*index);
                    draw::shape(frame, &edited, t, &style);
                }
            }
            Interaction::Drawing {
                shape_type, points, ..
            } => {
                let color = draw::drawing_color();
                draw::in_progress(
                    frame,
                    *shape_type,
                    points,
                    state.cursor.map(|c| self.clamp(t.to_image(c))),
                    t,
                    color,
                );
                if *shape_type == ShapeType::Polygon
                    && points.len() >= 3
                    && let Some(c) = state.cursor
                    && t.to_screen(points[0]).distance(c) <= CLOSE_TOLERANCE
                {
                    draw::close_marker(frame, t.to_screen(points[0]), color);
                }
            }
            // an edge grabbed but not dragged yet: the shape stays as it is
            Interaction::None | Interaction::InsertingVertex { .. } => {}
        }

        if let Some((i, hit)) = state.hover
            && let Some(shape) = self.shapes.get(i)
        {
            draw::hover(
                frame,
                shape,
                hit,
                t,
                draw::label_color(self.labels, &shape.label),
            );
        }

        if let Some(cursor) = state.cursor {
            if self.options.crosshair && matches!(self.mode, Mode::Create(_)) {
                draw::crosshair(frame, cursor);
            }
            draw::cursor_readout(frame, t.to_image(cursor), t.scale);
        }
    }
}
