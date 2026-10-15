//! Opening folders/files, navigating images, saving, importing and exporting.

use super::{App, Overlay};
use crate::canvas::ViewMode;
use crate::image_cache::{self, Loaded, LoadedImage, blocking};
use crate::message::Message;
use iced::Task;
use labeleasy_core::export::{self, Summary};
use labeleasy_core::import::{self, Source};
use labeleasy_core::{Dataset, LabelSet, image_io};
use std::path::{Path, PathBuf};

const ROW_HEIGHT: f32 = crate::ui::sidebar::FILE_ROW_HEIGHT;

fn start_dir(app: &App) -> Option<PathBuf> {
    app.dataset
        .root
        .clone()
        .or_else(|| app.settings.last_dir.clone())
}

fn dialog(app: &App) -> rfd::AsyncFileDialog {
    let mut dialog = rfd::AsyncFileDialog::new();
    if let Some(dir) = start_dir(app) {
        dialog = dialog.set_directory(dir);
    }
    dialog
}

pub fn pick_files(app: &App) -> Task<Message> {
    let mut extensions: Vec<&str> = image_io::SUPPORTED_EXTENSIONS.to_vec();
    extensions.push("json");
    let picker = dialog(app)
        .set_title("Open images or label files")
        .add_filter("Images and labelme files", &extensions)
        .pick_files();
    Task::perform(
        async move {
            picker
                .await
                .map(|files| files.into_iter().map(|f| f.path().to_path_buf()).collect())
        },
        Message::FilesPicked,
    )
}

pub fn pick_dir(app: &App) -> Task<Message> {
    let picker = dialog(app).set_title("Open folder").pick_folder();
    Task::perform(
        async move { picker.await.map(|f| f.path().to_path_buf()) },
        Message::DirPicked,
    )
}

pub fn pick_output_dir(app: &App) -> Task<Message> {
    let picker = dialog(app)
        .set_title("Folder to save label files in")
        .pick_folder();
    Task::perform(
        async move { picker.await.map(|f| f.path().to_path_buf()) },
        Message::OutputDirPicked,
    )
}

pub fn pick_export_dir(app: &mut App, format: usize) -> Task<Message> {
    if app.dataset.is_empty() {
        app.toasts.info("Open a folder of annotated images first");
        return Task::none();
    }
    let picker = dialog(app).set_title("Export to folder").pick_folder();
    Task::perform(
        async move { picker.await.map(|f| f.path().to_path_buf()) },
        move |dir| Message::ExportPicked(format, dir),
    )
}

pub fn pick_labels_file(app: &App) -> Task<Message> {
    let picker = dialog(app)
        .set_title("Load labels")
        .add_filter("Label list", &["txt"])
        .pick_file();
    Task::perform(
        async move { picker.await.map(|f| f.path().to_path_buf()) },
        Message::LabelsFilePicked,
    )
}

pub fn pick_labels_save(app: &App) -> Task<Message> {
    let picker = dialog(app)
        .set_title("Save labels")
        .set_file_name("labels.txt")
        .save_file();
    Task::perform(
        async move { picker.await.map(|f| f.path().to_path_buf()) },
        Message::LabelsFileSaveTo,
    )
}

/// runs `then` now, or once unsaved changes are dealt with: saved silently
/// with auto-save on, otherwise after asking (replaying `replay`).
pub fn guard_unsaved(
    app: &mut App,
    replay: Message,
    then: impl FnOnce(&mut App) -> Task<Message>,
) -> Task<Message> {
    let dirty = app.doc.as_ref().is_some_and(|d| d.is_dirty());
    if dirty {
        if app.settings.auto_save {
            if let Err(e) = save(app, false) {
                app.toasts.error(e);
                return Task::none();
            }
        } else {
            app.overlay = Overlay::ConfirmUnsaved {
                then: Box::new(replay),
            };
            return Task::none();
        }
    }
    then(app)
}

