//! A menu bar with dropdowns, generic over the action type.

use iced::widget::{Column, Space, button, column, container, mouse_area, opaque, row, text};
use iced::{Border, Color, Element, Length, Padding, Shadow, Vector, alignment};

/// tracks the open/closed state of a reusable dropdown menu.
#[derive(Debug, Clone, Default)]
pub struct DropdownMenuState {
    pub open_index: Option<usize>,
    /// the item of the open group whose submenu is showing.
    pub open_submenu: Option<usize>,
}

impl DropdownMenuState {
    /// Creates a new inactive dropdown menu state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles the internal state mutation when a menu item or header is clicked.
    pub fn update<T: Clone>(&mut self, message: DropdownMessage<T>) -> Option<T> {
        match message {
            DropdownMessage::Toggle(index) => {
                self.open_submenu = None;
                if self.open_index == Some(index) {
                    self.open_index = None;
                } else {
                    self.open_index = Some(index);
                }
                None
            }
            DropdownMessage::Close => {
                self.open_index = None;
                self.open_submenu = None;
                None
            }
            DropdownMessage::OpenSubmenu(index) => {
                self.open_submenu = index;
                None
            }
            DropdownMessage::TriggerAction(action) => {
                self.open_index = None; // Automatically close menu on action selection
                self.open_submenu = None;
                Some(action)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DropdownMessage<T> {
    Toggle(usize),
    Close,
    /// shows the submenu of an item of the open group (hovered or clicked), or hides it.
    OpenSubmenu(Option<usize>),
    TriggerAction(T),
}

/// represents an individual actionable item inside a dropdown list.
pub struct DropdownItem<T> {
    pub label: String,
    pub action: Option<T>,
    pub shortcut: Option<String>,
    pub is_separator: bool,
    /// items of the submenu this item opens (one level deep).
    pub children: Vec<DropdownItem<T>>,
}

impl<T> DropdownItem<T> {
    /// Creates a new actionable dropdown menu item.
    pub fn new(label: impl Into<String>, action: T) -> Self {
        Self {
            label: label.into(),
            action: Some(action),
            shortcut: None,
            is_separator: false,
            children: Vec::new(),
        }
    }

    /// Creates a visual divider line separating menu item groups.
    pub fn separator() -> Self {
        Self {
            label: String::new(),
            action: None,
            shortcut: None,
            is_separator: true,
            children: Vec::new(),
        }
    }

    /// a greyed-out item that can't be picked (e.g. "Undo" with nothing to undo).
    pub fn disabled(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action: None,
            shortcut: None,
            is_separator: false,
            children: Vec::new(),
        }
    }

    /// an item that opens `items` beside it. Greyed out when none of them can be picked.
    pub fn submenu(label: impl Into<String>, items: Vec<DropdownItem<T>>) -> Self {
        Self {
            label: label.into(),
            action: None,
            shortcut: None,
            is_separator: false,
            children: items,
        }
    }

    /// `new` when `enabled`, `disabled` otherwise.
    pub fn when(enabled: bool, label: impl Into<String>, action: T) -> Self {
        if enabled {
            Self::new(label, action)
        } else {
            Self::disabled(label)
        }
    }

    /// attaches a keyboard shortcut hint (e.g. "Ctrl+Shift+P") shown next to the label.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    fn is_submenu(&self) -> bool {
        !self.children.is_empty()
    }

    fn is_enabled(&self) -> bool {
        self.action.is_some() || self.children.iter().any(|c| c.is_enabled())
    }

    /// height in a dropdown; fixed so a submenu can be placed beside its item.
    fn height(&self) -> f32 {
        if self.is_separator {
            SEPARATOR_HEIGHT
        } else {
            ITEM_HEIGHT
        }
    }
}

/// configuration structure for a top-level menu column (e.g., "File" or "Help").
pub struct MenuGroup<T> {
    pub title: String,
    pub items: Vec<DropdownItem<T>>,
}

impl<T> MenuGroup<T> {
    /// Creates a new top-level menu group with the given title and items.
    pub fn new(title: impl Into<String>, items: Vec<DropdownItem<T>>) -> Self {
        Self {
            title: title.into(),
            items,
        }
    }
}

/// Computes the exact layout width for a menu header button based on its title.
pub fn menu_button_width(title: &str) -> f32 {
    let mut text_width: f32 = 0.0;
    for ch in title.chars() {
        text_width += match ch {
            'i' | 'l' | 't' | 'j' | 'r' | ' ' | '.' | ':' | '-' => 4.5,
            'f' | 'I' => 5.5,
            'm' | 'w' | 'M' | 'W' => 10.5,
            'A'..='Z' => 8.5,
            _ => 7.0,
        };
    }
    // 20.0 padding (10 left + 10 right) + 8.0 breathing room, with minimum width of 44.0
    (text_width + 28.0_f32).max(44.0).ceil()
}

const ITEM_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 5.0;
const ITEM_SPACING: f32 = 2.0;

fn item_style(theme: &iced::Theme, status: button::Status, highlighted: bool) -> button::Style {
    let colors = crate::theme::colors();
    let border = Border {
        radius: 4.0.into(),
        ..Default::default()
    };
    if highlighted || matches!(status, button::Status::Hovered | button::Status::Pressed) {
        button::Style {
            background: Some(
                Color::from_rgba(
                    colors.text_accent.r,
                    colors.text_accent.g,
                    colors.text_accent.b,
                    0.12,
                )
                .into(),
            ),
            text_color: colors.text_accent,
            border,
            ..button::text(theme, status)
        }
    } else {
        button::Style {
            text_color: colors.text,
            border,
            ..button::text(theme, status)
        }
    }
}

/// the rows of one dropdown panel. `top_level`: items report hovering, so
/// submenus open and close under the cursor.
fn items_column<'a, T: 'static + Clone>(
    items: &[DropdownItem<T>],
    open_submenu: Option<usize>,
    top_level: bool,
) -> Column<'a, DropdownMessage<T>> {
    let mut items_column = column![].spacing(ITEM_SPACING);

    for (index, item) in items.iter().enumerate() {
        if item.is_separator {
            items_column = items_column.push(
                container(
                    container(Space::new().height(1))
                        .width(Length::Fill)
                        .height(Length::Fixed(1.0))
                        .style(|_theme: &iced::Theme| container::Style {
                            background: Some(crate::theme::colors().border_variant.into()),
                            ..Default::default()
                        }),
                )
                .height(Length::Fixed(SEPARATOR_HEIGHT))
                .padding(Padding::from([0, 4]))
                .align_y(alignment::Vertical::Center),
            );
            continue;
        }

        let enabled = item.is_enabled();
        let colors = crate::theme::colors();
        let (label_color, hint_color) = if enabled {
            (None, colors.text_muted)
        } else {
            (Some(colors.text_placeholder), colors.text_placeholder)
        };
        let mut item_row = row![text(item.label.clone()).size(13).color_maybe(label_color)]
            .width(Length::Fill)
            .spacing(16)
            .align_y(alignment::Vertical::Center);
        if let Some(shortcut) = &item.shortcut {
            item_row = item_row.push(text(shortcut.clone()).size(11).color(hint_color));
        }
        if item.is_submenu() {
            item_row = item_row
                .push(Space::new().width(Length::Fill))
                .push(text("›").size(13).color(hint_color));
        }

        let open = open_submenu == Some(index);
        let on_press = match &item.action {
            Some(action) => Some(DropdownMessage::TriggerAction(action.clone())),
            None if item.is_submenu() && enabled => Some(DropdownMessage::OpenSubmenu(Some(index))),
            None => None,
        };
        let element: Element<'a, DropdownMessage<T>> = match on_press {
            Some(message) => button(item_row)
                .width(Length::Fill)
                .height(Length::Fixed(ITEM_HEIGHT))
                .padding([5, 10])
                .style(move |theme: &iced::Theme, status| item_style(theme, status, open))
                .on_press(message)
                .into(),
            None => container(item_row)
                .height(Length::Fixed(ITEM_HEIGHT))
                .padding([5, 10])
                .into(),
        };

        // hovering a submenu item opens it; hovering anything else closes an open one
        let hover = if !top_level {
            None
        } else if item.is_submenu() && enabled {
            (!open).then_some(Some(index))
        } else {
            open_submenu.map(|_| None)
        };
        items_column = items_column.push(match hover {
            Some(target) => mouse_area(element)
                .on_enter(DropdownMessage::OpenSubmenu(target))
                .into(),
            None => element,
        });
    }
    items_column
}

