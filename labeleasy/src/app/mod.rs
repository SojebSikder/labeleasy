//! Application state and the top-level `update`.
mod edit;
mod files;
mod theme;

pub use edit::image_size;
pub use theme::{apply as apply_theme, reload as reload_themes, user_themes_dir};

use crate::canvas::{Mode, ViewMode};
use crate::image_cache::{ImageCache, LoadedImage};
use crate::message::{Command, Message};
use crate::settings::{self, Settings};
use crate::ui::label_dialog::LabelDialog;
use crate::ui::settings_modal::SettingsDraft;
use iced::{Size, Task, window};
use labeleasy_core::{Dataset, Document, LabelSet, Shape};
use labeleasy_ui::theme::{ActiveTheme, Appearance, ThemeRegistry};
use labeleasy_ui::widgets::{DropdownMenuState, ToastManager, VirtualListState};
use std::collections::{BTreeSet, HashSet};
use std::path::PathBuf;

/// at most one dialog is open at a time.
pub enum Overlay {
    None,
    LabelDialog(LabelDialog),
    Settings(Box<SettingsDraft>),
    Shortcuts,
    About,
    /// unsaved changes: save or discard, then replay `then`.
    ConfirmUnsaved {
        then: Box<Message>,
    },
}

impl Overlay {
    pub fn is_open(&self) -> bool {
        !matches!(self, Overlay::None)
    }
}

pub struct App {
    pub window: window::Id,
    pub maximized: bool,
    pub window_width: f32,

    pub themes: ThemeRegistry,
    pub theme: ActiveTheme,
    pub system_appearance: Appearance,
    pub settings: Settings,

    pub menu: DropdownMenuState,
    pub toasts: ToastManager,
    pub overlay: Overlay,

    pub dataset: Dataset,
    pub file_filter: String,
    /// indices into `dataset.entries` matching `file_filter`.
    pub filtered: Vec<usize>,
    pub file_list: VirtualListState,
    pub current: Option<usize>,

    pub doc: Option<Document>,
    pub image: Option<LoadedImage>,
    /// bumped when another image is opened (resets canvas interaction).
    pub doc_id: u64,
    /// id of the newest load request; stale results are dropped.
    pub(crate) load_request: u64,
    pub loading: Option<PathBuf>,
    cache: ImageCache,
    /// images being decoded ahead of time.
    prefetching: HashSet<PathBuf>,

    /// every label known this session: predefined + seen in files.
    pub labels: LabelSet,
    pub mode: Mode,
    pub view: ViewMode,
    pub canvas_size: Size,
    pub selected: BTreeSet<usize>,
    pub hidden: HashSet<usize>,
    pub hidden_labels: HashSet<String>,
    /// bumped on any change to what the canvas' cached layer shows besides the document.
    pub canvas_rev: u64,
    pub shape_list: VirtualListState,

    clipboard: Vec<Shape>,
    /// shapes of the last image left, for "keep previous annotation".
    previous_shapes: Vec<Shape>,
    pub last_label: Option<String>,
    /// sidebar drag: (cursor x at start, width at start).
    pub sidebar_drag: Option<(Option<f32>, f32)>,
}

/// command-line options: `labeleasy [IMAGE|DIR|JSON] [--output DIR] [--labels FILE]`.
#[derive(Debug, Default)]
struct Args {
    path: Option<PathBuf>,
    output: Option<PathBuf>,
    labels: Option<PathBuf>,
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut it = std::env::args_os().skip(1);
    while let Some(arg) = it.next() {
        match arg.to_str() {
            Some("--output" | "-o") => args.output = it.next().map(PathBuf::from),
            Some("--labels") => args.labels = it.next().map(PathBuf::from),
            _ => args.path = Some(PathBuf::from(arg)),
        }
    }
    args
}

