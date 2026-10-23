//! An inline, searchable list of time zones.
//!
//! A [`TimezonePicker`] is an ordinary widget, not an overlay. Put it in a
//! [`popover`](crate::popover) to dock it under a field — beside a
//! [`time_picker`](crate::time_picker), say — in a
//! [`layer::dialog`](crate::layer) to make it modal, or leave it on the page.
//!
//! Each row shows a zone's city and region, its time right now, and its offset
//! from UTC with the local abbreviation (EDT, CET, IST). Rows are ordered by
//! offset, west to east. The zones come from the IANA database through
//! [`chrono_tz`], so offsets follow daylight saving time.
//!
//! The search row at the top filters the list. Once the picker has been
//! clicked, typing goes into it: every word typed must match somewhere in a
//! zone's city, region, abbreviation or offset, so `new york`, `europe`,
//! `edt`, `+5:30` and `utc-4` all work. Backspace deletes, Escape clears the
//! search (and, with nothing left to clear, lets go of the keyboard), Up and
//! Down move through the list, Page Up and Page Down move a page, and Enter
//! picks. The mouse wheel and the scrollbar scroll.
//!
//! The application owns the value: [`timezone_picker`] reports the clicked or
//! entered zone and highlights whatever it is given.
//!
//! # Example
//! ```no_run
//! use chrono_tz::Tz;
//! use iced::{Element, Widget};
//! use widgets::timezone_picker::timezone_picker;
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     ZonePicked(Tz),
//! }
//!
//! fn view(zone: Option<Tz>) -> Element<'static, Message> {
//!     timezone_picker(zone, Message::ZonePicked).rows(8).boxed()
//! }
//! ```

use chrono::{DateTime, Offset, Utc};
use chrono_tz::{OffsetName, TZ_VARIANTS, Tz};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{self, Operation, Tree, tree};
use iced::advanced::{Shell, Widget};
use iced::alignment::Vertical;
use iced::{
    Background, Border, Color, Event, Font, Length, Pixels, Point, Rectangle, Size, Theme,
    keyboard, mouse,
};

/// Space around the whole picker.
const PADDING: f32 = 8.0;
/// The height of the search row.
const SEARCH_HEIGHT: f32 = 36.0;
/// The gap between the search row and the list.
const GAP: f32 = 8.0;
/// Space inside a row, either side.
const ROW_PADDING: f32 = 10.0;
/// The width of the scrollbar, and of the strip it is grabbed by.
const SCROLLBAR_WIDTH: f32 = 4.0;
const SCROLLBAR_GRAB: f32 = 12.0;
/// The regions the default list is drawn from. Everything else in the IANA
/// database is a legacy alias (`US/Eastern`) or a fixed offset (`Etc/GMT+5`).
const REGIONS: [&str; 10] = [
    "Africa",
    "America",
    "Antarctica",
    "Arctic",
    "Asia",
    "Atlantic",
    "Australia",
    "Europe",
    "Indian",
    "Pacific",
];

/// Old spellings and duplicate names from the IANA `backward` file: renamed
/// cities (`Asia/Calcutta` for `Asia/Kolkata`), two-part names that gained a
/// country (`America/Buenos_Aires`), and locations that only repeat another.
/// Shown, they would list one city twice. Merged zones whose city still
/// exists — `Europe/Amsterdam`, `Europe/Oslo` — are kept: people look for
/// them by name. Generated from the tzdata bundled with `chrono-tz` 0.10.
const ALIASES: [&str; 71] = [
    "Africa/Asmera",
    "Africa/Timbuktu",
    "America/Argentina/ComodRivadavia",
    "America/Atka",
    "America/Buenos_Aires",
    "America/Catamarca",
    "America/Coral_Harbour",
    "America/Cordoba",
    "America/Ensenada",
    "America/Fort_Wayne",
    "America/Godthab",
    "America/Indianapolis",
    "America/Jujuy",
    "America/Knox_IN",
    "America/Louisville",
    "America/Mendoza",
    "America/Montreal",
    "America/Nipigon",
    "America/Pangnirtung",
    "America/Porto_Acre",
    "America/Rainy_River",
    "America/Rosario",
    "America/Santa_Isabel",
    "America/Shiprock",
    "America/Thunder_Bay",
    "America/Virgin",
    "America/Yellowknife",
    "Antarctica/South_Pole",
    "Asia/Ashkhabad",
    "Asia/Calcutta",
    "Asia/Choibalsan",
    "Asia/Chongqing",
    "Asia/Chungking",
    "Asia/Dacca",
    "Asia/Harbin",
    "Asia/Istanbul",
    "Asia/Kashgar",
    "Asia/Katmandu",
    "Asia/Macao",
    "Asia/Rangoon",
    "Asia/Saigon",
    "Asia/Tel_Aviv",
    "Asia/Thimbu",
    "Asia/Ujung_Pandang",
    "Asia/Ulan_Bator",
    "Atlantic/Faeroe",
    "Atlantic/Jan_Mayen",
    "Australia/ACT",
    "Australia/Canberra",
    "Australia/Currie",
    "Australia/LHI",
    "Australia/NSW",
    "Australia/North",
    "Australia/Queensland",
    "Australia/South",
    "Australia/Tasmania",
    "Australia/Victoria",
    "Australia/West",
    "Australia/Yancowinna",
    "Europe/Belfast",
    "Europe/Kiev",
    "Europe/Nicosia",
    "Europe/Tiraspol",
    "Europe/Uzhgorod",
    "Europe/Zaporozhye",
    "Pacific/Enderbury",
    "Pacific/Johnston",
    "Pacific/Ponape",
    "Pacific/Samoa",
    "Pacific/Truk",
    "Pacific/Yap",
];

