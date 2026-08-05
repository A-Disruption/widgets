//! Exercises the `layer` widget: a modal with a backdrop, and toasts anchored
//! to viewport corners.
//!
//! Layers are stacked over the page rather than laid out in it, so the buttons
//! below never move as they appear and disappear.

use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Element, Fill, Theme};

use widgets::layer::{Anchor, layer, modal};

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("layer")
        .theme(theme)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    Confirm,
    CancelConfirm,
    Deleted,
    ShowToast(Anchor),
    DismissToast,
    ShowSheet,
    DismissSheet,
}

#[derive(Default)]
struct Example {
    confirming: bool,
    toast: Option<Anchor>,
    sheet: bool,
    deleted: u32,
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::Confirm => self.confirming = true,
            Message::CancelConfirm => self.confirming = false,
            Message::Deleted => {
                self.confirming = false;
                self.deleted += 1;
                self.toast = Some(Anchor::BottomRight);
            }
            Message::ShowToast(corner) => self.toast = Some(corner),
            Message::DismissToast => self.toast = None,
            Message::ShowSheet => self.sheet = true,
            Message::DismissSheet => self.sheet = false,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let page = column![
            text("layer").size(24),
            text(format!("deleted {} time(s)", self.deleted)),
            row![
                button(text("Delete…")).on_press(Message::Confirm),
                button(text("Toast top-left")).on_press(Message::ShowToast(Anchor::TopLeft)),
                button(text("Toast bottom")).on_press(Message::ShowToast(Anchor::Bottom)),
                button(text("Bottom sheet")).on_press(Message::ShowSheet),
            ]
            .spacing(8),
            space().width(Fill).height(Fill),
        ]
        .spacing(16)
        .padding(24);

        // A `Stack` is the natural host: the layers report a zero size, and a
        // `Column` with spacing would still budget a gap for them.
        stack![
            container(page).width(Fill).height(Fill),
            self.confirm_modal(),
            self.toast_layer(),
            self.bottom_sheet(),
        ]
        .into()
    }

    /// A modal: centred, backdrop, and it refuses to close itself — the app
    /// decides, which is what `on_dismiss` reporting rather than acting buys.
    fn confirm_modal(&self) -> Element<'_, Message> {
        modal(
            column![
                text("Delete this project?").size(18),
                text("This cannot be undone.").size(13),
                row![
                    button(text("Cancel")).on_press(Message::CancelConfirm),
                    button(text("Delete")).on_press(Message::Deleted),
                ]
                .spacing(8),
            ]
            .spacing(12),
        )
        .open(self.confirming)
        .min_width(320.0)
        .on_dismiss(Message::CancelConfirm)
        .into()
    }

    /// A corner-anchored notice with no backdrop, so the page underneath stays
    /// live. For real notification stacks use `widgets::toast` instead — it adds
    /// per-item timers and reflow, which a single layer cannot do.
    fn toast_layer(&self) -> Element<'_, Message> {
        let corner = self.toast.unwrap_or(Anchor::BottomRight);

        layer(
            row![
                text("Saved."),
                button(text("Dismiss")).on_press(Message::DismissToast),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .anchor(corner)
        .open(self.toast.is_some())
        .into()
    }

    /// Anchored to an edge rather than a corner, sliding up from it.
    fn bottom_sheet(&self) -> Element<'_, Message> {
        layer(
            column![
                text("Bottom sheet").size(18),
                text("Anchored to the bottom edge, so it rises from there.").size(13),
                button(text("Close")).on_press(Message::DismissSheet),
            ]
            .spacing(12),
        )
        .anchor(Anchor::Bottom)
        .open(self.sheet)
        .min_width(420.0)
        .on_dismiss(Message::DismissSheet)
        .into()
    }
}
