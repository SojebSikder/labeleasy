//! Self-update

use super::App;
use crate::message::Message;
use crate::updater::{self, UpdateInfo, UpdateProgress};
use iced::Task;
use labeleasy_ui::widgets::ToastStatus;
use labeleasy_ui::widgets::toast::TOAST_DURATION_LONG;

#[derive(Debug, Default)]
pub struct UpdateState {
    /// a newer release found by the last check.
    pub available: Option<UpdateInfo>,
    /// the "update available" toast, so its action button can be matched.
    offer_toast: Option<usize>,
    /// the "Checking for updates…" toast of a manual check.
    checking_toast: Option<usize>,
    /// the progress toast; `Some` while downloading.
    download_toast: Option<usize>,
}

impl UpdateState {
    pub fn downloading(&self) -> bool {
        self.download_toast.is_some()
    }
}

/// `silent` checks only speak up when an update is found.
pub fn check(app: &mut App, silent: bool) -> Task<Message> {
    if !silent {
        if let Some(id) = app.update.checking_toast.take() {
            app.toasts.dismiss(id);
        }
        app.update.checking_toast = Some(app.toasts.info("Checking for updates…"));
    }
    Task::perform(
        async {
            tokio::task::spawn_blocking(updater::check_for_update)
                .await
                .unwrap_or_else(|e| Err(e.to_string()))
        },
        move |result| Message::UpdateChecked(silent, result),
    )
}

pub fn checked(
    app: &mut App,
    silent: bool,
    result: Result<Option<UpdateInfo>, String>,
) -> Task<Message> {
    if let Some(id) = app.update.checking_toast.take() {
        app.toasts.dismiss(id);
    }
    match result {
        Ok(Some(info)) => {
            if let Some(id) = app.update.offer_toast.take() {
                app.toasts.dismiss(id);
            }
            if !app.update.downloading() {
                app.update.offer_toast = Some(app.toasts.show_with_action(
                    format!("Update available: v{}", info.version),
                    ToastStatus::Info,
                    TOAST_DURATION_LONG,
                    "Update",
                ));
            }
            app.update.available = Some(info);
        }
        Ok(None) if !silent => {
            app.toasts.info("You're up to date.");
        }
        Err(e) if !silent => {
            app.toasts.error(format!("Update check failed: {e}"));
        }
        _ => {}
    }
    Task::none()
}

pub fn toast_action(app: &mut App, id: usize) -> Task<Message> {
    if app.update.offer_toast == Some(id) {
        return install(app);
    }
    Task::none()
}

pub fn install(app: &mut App) -> Task<Message> {
    // already downloading (e.g. both the toast and the status bar were clicked)
    if app.update.downloading() {
        return Task::none();
    }
    if let Some(id) = app.update.offer_toast.take() {
        app.toasts.dismiss(id);
    }
    let id = app
        .toasts
        .show_sticky("Downloading update…", ToastStatus::Info);
    app.toasts.set_progress(id, "Downloading update… 0%", 0.0);
    app.update.download_toast = Some(id);

    let download = iced::task::sipper(|mut sender| async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = tokio::spawn(updater::perform_update_with_progress(move |progress| {
            let _ = tx.send(progress);
        }));
        while let Some(progress) = rx.recv().await {
            sender.send(progress).await;
        }
        handle.await.unwrap_or_else(|e| Err(e.to_string()))
    });
    Task::sip(download, Message::UpdateProgress, Message::UpdateInstalled)
}

pub fn progress(app: &mut App, progress: UpdateProgress) -> Task<Message> {
    if let Some(id) = app.update.download_toast {
        let (message, fraction) = match progress.fraction() {
            Some(f) => (
                format!("Downloading update… {}%", (f * 100.0).round() as u32),
                f,
            ),
            None => ("Downloading update…".to_string(), 0.0),
        };
        app.toasts.set_progress(id, message, fraction);
    }
    Task::none()
}

pub fn installed(app: &mut App, result: Result<String, String>) -> Task<Message> {
    if let Some(id) = app.update.download_toast.take() {
        app.toasts.dismiss(id);
    }
    match result {
        Ok(version) => {
            // nothing left to offer; the status bar button goes away
            app.update.available = None;
            app.toasts.show_sticky(
                format!("Updated to v{version}. Please restart the app."),
                ToastStatus::Success,
            );
        }
        // `available` stays, so the status bar button allows a retry
        Err(e) => {
            app.toasts.error(format!("Update failed: {e}"));
        }
    }
    Task::none()
}
