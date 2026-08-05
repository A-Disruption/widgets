//! Exercises the `layer` widget: dialogs, sheets, and edge/corner anchoring.
//!
//! Layers are stacked over the page rather than laid out in it, so nothing
//! below ever moves as they appear and disappear.
//!
//! # What to look at
//!
//! * **Dialog** is the plain confirmation surface: centred, dimmed page, no
//!   chrome. Escape and a press on the backdrop both ask it to close — and it
//!   is the application that decides whether to.
//! * **Sheets** slide out of an edge and span it. The left and right ones run
//!   the full height; the bottom one runs the full width.
//! * **Refuses to close** shows why `on_dismiss` reports rather than acts: the
//!   layer asks, the application says no, and it stays put.

use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Element, Fill, Theme};

use widgets::layer::{Anchor, dialog, layer, sheet};

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("layer")
        .theme(theme)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

/// Which layer, if any, is showing. Only one at a time keeps the example
/// readable — layers themselves have no such restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Confirm,
    Stubborn,
    Sheet(Anchor),
    Corner(Anchor),
}

#[derive(Debug, Clone)]
enum Message {
    Show(Panel),
    Dismiss,
    Refused,
    Deleted,
}

struct Example {
    panel: Option<Panel>,
    deleted: u32,
    refusals: u32,
    /// The edge the last sheet came from.
    ///
    /// Kept separately from `panel`, because `panel` goes to `None` the moment
    /// the sheet is dismissed — and the sheet is still on screen animating out
    /// for a few hundred milliseconds after that. Deriving the edge from
    /// `panel` would snap it to the default mid-close and the sheet would leave
    /// through the wrong edge.
    sheet_side: Anchor,
    /// The same, for the corner-anchored panel.
    corner: Anchor,
}

impl Default for Example {
    fn default() -> Self {
        Self {
            panel: None,
            deleted: 0,
            refusals: 0,
            // `Anchor` defaults to `Center`, which is not an edge and so is not
            // a sheet. These only matter before the first press anyway.
            sheet_side: Anchor::Right,
            corner: Anchor::BottomRight,
        }
    }
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::Show(panel) => {
                match panel {
                    Panel::Sheet(side) => self.sheet_side = side,
                    Panel::Corner(corner) => self.corner = corner,
                    _ => {}
                }

                self.panel = Some(panel);
            }
            Message::Dismiss => self.panel = None,
            // The layer asked to close and the application declined.
            Message::Refused => self.refusals += 1,
            Message::Deleted => {
                self.panel = None;
                self.deleted += 1;
            }
        }
    }

    fn showing(&self, panel: Panel) -> bool {
        self.panel == Some(panel)
    }

    fn view(&self) -> Element<'_, Message> {
        let show = |label: &'static str, panel: Panel| {
            button(text(label)).on_press(Message::Show(panel))
        };

        let page = column![
            text("layer").size(24),
            text(format!(
                "deleted {} time(s) · {} dismissal(s) refused",
                self.deleted, self.refusals
            ))
            .size(13),
            text("Dialogs").size(13),
            row![
                show("Confirm", Panel::Confirm),
                show("Refuses to close", Panel::Stubborn),
            ]
            .spacing(8),
            text("Sheets — slide out of an edge and span it").size(13),
            row![
                show("Left", Panel::Sheet(Anchor::Left)),
                show("Right", Panel::Sheet(Anchor::Right)),
                show("Bottom", Panel::Sheet(Anchor::Bottom)),
                show("Top", Panel::Sheet(Anchor::Top)),
            ]
            .spacing(8),
            text("Corners — anchored, not stretched, no backdrop").size(13),
            row![
                show("Top left", Panel::Corner(Anchor::TopLeft)),
                show("Top right", Panel::Corner(Anchor::TopRight)),
                show("Bottom left", Panel::Corner(Anchor::BottomLeft)),
                show("Bottom right", Panel::Corner(Anchor::BottomRight)),
            ]
            .spacing(8),
            space().width(Fill).height(Fill),
        ]
        .spacing(16)
        .padding(24);

        // A `Stack` is the natural host: the layers report a zero size, and a
        // `Column` with spacing would still budget a gap for each of them.
        stack![
            container(page).width(Fill).height(Fill),
            self.confirm(),
            self.stubborn(),
            self.sheets(),
            self.corner(),
        ]
        .into()
    }

    /// The ordinary case: centred, dimmed, undecorated.
    fn confirm(&self) -> Element<'_, Message> {
        dialog(
            column![
                text("Delete this project?").size(18),
                text("This cannot be undone.").size(13),
                row![
                    button(text("Cancel")).on_press(Message::Dismiss),
                    button(text("Delete")).on_press(Message::Deleted),
                ]
                .spacing(8),
            ]
            .spacing(12),
        )
        .open(self.showing(Panel::Confirm))
        .min_width(340.0)
        .on_dismiss(Message::Dismiss)
        .into()
    }

    /// The same dialog, but its `on_dismiss` only counts the attempt. Escape
    /// and backdrop presses are reported and ignored — which is the whole point
    /// of the layer asking rather than acting.
    fn stubborn(&self) -> Element<'_, Message> {
        dialog(
            column![
                text("Saving…").size(18),
                text("Escape and the backdrop will not close this.").size(13),
                button(text("Let me out")).on_press(Message::Dismiss),
            ]
            .spacing(12),
        )
        .open(self.showing(Panel::Stubborn))
        .min_width(340.0)
        .on_dismiss(Message::Refused)
        .into()
    }

    fn sheets(&self) -> Element<'_, Message> {
        sheet(
            column![
                text("Sheet").size(18),
                text("Spans the edge it came from.").size(13),
                button(text("Close")).on_press(Message::Dismiss),
            ]
            .spacing(12)
            .padding(8),
            self.sheet_side,
        )
        .open(matches!(self.panel, Some(Panel::Sheet(_))))
        .min_width(300.0)
        .on_dismiss(Message::Dismiss)
        .into()
    }

    /// A corner-anchored panel with no backdrop, so the page stays live behind
    /// it. For notification stacks use `widgets::toast` — it adds per-item
    /// timers and reflow, which a single layer cannot do.
    fn corner(&self) -> Element<'_, Message> {
        layer(
            row![
                text("Anchored, page still live."),
                button(text("Close")).on_press(Message::Dismiss),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
        )
        .anchor(self.corner)
        .open(matches!(self.panel, Some(Panel::Corner(_))))
        .on_dismiss(Message::Dismiss)
        .into()
    }
}
