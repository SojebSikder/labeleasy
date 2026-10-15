//! A scrollable list that only builds the rows in view, so lists of tens of
//! thousands of items (image folders) cost the same as a screenful.
//!
//! Rows have a fixed height. The host keeps a [`VirtualListState`] and
//! forwards the scrollable's viewport to [`VirtualListState::on_scroll`].

use iced::widget::scrollable::{self, AbsoluteOffset, Viewport};
use iced::widget::{Column, Space, column, scrollable as scroll};
use iced::{Element, Length, Task};

#[derive(Debug, Clone)]
pub struct VirtualListState {
    pub id: iced::widget::Id,
    offset: f32,
    viewport_height: f32,
}

impl VirtualListState {
    pub fn new(id: &'static str) -> Self {
        Self {
            id: iced::widget::Id::new(id),
            offset: 0.0,
            // until the first scroll event reports the real height
            viewport_height: 1200.0,
        }
    }

    pub fn on_scroll(&mut self, viewport: Viewport) {
        self.offset = viewport.absolute_offset().y;
        self.viewport_height = viewport.bounds().height;
    }

    /// the visible row range (plus a little overscan) for `count` rows.
    pub fn visible_range(&self, count: usize, row_height: f32) -> std::ops::Range<usize> {
        const OVERSCAN: usize = 4;
        let first = (self.offset / row_height).floor().max(0.0) as usize;
        let visible = (self.viewport_height / row_height).ceil() as usize;
        let start = first.saturating_sub(OVERSCAN).min(count);
        let end = (first + visible + OVERSCAN).min(count);
        start..end
    }

    /// back to the top - call when the rows are replaced (a new filter, a
    /// new document), or the stale offset would show an empty window past
    /// the end of a shorter list.
    pub fn reset<Message: 'static>(&mut self) -> Task<Message> {
        self.offset = 0.0;
        iced::widget::operation::scroll_to(
            self.id.clone(),
            AbsoluteOffset {
                x: None,
                y: Some(0.0),
            },
        )
    }

    /// scrolls just enough to show row `index`.
    pub fn reveal<Message: 'static>(&mut self, index: usize, row_height: f32) -> Task<Message> {
        let top = index as f32 * row_height;
        let bottom = top + row_height;
        let target = if top < self.offset {
            top
        } else if bottom > self.offset + self.viewport_height {
            bottom - self.viewport_height
        } else {
            return Task::none();
        };
        self.offset = target.max(0.0);
        iced::widget::operation::scroll_to(
            self.id.clone(),
            AbsoluteOffset {
                x: None,
                y: Some(self.offset),
            },
        )
    }
}

/// builds the list; `row(i)` renders item `i` and must be `row_height` tall.
pub fn virtual_list<'a, Message: 'a>(
    state: &VirtualListState,
    count: usize,
    row_height: f32,
    row: impl Fn(usize) -> Element<'a, Message>,
    on_scroll: impl Fn(Viewport) -> Message + 'a,
) -> Element<'a, Message> {
    let range = state.visible_range(count, row_height);
    let above = range.start as f32 * row_height;
    let below = (count - range.end) as f32 * row_height;

    let rows: Column<'a, Message> = column(range.map(row));
    let content =
        column![Space::new().height(above), rows, Space::new().height(below)].width(Length::Fill);

    scroll(content)
        .id(state.id.clone())
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(8).scroller_width(6),
        ))
        .on_scroll(on_scroll)
        .height(Length::Fill)
        .into()
}