fn panel<'a, T: 'static>(items: Column<'a, DropdownMessage<T>>) -> Element<'a, DropdownMessage<T>> {
    opaque(
        container(items)
            .width(260)
            .padding(5)
            .style(|_theme: &iced::Theme| {
                let colors = crate::theme::colors();
                container::Style {
                    background: Some(colors.elevated_surface_background.into()),
                    text_color: Some(colors.text),
                    border: Border {
                        color: colors.border_variant,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.38),
                        offset: Vector::new(0.0, 6.0),
                        blur_radius: 20.0,
                    },
                    ..Default::default()
                }
            }),
    )
}

/// renders the floating dropdown overlay panel (and its open submenu) if one is open.
pub fn render_menu_overlay<'a, T: 'static + Clone>(
    state: &DropdownMenuState,
    groups: &[MenuGroup<T>],
) -> Option<Element<'a, DropdownMessage<T>>> {
    let open_idx = state.open_index?;
    let target_group = groups.get(open_idx)?;
    let open_submenu = state.open_submenu.filter(|&i| {
        target_group
            .items
            .get(i)
            .is_some_and(DropdownItem::is_submenu)
    });

    let mut panels = row![panel(items_column(&target_group.items, open_submenu, true))];
    if let Some(index) = open_submenu {
        // level the submenu's first item with the item that opened it
        let top: f32 = target_group.items[..index]
            .iter()
            .map(|item| item.height() + ITEM_SPACING)
            .sum();
        panels = panels.push(column![
            Space::new().height(top),
            panel(items_column(
                &target_group.items[index].children,
                None,
                false
            ))
        ]);
    }

    // Compute cumulative horizontal offset matching each header button's position
    let mut horizontal_offset: f32 = 8.0;
    for group in &groups[..open_idx] {
        horizontal_offset += menu_button_width(&group.title) + 2.0;
    }

    let overlay_layer = column![
        container(text("")).height(super::titlebar::TITLEBAR_HEIGHT),
        row![container(text("")).width(horizontal_offset), panels]
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    Some(overlay_layer.into())
}