/// A picker over the IANA time zones, reporting the picked one through
/// `on_select`.
pub fn timezone_picker<'a, Message, Theme>(
    value: Option<Tz>,
    on_select: impl Fn(Tz) -> Message + 'a,
) -> TimezonePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    TimezonePicker {
        id: None,
        value,
        on_select: Box::new(on_select),
        zones: None,
        now: None,
        twenty_four_hour: false,
        placeholder: "Search time zones".to_owned(),
        width: Length::Fixed(340.0),
        rows: 7,
        row_height: 44.0,
        text_size: None,
        font: None,
        class: Theme::default(),
    }
}

/// An inline, searchable list of time zones. See the
/// [module documentation](self).
pub struct TimezonePicker<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    value: Option<Tz>,
    on_select: Box<dyn Fn(Tz) -> Message + 'a>,
    zones: Option<Vec<Tz>>,
    now: Option<DateTime<Utc>>,
    twenty_four_hour: bool,
    placeholder: String,
    width: Length,
    rows: usize,
    row_height: f32,
    text_size: Option<Pixels>,
    font: Option<Font>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> TimezonePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Sets the [`widget::Id`], so operations can focus the picker.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Offers only these zones, instead of every city zone in the database —
    /// a shortlist for a team spread over a few offices, say.
    pub fn zones(mut self, zones: impl IntoIterator<Item = Tz>) -> Self {
        self.zones = Some(zones.into_iter().collect());
        self
    }

    /// Shows each zone's local time on a 24-hour clock.
    pub fn twenty_four_hour(mut self, enabled: bool) -> Self {
        self.twenty_four_hour = enabled;
        self
    }

    /// Overrides the instant offsets and local times are computed for, which
    /// is otherwise now.
    pub fn now(mut self, now: DateTime<Utc>) -> Self {
        self.now = Some(now);
        self
    }

    /// Sets the text shown in the empty search row.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Sets the width of the picker. 340 pixels by default.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// How many rows of the list are visible at once. 7 by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.max(1);
        self
    }

    /// Sets the height of one row of the list.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(28.0);
        self
    }

    /// Sets the size of the city names; the rest of the text scales with it.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the font of the picker's text.
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the style of the picker.
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the picker.
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn now_or_clock(&self) -> DateTime<Utc> {
        self.now.unwrap_or_else(Utc::now)
    }

    fn zone_list(&self) -> Vec<Tz> {
        self.zones.clone().unwrap_or_else(default_zones)
    }

    fn list_height(&self) -> f32 {
        self.rows as f32 * self.row_height
    }

    fn geometry(&self, bounds: Rectangle) -> Geometry {
        let inner = bounds.shrink(PADDING);

        Geometry {
            search: Rectangle {
                height: SEARCH_HEIGHT,
                ..inner
            },
            list: Rectangle {
                y: inner.y + SEARCH_HEIGHT + GAP,
                height: self.list_height(),
                ..inner
            },
        }
    }

    fn name_size(&self) -> f32 {
        self.text_size.map(|size| size.0).unwrap_or(14.0)
    }
}

