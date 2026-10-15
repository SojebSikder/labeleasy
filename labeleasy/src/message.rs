use crate::canvas::{CanvasEvent, Mode};
use crate::image_cache::{Loaded, LoadedImage};
use crate::ui::label_dialog::LabelDialogMessage;
use crate::ui::settings_modal::SettingsMessage;
use iced::widget::scrollable::Viewport;
use iced::window;
use labeleasy_core::Dataset;
use labeleasy_core::export::Summary;
use labeleasy_core::import;
use labeleasy_ui::widgets::DropdownMessage;
use std::path::PathBuf;

/// every user-invokable action: menus, toolbar buttons and shortcuts all
/// funnel into these, so each is implemented exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    OpenFiles,
    OpenDir,
    OpenRecent(usize),
    ChangeOutputDir,
    Save,
    DeleteLabelFile,
    NextImage,
    PrevImage,
    NextUnannotated,
    CloseFile,
    Quit,
    Export(usize),
    Import(usize),
    LoadLabels,
    SaveLabels,

    Undo,
    Redo,
    SetMode(Mode),
    DeleteSelected,
    DuplicateSelected,
    Copy,
    Paste,
    EditLabel,
    SelectAll,
    Deselect,
    Nudge(i8, i8),

    ZoomIn,
    ZoomOut,
    ZoomOriginal,
    FitWindow,
    FitWidth,
    ToggleShapesHidden,
    ToggleShowLabels,

    ToggleAutoSave,
    ToggleSaveImageData,
    ToggleKeepPrevious,
    OpenSettings,
    ShowShortcuts,
    About,
}

#[derive(Debug, Clone)]
pub enum Message {
    // window chrome
    TitleBarDrag,
    Minimize,
    ToggleMaximize,
    WindowResizeDrag(window::Direction),
    WindowEvent(window::Event),
    SystemThemeChanged(iced::theme::Mode),
    MaximizedChanged(bool),

    Menu(DropdownMessage<Command>),
    Command(Command),

    // files
    OpenPath(PathBuf),
    FilesPicked(Option<Vec<PathBuf>>),
    DirPicked(Option<PathBuf>),
    OutputDirPicked(Option<PathBuf>),
    DatasetScanned(Result<Dataset, String>, Option<PathBuf>),
    SelectImage(usize),
    ImageLoaded(u64, Result<Box<Loaded>, String>),
    Prefetched(PathBuf, Result<LoadedImage, String>),
    FileFilterChanged(String),
    FileListScrolled(Viewport),
    ExportPicked(usize, Option<PathBuf>),
    ExportFinished(Result<Summary, String>),
    ImportPicked(usize, Option<PathBuf>),
    /// carries the dataset opened for the import when none was open.
    ImportFinished(Result<(Option<Dataset>, import::Summary), String>),
    LabelsFilePicked(Option<PathBuf>),
    LabelsFileSaveTo(Option<PathBuf>),
    /// carries the path confirmed, in case another image was opened meanwhile.
    DeleteLabelFileConfirmed(PathBuf),

    // canvas
    Canvas(CanvasEvent),

    // side panels
    ShapeClicked(usize),
    ShapeDoubleClicked(usize),
    ToggleShapeVisible(usize),
    ShapeListScrolled(Viewport),
    ToggleLabelVisible(String),
    LabelClicked(String),
    ImageFlagToggled(String, bool),
    SidebarResizeStart,
    SidebarResizeMove(f32),
    SidebarResizeEnd,

    // dialogs
    LabelDialog(LabelDialogMessage),
    Settings(SettingsMessage),
    CloseOverlay,
    ConfirmSave,
    ConfirmDiscard,

    DismissToast(usize),
    Tick,
}

impl From<Command> for Message {
    fn from(command: Command) -> Self {
        Message::Command(command)
    }
}