/// opens a folder, an image (its folder, with it selected) or a label file.
pub fn open_path(app: &mut App, path: PathBuf, output_dir: Option<PathBuf>) -> Task<Message> {
    // dataset paths are absolute; a relative one (from the command line)
    // would never match its entry and silently open the first image instead
    let path = std::path::absolute(&path).unwrap_or(path);
    if !path.exists() {
        app.toasts.error(format!("{} not found", path.display()));
        return Task::none();
    }
    let is_label_file = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"));
    if path.is_file() && !is_label_file && !image_io::is_supported(&path) {
        app.toasts
            .error(format!("{} is not a supported image", path.display()));
        return Task::none();
    }
    let replay = Message::OpenPath(path.clone());
    guard_unsaved(app, replay, move |app| {
        let recursive = app.settings.recursive_scan;
        let is_json = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"));
        if is_json {
            let dataset = Dataset::from_files([path], output_dir);
            return Task::done(Message::DatasetScanned(Ok(dataset), None));
        }
        let (root, select) = if path.is_dir() {
            (path, None)
        } else {
            match path.parent() {
                Some(parent) => (parent.to_path_buf(), Some(path)),
                None => return Task::none(),
            }
        };
        Task::perform(
            blocking(move || {
                // Dataset::scan(&root, recursive, output_dir).map_err(|e| e.to_string())
                let dataset = Dataset::scan(&root, recursive, output_dir.clone())
                    .map_err(|e| e.to_string())?;
                // dataset layouts (YOLO's images/, VOC's JPEGImages/) keep
                // the images in subfolders: look there before giving up
                if dataset.is_empty() && !recursive {
                    return Dataset::scan(&root, true, output_dir).map_err(|e| e.to_string());
                }
                Ok(dataset)
            }),
            move |result| Message::DatasetScanned(result, select.clone()),
        )
    })
}

pub fn files_picked(app: &mut App, paths: Option<Vec<PathBuf>>) -> Task<Message> {
    let Some(mut paths) = paths.filter(|p| !p.is_empty()) else {
        return Task::none();
    };
    if paths.len() == 1 {
        let output_dir = app.dataset.output_dir.clone();
        return open_path(app, paths.remove(0), output_dir);
    }
    guard_unsaved(app, Message::FilesPicked(Some(paths.clone())), move |app| {
        let dataset = Dataset::from_files(paths, app.dataset.output_dir.clone());
        Task::done(Message::DatasetScanned(Ok(dataset), None))
    })
}

pub fn dir_picked(app: &mut App, dir: Option<PathBuf>) -> Task<Message> {
    match dir {
        Some(dir) => open_path(app, dir, None),
        None => Task::none(),
    }
}

pub fn output_dir_picked(app: &mut App, dir: Option<PathBuf>) -> Task<Message> {
    let Some(dir) = dir else { return Task::none() };
    // pending edits belong to the old location: settle them there first
    guard_unsaved(
        app,
        Message::OutputDirPicked(Some(dir.clone())),
        move |app| {
            let open = doc_index(app);
            app.dataset.set_output_dir(Some(dir.clone()));
            app.toasts
                .info(format!("Saving labels to {}", dir.display()));
            // reload the open image from its new label location, so a file
            // already there is shown (and not blindly overwritten on save)
            let Some(index) = open else {
                return Task::none();
            };
            let previous = std::mem::take(&mut app.previous_shapes);
            let task = begin_load(app, index);
            app.previous_shapes = previous;
            task
        },
    )
}

pub fn dataset_scanned(
    app: &mut App,
    result: Result<Dataset, String>,
    select: Option<PathBuf>,
) -> Task<Message> {
    // the unsaved-changes check ran before the (async) scan; edits made
    // while it ran need settling again before the document is replaced
    if let Ok(dataset) = &result
        && !dataset.is_empty()
    {
        let replay = Message::DatasetScanned(Ok(dataset.clone()), select.clone());
        return guard_unsaved(app, replay, move |app| apply_dataset(app, result, select));
    }
    apply_dataset(app, result, select)
}