pub fn init() -> (App, Task<Message>) {
    // a daemon doesn't open a window by itself. opening it here gives `App`
    // the real window id from the start.
    let icon = window::icon::from_file_data(crate::APP_ICON, None).ok();
    let (window, open) = window::open(window::Settings {
        size: Size::new(1360.0, 860.0),
        min_size: Some(Size::new(720.0, 480.0)),
        icon,
        exit_on_close_request: false,
        decorations: false,
        ..Default::default()
    });

    let settings = settings::load();
    let themes = theme::load_registry();
    let system_appearance = Appearance::Dark;
    let active = theme::resolve(&themes, &settings, system_appearance);
    labeleasy_ui::theme::publish(&active);

    let mut app = App {
        window,
        maximized: false,
        window_width: 1360.0,
        themes,
        theme: active,
        system_appearance,
        labels: LabelSet::from_iter(&settings.labels),
        settings,
        menu: DropdownMenuState::new(),
        toasts: ToastManager::new(),
        overlay: Overlay::None,
        dataset: Dataset::default(),
        file_filter: String::new(),
        filtered: Vec::new(),
        file_list: VirtualListState::new("file-list"),
        current: None,
        doc: None,
        image: None,
        doc_id: 0,
        load_request: 0,
        loading: None,
        cache: ImageCache::default(),
        prefetching: HashSet::new(),
        mode: Mode::Edit,
        view: ViewMode::FitWindow,
        canvas_size: Size::new(1000.0, 700.0),
        selected: BTreeSet::new(),
        hidden: HashSet::new(),
        hidden_labels: HashSet::new(),
        canvas_rev: 0,
        shape_list: VirtualListState::new("shape-list"),
        clipboard: Vec::new(),
        previous_shapes: Vec::new(),
        last_label: None,
        sidebar_drag: None,
    };

    let args = parse_args();
    if let Some(labels) = &args.labels {
        match LabelSet::load_txt(labels) {
            Ok(set) => {
                for label in set.iter() {
                    app.labels.insert(label);
                }
            }
            Err(e) => {
                app.toasts.error(e.to_string());
            }
        }
    }
    let startup = match args.path {
        Some(path) => files::open_path(&mut app, path, args.output),
        None => Task::none(),
    };

    let tasks = Task::batch([
        open.then(|id| {
            window::run(id, labeleasy_ui::widgets::titlebar::apply_rounded_corners).discard()
        }),
        iced::system::theme().map(Message::SystemThemeChanged),
        startup,
    ]);
    (app, tasks)
}

impl App {
    pub fn title(&self) -> String {
        match (&self.doc, self.current) {
            (Some(doc), _) => {
                let name = doc
                    .image_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let dirty = if doc.is_dirty() { " •" } else { "" };
                format!("{name}{dirty} - {}", crate::APP_NAME)
            }
            _ => crate::APP_NAME.to_string(),
        }
    }

    /// whether the open document may be edited: not while the next image
    /// loads (it's about to be replaced).
    pub fn editable(&self) -> bool {
        self.doc.is_some() && self.loading.is_none()
    }

    /// not hidden on its own nor through its label.
    pub fn is_shape_visible(&self, index: usize) -> bool {
        !self.hidden.contains(&index)
            && self
                .doc
                .as_ref()
                .and_then(|d| d.shapes().get(index))
                .is_some_and(|s| !self.hidden_labels.contains(&s.label))
    }

    /// invalidates the canvas' cached layer for non-document changes.
    pub fn touch_canvas(&mut self) {
        self.canvas_rev = self.canvas_rev.wrapping_add(1);
    }

    pub fn persist_settings(&mut self) {
        if let Err(e) = settings::save(&self.settings) {
            self.toasts.error(format!("Couldn't save settings: {e}"));
        }
    }

    fn toggle_setting(&mut self, f: impl FnOnce(&mut Settings) -> bool, what: &str) {
        let on = f(&mut self.settings);
        self.persist_settings();
        self.toasts
            .info(format!("{what} {}", if on { "on" } else { "off" }));
    }
}

pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::TitleBarDrag => window::drag(app.window),
        Message::Minimize => window::minimize(app.window, true),
        Message::ToggleMaximize => {
            app.maximized = !app.maximized;
            window::maximize(app.window, app.maximized)
        }
        Message::WindowResizeDrag(direction) => {
            if app.maximized {
                Task::none()
            } else {
                window::drag_resize(app.window, direction)
            }
        }
        Message::WindowEvent(event) => match event {
            window::Event::CloseRequested => update(app, Message::Command(Command::Quit)),
            window::Event::Opened { size, .. } => {
                app.window_width = size.width;
                Task::none()
            }
            window::Event::Resized(size) => {
                app.window_width = size.width;
                // maximizing through the OS (Win+Up, snapping, double-click
                // on a snapped edge) doesn't go through our titlebar button
                window::is_maximized(app.window).map(Message::MaximizedChanged)
            }
            // a drop while a dialog is up would swap the document out from under it
            window::Event::FileDropped(_) if app.overlay.is_open() => Task::none(),
            window::Event::FileDropped(path) => files::open_path(app, path, None),
            _ => Task::none(),
        },
        Message::SystemThemeChanged(mode) => theme::system_changed(app, mode),
        Message::MaximizedChanged(maximized) => {
            app.maximized = maximized;
            Task::none()
        }

        Message::Menu(msg) => match app.menu.update(msg) {
            Some(command) => update(app, Message::Command(command)),
            None => Task::none(),
        },
        Message::Command(command) => {
            app.menu.open_index = None;
            run_command(app, command)
        }

        Message::OpenPath(path) => files::open_path(app, path, None),
        Message::FilesPicked(paths) => files::files_picked(app, paths),
        Message::DirPicked(dir) => files::dir_picked(app, dir),
        Message::OutputDirPicked(dir) => files::output_dir_picked(app, dir),
        Message::DatasetScanned(result, select) => files::dataset_scanned(app, result, select),
        Message::SelectImage(index) => files::select_image(app, index),
        Message::ImageLoaded(request, result) => files::image_loaded(app, request, result),
        Message::Prefetched(path, result) => {
            app.prefetching.remove(&path);
            if let Ok(image) = result {
                app.cache.insert(image);
            }
            Task::none()
        }
        Message::FileFilterChanged(filter) => {
            app.file_filter = filter;
            app.filtered = app.dataset.filter(&app.file_filter);
            app.file_list.reset()
        }
        Message::FileListScrolled(viewport) => {
            app.file_list.on_scroll(viewport);
            Task::none()
        }
        Message::ExportPicked(format, dir) => files::export_picked(app, format, dir),
        Message::ExportFinished(result) => files::export_finished(app, result),
        Message::ImportPicked(format, source) => files::import_picked(app, format, source),
        Message::ImportFinished(result) => files::import_finished(app, result),
        Message::LabelsFilePicked(path) => files::labels_file_picked(app, path),
        Message::LabelsFileSaveTo(path) => files::labels_file_save_to(app, path),
        Message::DeleteLabelFileConfirmed(path) => files::delete_label_file_confirmed(app, path),

        Message::Canvas(event) => edit::canvas_event(app, event),
        Message::ShapeClicked(index) => edit::shape_clicked(app, index),
        Message::ShapeDoubleClicked(index) => edit::open_label_dialog_for(app, index),
        Message::ToggleShapeVisible(index) => {
            if !app.hidden.remove(&index) {
                app.hidden.insert(index);
                app.selected.remove(&index);
            }
            app.touch_canvas();
            Task::none()
        }
        Message::ShapeListScrolled(viewport) => {
            app.shape_list.on_scroll(viewport);
            Task::none()
        }
        Message::ToggleLabelVisible(label) => {
            if !app.hidden_labels.remove(&label) {
                app.hidden_labels.insert(label);
                // hidden shapes mustn't stay selected (and so deletable/movable)
                let visible: std::collections::BTreeSet<usize> = app
                    .selected
                    .iter()
                    .copied()
                    .filter(|&i| app.is_shape_visible(i))
                    .collect();
                app.selected = visible;
            }
            app.touch_canvas();
            Task::none()
        }
        Message::LabelClicked(label) => edit::select_label(app, &label),
        Message::ImageFlagToggled(flag, value) => {
            if app.editable()
                && let Some(doc) = &mut app.doc
            {
                doc.set_flag(&flag, value);
            }
            Task::none()
        }
        Message::SidebarResizeStart => {
            app.sidebar_drag = Some((None, app.settings.sidebar_width));
            Task::none()
        }
        Message::SidebarResizeMove(x) => {
            if let Some((start, width)) = &mut app.sidebar_drag {
                let start_x = *start.get_or_insert(x);
                let max = (app.window_width - 300.0).max(200.0);
                app.settings.sidebar_width = (*width + start_x - x).clamp(200.0, max);
            }
            Task::none()
        }
        Message::SidebarResizeEnd => {
            if app.sidebar_drag.take().is_some() {
                app.persist_settings();
            }
            Task::none()
        }

        Message::LabelDialog(msg) => edit::label_dialog(app, msg),
        Message::Settings(msg) => crate::ui::settings_modal::update(app, msg),
        Message::CloseOverlay => {
            app.overlay = Overlay::None;
            app.menu.open_index = None;
            Task::none()
        }
        Message::ConfirmSave => {
            let Overlay::ConfirmUnsaved { then } =
                std::mem::replace(&mut app.overlay, Overlay::None)
            else {
                return Task::none();
            };
            match files::save_current(app) {
                Ok(()) => update(app, *then),
                Err(e) => {
                    app.toasts.error(e);
                    Task::none()
                }
            }
        }
        Message::ConfirmDiscard => {
            let Overlay::ConfirmUnsaved { then } =
                std::mem::replace(&mut app.overlay, Overlay::None)
            else {
                return Task::none();
            };
            if let Some(doc) = &mut app.doc {
                doc.mark_saved();
            }
            update(app, *then)
        }

        Message::DismissToast(id) => {
            app.toasts.dismiss(id);
            Task::none()
        }
        Message::Tick => {
            app.toasts.tick();
            Task::none()
        }
    }
}

