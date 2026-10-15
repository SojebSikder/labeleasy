//! The menu bar's contents, built from the app's current state.

use crate::app::App;
use crate::canvas::Mode;
use crate::message::Command;
use crate::shortcuts::hint;
use labeleasy_core::ShapeType;
use labeleasy_ui::widgets::{DropdownItem, MenuGroup};

fn item(label: impl Into<String>, command: Command) -> DropdownItem<Command> {
    let item = DropdownItem::new(label, command);
    match hint(command) {
        Some(h) => item.with_shortcut(h),
        None => item,
    }
}

fn item_if(enabled: bool, label: impl Into<String>, command: Command) -> DropdownItem<Command> {
    let item = DropdownItem::when(enabled, label, command);
    match hint(command) {
        Some(h) => item.with_shortcut(h),
        None => item,
    }
}

fn check(on: bool, label: &str) -> String {
    format!("{} {label}", if on { "✓" } else { "  " })
}

pub fn groups(app: &App) -> Vec<MenuGroup<Command>> {
    let has_doc = app.doc.is_some();
    let has_dataset = !app.dataset.is_empty();
    let has_selection = !app.selected.is_empty();
    let s = &app.settings;

    let mut file = vec![
        item("Open…", Command::OpenFiles),
        item("Open Folder…", Command::OpenDir),
    ];

    for (i, dir) in app.settings.recent_dirs.iter().take(5).enumerate() {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| dir.display().to_string());
        file.push(DropdownItem::new(
            format!("    {name}"),
            Command::OpenRecent(i),
        ));
    }

    file.extend([
        DropdownItem::separator(),
        item_if(
            has_dataset,
            "Change Output Folder…",
            Command::ChangeOutputDir,
        ),
        item_if(has_doc, "Save", Command::Save),
        item_if(has_doc, "Delete Label File", Command::DeleteLabelFile),
        item_if(has_doc, "Close Image", Command::CloseFile),
        DropdownItem::separator(),
        DropdownItem::submenu(
            "Import",
            labeleasy_core::import::all()
                .iter()
                .enumerate()
                .map(|(i, importer)| item(format!("{}…", importer.name()), Command::Import(i)))
                .collect(),
        ),
        DropdownItem::submenu(
            "Export",
            labeleasy_core::export::all()
                .iter()
                .enumerate()
                .map(|(i, exporter)| {
                    item_if(
                        has_dataset,
                        format!("{}…", exporter.name()),
                        Command::Export(i),
                    )
                })
                .collect(),
        ),
        DropdownItem::separator(),
        DropdownItem::new(check(s.auto_save, "Auto Save"), Command::ToggleAutoSave),
        DropdownItem::new(
            check(s.save_image_data, "Embed Image Data"),
            Command::ToggleSaveImageData,
        ),
        DropdownItem::new(
            check(s.keep_previous, "Keep Previous Annotation"),
            Command::ToggleKeepPrevious,
        ),
        DropdownItem::separator(),
        item("Quit", Command::Quit),
    ]);

    let doc = app.doc.as_ref();
    let mut edit = vec![
        item_if(doc.is_some_and(|d| d.can_undo()), "Undo", Command::Undo),
        item_if(doc.is_some_and(|d| d.can_redo()), "Redo", Command::Redo),
        DropdownItem::separator(),
        item_if(has_doc, "Edit Mode", Command::SetMode(Mode::Edit)),
    ];
    for shape in ShapeType::DRAWABLE {
        edit.push(item_if(
            has_doc,
            format!("Draw {}", shape.label()),
            Command::SetMode(Mode::Create(shape)),
        ));
    }
    edit.extend([
        DropdownItem::separator(),
        item_if(has_selection, "Edit Label", Command::EditLabel),
        item_if(has_selection, "Duplicate", Command::DuplicateSelected),
        item_if(has_selection, "Copy", Command::Copy),
        item_if(has_doc, "Paste", Command::Paste),
        item_if(has_selection, "Delete", Command::DeleteSelected),
        item_if(has_doc, "Select All", Command::SelectAll),
        DropdownItem::separator(),
        item("Load Labels…", Command::LoadLabels),
        item_if(!app.labels.is_empty(), "Save Labels…", Command::SaveLabels),
        item("Settings…", Command::OpenSettings),
    ]);

    let view = vec![
        item_if(has_doc, "Zoom In", Command::ZoomIn),
        item_if(has_doc, "Zoom Out", Command::ZoomOut),
        item_if(has_doc, "Original Size", Command::ZoomOriginal),
        item_if(has_doc, "Fit Window", Command::FitWindow),
        item_if(has_doc, "Fit Width", Command::FitWidth),
        DropdownItem::separator(),
        item_if(has_dataset, "Next Image", Command::NextImage),
        item_if(has_dataset, "Previous Image", Command::PrevImage),
        item_if(has_dataset, "Next Unannotated", Command::NextUnannotated),
        DropdownItem::separator(),
        item_if(has_doc, "Hide / Show Shapes", Command::ToggleShapesHidden),
        item(
            check(s.show_labels, "Label Tags"),
            Command::ToggleShowLabels,
        ),
    ];

    let help = vec![
        item("Keyboard Shortcuts", Command::ShowShortcuts),
        DropdownItem::new("About", Command::About),
    ];

    vec![
        MenuGroup::new("File", file),
        MenuGroup::new("Edit", edit),
        MenuGroup::new("View", view),
        MenuGroup::new("Help", help),
    ]
}