fn apply_dataset(
    app: &mut App,
    result: Result<Dataset, String>,
    select: Option<PathBuf>,
) -> Task<Message> {
    let dataset = match result {
        Ok(dataset) => dataset,
        Err(e) => {
            app.toasts.error(e);
            return Task::none();
        }
    };
    if dataset.is_empty() {
        app.toasts.info("No images found");
        return Task::none();
    }
    if let Some(root) = &dataset.root {
        app.settings.push_recent(root.clone());
        app.persist_settings();
    }
    let index = select.and_then(|p| dataset.position(&p)).unwrap_or(0);
    app.dataset = dataset;
    app.file_filter.clear();
    app.filtered = app.dataset.filter("");
    // nothing of the old dataset carries over (incl. "keep previous annotation")
    clear_document(app);
    app.previous_shapes.clear();
    let reset = app.file_list.reset();
    Task::batch([reset, begin_load(app, index)])
}

pub fn select_image(app: &mut App, index: usize) -> Task<Message> {
    if app.current == Some(index) && app.loading.is_none() {
        return Task::none();
    }
    guard_unsaved(app, Message::SelectImage(index), move |app| {
        begin_load(app, index)
    })
}

fn begin_load(app: &mut App, index: usize) -> Task<Message> {
    let Some(entry) = app.dataset.entries.get(index).cloned() else {
        return Task::none();
    };
    if let Some(doc) = &app.doc {
        app.previous_shapes = doc.shapes().to_vec();
    }
    app.current = Some(index);
    app.load_request += 1;
    app.loading = Some(entry.image.clone());

    let request = app.load_request;
    let cached = app.cache.get(&entry.image);
    let output_dir = app.dataset.output_dir.clone();
    let load = Task::perform(
        blocking(move || {
            image_cache::load(
                &entry.image,
                Some(entry.label),
                output_dir.as_deref(),
                cached,
            )
            .map(Box::new)
        }),
        move |result| Message::ImageLoaded(request, result),
    );

    let reveal = match app.filtered.iter().position(|&i| i == index) {
        Some(row) => app.file_list.reveal(row, ROW_HEIGHT),
        None => Task::none(),
    };
    Task::batch([load, reveal])
}

pub fn image_loaded(
    app: &mut App,
    request: u64,
    result: Result<Box<Loaded>, String>,
) -> Task<Message> {
    // a newer selection superseded this one (e.g. holding "D")
    if request != app.load_request {
        if let Ok(loaded) = result {
            cache_insert(app, loaded.image);
        }
        return Task::none();
    }
    app.loading = None;
    let Loaded { image, mut doc } = match result {
        Ok(loaded) => *loaded,
        Err(e) => {
            // don't leave the previous image up while the list highlights
            // this one (edits would be attributed to the wrong entry);
            // `current` stays here so next/previous step past the bad file
            app.toasts.error(e);
            if let Some(doc) = app.doc.take() {
                app.previous_shapes = doc.shapes().to_vec();
            }
            app.image = None;
            app.selected.clear();
            app.doc_id += 1;
            return Task::none();
        }
    };

    // a label dialog opened on the previous image would otherwise write into this one
    if matches!(app.overlay, Overlay::LabelDialog(_)) {
        app.overlay = Overlay::None;
    }
    doc.ensure_flags(app.settings.image_flags.iter().map(String::as_str));
    for shape in doc.shapes().iter().filter(|s| !s.label.is_empty()) {
        app.labels.insert(&shape.label);
    }
    if app.settings.keep_previous && !doc.has_label_file() && !app.previous_shapes.is_empty() {
        doc.add_shapes(app.previous_shapes.clone());
    }

    app.doc = Some(doc);
    app.image = Some(image.clone());
    cache_insert(app, image);

    app.doc_id += 1;
    app.selected.clear();
    app.hidden.clear();
    app.view = ViewMode::FitWindow;
    app.touch_canvas();

    let reset = app.shape_list.reset();

    Task::batch([reset, prefetch_neighbors(app)])
}

/// the images worth keeping decoded: the selected one and its neighbors in
/// the file list (next ones first, as stepping forward is most common).
fn cache_window(app: &App) -> Vec<PathBuf> {
    let Some(current) = app.current else {
        return Vec::new();
    };
    let Some(pos) = app.filtered.iter().position(|&i| i == current) else {
        return app
            .dataset
            .entries
            .get(current)
            .map(|e| vec![e.image.clone()])
            .unwrap_or_default();
    };
    [pos, pos + 1, pos.wrapping_sub(1), pos + 2]
        .into_iter()
        .filter_map(|p| app.filtered.get(p))
        .filter_map(|&i| app.dataset.entries.get(i))
        .map(|e| e.image.clone())
        .collect()
}

