//! The time zone picker, beside a time picker and on its own.
//!
//! * **Docked** — a meeting time on a [`time_picker`], with its zone picked
//!   from a [`popover`] beside it. The meeting is converted into the local
//!   zone below.
//! * **Inline** — a shortlist of office zones on a card, restricted with
//!   `zones(..)`.
//!
//! Click a picker and type to search: a city (`tokyo`), a region (`europe`),
//! an abbreviation (`pdt`) or an offset (`+5:30`, `utc-4`).

use chrono::{Local, NaiveTime, TimeZone};
use chrono_tz::Tz;
use iced::Widget as _;
use iced::widget::{button, column, row, text};
use iced::{Element, Theme};

use widgets::anchor::Align;
use widgets::popover::popover;
use widgets::time_picker::time_picker;
use widgets::timezone_picker::{self, timezone_picker};

pub fn main() -> iced::Result {
    iced::application(Example::new, Example::update, Example::view)
        .title("time zone picker")
        .theme(theme)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    MeetingChanged(NaiveTime),
    /// The popover trigger's own press. The popover reports opening and
    /// closing through `on_toggle`; the button only needs a message to be
    /// enabled, and toggling here too would undo the popover's toggle.
    TriggerPressed,
    ZoneToggled(bool),
    ZonePicked(Tz),
    OfficePicked(Tz),
}

struct Example {
    meeting: NaiveTime,
    zone: Tz,
    zone_open: bool,
    office: Option<Tz>,
}

impl Example {
    fn new() -> Self {
        Self {
            meeting: NaiveTime::from_hms_opt(9, 30, 0).expect("valid time"),
            zone: Tz::America__New_York,
            zone_open: false,
            office: None,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::MeetingChanged(time) => self.meeting = time,
            Message::TriggerPressed => {}
            Message::ZoneToggled(open) => self.zone_open = open,
            Message::ZonePicked(zone) => {
                self.zone = zone;
                self.zone_open = false;
            }
            Message::OfficePicked(zone) => self.office = Some(zone),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text("time zone picker").size(24),
            text("Docked — a meeting time and the zone it is in").size(13),
            row![
                time_picker(Some(self.meeting), Message::MeetingChanged).minute_step(15),
                self.zone_button(),
            ]
            .spacing(16)
            .align_y(iced::Center),
            text(self.in_local_time()).size(13),
            text("Inline — a shortlist of offices").size(13),
            timezone_picker(self.office, Message::OfficePicked)
                .zones([
                    Tz::America__Los_Angeles,
                    Tz::America__New_York,
                    Tz::America__Sao_Paulo,
                    Tz::Europe__London,
                    Tz::Europe__Berlin,
                    Tz::Asia__Kolkata,
                    Tz::Asia__Singapore,
                    Tz::Asia__Tokyo,
                    Tz::Australia__Sydney,
                ])
                .rows(5)
                .style(timezone_picker::card),
        ]
        .spacing(12)
        .padding(24)
        .boxed()
    }

    fn zone_button(&self) -> Element<'_, Message> {
        popover(
            button(text(self.zone.name())).on_press(Message::TriggerPressed),
            timezone_picker(Some(self.zone), Message::ZonePicked),
        )
        .open(self.zone_open)
        .on_toggle(Message::ZoneToggled)
        .align(Align::Start)
        .boxed()
    }

    /// Today's meeting, read in its zone and shown in the local one.
    fn in_local_time(&self) -> String {
        let today = Local::now().with_timezone(&self.zone).date_naive();
        let meeting = self
            .zone
            .from_local_datetime(&today.and_time(self.meeting))
            .earliest();

        match meeting {
            Some(meeting) => format!(
                "{} in {} is {} here",
                meeting.format("%-I:%M %p %Z"),
                self.zone.name(),
                meeting
                    .with_timezone(&Local)
                    .format("%-I:%M %p on %a %-d %b"),
            ),
            // The clocks skip that time today, for daylight saving.
            None => format!(
                "{} does not happen today in {}",
                self.meeting,
                self.zone.name()
            ),
        }
    }
}
