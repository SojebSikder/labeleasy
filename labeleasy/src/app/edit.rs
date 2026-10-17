//! Shape editing: canvas events, selection, clipboard, the label dialog.

use super::{App, Overlay};
use crate::canvas::{CanvasEvent, Mode};
use crate::message::Message;
use crate::ui::label_dialog::{self, LabelDialog, LabelDialogMessage, LabelTarget, Outcome};
use iced::{Size, Task};
use labeleasy_core::Point;
use labeleasy_core::document::ShapeMeta;

const SHAPE_ROW_HEIGHT: f32 = crate::ui::sidebar::SHAPE_ROW_HEIGHT;
const DUPLICATE_OFFSET: Point = Point::new(10.0, 10.0);

pub fn image_size(app: &App) -> Size {
    app.image
        .as_ref()
        .map(|i| Size::new(i.width as f32, i.height as f32))
        .unwrap_or(Size::new(1.0, 1.0))
}

/// structural edits (delete, undo, ...) can shift indices: drop anything index-based.
fn reset_index_state(app: &mut App) {
    app.selected.clear();
    app.hidden.clear();
    app.touch_canvas();
}

fn select_only(app: &mut App, index: usize) -> Task<Message> {
    app.selected.clear();
    app.selected.insert(index);
    app.touch_canvas();
    app.shape_list.reveal(index, SHAPE_ROW_HEIGHT)
}

/// edits are refused while another image loads: the old document is
/// about to be replaced, and anything done to it now would be lost (or,
/// for a pending dialog, land on the new image).
fn is_edit(event: &CanvasEvent) -> bool {
    !matches!(
        event,
        CanvasEvent::Resized(_) | CanvasEvent::ViewChanged(_) | CanvasEvent::Select { .. }
    )
}

pub fn canvas_event(app: &mut App, event: CanvasEvent) -> Task<Message> {
    if is_edit(&event) && !app.editable() {
        return Task::none();
    }
    match event {
        CanvasEvent::Resized(size) => app.canvas_size = size,
        CanvasEvent::ViewChanged(view) => app.view = view,
        CanvasEvent::ShapeDrawn(shape) => {
            if app.doc.is_some() {
                let dialog = LabelDialog::new(
                    LabelTarget::New(Box::new(shape)),
                    app.doc_id,
                    ShapeMeta {
                        label: app.last_label.clone().unwrap_or_default(),
                        flags: default_shape_flags(app),
                        ..Default::default()
                    },
                );
                app.overlay = Overlay::LabelDialog(dialog);
                return label_dialog::focus();
            }
        }
        CanvasEvent::Select { index: None, .. } => {
            app.selected.clear();
            app.touch_canvas();
        }
        CanvasEvent::Select {
            index: Some(i),
            additive: true,
        } => {
            if !app.selected.remove(&i) {
                app.selected.insert(i);
            }
            app.touch_canvas();
        }
        CanvasEvent::Select {
            index: Some(i),
            additive: false,
        } => return select_only(app, i),
        CanvasEvent::MoveShapes { indices, delta } => {
            if let Some(doc) = &mut app.doc {
                doc.translate(&indices, delta);
            }
        }
        CanvasEvent::MoveHandle { shape, handle, to } => {
            if let Some(doc) = &mut app.doc {
                doc.move_handle(shape, handle, to);
            }
        }
        CanvasEvent::InsertVertex { shape, index, at } => {
            if let Some(doc) = &mut app.doc {
                doc.insert_vertex(shape, index, at);
            }
        }
        CanvasEvent::RemoveVertex { shape, index } => {
            if let Some(doc) = &mut app.doc
                && !doc.remove_vertex(shape, index)
            {
                app.toasts
                    .info("A shape needs at least its minimum number of points");
            }
        }
        CanvasEvent::EditShape(i) => return open_label_dialog_for(app, i),
    }
    Task::none()
}

fn default_shape_flags(app: &App) -> std::collections::BTreeMap<String, bool> {
    app.settings
        .shape_flags
        .iter()
        .map(|f| (f.clone(), false))
        .collect()
}

pub fn open_label_dialog_for(app: &mut App, index: usize) -> Task<Message> {
    if !app.editable() {
        return Task::none();
    }
    let Some(shape) = app.doc.as_ref().and_then(|d| d.shapes().get(index)) else {
        return Task::none();
    };
    let mut meta = ShapeMeta::of(shape);
    for (flag, value) in default_shape_flags(app) {
        meta.flags.entry(flag).or_insert(value);
    }

    app.overlay = Overlay::LabelDialog(LabelDialog::new(
        LabelTarget::Existing(index),
        app.doc_id,
        meta,
    ));
    Task::batch([select_only(app, index), label_dialog::focus()])
}

