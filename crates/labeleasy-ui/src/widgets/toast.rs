//! Transient notifications stacked in the bottom-right corner.
//!
//! Hosts call [`ToastManager::tick`] from a timer subscription that only runs
//! while [`ToastManager::is_empty`] is false, so idle apps don't wake up.

use iced::widget::{button, column, container, progress_bar, row, text};
use iced::{Alignment, Border, Element, Length};
use std::time::{Duration, Instant};

pub const TOAST_DURATION: Duration = Duration::from_secs(4);
pub const TOAST_DURATION_LONG: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStatus {
    Success,
    Error,
    Info,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub id: usize,
    pub message: String,
    pub status: ToastStatus,
    pub expires_at: Instant,
    /// label of a button reporting the toast's id through `on_action`.
    pub action: Option<String>,
    /// 0.0..=1.0, drawn as a bar under the message.
    pub progress: Option<f32>,
    /// stays until dismissed.
    pub sticky: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ToastManager {
    toasts: Vec<Toast>,
    next_id: usize,
}

impl ToastManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// drops expired toasts.
    pub fn tick(&mut self) {
        let now = Instant::now();
        self.toasts
            .retain(|toast| toast.sticky || toast.expires_at > now);
    }

    pub fn show(&mut self, message: impl Into<String>, status: ToastStatus) -> usize {
        let duration = match status {
            ToastStatus::Error => TOAST_DURATION_LONG,
            _ => TOAST_DURATION,
        };
        self.show_for(message, status, duration)
    }

    pub fn show_for(
        &mut self,
        message: impl Into<String>,
        status: ToastStatus,
        duration: Duration,
    ) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        // keep the stack short; the oldest go first
        if self.toasts.len() >= 5 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast {
            id,
            message: message.into(),
            status,
            expires_at: Instant::now() + duration,
            action: None,
            progress: None,
            sticky: false,
        });
        id
    }

    /// a toast with a button; pressing it emits `on_action(id)`.
    pub fn show_with_action(
        &mut self,
        message: impl Into<String>,
        status: ToastStatus,
        duration: Duration,
        action: impl Into<String>,
    ) -> usize {
        let id = self.show_for(message, status, duration);
        if let Some(toast) = self.get_mut(id) {
            toast.action = Some(action.into());
        }
        id
    }

    /// a toast that stays until [`ToastManager::dismiss`]ed.
    pub fn show_sticky(&mut self, message: impl Into<String>, status: ToastStatus) -> usize {
        let id = self.show_for(message, status, Duration::ZERO);
        if let Some(toast) = self.get_mut(id) {
            toast.sticky = true;
        }
        id
    }

    /// updates a toast's message and its progress bar (0.0..=1.0).
    pub fn set_progress(&mut self, id: usize, message: impl Into<String>, progress: f32) {
        if let Some(toast) = self.get_mut(id) {
            toast.message = message.into();
            toast.progress = Some(progress.clamp(0.0, 1.0));
        }
    }

    fn get_mut(&mut self, id: usize) -> Option<&mut Toast> {
        self.toasts.iter_mut().find(|toast| toast.id == id)
    }

    pub fn success(&mut self, message: impl Into<String>) -> usize {
        self.show(message, ToastStatus::Success)
    }

    pub fn error(&mut self, message: impl Into<String>) -> usize {
        self.show(message, ToastStatus::Error)
    }

    pub fn info(&mut self, message: impl Into<String>) -> usize {
        self.show(message, ToastStatus::Info)
    }

    pub fn dismiss(&mut self, id: usize) {
        self.toasts.retain(|toast| toast.id != id);
    }

    /// a full-size layer with the toasts pinned bottom-right; stack it over the UI.
    pub fn view<'a, Message: Clone + 'a>(
        &'a self,
        on_dismiss: impl Fn(usize) -> Message + 'a,
        on_action: impl Fn(usize) -> Message + 'a,
    ) -> Element<'a, Message> {
        let colors = crate::theme::colors();
        let mut list = column![].spacing(10).align_x(Alignment::End);
        for toast in &self.toasts {
            let accent = match toast.status {
                ToastStatus::Success => colors.success,
                ToastStatus::Error => colors.error,
                ToastStatus::Info => colors.info,
            };
            let (background, foreground) = (colors.elevated_surface_background, colors.text);
            let mut line = row![text(&toast.message).size(13).width(Length::Fill)]
                .spacing(10)
                .align_y(Alignment::Center);
            if let Some(label) = &toast.action {
                line = line.push(
                    button(text(label).size(12))
                        .style(crate::widgets::style::primary_button)
                        .padding([3, 8])
                        .on_press(on_action(toast.id)),
                );
            }
            line = line.push(
                button(text("✕").size(12))
                    .style(button::text)
                    .padding([2, 6])
                    .on_press(on_dismiss(toast.id)),
            );
            let mut content = column![line].spacing(8);
            if let Some(progress) = toast.progress {
                content = content.push(progress_bar(0.0..=1.0, progress).girth(4));
            }

            list = list.push(container(content).width(320).padding([10, 12]).style(
                move |_theme| container::Style {
                    background: Some(background.into()),
                    border: Border {
                        color: accent,
                        width: 1.5,
                        radius: 6.0.into(),
                    },
                    text_color: Some(foreground),
                    ..Default::default()
                },
            ));
        }
        container(list)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::End)
            .align_y(Alignment::End)
            .padding([40, 20])
            .into()
    }
}
