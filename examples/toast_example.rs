//! Exercises the `toast` widget: stacking, per-toast countdowns, and the
//! reflow that runs when one expires out of the middle of the stack.
//!
//! # What to look at
//!
//! * **Burst** spawns four toasts with deliberately uneven timeouts — 8s, 2s,
//!   6s and 4s. They therefore expire *out of order*, so the second one goes
//!   first and the ones around it have to slide into the gap. That is the case
//!   the whole reflow design exists for, and the one most likely to look wrong.
//! * **Dismiss** on any toast removes it by hand, which should reflow the same
//!   way as an expiry.
//! * The corner buttons move the stack. A stack at the bottom grows upward and
//!   one at the top grows downward, so the reflow runs in both directions.
//! * The bar along the bottom edge of each toast is its countdown.

use iced::time::Duration;
use iced::widget::{button, column, container, row, space, text};
use iced::{Element, Fill, Theme};

use widgets::anchor::Anchor;
use widgets::toast::{Toast, Variant, danger, info, success, toast, toasts, warning};

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("toast")
        .theme(theme)
        // Toasts draw their variant icons from Lucide.
        .font(widgets::lucide::FONT_BYTES)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    Spawn(Variant, u64),
    Burst,
    Dismiss(u64),
    DismissAll,
    Corner(Anchor),
}

/// One notification the application is keeping track of.
///
/// The widget never owns this list — it only ever asks for a toast to be
/// removed, and this is what does the removing.
struct Notice {
    id: u64,
    variant: Variant,
    body: String,
    timeout: u64,
}

struct Example {
    notices: Vec<Notice>,
    next_id: u64,
    corner: Anchor,
}

impl Default for Example {
    fn default() -> Self {
        Self {
            notices: Vec::new(),
            next_id: 1,
            corner: Anchor::BottomRight,
        }
    }
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::Spawn(variant, timeout) => self.spawn(variant, timeout),
            Message::Burst => {
                // Uneven timeouts on purpose: these expire in the order
                // 2s, 4s, 6s, 8s — which is *not* the order they were added,
                // so the stack has to reflow around a gap in the middle.
                self.spawn(Variant::Info, 8);
                self.spawn(Variant::Success, 2);
                self.spawn(Variant::Warning, 6);
                self.spawn(Variant::Danger, 4);
            }
            Message::Dismiss(id) => self.notices.retain(|notice| notice.id != id),
            Message::DismissAll => self.notices.clear(),
            Message::Corner(corner) => self.corner = corner,
        }
    }

    fn spawn(&mut self, variant: Variant, timeout: u64) {
        let id = self.next_id;
        self.next_id += 1;

        self.notices.push(Notice {
            id,
            variant,
            body: format!("Notice #{id} — closes in {timeout}s"),
            timeout,
        });
    }

    fn view(&self) -> Element<'_, Message> {
        let spawn = |label: &'static str, variant: Variant, timeout: u64| {
            button(text(label)).on_press(Message::Spawn(variant, timeout))
        };

        let corner = |label: &'static str, anchor: Anchor| {
            button(text(label))
                .on_press(Message::Corner(anchor))
                .style(if self.corner == anchor {
                    button::primary
                } else {
                    button::secondary
                })
        };

        let page = column![
            text("toast").size(24),
            text(format!("{} showing", self.notices.len())).size(13),
            row![
                spawn("Info 5s", Variant::Info, 5),
                spawn("Success 3s", Variant::Success, 3),
                spawn("Warning 7s", Variant::Warning, 7),
                spawn("Danger 4s", Variant::Danger, 4),
                spawn("No timeout", Variant::Neutral, 0),
            ]
            .spacing(8),
            row![
                button(text("Burst (uneven timeouts)")).on_press(Message::Burst),
                button(text("Dismiss all")).on_press(Message::DismissAll),
            ]
            .spacing(8),
            text("Stack corner").size(13),
            row![
                corner("Top left", Anchor::TopLeft),
                corner("Top right", Anchor::TopRight),
                corner("Bottom left", Anchor::BottomLeft),
                corner("Bottom right", Anchor::BottomRight),
            ]
            .spacing(8),
            space().width(Fill).height(Fill),
        ]
        .spacing(16)
        .padding(24);

        // The toast host reports a zero size, so it can sit at the end of the
        // column without moving anything. A `Stack` would work equally well.
        column![container(page).width(Fill).height(Fill), self.toasts()].into()
    }

    fn toasts(&self) -> Element<'_, Message> {
        let items: Vec<Toast<'_, Message>> = self
            .notices
            .iter()
            .map(|notice| {
                let body = row![
                    text(&notice.body),
                    space().width(Fill),
                    button(text("Dismiss").size(12))
                        .style(button::text)
                        .on_press(Message::Dismiss(notice.id)),
                ]
                .align_y(iced::Alignment::Center);

                let item = match notice.variant {
                    Variant::Info => info(notice.id, body),
                    Variant::Success => success(notice.id, body),
                    Variant::Warning => warning(notice.id, body),
                    Variant::Danger => danger(notice.id, body),
                    Variant::Neutral => toast(notice.id, body),
                }
                .on_close(Message::Dismiss(notice.id));

                // A zero timeout means "stay until dismissed", which also
                // leaves the countdown bar off.
                if notice.timeout == 0 {
                    item
                } else {
                    item.timeout(Duration::from_secs(notice.timeout))
                }
            })
            .collect();

        toasts(items).anchor(self.corner).into()
    }
}