/// Every zone in [`REGIONS`] that is not one of the [`ALIASES`], plus UTC.
fn default_zones() -> Vec<Tz> {
    TZ_VARIANTS
        .iter()
        .copied()
        .filter(|zone| {
            let name = zone.name();

            name == "UTC"
                || (name
                    .split_once('/')
                    .is_some_and(|(region, _)| REGIONS.contains(&region))
                    && !ALIASES.contains(&name))
        })
        .collect()
}

// ── Entries ──────────────────────────────────────────────────────────────────

/// One row of the list, worked out once per minute rather than per frame.
#[derive(Debug, Clone)]
struct Entry {
    zone: Tz,
    city: String,
    region: String,
    /// Seconds east of UTC, right now.
    offset: i32,
    /// `UTC−04:00 · EDT`.
    detail: String,
    /// The local time, formatted.
    time: String,
    /// Everything searchable, lowercase.
    haystack: String,
}

impl Entry {
    fn new(zone: Tz, now: DateTime<Utc>, twenty_four_hour: bool) -> Self {
        let name = zone.name();
        let (region, city) = match name.rsplit_once('/') {
            Some((region, city)) => (region.replace('/', " / "), city),
            None if name == "UTC" => ("Coordinated Universal Time".to_owned(), name),
            None => (String::new(), name),
        };
        let city = city.replace('_', " ");

        let local = now.with_timezone(&zone);
        let offset = local.offset().fix().local_minus_utc();
        let utc = format_offset(offset);
        // Abbreviations that only repeat the offset (`-03`, or `UTC` for UTC)
        // add nothing.
        let abbreviation = local
            .offset()
            .abbreviation()
            .filter(|abbreviation| {
                abbreviation.chars().any(char::is_alphabetic) && *abbreviation != utc
            })
            .map(str::to_owned);

        let detail = match &abbreviation {
            Some(abbreviation) => format!("{utc} · {abbreviation}"),
            None => utc.clone(),
        };

        let time = if twenty_four_hour {
            local.format("%H:%M").to_string()
        } else {
            local.format("%-I:%M %p").to_string()
        };

        let haystack = format!(
            "{} {} {} {} {}",
            city,
            region,
            abbreviation.unwrap_or_default(),
            searchable_offsets(offset),
            name,
        )
        .to_lowercase();

        Self {
            zone,
            city,
            region,
            offset,
            detail,
            time,
            haystack,
        }
    }

    /// Whether every word of `query` appears somewhere in the entry.
    fn matches(&self, query: &str) -> bool {
        normalize(query)
            .split_whitespace()
            .all(|word| self.haystack.contains(word))
    }
}

/// `UTC−04:00`, with a real minus sign; `UTC` for zero.
fn format_offset(seconds: i32) -> String {
    if seconds == 0 {
        return "UTC".to_owned();
    }

    let sign = if seconds < 0 { '−' } else { '+' };
    let minutes = seconds.abs() / 60;

    format!("UTC{sign}{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The ways someone might type an offset: `utc-4`, `-4`, `-04:00`, `gmt-4`,
/// `+5:30`.
fn searchable_offsets(seconds: i32) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.abs() / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);

    let short = if minutes == 0 {
        format!("{sign}{hours}")
    } else {
        format!("{sign}{hours}:{minutes:02}")
    };
    let long = format!("{sign}{hours:02}:{minutes:02}");

    format!("utc{short} gmt{short} utc{long} {short} {long}")
}

/// Lowercases a query and turns typographic minus signs into hyphens.
fn normalize(query: &str) -> String {
    query.to_lowercase().replace(['−', '–'], "-")
}

/// Every zone of `zones` as an [`Entry`], ordered west to east and then by
/// city.
fn entries(zones: &[Tz], now: DateTime<Utc>, twenty_four_hour: bool) -> Vec<Entry> {
    let mut entries: Vec<Entry> = zones
        .iter()
        .map(|&zone| Entry::new(zone, now, twenty_four_hour))
        .collect();

    entries.sort_by(|a, b| a.offset.cmp(&b.offset).then_with(|| a.city.cmp(&b.city)));
    entries
}

// ── State ────────────────────────────────────────────────────────────────────

