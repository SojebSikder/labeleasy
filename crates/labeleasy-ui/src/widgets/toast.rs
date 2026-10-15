//! Transient notifications stacked in the bottom-right corner.
//!
//! Hosts call [`ToastManager::tick`] from a timer subscription that only runs
//! while [`ToastManager::is_empty`] is false, so idle apps don't wake up.

use iced::widget::{button, column, container, row, text};
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
        self.toasts.retain(|toast| toast.expires_at > now);
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
        });
        id
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
            let content = row![
                text(&toast.message).size(13).width(Length::Fill),
                button(text("✕").size(12))
                    .style(button::text)
                    .padding([2, 6])
                    .on_press(on_dismiss(toast.id)),
            ]
            .spacing(10)
            .align_y(Alignment::Center);

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
