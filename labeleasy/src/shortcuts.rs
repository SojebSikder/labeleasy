//! Keyboard shortcuts. [`BINDINGS`] is the single source of truth: it drives
//! the key handler, the hints shown in menus and the shortcuts dialog.

use crate::canvas::Mode;
use crate::message::{Command, Message};
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Event, Subscription, event};
use labeleasy_core::ShapeType;

#[derive(Debug, Clone, Copy)]
pub enum KeySpec {
    Char(&'static str),
    Named(Named),
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub key: KeySpec,
    pub ctrl: bool,
    pub shift: bool,
    pub command: Command,
    pub help: &'static str,
}

const fn bind(
    key: &'static str,
    ctrl: bool,
    shift: bool,
    command: Command,
    help: &'static str,
) -> Binding {
    Binding {
        key: KeySpec::Char(key),
        ctrl,
        shift,
        command,
        help,
    }
}

const fn named(key: Named, ctrl: bool, command: Command, help: &'static str) -> Binding {
    Binding {
        key: KeySpec::Named(key),
        ctrl,
        shift: false,
        command,
        help,
    }
}

const fn tool(key: &'static str, shape: ShapeType, help: &'static str) -> Binding {
    bind(
        key,
        false,
        false,
        Command::SetMode(Mode::Create(shape)),
        help,
    )
}

pub const BINDINGS: &[Binding] = &[
    bind(
        "o",
        true,
        false,
        Command::OpenFiles,
        "Open images or label files",
    ),
    bind("u", true, false, Command::OpenDir, "Open folder"),
    bind("s", true, false, Command::Save, "Save"),
    bind("w", true, false, Command::CloseFile, "Close image"),
    bind("q", true, false, Command::Quit, "Quit"),
    bind("d", false, false, Command::NextImage, "Next image"),
    bind("a", false, false, Command::PrevImage, "Previous image"),
    bind(
        "d",
        false,
        true,
        Command::NextUnannotated,
        "Next unannotated image",
    ),
    bind("z", true, false, Command::Undo, "Undo"),
    bind("y", true, false, Command::Redo, "Redo"),
    bind("z", true, true, Command::Redo, "Redo"),
    bind(
        "e",
        false,
        false,
        Command::SetMode(Mode::Edit),
        "Edit mode (select / move)",
    ),
    bind("j", true, false, Command::SetMode(Mode::Edit), "Edit mode"),
    tool("p", ShapeType::Polygon, "Draw polygons"),
    bind(
        "n",
        true,
        false,
        Command::SetMode(Mode::Create(ShapeType::Polygon)),
        "Draw polygons",
    ),
    tool("r", ShapeType::Rectangle, "Draw rectangles"),
    bind(
        "r",
        true,
        false,
        Command::SetMode(Mode::Create(ShapeType::Rectangle)),
        "Draw rectangles",
    ),
    tool("c", ShapeType::Circle, "Draw circles"),
    tool("l", ShapeType::Line, "Draw lines"),
    tool("t", ShapeType::Point, "Draw points"),
    tool("s", ShapeType::LineStrip, "Draw line strips"),
    named(
        Named::Delete,
        false,
        Command::DeleteSelected,
        "Delete selected shapes",
    ),
    named(
        Named::Backspace,
        false,
        Command::DeleteSelected,
        "Delete selected shapes",
    ),
    bind(
        "d",
        true,
        false,
        Command::DuplicateSelected,
        "Duplicate selected shapes",
    ),
    bind("c", true, false, Command::Copy, "Copy selected shapes"),
    bind("v", true, false, Command::Paste, "Paste shapes"),
    bind(
        "e",
        true,
        false,
        Command::EditLabel,
        "Edit label of selected shape",
    ),
    bind("a", true, false, Command::SelectAll, "Select all shapes"),
    named(
        Named::Escape,
        false,
        Command::Deselect,
        "Deselect / cancel drawing",
    ),
    named(
        Named::ArrowLeft,
        false,
        Command::Nudge(-1, 0),
        "Nudge selection",
    ),
    named(
        Named::ArrowRight,
        false,
        Command::Nudge(1, 0),
        "Nudge selection",
    ),
    named(
        Named::ArrowUp,
        false,
        Command::Nudge(0, -1),
        "Nudge selection",
    ),
    named(
        Named::ArrowDown,
        false,
        Command::Nudge(0, 1),
        "Nudge selection",
    ),
    bind("=", true, false, Command::ZoomIn, "Zoom in"),
    bind("+", true, true, Command::ZoomIn, "Zoom in"),
    bind("-", true, false, Command::ZoomOut, "Zoom out"),
    bind("0", true, false, Command::ZoomOriginal, "Zoom to 100%"),
    bind("f", true, false, Command::FitWindow, "Fit window"),
    bind("f", true, true, Command::FitWidth, "Fit width"),
    bind(
        "h",
        true,
        false,
        Command::ToggleShapesHidden,
        "Hide / show all shapes",
    ),
    bind(
        "l",
        true,
        true,
        Command::ToggleShowLabels,
        "Show / hide label tags",
    ),
    bind(",", true, false, Command::OpenSettings, "Settings"),
    named(
        Named::F1,
        false,
        Command::ShowShortcuts,
        "Keyboard shortcuts",
    ),
];

impl Binding {
    pub fn label(&self) -> String {
        let mut s = String::new();
        if self.ctrl {
            s.push_str(if cfg!(target_os = "macos") {
                "Cmd+"
            } else {
                "Ctrl+"
            });
        }
        if self.shift {
            s.push_str("Shift+");
        }
        match self.key {
            KeySpec::Char(c) => s.push_str(&c.to_uppercase()),
            KeySpec::Named(n) => s.push_str(&format!("{n:?}")),
        }
        s
    }

    fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        let key_matches = match (self.key, key) {
            (KeySpec::Char(c), Key::Character(k)) => c.eq_ignore_ascii_case(k.as_str()),
            (KeySpec::Named(n), Key::Named(k)) => n == *k,
            _ => false,
        };
        // shift changes the character for symbols ("=" -> "+"), so only
        // letters and named keys require an exact shift match
        let shift_ok = match self.key {
            KeySpec::Char(c) if !c.chars().all(char::is_alphabetic) => true,
            _ => modifiers.shift() == self.shift,
        };
        key_matches && shift_ok && modifiers.command() == self.ctrl && !modifiers.alt()
    }
}

/// the first shortcut bound to `command`, for menu hints.
pub fn hint(command: Command) -> Option<String> {
    BINDINGS
        .iter()
        .find(|b| b.command == command)
        .map(Binding::label)
}

fn lookup(key: &Key, modifiers: Modifiers) -> Option<Command> {
    // prefer the binding that also matches shift exactly (Ctrl+Z vs Ctrl+Shift+Z)
    BINDINGS
        .iter()
        .filter(|b| b.matches(key, modifiers))
        .max_by_key(|b| b.shift == modifiers.shift())
        .map(|b| b.command)
}

/// key presses nothing else handled (a focused text field captures its own).
pub fn subscription() -> Subscription<Message> {
    event::listen_with(|event, status, _window| match (event, status) {
        (
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }),
            event::Status::Ignored,
        ) => lookup(&key, modifiers).map(Message::Command),
        _ => None,
    })
}

/// Escape (even from inside a text field) and Enter while a dialog is open.
pub fn dialog_subscription() -> Subscription<Message> {
    event::listen_with(|event, _status, _window| match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        }) => Some(Message::CloseOverlay),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str, ctrl: bool, shift: bool) -> Option<Command> {
        let mut m = Modifiers::empty();
        if ctrl {
            m |= Modifiers::CTRL;
            if cfg!(target_os = "macos") {
                m |= Modifiers::LOGO;
            }
        }
        if shift {
            m |= Modifiers::SHIFT;
        }
        lookup(&Key::Character(key.into()), m)
    }

    #[test]
    fn resolves_modifier_variants() {
        if cfg!(target_os = "macos") {
            return;
        }
        assert_eq!(press("d", false, false), Some(Command::NextImage));
        assert_eq!(press("D", false, true), Some(Command::NextUnannotated));
        assert_eq!(press("d", true, false), Some(Command::DuplicateSelected));
        assert_eq!(press("z", true, false), Some(Command::Undo));
        assert_eq!(press("Z", true, true), Some(Command::Redo));
        assert_eq!(press("+", true, true), Some(Command::ZoomIn));
        assert_eq!(press("=", true, false), Some(Command::ZoomIn));
        assert_eq!(press("x", false, false), None);
    }

    #[test]
    fn hints() {
        assert!(hint(Command::Save).unwrap().ends_with("+S"));
        assert_eq!(hint(Command::NextImage).as_deref(), Some("D"));
    }
}