/// caches `image` if caching is on and it's near the selection, and drops
/// whatever the selection has moved away from.
pub fn cache_insert(app: &mut App, image: LoadedImage) {
    if !app.settings.cache_images {
        return;
    }
    let window = cache_window(app);
    if window.contains(&image.path) {
        app.cache.insert(image);
    }
    app.cache.retain(&window);
}

fn prefetch_neighbors(app: &mut App) -> Task<Message> {
    if !app.settings.cache_images {
        return Task::none();
    }
    let neighbors = cache_window(app)
        .into_iter()
        .skip(1)
        .filter(|path| {
            image_io::is_supported(path)
                && !app.cache.contains(path)
                && !app.prefetching.contains(path)
                && path.is_file()
        })
        .collect::<Vec<_>>();
    // remembered so fast navigation doesn't decode the same image repeatedly
    app.prefetching.extend(neighbors.iter().cloned());
    Task::batch(neighbors.into_iter().map(|path| {
        let requested = path.clone();
        Task::perform(
            blocking(move || image_cache::decode(&path)),
            move |result| Message::Prefetched(requested.clone(), result),
        )
    }))
}

pub fn step(app: &mut App, delta: isize) -> Task<Message> {
    if app.filtered.is_empty() {
        return Task::none();
    }
    let pos = app
        .current
        .and_then(|c| app.filtered.iter().position(|&i| i == c));
    let next = match pos {
        Some(p) => p.saturating_add_signed(delta).min(app.filtered.len() - 1),
        None => 0,
    };
    if Some(next) == pos {
        return Task::none();
    }
    select_image(app, app.filtered[next])
}

pub fn next_unannotated(app: &mut App) -> Task<Message> {
    let start = app
        .current
        .and_then(|c| app.filtered.iter().position(|&i| i == c))
        .map_or(0, |p| p + 1);
    let found = app.filtered[start.min(app.filtered.len())..]
        .iter()
        .chain(&app.filtered[..start.min(app.filtered.len())])
        .copied()
        .find(|&i| Some(i) != app.current && !app.dataset.entries[i].annotated);
    match found {
        Some(i) => select_image(app, i),
        None => {
            app.toasts.success("Every image is annotated");
            Task::none()
        }
    }
}

/// the dataset entry of the open document. Not `app.current`: that already
/// points at the next image while it loads.
pub fn doc_index(app: &App) -> Option<usize> {
    let doc = app.doc.as_ref()?;
    app.dataset
        .entries
        .iter()
        .position(|e| e.label == doc.label_path)
        .or_else(|| app.dataset.position(&doc.image_path))
}

/// saves the open document (always writes, even when unchanged).
pub fn save_current(app: &mut App) -> Result<(), String> {
    save(app, true)
}

/// `notify`: confirm with a toast (explicit saves; auto-saves stay quiet).
fn save(app: &mut App, notify: bool) -> Result<(), String> {
    let index = doc_index(app);
    let Some(doc) = &mut app.doc else {
        return Ok(());
    };
    doc.save(app.settings.save_image_data)
        .map_err(|e| format!("Save failed: {e}"))?;
    let label_path = doc.label_path.clone();
    if let Some(entry) = index.and_then(|i| app.dataset.entries.get_mut(i)) {
        entry.annotated = true;
    }
    if notify {
        app.toasts.success(format!(
            "Saved {}",
            label_path.file_name().unwrap_or_default().to_string_lossy()
        ));
    }
    Ok(())
}

pub fn delete_label_file(app: &mut App) -> Task<Message> {
    let Some(doc) = &app.doc else {
        return Task::none();
    };
    if !doc.label_path.is_file() {
        app.toasts.info("This image has no label file");
        return Task::none();
    }
    let path = doc.label_path.clone();
    let name = path.display().to_string();
    let confirm = rfd::AsyncMessageDialog::new()
        .set_title("Delete label file")
        .set_description(format!("Permanently delete {name}?"))
        .set_buttons(rfd::MessageButtons::YesNo)
        .set_level(rfd::MessageLevel::Warning)
        .show();
    Task::perform(confirm, move |answer| {
        if answer == rfd::MessageDialogResult::Yes {
            Message::DeleteLabelFileConfirmed(path.clone())
        } else {
            Message::Tick
        }
    })
}

