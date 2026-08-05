//! Exercises the `popover` widget: press and hover triggers, the safe corridor,
//! edge flipping, and application-controlled visibility.
//!
//! The right-hand popovers are pushed against the window edge so they have to
//! flip. Drag the window narrow to watch the rest follow.

use iced::widget::{button, checkbox, column, container, row, space, text};
use iced::{Element, Fill, Theme};

use widgets::anchor::{Align, Side};
use widgets::animation::Motion;
use widgets::popover::{hover_popover, popover};

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("popover")
        .theme(theme)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    Toggled(&'static str, bool),
    FilterChanged(&'static str, bool),
    Chose(&'static str),
    OpenControlled,
    CloseControlled,
    /// A trigger `button` renders as disabled unless it has an `on_press`.
    /// The popover no longer swallows that press, so the button's own message
    /// fires as well as the popover opening — this one just has nothing to do.
    TriggerPressed,
}

#[derive(Default)]
struct Example {
    last: Option<&'static str>,
    open_filters: Vec<&'static str>,
    controlled_open: bool,
    /// Which popover most recently reported itself open, via `on_toggle`.
    open_popover: Option<&'static str>,
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::Toggled(name, true) => self.open_popover = Some(name),
            Message::Toggled(name, false) => {
                if self.open_popover == Some(name) {
                    self.open_popover = None;
                }
            }
            Message::TriggerPressed => {}
            Message::Chose(label) => self.last = Some(label),
            Message::FilterChanged(name, true) => self.open_filters.push(name),
            Message::FilterChanged(name, false) => self.open_filters.retain(|f| *f != name),
            Message::OpenControlled => {
                self.controlled_open = true;
                println!("Controlled open: {}", self.controlled_open);
            }
            Message::CloseControlled => {
                self.controlled_open = false;
                println!("Controlled open: {}", self.controlled_open);
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let bar = row![
            self.filters_popover(),
            hover_card(),
            space().width(Fill),
            self.controlled_popover(),
            side_popover(),
        ]
        .spacing(8)
        .padding(8);

        let status = match (self.last, self.open_popover) {
            (_, Some(open)) => text(format!("{open} is open")),
            (Some(label), None) => text(format!("chose: {label}")),
            (None, None) => text("nothing chosen yet"),
        };

        column![
            container(bar).width(Fill).style(container::bordered_box),
            container(status).padding(16),
            container(text(
                "Hover \"Details\" and move diagonally into the card — the safe \
                 corridor keeps it open."
            ))
            .padding([0, 16]),
            space().width(Fill).height(Fill),
        ]
        .into()
    }

    /// Interactive content: checkboxes the user clicks inside the popover.
    /// The popover stays open while they do, and closes on an outside press.
    fn filters_popover(&self) -> Element<'_, Message> {
        let filter = |name: &'static str| {
            checkbox(self.open_filters.contains(&name))
                .label(name)
                .on_toggle(move |on| Message::FilterChanged(name, on))
        };

        popover(
            button(text("Filters")).on_press(Message::TriggerPressed),
            column![
                text("Show issues that are").size(12),
                filter("Open"),
                filter("Assigned to me"),
                filter("Recently updated"),
            ]
            .spacing(8),
        )
        .align(Align::Start)
        .min_width(200.0)
        .on_toggle(|open| Message::Toggled("filters", open))
        .into()
    }

    /// Visibility owned by the application rather than the widget.
    fn controlled_popover(&self) -> Element<'_, Message> {
        popover(
            button(text("Controlled")).on_press(Message::OpenControlled),
            column![
                text("The app decides when this is open."),
                button(text("Close")).on_press(Message::CloseControlled),
            ]
            .spacing(8),
        )
        .open(self.controlled_open)
        .align(Align::End)
        .min_width(220.0)
        .into()
    }
}

/// A hover-triggered card with a clickable link inside it — the case the safe
/// corridor exists for.
fn hover_card<'a>() -> Element<'a, Message> {
    hover_popover(
        text("Details"),
        column![
            text("Interactive tooltip").size(14),
            text("The cursor can travel into this card and click things.").size(12),
            button(text("Act on it")).on_press(Message::Chose("hover card action")),
        ]
        .spacing(8),
    )
    .gap(10.0)
    .max_width(260.0)
    .on_toggle(|open| Message::Toggled("hover card", open))
    .into()
}

/// Anchored to the right of its trigger with a slower motion, near the window
/// edge so it has to flip to the left.
fn side_popover<'a>() -> Element<'a, Message> {
    popover(
        button(text("Side")).on_press(Message::TriggerPressed),
        column![
            text("Anchored to the right…"),
            text("…until there is no room, then it flips."),
        ]
        .spacing(6),
    )
    .side(Side::Right)
    .align(Align::Start)
    .motion(Motion::SMOOTH)
    .max_width(240.0)
    .into()
}
