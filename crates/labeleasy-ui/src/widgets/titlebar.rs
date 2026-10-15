//! Custom window title bar for undecorated windows: draggable area, an
//! embedded menu strip, and OS-style window controls.

use iced::widget::{Space, button, column, container, mouse_area, row, stack, text};
use iced::{Alignment, Border, Color, Element, Length, Padding};

/// Height of the title bar in logical pixels.
pub const TITLEBAR_HEIGHT: f32 = 36.0;

/// the messages the title bar's controls send.
#[derive(Debug, Clone)]
pub struct TitlebarActions<Message> {
    pub drag: Message,
    pub minimize: Message,
    pub toggle_maximize: Message,
    pub close: Message,
}

/// Renders the full-width custom titlebar: `leading` (usually the menu bar)
/// on the left, `center` text, then the window controls.
pub fn titlebar<'a, Message: Clone + 'a>(
    leading: Element<'a, Message>,
    title: String,
    maximized: bool,
    actions: TitlebarActions<Message>,
) -> Element<'a, Message> {
    let colors = crate::theme::colors();

    let neutral_hover = Color::from_rgba(colors.text.r, colors.text.g, colors.text.b, 0.12);
    let neutral_pressed = Color::from_rgba(colors.text.r, colors.text.g, colors.text.b, 0.22);
    let close_hover = Color::from_rgba(0.86, 0.21, 0.27, 0.90);
    let close_pressed = Color::from_rgba(0.72, 0.15, 0.20, 0.95);

    let btn_minimize = win_btn("━", 20.0, neutral_hover, neutral_pressed, actions.minimize);
    let (maximize_icon, maximize_size) = if maximized {
        ("❐", 20.0)
    } else {
        ("□", 22.0)
    };
    let btn_maximize = win_btn(
        maximize_icon,
        maximize_size,
        neutral_hover,
        neutral_pressed,
        actions.toggle_maximize.clone(),
    );
    let btn_close = win_btn_close("✕", 18.0, close_hover, close_pressed, actions.close);

    let controls = row![btn_minimize, btn_maximize, btn_close]
        .spacing(0)
        .align_y(Alignment::Center)
        .height(Length::Fill);

    let title = text(title).size(12).color(colors.text_muted);
    let bar_content = row![
        leading,
        Space::new().width(Length::Fill),
        title,
        Space::new().width(Length::Fill),
        controls
    ]
    .width(Length::Fill)
    .height(Length::Fixed(TITLEBAR_HEIGHT - 1.0))
    .align_y(Alignment::Center);

    // drag anywhere not captured by a button moves the window; double-click toggles maximize.
    let draggable: Element<'a, Message> = mouse_area(bar_content)
        .on_press(actions.drag)
        .on_double_click(actions.toggle_maximize)
        .into();

    let bar_body = container(draggable)
        .width(Length::Fill)
        .height(Length::Fixed(TITLEBAR_HEIGHT - 1.0))
        .padding(Padding::ZERO)
        .style(|_theme| container::Style {
            background: Some(crate::theme::colors().title_bar_background.into()),
            ..Default::default()
        });

    column![bar_body, super::style::divider()]
        .width(Length::Fill)
        .height(Length::Fixed(TITLEBAR_HEIGHT))
        .into()
}