pub fn delete_label_file_confirmed(app: &mut App, path: PathBuf) -> Task<Message> {
    let index = doc_index(app);
    let Some(doc) = &mut app.doc else {
        return Task::none();
    };
    // the confirmation is non-modal: another image may be open by now
    if doc.label_path != path {
        app.toasts
            .info("Another image was opened; nothing was deleted");
        return Task::none();
    }
    match doc.delete_label_file() {
        Ok(()) => {
            if let Some(entry) = index.and_then(|i| app.dataset.entries.get_mut(i)) {
                entry.annotated = false;
            }
            app.selected.clear();
            app.touch_canvas();
            app.toasts.success("Label file deleted");
        }
        Err(e) => {
            app.toasts.error(e.to_string());
        }
    }
    Task::none()
}

pub fn close_file(app: &mut App) -> Task<Message> {
    clear_document(app);
    Task::none()
}

fn clear_document(app: &mut App) {
    // a load still in flight must not reopen an image after closing
    app.load_request += 1;
    app.loading = None;
    app.doc = None;
    app.image = None;
    app.current = None;
    app.selected.clear();
    app.doc_id += 1;
    app.touch_canvas();
}

pub fn export_picked(app: &mut App, format: usize, dir: Option<PathBuf>) -> Task<Message> {
    let Some(out_dir) = dir else {
        return Task::none();
    };
    // the export reads label files from disk, so flush pending edits first
    if app.doc.as_ref().is_some_and(|d| d.is_dirty())
        && let Err(e) = save_current(app)
    {
        app.toasts.error(e);
        return Task::none();
    }
    let dataset = app.dataset.clone();
    let predefined = app.settings.labels.clone();
    app.toasts.info("Exporting…");
    Task::perform(
        blocking(move || {
            let exporter = export::all()
                .into_iter()
                .nth(format)
                .ok_or("unknown export format")?;
            let (items, warnings) = export::collect(&dataset);
            if items.is_empty() {
                return Err("Nothing to export: no image has a label file yet".to_string());
            }
            let mut labels = LabelSet::from_iter(&predefined);
            for label in export::labels_in(&items).iter() {
                labels.insert(label);
            }
            let mut summary = exporter
                .export(&items, &labels, &out_dir)
                .map_err(|e| e.to_string())?;
            summary.warnings.extend(warnings);
            Ok(summary)
        }),
        Message::ExportFinished,
    )
}

pub fn export_finished(app: &mut App, result: Result<Summary, String>) -> Task<Message> {
    match result {
        Ok(s) => {
            let mut text = format!(
                "Exported {} images, {} annotations",
                s.images, s.annotations
            );
            if s.skipped_shapes > 0 {
                text.push_str(&format!(" ({} shapes not representable)", s.skipped_shapes));
            }
            app.toasts.success(text);
            for warning in s.warnings.iter().take(3) {
                app.toasts.error(warning.clone());
            }
        }
        Err(e) => {
            app.toasts.error(e);
        }
    }
    Task::none()
}

pub fn pick_import_source(app: &mut App, format: usize) -> Task<Message> {
    let Some(importer) = import::all().into_iter().nth(format) else {
        return Task::none();
    };
    let title = format!("Import {}", importer.name());
    let picker = dialog(app).set_title(title);
    let picked: std::pin::Pin<Box<dyn Future<Output = _> + Send>> = match importer.source() {
        Source::File(extensions) => {
            Box::pin(picker.add_filter(importer.name(), extensions).pick_file())
        }
        Source::Folder => Box::pin(picker.pick_folder()),
    };

    Task::perform(
        async move { picked.await.map(|f| f.path().to_path_buf()) },
        move |source| Message::ImportPicked(format, source),
    )
}

