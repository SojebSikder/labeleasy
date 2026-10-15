//! The dialog that names a shape: label (with suggestions from the label list), group id, description and shape flags.

use crate::app::App;
use crate::message::Message;
use iced::widget::{Id, button, checkbox, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Element, Length, Task};
use labeleasy_core::Shape;
use labeleasy_core::document::ShapeMeta;
use labeleasy_ui::widgets::style;

const LABEL_INPUT: &str = "label-dialog-label";
const MAX_SUGGESTIONS: usize = 50;

pub enum LabelTarget {
    /// a freshly drawn shape; cancelling discards it.
    New(Box<Shape>),
    Existing(usize),
}

pub struct LabelDialog {
    pub target: LabelTarget,
    /// the document (`App::doc_id`) the target belongs to; a dialog that
    /// outlives its document must not write into the next one.
    pub doc_id: u64,
    label: String,
    group_id: String,
    description: String,
    flags: Vec<(String, bool)>,
    pub error: Option<String>,
    /// the label was typed in (not just prefilled): only then does it
    /// filter the suggestions, otherwise every label is offered.
    typed: bool,
}

#[derive(Debug, Clone)]
pub enum LabelDialogMessage {
    Label(String),
    GroupId(String),
    Description(String),
    Flag(usize, bool),
    Pick(String),
    Submit,
    Cancel,
}

pub enum Outcome {
    None,
    Submit(ShapeMeta),
    Cancel,
}

/// focuses (and selects) the label field.
pub fn focus() -> Task<Message> {
    Task::batch([
        iced::widget::operation::focus(Id::new(LABEL_INPUT)),
        iced::widget::operation::select_all(Id::new(LABEL_INPUT)),
    ])
}

impl LabelDialog {
    pub fn new(target: LabelTarget, doc_id: u64, meta: ShapeMeta) -> Self {
        Self {
            target,
            doc_id,
            label: meta.label,
            group_id: meta.group_id.map(|g| g.to_string()).unwrap_or_default(),
            description: meta.description,
            flags: meta.flags.into_iter().collect(),
            error: None,
            typed: false,
        }
    }

    pub fn update(&mut self, msg: LabelDialogMessage) -> Outcome {
        match msg {
            LabelDialogMessage::Label(v) => {
                self.label = v;
                self.typed = true;
                self.error = None;
            }
            LabelDialogMessage::GroupId(v) => {
                if v.is_empty() || v == "-" || v.parse::<i64>().is_ok() {
                    self.group_id = v;
                }
            }
            LabelDialogMessage::Description(v) => self.description = v,
            LabelDialogMessage::Flag(i, v) => {
                if let Some(flag) = self.flags.get_mut(i) {
                    flag.1 = v;
                }
            }
            LabelDialogMessage::Pick(label) => {
                self.label = label;
                return self.submit();
            }
            LabelDialogMessage::Submit => return self.submit(),
            LabelDialogMessage::Cancel => return Outcome::Cancel,
        }
        Outcome::None
    }

    fn submit(&self) -> Outcome {
        Outcome::Submit(ShapeMeta {
            label: self.label.trim().to_string(),
            group_id: self.group_id.parse().ok(),
            description: self.description.clone(),
            flags: self.flags.iter().cloned().collect(),
        })
    }

    pub fn view<'a>(&'a self, app: &'a App) -> Element<'a, Message> {
        let msg = Message::LabelDialog;
        let title = match self.target {
            LabelTarget::New(ref shape) => {
                format!("New {}", shape.shape_type.label().to_lowercase())
            }
            LabelTarget::Existing(_) => "Edit shape".to_string(),
        };

        let label_input = text_input("Label", &self.label)
            .id(Id::new(LABEL_INPUT))
            .on_input(move |v| msg(LabelDialogMessage::Label(v)))
            .on_submit(msg(LabelDialogMessage::Submit))
            .padding(8);

        let suggestions = app
            .labels
            .suggest(if self.typed { &self.label } else { "" });
        let empty_hint = if suggestions.is_empty() {
            let hint = if app.labels.is_empty() {
                "No labels yet: type one and press Enter"
            } else {
                "New label: press Enter to add it"
            };
            Some(container(style::hint(hint)).padding([6, 8]))
        } else {
            None
        };
        let mut list = column(suggestions.into_iter().take(MAX_SUGGESTIONS).map(|label| {
            let selected = label == self.label.trim();
            button(
                row![
                    style::swatch(crate::canvas::label_color(&app.labels, label), 10.0),
                    text(label.to_string()).size(13)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([4, 8])
            .style(style::list_row(selected))
            .on_press(msg(LabelDialogMessage::Pick(label.to_string())))
            .into()
        }))
        .spacing(1);
        if let Some(hint) = empty_hint {
            list = list.push(hint);
        }

        let group_input = text_input("Group ID", &self.group_id)
            .on_input(move |v| msg(LabelDialogMessage::GroupId(v)))
            .on_submit(msg(LabelDialogMessage::Submit))
            .padding(8)
            .width(Length::FillPortion(1));
        let description_input = text_input("Description", &self.description)
            .on_input(move |v| msg(LabelDialogMessage::Description(v)))
            .on_submit(msg(LabelDialogMessage::Submit))
            .padding(8)
            .width(Length::FillPortion(3));

        let mut body = column![
            text(title).size(16),
            label_input,
            container(scrollable(list).height(Length::Fixed(160.0)))
                .style(|_t| iced::widget::container::Style {
                    border: iced::Border {
                        color: labeleasy_ui::theme::colors().border_variant,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .padding(2),
            row![group_input, description_input].spacing(8),
        ]
        .spacing(10);

        if !self.flags.is_empty() {
            let flags = row(self.flags.iter().enumerate().map(|(i, (name, value))| {
                checkbox(*value)
                    .label(name.as_str())
                    .on_toggle(move |v| msg(LabelDialogMessage::Flag(i, v)))
                    .size(14)
                    .text_size(13)
                    .into()
            }))
            .spacing(14)
            .wrap();
            body = body.push(flags);
        }

        if let Some(error) = &self.error {
            body = body.push(
                text(error)
                    .size(12)
                    .color(labeleasy_ui::theme::colors().error),
            );
        }

        body = body.push(
            row![
                style::hint("Enter to confirm · Esc to cancel").width(Length::Fill),
                button(text("Cancel").size(13))
                    .style(style::secondary_button)
                    .on_press(msg(LabelDialogMessage::Cancel)),
                button(text("OK").size(13))
                    .style(style::primary_button)
                    .on_press(msg(LabelDialogMessage::Submit)),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );

        labeleasy_ui::widgets::card(body, 420.0)
    }
}