/// Window control button (minimize / maximize).
fn win_btn<'a, Message: Clone + 'a>(
    icon: &'static str,
    font_size: f32,
    hover_bg: Color,
    pressed_bg: Color,
    msg: Message,
) -> Element<'a, Message> {
    let colors = crate::theme::colors();
    button(
        container(
            text(icon)
                .size(font_size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(move |_theme: &iced::Theme, status| {
        let (bg, text_color) = match status {
            iced::widget::button::Status::Pressed => (Some(pressed_bg.into()), colors.text),
            iced::widget::button::Status::Hovered => (Some(hover_bg.into()), colors.text),
            _ => (None, colors.text_muted),
        };
        button::Style {
            background: bg,
            text_color,
            border: Border {
                radius: 0.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(msg)
    .into()
}

/// Close button — turns white-on-red on hover.
fn win_btn_close<'a, Message: Clone + 'a>(
    icon: &'static str,
    font_size: f32,
    hover_bg: Color,
    pressed_bg: Color,
    msg: Message,
) -> Element<'a, Message> {
    let colors = crate::theme::colors();
    button(
        container(
            text(icon)
                .size(font_size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .width(Length::Fixed(46.0))
    .height(Length::Fill)
    .style(move |_theme: &iced::Theme, status| {
        let (bg, text_color) = match status {
            iced::widget::button::Status::Pressed => (Some(pressed_bg.into()), Color::WHITE),
            iced::widget::button::Status::Hovered => (Some(hover_bg.into()), Color::WHITE),
            _ => (None, colors.text_muted),
        };
        button::Style {
            background: bg,
            text_color,
            border: Border {
                radius: 0.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .on_press(msg)
    .into()
}

/// Corner radius of the (non-maximized) window
/// matches the radius Windows 11 uses for `DWMWCP_ROUND`.
pub const WINDOW_CORNER_RADIUS: f32 = 8.0;

/// Asks the OS to round the corners of the undecorated window.
/// Only Windows 11 supports this natively, elsewhere it's a no-op and only the
/// outline from [`render_window_border`] is rounded.
pub fn apply_rounded_corners(window: &dyn iced::window::Window) {
    #[cfg(windows)]
    {
        use raw_window_handle::RawWindowHandle;
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
            DwmSetWindowAttribute,
        };

        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;

        let corner_preference = DWMWCP_ROUND;
        let border_color = DWMWA_COLOR_NONE;

        // SAFETY: `hwnd` is the live window handle iced gave us, and each attribute
        // value points to a correctly sized local.
        // Failures (e.g. on Windows 10, which lacks these attributes) are harmless and ignored.
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &corner_preference as *const _ as *const _,
                std::mem::size_of_val(&corner_preference) as u32,
            );
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &border_color as *const _ as *const _,
                std::mem::size_of_val(&border_color) as u32,
            );
        }
    }

    #[cfg(not(windows))]
    let _ = window;
}

/// Renders a rounded outline around the undecorated window.
pub fn window_border<'a, Message: 'a>() -> Element<'a, Message> {
    let colors = crate::theme::colors();
    let border_color = colors.border;

    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            border: Border {
                color: border_color,
                width: 1.0,
                radius: WINDOW_CORNER_RADIUS.into(),
            },
            ..Default::default()
        })
        .into()
}

/// Renders invisible edge and corner handles along the perimeter of the window
/// to allow drag-resizing an undecorated (`decorations: false`) window.
pub fn window_resize_handles<'a, Message: Clone + 'a>(
    on_drag: impl Fn(iced::window::Direction) -> Message,
) -> Element<'a, Message> {
    const HANDLE_THICKNESS: f32 = 5.0;
    const CORNER_SIZE: f32 = 14.0;

    // Left edge
    let left_edge = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(HANDLE_THICKNESS))
                .height(Length::Fill),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(on_drag(iced::window::Direction::West)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Start);

    // Right edge
    let right_edge = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(HANDLE_THICKNESS))
                .height(Length::Fill),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(on_drag(iced::window::Direction::East)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::End);

    // Bottom edge
    let bottom_edge = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(HANDLE_THICKNESS)),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(on_drag(iced::window::Direction::South)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(Alignment::End);

    // Top edge (excludes the top-right 140px window control button area)
    let top_row = row![
        mouse_area(
            container(Space::new())
                .width(Length::Fill)
                .height(Length::Fixed(HANDLE_THICKNESS)),
        )
        .interaction(iced::mouse::Interaction::ResizingVertically)
        .on_press(on_drag(iced::window::Direction::North)),
        Space::new().width(Length::Fixed(140.0)),
    ]
    .width(Length::Fill)
    .height(Length::Fixed(HANDLE_THICKNESS));

    let top_edge = container(top_row)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Start);

    // Bottom-Left corner
    let bottom_left_corner = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(CORNER_SIZE))
                .height(Length::Fixed(CORNER_SIZE)),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(on_drag(iced::window::Direction::SouthWest)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Start)
    .align_y(Alignment::End);

    // Bottom-Right corner
    let bottom_right_corner = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(CORNER_SIZE))
                .height(Length::Fixed(CORNER_SIZE)),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(on_drag(iced::window::Direction::SouthEast)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::End)
    .align_y(Alignment::End);

    // Top-Left corner
    let top_left_corner = container(
        mouse_area(
            container(Space::new())
                .width(Length::Fixed(CORNER_SIZE))
                .height(Length::Fixed(CORNER_SIZE)),
        )
        .interaction(iced::mouse::Interaction::ResizingHorizontally)
        .on_press(on_drag(iced::window::Direction::NorthWest)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Start)
    .align_y(Alignment::Start);

    stack![
        left_edge,
        right_edge,
        bottom_edge,
        top_edge,
        bottom_left_corner,
        bottom_right_corner,
        top_left_corner,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
