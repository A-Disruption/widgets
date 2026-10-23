//! The scroll-wheel time picker, placed three ways.
//!
//! * **Docked** — a 12-hour picker in 5-minute steps, in a [`popover`] under a
//!   button. The lunch hour cannot be picked.
//! * **Modal** — a range in a [`dialog`], kept between 30 minutes and 8 hours.
//!   Try a night shift: the end may come after midnight.
//! * **Inline** — a 24-hour picker with seconds, on a card.
//!
//! Drag a wheel, scroll it, click a row to bring it to the middle, or click the
//! picker and use the arrow keys.

use chrono::{Duration, NaiveTime, Timelike};
use iced::Widget as _;
use iced::widget::{button, column, row, text};
use iced::{Element, Theme};

use widgets::anchor::Align;
use widgets::layer::dialog;
use widgets::popover::popover;
use widgets::time_picker::{self, Clock, TimeRange};

pub fn main() -> iced::Result {
    iced::application(Example::new, Example::update, Example::view)
        .title("time picker")
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
    MeetingToggled(bool),
    MeetingPicked(NaiveTime),
    OpenShift,
    CloseShift,
    ShiftChanged(TimeRange),
    TimerChanged(NaiveTime),
}

struct Example {
    meeting: Option<NaiveTime>,
    meeting_open: bool,
    shift: TimeRange,
    shift_open: bool,
    timer: NaiveTime,
}

impl Example {
    fn new() -> Self {
        Self {
            meeting: None,
            meeting_open: false,
            shift: TimeRange::new(time(9, 0), time(17, 0)),
            shift_open: false,
            timer: NaiveTime::from_hms_opt(0, 5, 30).expect("valid time"),
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TriggerPressed => {}
            Message::MeetingToggled(open) => self.meeting_open = open,
            // The wheels keep turning while the popover is open; it closes
            // when the user clicks away.
            Message::MeetingPicked(time) => self.meeting = Some(time),
            Message::OpenShift => self.shift_open = true,
            Message::CloseShift => self.shift_open = false,
            Message::ShiftChanged(shift) => self.shift = shift,
            Message::TimerChanged(time) => self.timer = time,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text("time picker").size(24),
            text("Docked — 5-minute steps, no lunch-hour meetings").size(13),
            self.meeting_picker(),
            text("Modal — a shift of 30 minutes to 8 hours").size(13),
            row![
                button(text("Set shift")).on_press(Message::OpenShift),
                text(describe(self.shift)).size(13),
            ]
            .spacing(12)
            .align_y(iced::Center),
            self.shift_dialog(),
            text("Inline — 24-hour, with seconds").size(13),
            time_picker::time_picker(Some(self.timer), Message::TimerChanged)
                .clock(Clock::TwentyFourHour)
                .seconds(true)
                .style(time_picker::card),
            text(format!("Timer: {}", self.timer.format("%H:%M:%S"))).size(13),
        ]
        .spacing(12)
        .padding(24)
        .boxed()
    }

    fn meeting_picker(&self) -> Element<'_, Message> {
        let label = match self.meeting {
            Some(time) => format!("Meeting at {}", time.format("%-I:%M %p")),
            None => "Pick a meeting time".to_owned(),
        };

        popover(
            button(text(label)).on_press(Message::TriggerPressed),
            time_picker::time_picker(self.meeting, Message::MeetingPicked)
                .minute_step(5)
                .disabled(|time: NaiveTime| time.hour() == 12),
        )
        .open(self.meeting_open)
        .on_toggle(Message::MeetingToggled)
        .align(Align::Start)
        .boxed()
    }

    fn shift_dialog(&self) -> Element<'_, Message> {
        dialog(
            column![
                text("Shift hours").size(18),
                time_picker::range(self.shift, Message::ShiftChanged)
                    .minute_step(15)
                    .min_duration(Duration::minutes(30))
                    .max_duration(Duration::hours(8)),
                row![
                    text(describe(self.shift)).size(13),
                    button(text("Done")).on_press(Message::CloseShift),
                ]
                .spacing(16)
                .align_y(iced::Center),
            ]
            .spacing(12),
        )
        .open(self.shift_open)
        .on_dismiss(Message::CloseShift)
        .boxed()
    }
}

fn time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("valid time")
}

fn describe(shift: TimeRange) -> String {
    let length = shift.duration();
    let overnight = if shift.crosses_midnight() {
        " (overnight)"
    } else {
        ""
    };

    format!(
        "{} – {} · {}h {:02}m{}",
        shift.start.format("%-I:%M %p"),
        shift.end.format("%-I:%M %p"),
        length.num_hours(),
        length.num_minutes() % 60,
        overnight
    )
}