struct State {
    entries: Vec<Entry>,
    /// What the entries were built from: the zones, the minute and the clock.
    built_for: Option<(Vec<Tz>, i64, bool)>,
    query: String,
    /// Indices into `entries` of the rows the query lets through.
    visible: Vec<usize>,
    /// How far the list is scrolled, in pixels.
    scroll: f32,
    hovered: Option<usize>,
    /// The visible row the keyboard acts on.
    cursor: usize,
    is_focused: bool,
    /// The offset into the scrollbar thumb at which it was grabbed.
    dragging_scrollbar: Option<f32>,
    /// The value the list was last scrolled to.
    revealed: Option<Tz>,
}

impl State {
    fn filter(&mut self) {
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.matches(&self.query))
            .map(|(index, _)| index)
            .collect();
        self.cursor = 0;
        self.scroll = 0.0;
    }

    fn max_scroll(&self, row_height: f32, list_height: f32) -> f32 {
        (self.visible.len() as f32 * row_height - list_height).max(0.0)
    }

    fn scroll_by(&mut self, pixels: f32, row_height: f32, list_height: f32) {
        self.scroll = (self.scroll + pixels).clamp(0.0, self.max_scroll(row_height, list_height));
    }

    /// Scrolls the least distance that shows visible row `row` whole.
    fn reveal(&mut self, row: usize, row_height: f32, list_height: f32) {
        let top = row as f32 * row_height;

        if top < self.scroll {
            self.scroll = top;
        } else if top + row_height > self.scroll + list_height {
            self.scroll = top + row_height - list_height;
        }

        self.scroll = self
            .scroll
            .clamp(0.0, self.max_scroll(row_height, list_height));
    }

    /// Scrolls so visible row `row` sits in the middle, where it can be.
    fn centre(&mut self, row: usize, row_height: f32, list_height: f32) {
        self.scroll = (row as f32 * row_height - (list_height - row_height) / 2.0)
            .clamp(0.0, self.max_scroll(row_height, list_height));
    }

    fn row_at(&self, list: Rectangle, row_height: f32, position: Point) -> Option<usize> {
        if !list.contains(position) {
            return None;
        }

        let row = ((position.y - list.y + self.scroll) / row_height) as usize;

        (row < self.visible.len()).then_some(row)
    }

    /// The scrollbar's track and thumb, when the list overflows.
    fn scrollbar(&self, list: Rectangle, row_height: f32) -> Option<(Rectangle, Rectangle)> {
        let content = self.visible.len() as f32 * row_height;

        if content <= list.height {
            return None;
        }

        let track = Rectangle {
            x: list.x + list.width - SCROLLBAR_GRAB,
            width: SCROLLBAR_GRAB,
            ..list
        };
        let height = (list.height * list.height / content).max(24.0);
        let travel = list.height - height;
        let max = content - list.height;

        let thumb = Rectangle {
            x: list.x + list.width - SCROLLBAR_WIDTH - 2.0,
            y: list.y + travel * (self.scroll / max),
            width: SCROLLBAR_WIDTH,
            height,
        };

        Some((track, thumb))
    }
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.is_focused
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
    }
}

#[derive(Debug, Clone, Copy)]
struct Geometry {
    search: Rectangle,
    list: Rectangle,
}

