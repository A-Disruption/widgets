//! The calendar, placed three ways.
//!
//! * **Docked** — a single-date calendar in a [`popover`] under a button,
//!   closing as soon as a day is picked.
//! * **Modal** — a two-month range calendar in a [`dialog`], limited to stays
//!   of 2 to 14 nights that start today or later.
//! * **Inline** — a multi-date calendar sitting on the page, with weekends
//!   disabled.
//!
//! The calendar itself never floats: where it appears is decided by what it is
//! placed in.

use chrono::{Datelike, Local, NaiveDate, Weekday};
use iced::Widget as _;
use iced::widget::{button, column, row, text};
use iced::{Element, Theme};

use widgets::anchor::Align;
use widgets::calendar::{self, DateRange};
use widgets::layer::dialog;
use widgets::popover::popover;

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("calendar")
        .theme(theme)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    /// The popover trigger's own press. The popover reports opening and
    /// closing through `on_toggle`; the button only needs a message to be
    /// enabled, and toggling here too would undo the popover's toggle.
    TriggerPressed,
    DueToggled(bool),
    DuePicked(NaiveDate),
    OpenStay,
    CloseStay,
    StayChanged(DateRange),
    DayToggled(NaiveDate),
}

#[derive(Default)]
struct Example {
    due: Option<NaiveDate>,
    due_open: bool,
    stay: Option<DateRange>,
    stay_open: bool,
    days_off: Vec<NaiveDate>,
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::TriggerPressed => {}
            Message::DueToggled(open) => self.due_open = open,
            Message::DuePicked(date) => {
                self.due = Some(date);
                self.due_open = false;
            }
            Message::OpenStay => self.stay_open = true,
            Message::CloseStay => self.stay_open = false,
            Message::StayChanged(range) => self.stay = Some(range),
            Message::DayToggled(date) => {
                if let Some(index) = self.days_off.iter().position(|day| *day == date) {
                    self.days_off.remove(index);
                } else {
                    self.days_off.push(date);
                }
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text("calendar").size(24),
            text("Docked — picks one date and closes").size(13),
            self.due_picker(),
            text("Modal — a range of 2 to 14 nights, from today on").size(13),
            row![
                button(text("Book a stay")).on_press(Message::OpenStay),
                text(describe_stay(self.stay)).size(13),
            ]
            .spacing(12)
            .align_y(iced::Center),
            self.stay_dialog(),
            text("Inline — any number of weekdays").size(13),
            calendar::multiple(&self.days_off, Message::DayToggled)
                .first_weekday(Weekday::Mon)
                .disabled(is_weekend)
                .style(calendar::card),
            text(format!("{} day(s) off picked", self.days_off.len())).size(13),
        ]
        .spacing(12)
        .padding(24)
        .boxed()
    }

    fn due_picker(&self) -> Element<'_, Message> {
        let label = match self.due {
            Some(date) => format!("Due {}", date.format("%a %-d %b %Y")),
            None => "Pick a due date".to_owned(),
        };

        popover(
            button(text(label)).on_press(Message::TriggerPressed),
            calendar::calendar(self.due, Message::DuePicked),
        )
        .open(self.due_open)
        .on_toggle(Message::DueToggled)
        .align(Align::Start)
        .boxed()
    }

    fn stay_dialog(&self) -> Element<'_, Message> {
        let today = Local::now().date_naive();

        dialog(
            column![
                text("Choose your dates").size(18),
                calendar::range(self.stay, Message::StayChanged)
                    .months(2)
                    .min(today)
                    .min_days(3)
                    .max_days(15),
                row![
                    text(describe_stay(self.stay)).size(13),
                    button(text("Done")).on_press(Message::CloseStay),
                ]
                .spacing(16)
                .align_y(iced::Center),
            ]
            .spacing(12),
        )
        .open(self.stay_open)
        .on_dismiss(Message::CloseStay)
        .boxed()
    }
}

fn is_weekend(date: NaiveDate) -> bool {
    matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

/// Nights are days minus one: a stay from the 1st to the 3rd is two nights.
fn describe_stay(stay: Option<DateRange>) -> String {
    match stay {
        None => "No dates yet".to_owned(),
        Some(DateRange { start, end: None }) => {
            format!("Check-in {} — now pick check-out", start.format("%-d %b"))
        }
        Some(
            range @ DateRange {
                start,
                end: Some(end),
            },
        ) => format!(
            "{} → {} · {} night(s)",
            start.format("%-d %b"),
            end.format("%-d %b"),
            range.days() - 1
        ),
    }
}