/// renders the horizontal menu bar strip with active group highlights.
pub fn render_menu_bar<'a, T: 'static + Clone>(
    state: &DropdownMenuState,
    groups: &[MenuGroup<T>],
) -> Element<'a, DropdownMessage<T>> {
    let mut menu_row = row![].spacing(2).align_y(alignment::Vertical::Center);

    for (group_idx, group) in groups.iter().enumerate() {
        let is_open = state.open_index == Some(group_idx);
        let button_w = menu_button_width(&group.title);
        let header_button = button(
            container(text(group.title.clone()).size(13).font(iced::Font {
                weight: if is_open {
                    iced::font::Weight::Bold
                } else {
                    iced::font::Weight::Normal
                },
                ..iced::Font::DEFAULT
            }))
            .width(Length::Fill)
            .align_x(alignment::Horizontal::Center),
        )
        .width(Length::Fixed(button_w))
        .padding([4, 10])
        .style(move |theme: &iced::Theme, status| {
            let colors = crate::theme::colors();
            if is_open {
                button::Style {
                    background: Some(
                        Color::from_rgba(
                            colors.text_accent.r,
                            colors.text_accent.g,
                            colors.text_accent.b,
                            0.15,
                        )
                        .into(),
                    ),
                    text_color: colors.text_accent,
                    border: Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: Color::from_rgba(
                            colors.text_accent.r,
                            colors.text_accent.g,
                            colors.text_accent.b,
                            0.35,
                        ),
                    },
                    ..button::text(theme, status)
                }
            } else {
                button::Style {
                    text_color: colors.text,
                    border: Border {
                        radius: 4.0.into(),
                        ..Default::default()
                    },
                    ..button::text(theme, status)
                }
            }
        })
        .on_press(DropdownMessage::Toggle(group_idx));

        menu_row = menu_row.push(header_button);
    }

    container(menu_row)
        .width(Length::Shrink)
        .padding([0, 8])
        .style(|_theme| container::Style {
            background: Some(Color::TRANSPARENT.into()),
            ..Default::default()
        })
        .into()
}