impl<Message, Theme> widget::Meta for TimezonePicker<'_, Message, Theme> where Theme: Catalog {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TimezonePicker<'_, Message, Theme>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            entries: Vec::new(),
            built_for: None,
            query: String::new(),
            visible: Vec::new(),
            scroll: 0.0,
            hovered: None,
            cursor: 0,
            is_focused: false,
            dragging_scrollbar: None,
            revealed: None,
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        let zones = self.zone_list();
        let now = self.now_or_clock();
        let key = (zones, now.timestamp() / 60, self.twenty_four_hour);

        // Offsets and local times move with the clock; rebuild once a minute.
        if state.built_for.as_ref() != Some(&key) {
            let scroll = state.scroll;

            state.entries = entries(&key.0, now, self.twenty_four_hour);
            state.built_for = Some(key);

            let cursor = state.cursor;
            state.filter();
            state.cursor = cursor.min(state.visible.len().saturating_sub(1));
            state.scroll = scroll.clamp(0.0, state.max_scroll(self.row_height, self.list_height()));
        }

        // Bring a value the application changed into view, once.
        if self.value != state.revealed {
            state.revealed = self.value;

            if let Some(row) = self.value.and_then(|zone| {
                state
                    .visible
                    .iter()
                    .position(|&index| state.entries[index].zone == zone)
            }) {
                state.cursor = row;
                state.centre(row, self.row_height, self.list_height());
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        let height = PADDING * 2.0 + SEARCH_HEIGHT + GAP + self.list_height();

        tree.size = limits.resolve(self.width, Length::Shrink, Size::new(340.0, height));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        _viewport: &Rectangle,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();

        operation.focusable(self.id.as_ref(), layout.bounds(), state);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let geometry = self.geometry(bounds);
        let row_height = self.row_height;
        let list_height = self.list_height();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if let Some(grab) = state.dragging_scrollbar {
                    if let Some((_, thumb)) = state.scrollbar(geometry.list, row_height) {
                        let travel = geometry.list.height - thumb.height;
                        let fraction =
                            ((position.y - grab - geometry.list.y) / travel).clamp(0.0, 1.0);

                        state.scroll = fraction * state.max_scroll(row_height, list_height);
                        shell.request_redraw();
                    }

                    shell.capture_event();
                    return;
                }

                let hovered = cursor
                    .position()
                    .and_then(|position| state.row_at(geometry.list, row_height, position));

                if hovered != state.hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorLeft) => {
                if state.hovered.take().is_some() {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    if state.is_focused {
                        state.is_focused = false;
                        shell.request_redraw();
                    }
                    return;
                };

                state.is_focused = true;
                shell.capture_event();
                shell.request_redraw();

                if let Some((track, thumb)) = state.scrollbar(geometry.list, row_height)
                    && track.contains(position)
                {
                    // Grab the thumb where it was pressed; a press elsewhere on
                    // the track jumps the thumb's middle to the pointer.
                    let grab = if thumb.contains(Point::new(thumb.center_x(), position.y)) {
                        position.y - thumb.y
                    } else {
                        thumb.height / 2.0
                    };

                    state.dragging_scrollbar = Some(grab);

                    let travel = geometry.list.height - thumb.height;
                    let fraction = ((position.y - grab - geometry.list.y) / travel).clamp(0.0, 1.0);
                    state.scroll = fraction * state.max_scroll(row_height, list_height);
                    return;
                }

                if let Some(row) = state.row_at(geometry.list, row_height, position) {
                    state.cursor = row;
                    shell.publish((self.on_select)(state.entries[state.visible[row]].zone));
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.dragging_scrollbar.take().is_some() {
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(geometry.list) =>
            {
                let pixels = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * row_height,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };

                let before = state.scroll;
                state.scroll_by(pixels, row_height, list_height);

                if state.scroll != before {
                    state.hovered = cursor
                        .position()
                        .and_then(|position| state.row_at(geometry.list, row_height, position));
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) if state.is_focused => {
                use keyboard::key::Named;

                let last = state.visible.len().saturating_sub(1);
                let page = self.rows.saturating_sub(1).max(1);

                match key {
                    keyboard::Key::Named(Named::ArrowDown) => {
                        state.cursor = (state.cursor + 1).min(last)
                    }
                    keyboard::Key::Named(Named::ArrowUp) => {
                        state.cursor = state.cursor.saturating_sub(1)
                    }
                    keyboard::Key::Named(Named::PageDown) => {
                        state.cursor = (state.cursor + page).min(last)
                    }
                    keyboard::Key::Named(Named::PageUp) => {
                        state.cursor = state.cursor.saturating_sub(page)
                    }
                    keyboard::Key::Named(Named::Enter) => {
                        if let Some(&index) = state.visible.get(state.cursor) {
                            shell.publish((self.on_select)(state.entries[index].zone));
                        }
                    }
                    keyboard::Key::Named(Named::Backspace) => {
                        if modifiers.command() {
                            state.query.clear();
                        } else {
                            state.query.pop();
                        }

                        state.filter();
                    }
                    keyboard::Key::Named(Named::Escape) => {
                        if state.query.is_empty() {
                            state.is_focused = false;
                        } else {
                            state.query.clear();
                            state.filter();
                        }
                    }
                    _ => {
                        // Typed text goes into the search, unless a shortcut
                        // modifier says it is meant for something else.
                        let typed = text
                            .as_deref()
                            .filter(|_| !modifiers.command() && !modifiers.alt())
                            .filter(|typed| typed.chars().all(|c| !c.is_control()));

                        let Some(typed) = typed else {
                            return;
                        };

                        state.query.push_str(typed);
                        state.filter();
                    }
                }

                state.reveal(state.cursor, row_height, list_height);
                shell.capture_event();
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let geometry = self.geometry(layout.bounds());

        let Some(position) = cursor.position_over(layout.bounds()) else {
            return mouse::Interaction::None;
        };

        if state.dragging_scrollbar.is_some()
            || state
                .scrollbar(geometry.list, self.row_height)
                .is_some_and(|(track, _)| track.contains(position))
        {
            mouse::Interaction::Grab
        } else if geometry.search.contains(position) {
            mouse::Interaction::Text
        } else if state
            .row_at(geometry.list, self.row_height, position)
            .is_some()
        {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let geometry = self.geometry(bounds);
        let style = theme.style(&self.class);
        let font = self.font.unwrap_or_else(|| renderer.font());
        let size = self.name_size();
        let text_color = if style.text_color.a > 0.0 {
            style.text_color
        } else {
            defaults.text_color
        };
        let painter = Painter {
            font,
            clip: *viewport,
        };

        if style.background.is_some() || style.border.width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    ..renderer::Quad::default()
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        // The search row.
        renderer.fill_quad(
            renderer::Quad {
                bounds: geometry.search,
                border: if state.is_focused {
                    Border {
                        color: style.focus_border,
                        ..style.search_border
                    }
                } else {
                    style.search_border
                },
                ..renderer::Quad::default()
            },
            style.search_background,
        );

        let field = geometry.search.shrink([0.0, ROW_PADDING]);
        let typed_width = if state.query.is_empty() {
            painter.left(
                renderer,
                &self.placeholder,
                size,
                field,
                style.placeholder_color,
            );
            0.0
        } else {
            painter.left(renderer, &state.query, size, field, text_color);
            painter.width::<Renderer>(&state.query, size)
        };

        if state.is_focused {
            let caret = Rectangle {
                x: (field.x + typed_width + 1.0).min(field.x + field.width),
                y: field.y + (field.height - size * 1.3) / 2.0,
                width: 1.5,
                height: size * 1.3,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: caret,
                    ..renderer::Quad::default()
                },
                style.caret_color,
            );
        }

        // The list, cut off at its edges as it scrolls.
        renderer.with_layer(geometry.list, |renderer| {
            if state.visible.is_empty() {
                painter.centered(
                    renderer,
                    "No matching time zones",
                    size,
                    Rectangle {
                        height: self.row_height,
                        ..geometry.list
                    },
                    style.muted_text_color,
                );
                return;
            }

            let first = (state.scroll / self.row_height) as usize;
            let last = ((state.scroll + geometry.list.height) / self.row_height).ceil() as usize;

            for row in first..last.min(state.visible.len()) {
                let entry = &state.entries[state.visible[row]];
                let rect = Rectangle {
                    x: geometry.list.x,
                    y: geometry.list.y + row as f32 * self.row_height - state.scroll,
                    width: geometry.list.width - SCROLLBAR_GRAB,
                    height: self.row_height,
                };
                let selected = self.value == Some(entry.zone);

                let background = if selected {
                    style.selected_background
                } else if state.hovered == Some(row) {
                    style.hover_background
                } else {
                    Color::TRANSPARENT
                };

                let row_border = Border::default().rounded(style.row_radius);

                if background.a > 0.0 {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: rect,
                            border: row_border,
                            ..renderer::Quad::default()
                        },
                        background,
                    );
                }

                if state.is_focused && state.cursor == row {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: rect.shrink(1.0),
                            border: Border {
                                color: style.focus_border,
                                width: 1.0,
                                ..row_border
                            },
                            ..renderer::Quad::default()
                        },
                        Color::TRANSPARENT,
                    );
                }

                let (primary, secondary) = if selected {
                    (style.selected_text_color, style.selected_muted_text_color)
                } else {
                    (text_color, style.muted_text_color)
                };

                let inner = rect.shrink([0.0, ROW_PADDING]);
                let top = Rectangle {
                    height: inner.height / 2.0 + 2.0,
                    y: inner.y + 3.0,
                    ..inner
                };
                let bottom = Rectangle {
                    y: inner.y + inner.height / 2.0 - 1.0,
                    height: inner.height / 2.0,
                    ..inner
                };

                painter.left(renderer, &entry.city, size, top, primary);
                painter.right(renderer, &entry.time, size, top, primary);
                painter.left(renderer, &entry.region, size * 0.8, bottom, secondary);
                painter.right(renderer, &entry.detail, size * 0.8, bottom, secondary);
            }
        });

        if let Some((_, thumb)) = state.scrollbar(geometry.list, self.row_height) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: thumb,
                    border: Border::default().rounded(SCROLLBAR_WIDTH / 2.0),
                    ..renderer::Quad::default()
                },
                style.scrollbar_color,
            );
        }
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