pub fn import_picked(app: &mut App, format: usize, source: Option<PathBuf>) -> Task<Message> {
    let Some(source) = source else {
        return Task::none();
    };
    // the open image's edits go to disk first, so the import can't clobber them
    if app.doc.as_ref().is_some_and(|d| d.is_dirty())
        && let Err(e) = save_current(app)
    {
        app.toasts.error(e);
        return Task::none();
    }
    let open = (!app.dataset.is_empty()).then(|| app.dataset.clone());
    let output_dir = app.dataset.output_dir.clone();
    app.toasts.info("Importing…");
    Task::perform(
        blocking(move || {
            let importer = import::all()
                .into_iter()
                .nth(format)
                .ok_or("unknown import format")?;
            let parsed = importer.read(&source).map_err(|e| e.to_string())?;
            if parsed.records.is_empty() {
                return Err(format!("No annotations found in {}", source.display()));
            }
            // nothing open: the dataset being imported holds its own images
            let (dataset, opened) = match open {
                Some(dataset) => (dataset, false),
                None => {
                    let root = import::dataset_root(&source);
                    let dataset =
                        Dataset::scan(&root, true, output_dir).map_err(|e| e.to_string())?;
                    if dataset.is_empty() {
                        return Err(format!(
                            "No images found in {}; open the image folder first",
                            root.display()
                        ));
                    }
                    (dataset, true)
                }
            };
            let summary = import::apply(parsed, &dataset).map_err(|e| e.to_string())?;
            Ok((opened.then_some(dataset), summary))
        }),
        Message::ImportFinished,
    )
}

pub fn import_finished(
    app: &mut App,
    result: Result<(Option<Dataset>, import::Summary), String>,
) -> Task<Message> {
    let (opened, s) = match result {
        Ok(result) => result,
        Err(e) => {
            app.toasts.error(e);
            return Task::none();
        }
    };
    for label in &s.labels {
        app.labels.insert(label);
    }

    let mut text = format!(
        "Imported {} annotations into {} images",
        s.annotations, s.images
    );
    if s.skipped_existing > 0 {
        text.push_str(&format!(
            " ({} already labelled, left as is)",
            s.skipped_existing
        ));
    }
    if s.images > 0 {
        app.toasts.success(text);
    } else {
        app.toasts.info(text);
    }
    if s.unmatched > 0 {
        app.toasts.error(format!(
            "{} annotated images aren't in the open folder",
            s.unmatched
        ));
    }
    for warning in s.warnings.iter().take(3) {
        app.toasts.error(warning.clone());
    }

    if let Some(mut dataset) = opened {
        // scanned before the import wrote its label files
        for entry in &mut dataset.entries {
            entry.annotated = entry.label.is_file();
        }
        return dataset_scanned(app, Ok(dataset), None);
    }
    for entry in &mut app.dataset.entries {
        entry.annotated = entry.label.is_file();
    }

    // the open image may just have received a label file: show it
    let reload = app
        .doc
        .as_ref()
        .is_some_and(|d| !d.has_label_file() && !d.is_dirty() && d.label_path.is_file());
    match doc_index(app).filter(|_| reload) {
        Some(index) => {
            let previous = std::mem::take(&mut app.previous_shapes);
            let task = begin_load(app, index);
            app.previous_shapes = previous;
            task
        }
        None => Task::none(),
    }
}

pub fn labels_file_picked(app: &mut App, path: Option<PathBuf>) -> Task<Message> {
    let Some(path) = path else {
        return Task::none();
    };
    match LabelSet::load_txt(&path) {
        Ok(set) => {
            for label in set.iter() {
                app.labels.insert(label);
                if !app.settings.labels.iter().any(|l| l == label) {
                    app.settings.labels.push(label.to_string());
                }
            }
            app.persist_settings();
            app.touch_canvas();
            app.toasts.success(format!("Loaded {} labels", set.len()));
        }
        Err(e) => {
            app.toasts.error(e.to_string());
        }
    }
    Task::none()
}

pub fn labels_file_save_to(app: &mut App, path: Option<PathBuf>) -> Task<Message> {
    let Some(path) = path else {
        return Task::none();
    };
    match app.labels.save_txt(&path) {
        Ok(()) => app.toasts.success(format!("Saved {}", display_name(&path))),
        Err(e) => app.toasts.error(e.to_string()),
    };
    Task::none()
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}
