use iced::{
    Element, Length, Task, Theme,
    alignment::Horizontal,
    widget::{button, column, container, text},
};
use widgets::date_picker::{DateSelection, TimeSelection, date_picker};

fn main() -> iced::Result {
    iced::application(
        DatePickerApp::new,
        DatePickerApp::update,
        DatePickerApp::view,
    )
    .theme(DatePickerApp::theme)
    .title(DatePickerApp::title)
    .run()
}

// ── App State ──────────────────────────────────────────────────────────────────

struct DatePickerApp {
    // Single-date picker
    single_open: bool,
    single_selection: DateSelection,
    single_time: TimeSelection,

    // Range picker
    range_open: bool,
    range_selection: DateSelection,

    // Toggle time picker visibility
    show_time: bool,
}

impl DatePickerApp {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                single_open: true,
                single_selection: DateSelection::single(),
                single_time: TimeSelection::default(),
                range_open: false,
                range_selection: DateSelection::range(),
                show_time: false,
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        "Date Picker Example".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::TokyoNight
    }
}

// ── Messages ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Message {
    OpenSingle,
    CloseSingle,
    SingleChanged(DateSelection),
    SingleChangedWithTime(DateSelection, TimeSelection),
    SingleToday,

    OpenRange,
    CloseRange,
    RangeChanged(DateSelection),
    RangeToday,

    ToggleTime,
}

// ── Update ─────────────────────────────────────────────────────────────────────

impl DatePickerApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OpenSingle => self.single_open = true,
            Message::CloseSingle => self.single_open = false,
            Message::SingleChanged(sel) => {
                self.single_selection = sel;
            }
            Message::SingleChangedWithTime(sel, time) => {
                self.single_selection = sel;
                self.single_time = time;
            }
            Message::SingleToday => {
                // Selection changes come from the picker itself; keep the overlay open.
            }

            Message::OpenRange => self.range_open = true,
            Message::CloseRange => self.range_open = false,
            Message::RangeChanged(sel) => {
                self.range_selection = sel;
            }
            Message::RangeToday => {}

            Message::ToggleTime => self.show_time = !self.show_time,
        }
        Task::none()
    }
}

// ── View ───────────────────────────────────────────────────────────────────────

impl DatePickerApp {
    fn view(&self) -> Element<'_, Message> {
        // ── Single-date button label ──
        let single_label = match &self.single_selection {
            DateSelection::Single(Some(d)) if self.show_time => {
                let (h12, is_pm) = to_12h(self.single_time.hour);
                format!(
                    "{} {:02}:{:02} {}",
                    d.format("%m/%d/%Y"),
                    h12,
                    self.single_time.minute,
                    if is_pm { "PM" } else { "AM" },
                )
            }
            DateSelection::Single(Some(d)) => d.format("%m/%d/%Y").to_string(),
            _ => "Select a date…".to_string(),
        };

        // ── Range button label ──
        let range_label = match &self.range_selection {
            DateSelection::Range {
                start: Some(s),
                end: Some(e),
            } => {
                format!("{} → {}", s.format("%m/%d/%Y"), e.format("%m/%d/%Y"))
            }
            DateSelection::Range {
                start: Some(s),
                end: None,
            } => {
                format!("{} → …", s.format("%m/%d/%Y"))
            }
            _ => "Select a date range…".to_string(),
        };

        // ── Buttons ──
        let single_btn = button(text(single_label).align_x(Horizontal::Center))
            .width(220)
            .on_press(Message::OpenSingle);

        let range_btn = button(text(range_label).align_x(Horizontal::Center))
            .width(280)
            .on_press(Message::OpenRange);

        let time_toggle = button(text(if self.show_time {
            "Hide time picker"
        } else {
            "Show time picker"
        }))
        .on_press(Message::ToggleTime);

        // ── Content layout ──
        let content = column![
            text("Date Picker Demo").size(22),
            iced::widget::Space::new(),
            text("Single date:").size(14),
            single_btn,
            iced::widget::Space::new(),
            text("Date range:").size(14),
            range_btn,
            iced::widget::Space::new(),
            time_toggle,
        ]
        .spacing(6)
        .padding(40);

        // ── Invisible picker anchors ──
        //
        // date_picker widgets are effectively invisible. They hook into iced's
        // overlay system and draw the calendar popup when is_open = true.
        // They must be present in the widget tree at all times.

        let single_picker: Element<'_, Message> = if self.show_time {
            date_picker(self.single_open, self.single_selection.clone())
                .show_time()
                .initial_time(self.single_time)
                .on_change_with_time(Message::SingleChangedWithTime)
                .on_close(|| Message::CloseSingle)
                .on_today(|_| Message::SingleToday)
                .into()
        } else {
            date_picker(self.single_open, self.single_selection.clone())
                .on_change(Message::SingleChanged)
                .on_close(|| Message::CloseSingle)
                .on_today(|_| Message::SingleToday)
                .into()
        };

        let range_picker: Element<'_, Message> =
            date_picker(self.range_open, self.range_selection.clone())
                .on_change(Message::RangeChanged)
                .on_close(|| Message::CloseRange)
                .on_today(|_| Message::RangeToday)
                .into();

        container(column![content, single_picker, range_picker])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn to_12h(hour: u32) -> (u32, bool) {
    let is_pm = hour >= 12;
    (
        match hour % 12 {
            0 => 12,
            h => h,
        },
        is_pm,
    )
}