pub fn label_dialog(app: &mut App, msg: LabelDialogMessage) -> Task<Message> {
    let editable = app.editable();
    let Overlay::LabelDialog(dialog) = &mut app.overlay else {
        return Task::none();
    };
    match dialog.update(msg) {
        Outcome::None => Task::none(),
        Outcome::Cancel => {
            app.overlay = Overlay::None;
            Task::none()
        }
        Outcome::Submit(meta) => {
            if dialog.doc_id != app.doc_id || !editable {
                app.overlay = Overlay::None;
                app.toasts
                    .info("The image changed; that edit was discarded");
                return Task::none();
            }
            if meta.label.is_empty() {
                dialog.error = Some("Enter a label".into());
                return Task::none();
            }
            if app.settings.validate_labels && !app.labels.contains(&meta.label) {
                dialog.error = Some(format!("\"{}\" isn't in the label list", meta.label));
                return Task::none();
            }
            let Overlay::LabelDialog(dialog) = std::mem::replace(&mut app.overlay, Overlay::None)
            else {
                return Task::none();
            };
            let Some(doc) = &mut app.doc else {
                return Task::none();
            };
            app.labels.insert(&meta.label);
            app.last_label = Some(meta.label.clone());
            match dialog.target {
                LabelTarget::New(mut shape) => {
                    meta.apply(&mut shape);
                    let index = doc.add_shape(*shape);
                    return select_only(app, index);
                }
                LabelTarget::Existing(index) => doc.update_meta(index, meta),
            }
            app.touch_canvas();
            Task::none()
        }
    }
}

pub fn shape_clicked(app: &mut App, index: usize) -> Task<Message> {
    app.mode = Mode::Edit;
    select_only(app, index)
}

pub fn select_label(app: &mut App, label: &str) -> Task<Message> {
    let Some(doc) = &app.doc else {
        return Task::none();
    };
    app.selected = doc
        .shapes()
        .iter()
        .enumerate()
        .filter(|(_, s)| s.label == label)
        .map(|(i, _)| i)
        .filter(|&i| app.is_shape_visible(i))
        .collect();
    app.mode = Mode::Edit;
    app.touch_canvas();
    Task::none()
}

pub fn select_all(app: &mut App) -> Task<Message> {
    let Some(doc) = &app.doc else {
        return Task::none();
    };
    app.selected = (0..doc.shapes().len())
        .filter(|&i| app.is_shape_visible(i))
        .collect();
    app.mode = Mode::Edit;
    app.touch_canvas();
    Task::none()
}

pub fn undo(app: &mut App, undo: bool) -> Task<Message> {
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    let changed = if undo { doc.undo() } else { doc.redo() };
    if changed {
        reset_index_state(app);
    }
    Task::none()
}

pub fn delete_selected(app: &mut App) -> Task<Message> {
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    let indices: Vec<usize> = app.selected.iter().copied().collect();
    if !indices.is_empty() {
        doc.delete(&indices);
        reset_index_state(app);
    }
    Task::none()
}

pub fn duplicate_selected(app: &mut App) -> Task<Message> {
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    let indices: Vec<usize> = app.selected.iter().copied().collect();
    let copies = doc.duplicate(&indices, DUPLICATE_OFFSET);
    if !copies.is_empty() {
        app.selected = copies.into_iter().collect();
        app.touch_canvas();
    }
    Task::none()
}

pub fn copy(app: &mut App) -> Task<Message> {
    let Some(doc) = &app.doc else {
        return Task::none();
    };
    app.clipboard = app
        .selected
        .iter()
        .filter_map(|&i| doc.shapes().get(i).cloned())
        .collect();
    if !app.clipboard.is_empty() {
        app.toasts
            .info(format!("Copied {} shape(s)", app.clipboard.len()));
    }
    Task::none()
}

/// pastes at the original position, so shapes can be carried across images.
pub fn paste(app: &mut App) -> Task<Message> {
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    let pasted = doc.add_shapes(app.clipboard.clone());
    if !pasted.is_empty() {
        app.selected = pasted.into_iter().collect();
        app.mode = Mode::Edit;
        app.touch_canvas();
    }
    Task::none()
}

pub fn nudge(app: &mut App, dx: i8, dy: i8) -> Task<Message> {
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    let indices: Vec<usize> = app.selected.iter().copied().collect();
    if !indices.is_empty() {
        doc.translate(&indices, Point::new(dx as f32, dy as f32));
    }
    Task::none()
}

pub fn zoom(app: &mut App, factor: f32) -> Task<Message> {
    let image = image_size(app);
    app.view = app.view.zoom(app.canvas_size, image, factor, None);
    Task::none()
}

pub fn toggle_all_hidden(app: &mut App) -> Task<Message> {
    let Some(doc) = &app.doc else {
        return Task::none();
    };
    if app.hidden.is_empty() {
        app.hidden = (0..doc.shapes().len()).collect();
        app.selected.clear();
    } else {
        app.hidden.clear();
    }
    app.touch_canvas();
    Task::none()
}