struct Painter {
    font: Font,
    clip: Rectangle,
}

impl Painter {
    /// Draws `content` at the left of `rect`.
    fn left<Renderer: text::Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        size: f32,
        rect: Rectangle,
        color: Color,
    ) {
        self.fill(renderer, content, size, rect, color, text::Alignment::Left);
    }

    /// How wide `content` is when drawn, for placing the caret after it.
    fn width<Renderer: text::Renderer>(&self, content: &str, size: f32) -> f32 {
        use text::Paragraph as _;

        Renderer::Paragraph::with_text(self.text(
            content,
            size,
            Size::INFINITE,
            text::Alignment::Left,
        ))
        .min_width()
    }

    fn text<T>(
        &self,
        content: T,
        size: f32,
        bounds: Size,
        align: text::Alignment,
    ) -> iced::advanced::Text<T> {
        iced::advanced::Text {
            content,
            bounds,
            size: Pixels(size),
            font: self.font,
            align_x: align,
            align_y: Vertical::Center,
            line_height: text::LineHeight::default(),
            shaping: text::Shaping::Advanced,
            wrapping: text::Wrapping::None,
            ellipsis: text::Ellipsis::End,
            hint_factor: None,
        }
    }

    fn right<Renderer: text::Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        size: f32,
        rect: Rectangle,
        color: Color,
    ) {
        self.fill(renderer, content, size, rect, color, text::Alignment::Right);
    }

    fn centered<Renderer: text::Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        size: f32,
        rect: Rectangle,
        color: Color,
    ) {
        self.fill(
            renderer,
            content,
            size,
            rect,
            color,
            text::Alignment::Center,
        );
    }

    fn fill<Renderer: text::Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        size: f32,
        rect: Rectangle,
        color: Color,
        align: text::Alignment,
    ) {
        let x = match align {
            text::Alignment::Right => rect.x + rect.width,
            text::Alignment::Center => rect.center_x(),
            _ => rect.x,
        };

        renderer.fill_text(
            self.text(content.to_owned(), size, rect.size(), align),
            Point::new(x, rect.center_y()),
            color,
            self.clip,
        );
    }
}

