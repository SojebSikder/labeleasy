//! Reusable iced building blocks.
//!
//! - [`theme`]: Zed-compatible theme files, a registry, light/dark/system
//!   selection, and a process-wide [`theme::colors`] for style closures.
//! - [`widgets`]: custom titlebar for undecorated windows, menu bar with
//!   dropdowns, modal cards, toasts, split handles, a virtualized list and
//!   shared styles.
//!
//! Everything is generic over the host's `Message` type.

pub mod theme;
pub mod widgets;
