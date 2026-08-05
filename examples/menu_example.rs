//! Exercises the `menu` widget: intrinsic uniform row widths, gutters for icons
//! and checkmarks, deep submenu chains, and edge flipping.
//!
//! The menu bar is deliberately spread across the whole window so the rightmost
//! menus have to flip their submenus to the left to stay on screen. Resize the
//! window narrow to watch the root panels flip too.

use iced::widget::{column, container, row, space, text};
use iced::time::Duration;
use iced::{Element, Fill, Theme};

use widgets::anchor::{Align, Side};
use widgets::animation::Motion;
use widgets::menu::{CheckSide, Item, Menu, item, menu, menu_bar, separator, submenu, toggle};

pub fn main() -> iced::Result {
    iced::application(Example::default, Example::update, Example::view)
        .title("menu")
        .theme(theme)
        // The menu draws its checkmark and submenu chevron from Lucide, so the
        // font has to be registered once at startup.
        .font(widgets::lucide::FONT_BYTES)
        .run()
}

fn theme(_state: &Example) -> Theme {
    Theme::Dark
}

#[derive(Debug, Clone)]
enum Message {
    Chose(&'static str),
    Toggled(&'static str, bool),
    ToggleWrap,
    ToggleMinimap,
    SetIndent(u8),
}

struct Example {
    last: Option<&'static str>,
    open: Option<&'static str>,
    word_wrap: bool,
    minimap: bool,
    indent: u8,
}

impl Default for Example {
    fn default() -> Self {
        Self {
            last: None,
            open: None,
            word_wrap: true,
            minimap: false,
            indent: 4,
        }
    }
}

impl Example {
    fn update(&mut self, message: Message) {
        match message {
            Message::Chose(label) => self.last = Some(label),
            Message::Toggled(name, true) => self.open = Some(name),
            Message::Toggled(name, false) => {
                if self.open == Some(name) {
                    self.open = None;
                }
            }
            Message::ToggleWrap => self.word_wrap = !self.word_wrap,
            Message::ToggleMinimap => self.minimap = !self.minimap,
            Message::SetIndent(width) => self.indent = width,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // Grouping the menus into a bar is what makes them hand over to each
        // other: with one open, sliding onto another trigger opens it, and the
        // left and right arrows walk between them.
        let bar = menu_bar(vec![file_menu(), edit_menu(), self.view_menu()]).padding(6);

        // These two stay outside the bar, pushed against the right edge where
        // their submenus have to flip. They still work on their own — they just
        // never hand over.
        let strays = row![deep_menu(), aligned_menu()].spacing(2);

        let status = match self.last {
            Some(label) => text(format!("chose: {label}")),
            None => text("nothing chosen yet"),
        };

        column![
            container(row![bar, space().width(Fill), strays].padding(6))
                .width(Fill)
                .style(container::bordered_box),
            container(status).padding(16),
            space().width(Fill).height(Fill),
        ]
        .into()
    }

    /// Checkmarks in the leading gutter, mixed with plain and icon rows, plus a
    /// trailing-gutter submenu that behaves like a radio group.
    fn view_menu(&self) -> Menu<'_, Message> {
        menu(
            text("View"),
            vec![
                toggle(text("Word Wrap"), self.word_wrap).on_press(Message::ToggleWrap),
                toggle(text("Minimap"), self.minimap).on_press(Message::ToggleMinimap),
                separator(),
                // No check and no icon: its label still lines up with the rows
                // above, because the gutter is sized for the whole panel.
                item(text("Reset Layout")).on_press(Message::Chose("reset layout")),
                submenu(
                    text("Indent Width"),
                    vec![
                        toggle(text("2 spaces"), self.indent == 2).on_press(Message::SetIndent(2)),
                        toggle(text("4 spaces"), self.indent == 4).on_press(Message::SetIndent(4)),
                        toggle(text("8 spaces"), self.indent == 8).on_press(Message::SetIndent(8)),
                    ],
                ),
            ],
        )
        .on_toggle(|open| Message::Toggled("view", open))
    }
}

/// A row whose trailing shortcut is pushed right by a fluid space.
///
/// This is the case the two-pass layout exists for: the row measures at the
/// width of its text, then fills to the width of the widest row in its panel.
fn shortcut_item<'a>(label: &'static str, keys: &'static str) -> Item<'a, Message> {
    item(
        row![text(label), space().width(Fill), text(keys).size(12)]
            .spacing(24)
            .align_y(iced::Alignment::Center),
    )
    .on_press(Message::Chose(label))
}

/// Icons in the leading gutter, alongside rows that have none.
fn file_menu<'a>() -> Menu<'a, Message> {
    menu(
        text("File"),
        vec![
            shortcut_item("New", "Ctrl+N").icon(text("＋")),
            shortcut_item("Open…", "Ctrl+O").icon(text("📂")),
            submenu(
                text("Open Recent"),
                vec![
                    item(text("project-alpha.iced")).on_press(Message::Chose("project-alpha")),
                    item(text("a-much-longer-file-name.iced"))
                        .on_press(Message::Chose("longer-name")),
                    separator(),
                    item(text("Clear")).on_press(Message::Chose("clear recent")),
                ],
            ),
            separator(),
            shortcut_item("Save", "Ctrl+S").icon(text("💾")),
            item(text("Save As…")).enabled(false),
            separator(),
            shortcut_item("Quit", "Ctrl+Q"),
        ],
    )
    .on_toggle(|open| Message::Toggled("file", open))
}

/// Checkmarks in the trailing gutter instead of the leading one.
fn edit_menu<'a>() -> Menu<'a, Message> {
    menu(
        text("Edit"),
        vec![
            shortcut_item("Undo", "Ctrl+Z"),
            shortcut_item("Redo", "Ctrl+Shift+Z"),
            separator(),
            shortcut_item("Cut", "Ctrl+X"),
            shortcut_item("Copy", "Ctrl+C"),
            toggle(text("Overwrite Mode"), true).on_press(Message::Chose("overwrite")),
            submenu(
                text("Paste Special"),
                vec![
                    item(text("Values")).on_press(Message::Chose("paste values")),
                    item(text("Formatting")).on_press(Message::Chose("paste formatting")),
                    item(text("Everything")).on_press(Message::Chose("paste everything")),
                ],
            ),
        ],
    )
    .check_side(CheckSide::Trailing)
    .on_toggle(|open| Message::Toggled("edit", open))
}

/// Four levels deep, anchored at the right of the window: every level after the
/// first should flip to the left rather than cover its parent.
fn deep_menu<'a>() -> Element<'a, Message> {
    menu(
        text("Deep"),
        vec![
            item(text("Level one")).on_press(Message::Chose("level one")),
            submenu(
                text("Level two"),
                vec![
                    item(text("Two: a")).on_press(Message::Chose("two a")),
                    submenu(
                        text("Level three"),
                        vec![
                            item(text("Three: a")).on_press(Message::Chose("three a")),
                            submenu(
                                text("Level four"),
                                vec![
                                    item(text("Four: a")).on_press(Message::Chose("four a")),
                                    item(text("Four: b")).on_press(Message::Chose("four b")),
                                ],
                            ),
                        ],
                    ),
                ],
            ),
        ],
    )
    // A slower, longer travel than the default, to show the motion is
    // configurable — and to make the transition easy to actually see.
    .motion(Motion::SMOOTH.duration(Duration::from_millis(600)).slide(24.0))
    .on_toggle(|open| Message::Toggled("deep", open))
    .into()
}

/// A trigger that paints its own background, so the menu stays out of the way.
/// Also checks that alignment and `min_width` compose with the intrinsic sizing.
///
/// A `button` would work here too, but its `on_press` could never fire: the
/// menu claims a press on its trigger before the trigger element sees it.
fn aligned_menu<'a>() -> Element<'a, Message> {
    menu(
        container(text("Aligned"))
            .padding([4, 10])
            .style(container::bordered_box),
        vec![
            item(text("Short")).on_press(Message::Chose("short")),
            item(text("A considerably longer entry")).on_press(Message::Chose("long")),
            separator(),
            item(text("Tail")).on_press(Message::Chose("tail")),
        ],
    )
    .plain_trigger()
    // Opted out entirely: this menu never schedules an animation frame.
    .no_animation()
    .side(Side::Bottom)
    .align(Align::End)
    .min_width(220.0)
    .on_toggle(|open| Message::Toggled("aligned", open))
    .into()
}