// ── Style ────────────────────────────────────────────────────────────────────

/// The appearance of a [`TimezonePicker`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The picker's own background. `None` by default: inside a popover or
    /// dialog the surface around it already has one.
    pub background: Option<Background>,
    /// The border around the picker.
    pub border: Border,
    /// City names, local times and typed text. A fully transparent color
    /// inherits the surrounding text color.
    pub text_color: Color,
    /// Regions, offsets and the "no matches" note.
    pub muted_text_color: Color,
    /// The search row's placeholder.
    pub placeholder_color: Color,
    /// Behind the search row.
    pub search_background: Color,
    /// Around the search row; its color turns to `focus_border` while the
    /// picker has the keyboard.
    pub search_border: Border,
    /// The search row's caret.
    pub caret_color: Color,
    /// Behind a hovered row.
    pub hover_background: Color,
    /// Behind the picked zone.
    pub selected_background: Color,
    /// The picked zone's city and time.
    pub selected_text_color: Color,
    /// The picked zone's region and offset.
    pub selected_muted_text_color: Color,
    /// The corner radius of a row's highlight.
    pub row_radius: f32,
    /// The search row's border while focused, and the ring around the row the
    /// keyboard acts on.
    pub focus_border: Color,
    /// The scrollbar thumb.
    pub scrollbar_color: Color,
}

/// A styling function for a [`TimezonePicker`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`TimezonePicker`].
pub trait Catalog {
    /// The style class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a [`Style`].
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default style of a [`TimezonePicker`], from the theme's palette.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();
    let text = palette.background.base.text;
    let primary = palette.primary.base.color;