fn run_command(app: &mut App, command: Command) -> Task<Message> {
    // while a dialog is up, only commands that make sense over it run
    if app.overlay.is_open() && !matches!(command, Command::Quit | Command::Save) {
        return Task::none();
    }
    let edits = matches!(
        command,
        Command::Undo
            | Command::Redo
            | Command::DeleteSelected
            | Command::DuplicateSelected
            | Command::Paste
            | Command::EditLabel
            | Command::Nudge(..)
    );
    if edits && !app.editable() {
        return Task::none();
    }
    match command {
        Command::OpenFiles => files::pick_files(app),
        Command::OpenDir => files::pick_dir(app),
        Command::OpenRecent(i) => match app.settings.recent_dirs.get(i).cloned() {
            Some(dir) => files::open_path(app, dir, None),
            None => Task::none(),
        },
        Command::ChangeOutputDir => files::pick_output_dir(app),
        Command::Save => {
            if let Err(e) = files::save_current(app) {
                app.toasts.error(e);
            }
            Task::none()
        }
        Command::DeleteLabelFile => files::delete_label_file(app),
        Command::NextImage => files::step(app, 1),
        Command::PrevImage => files::step(app, -1),
        Command::NextUnannotated => files::next_unannotated(app),
        Command::CloseFile => {
            files::guard_unsaved(app, Message::Command(command), files::close_file)
        }
        Command::Quit => files::guard_unsaved(app, Message::Command(command), |_| iced::exit()),
        Command::Export(format) => files::pick_export_dir(app, format),
        Command::Import(format) => files::pick_import_source(app, format),
        Command::LoadLabels => files::pick_labels_file(app),
        Command::SaveLabels => files::pick_labels_save(app),

        Command::Undo => edit::undo(app, true),
        Command::Redo => edit::undo(app, false),
        Command::SetMode(mode) => {
            app.mode = mode;
            if mode != Mode::Edit {
                app.selected.clear();
                app.touch_canvas();
            }
            Task::none()
        }
        Command::DeleteSelected => edit::delete_selected(app),
        Command::DuplicateSelected => edit::duplicate_selected(app),
        Command::Copy => edit::copy(app),
        Command::Paste => edit::paste(app),
        Command::EditLabel => match app.selected.first().copied() {
            Some(i) => edit::open_label_dialog_for(app, i),
            None => Task::none(),
        },
        Command::SelectAll => edit::select_all(app),
        Command::Deselect => {
            if !app.selected.is_empty() {
                app.selected.clear();
                app.touch_canvas();
            }
            Task::none()
        }
        Command::Nudge(dx, dy) => edit::nudge(app, dx, dy),

        Command::ZoomIn => edit::zoom(app, 1.25),
        Command::ZoomOut => edit::zoom(app, 0.8),
        Command::ZoomOriginal => {
            let image = edit::image_size(app);
            app.view = app.view.zoom_to(app.canvas_size, image, 1.0, None);
            Task::none()
        }
        Command::FitWindow => {
            app.view = ViewMode::FitWindow;
            Task::none()
        }
        Command::FitWidth => {
            app.view = ViewMode::FitWidth;
            Task::none()
        }
        Command::ToggleShapesHidden => edit::toggle_all_hidden(app),
        Command::ToggleShowLabels => {
            app.settings.show_labels = !app.settings.show_labels;
            app.persist_settings();
            Task::none()
        }

        Command::ToggleAutoSave => {
            app.toggle_setting(
                |s| {
                    s.auto_save = !s.auto_save;
                    s.auto_save
                },
                "Auto save",
            );
            Task::none()
        }
        Command::ToggleSaveImageData => {
            app.toggle_setting(
                |s| {
                    s.save_image_data = !s.save_image_data;
                    s.save_image_data
                },
                "Embed image data",
            );
            Task::none()
        }
        Command::ToggleKeepPrevious => {
            app.toggle_setting(
                |s| {
                    s.keep_previous = !s.keep_previous;
                    s.keep_previous
                },
                "Keep previous annotation",
            );
            Task::none()
        }
        Command::OpenSettings => {
            app.overlay = Overlay::Settings(Box::new(SettingsDraft::new(&app.settings)));
            Task::none()
        }
        Command::ShowShortcuts => {
            app.overlay = Overlay::Shortcuts;
            Task::none()
        }
        Command::About => {
            app.overlay = Overlay::About;
            Task::none()
        }
    }
}