    Style {
        background: None,
        border: Border::default(),
        text_color: text,
        muted_text_color: text.scale_alpha(0.55),
        placeholder_color: text.scale_alpha(0.4),
        search_background: text.scale_alpha(0.06),
        search_border: Border {
            color: text.scale_alpha(0.15),
            width: 1.0,
            radius: 8.0.into(),
        },
        caret_color: text,
        hover_background: text.scale_alpha(0.07),
        selected_background: primary.scale_alpha(0.22),
        selected_text_color: text,
        selected_muted_text_color: text.scale_alpha(0.7),
        row_radius: 8.0,
        focus_border: primary,
        scrollbar_color: text.scale_alpha(0.3),
    }
}

/// A [`default`] picker on a card of its own, for use inline on a page.
pub fn card(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            color: palette.background.strong.color,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..default(theme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn summer() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 1, 12, 0, 0).unwrap()
    }

    fn winter() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn the_default_list_has_cities_but_no_legacy_aliases() {
        let zones = default_zones();
        let names: Vec<&str> = zones.iter().map(|zone| zone.name()).collect();

        assert!(names.contains(&"America/New_York"));
        assert!(names.contains(&"Asia/Kolkata"));
        assert!(names.contains(&"UTC"));
        assert!(!names.contains(&"US/Eastern"));
        // Renamed cities appear once, under their current name…
        assert!(!names.contains(&"Asia/Calcutta"));
        assert!(!names.contains(&"Europe/Kiev"));
        assert!(names.contains(&"Europe/Kyiv"));
        // …but merged cities people still look for stay.
        assert!(names.contains(&"Europe/Amsterdam"));
        assert!(!names.iter().any(|name| name.starts_with("Etc/")));
    }

    #[test]
    fn entries_follow_daylight_saving_time() {
        let summer = Entry::new(Tz::America__New_York, summer(), false);
        let winter = Entry::new(Tz::America__New_York, winter(), false);

        assert_eq!(summer.offset, -4 * 3600);
        assert_eq!(summer.detail, "UTC−04:00 · EDT");
        assert_eq!(winter.detail, "UTC−05:00 · EST");
        assert_eq!(summer.city, "New York");
        assert_eq!(summer.region, "America");
        assert_eq!(summer.time, "8:00 AM");
    }

    #[test]
    fn search_matches_cities_regions_abbreviations_and_offsets() {
        let kolkata = Entry::new(Tz::Asia__Kolkata, summer(), true);
        let buenos_aires = Entry::new(Tz::America__Argentina__Buenos_Aires, summer(), true);

        assert!(kolkata.matches("kolk"));
        assert!(kolkata.matches("ASIA kol"));
        assert!(kolkata.matches("+5:30"));
        assert!(kolkata.matches("utc+5:30"));
        assert!(!kolkata.matches("europe"));

        assert!(buenos_aires.matches("buenos aires"));
        assert!(buenos_aires.matches("argentina"));
        assert!(buenos_aires.matches("utc−3"));
        assert!(buenos_aires.matches("gmt-3"));
        assert_eq!(buenos_aires.region, "America / Argentina");
        // Argentina's abbreviation is just its offset, so it is left out.
        assert_eq!(buenos_aires.detail, "UTC−03:00");
    }

    #[test]
    fn entries_run_west_to_east() {
        let list = entries(
            &[
                Tz::Asia__Tokyo,
                Tz::Europe__London,
                Tz::America__Los_Angeles,
                Tz::UTC,
            ],
            winter(),
            false,
        );
        let names: Vec<&str> = list.iter().map(|entry| entry.zone.name()).collect();

        assert_eq!(
            names,
            ["America/Los_Angeles", "Europe/London", "UTC", "Asia/Tokyo"]
        );
        assert_eq!(list[2].detail, "UTC");
    }

    #[test]
    fn revealing_a_row_scrolls_the_least_distance() {
        let mut state = State {
            entries: Vec::new(),
            built_for: None,
            query: String::new(),
            visible: (0..20).collect(),
            scroll: 0.0,
            hovered: None,
            cursor: 0,
            is_focused: false,
            dragging_scrollbar: None,
            revealed: None,
        };

        // Rows of 40 in a list 200 tall: row 7 ends at 320, so the list
        // scrolls to 120 to show it whole.
        state.reveal(7, 40.0, 200.0);
        assert_eq!(state.scroll, 120.0);

        state.reveal(4, 40.0, 200.0);
        assert_eq!(state.scroll, 120.0);

        state.reveal(1, 40.0, 200.0);
        assert_eq!(state.scroll, 40.0);

        state.centre(19, 40.0, 200.0);
        assert_eq!(state.scroll, state.max_scroll(40.0, 200.0));
    }
}
