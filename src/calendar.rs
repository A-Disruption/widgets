//! An inline calendar for picking one date, a range of dates, or several dates.
//!
//! A [`Calendar`] is an ordinary widget, not an overlay: it takes space in the
//! layout like any other. To dock it under a field or show it as a dialog, put
//! it inside one of the overlay widgets — [`popover`](crate::popover) for a
//! docked picker, [`layer::dialog`](crate::layer) for a modal one — or leave it
//! inline on the page.
//!
//! The application owns the value. The calendar draws whatever it is given and
//! reports picks through the callback passed to its constructor:
//!
//! - [`calendar`] picks one date and reports it.
//! - [`range`] picks a [`DateRange`]. The first click reports a range with no
//!   end yet; the second completes it, in whichever order the days were
//!   clicked. While a range is open, the days up to the cursor are previewed.
//! - [`multiple`] toggles days in and out of a set and reports each toggle.
//!
//! The title of each month opens a month grid, and the title of that opens a
//! year grid. The mouse wheel pages months. Once clicked (or focused through
//! an operation), the calendar takes the keyboard: arrows move a day or a
//! week, Page Up / Page Down move a month (a year with Shift), Home / End jump
//! to the start or end of the week, and Enter or Space picks the focused day.
//!
//! # Example
//! ```no_run
//! use chrono::NaiveDate;
//! use iced::{Element, Widget};
//! use widgets::calendar::{self, DateRange};
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     Picked(NaiveDate),
//!     RangeChanged(DateRange),
//! }
//!
//! struct State {
//!     date: Option<NaiveDate>,
//!     stay: Option<DateRange>,
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     iced::widget::column![
//!         calendar::calendar(state.date, Message::Picked),
//!         calendar::range(state.stay, Message::RangeChanged).months(2),
//!     ]
//!     .spacing(24)
//!     .boxed()
//! }
//! ```

use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
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

use crate::lucide::{self, Icon};

/// Space around the whole calendar.
const PADDING: f32 = 8.0;
/// Height of the row holding the month title and the paging arrows.
const HEADER_HEIGHT: f32 = 40.0;
/// Horizontal gap between months shown side by side.
const MONTH_GAP: f32 = 24.0;
/// Weekday labels are shorter than a day row.
const WEEKDAY_ROW_RATIO: f32 = 0.8;
/// Months and years are offered as a 3 × 4 grid.
const OPTION_COLUMNS: usize = 3;
const OPTION_ROWS: usize = 4;
/// Pixels of trackpad scrolling that page one month.
const PIXELS_PER_PAGE: f32 = 60.0;

// ── Values ───────────────────────────────────────────────────────────────────

/// A range of days, as picked by a [`range`] calendar.
///
/// `end` is `None` between the two clicks that make a range: the start has
/// been picked and the end has not. A complete range always has
/// `start <= end`; a single-day range has `start == end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DateRange {
    /// The first day of the range.
    pub start: NaiveDate,
    /// The last day of the range, once picked.
    pub end: Option<NaiveDate>,
}

impl DateRange {
    /// A complete range between two days, in either order.
    pub fn new(a: NaiveDate, b: NaiveDate) -> Self {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };

        Self {
            start,
            end: Some(end),
        }
    }

    /// A range whose start has been picked and whose end has not.
    pub fn starting(start: NaiveDate) -> Self {
        Self { start, end: None }
    }

    /// Whether both ends have been picked.
    pub fn is_complete(&self) -> bool {
        self.end.is_some()
    }

    /// Whether `date` falls within the range. An open range contains only its
    /// start.
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end.unwrap_or(self.start)
    }

    /// The number of days in the range, counting both ends.
    pub fn days(&self) -> i64 {
        (self.end.unwrap_or(self.start) - self.start).num_days() + 1
    }
}

// ── Constructors ─────────────────────────────────────────────────────────────

/// A calendar that picks one date, reporting it through `on_select`.
pub fn calendar<'a, Message, Theme>(
    value: Option<NaiveDate>,
    on_select: impl Fn(NaiveDate) -> Message + 'a,
) -> Calendar<'a, Message, Theme>
where
    Theme: Catalog,
{
    Calendar::new(Mode::Single {
        value,
        on_select: Box::new(on_select),
    })
}

/// A calendar that picks a [`DateRange`], reporting every change through
/// `on_change`: once when the start is picked, and once when the range is
/// completed.
pub fn range<'a, Message, Theme>(
    value: Option<DateRange>,
    on_change: impl Fn(DateRange) -> Message + 'a,
) -> Calendar<'a, Message, Theme>
where
    Theme: Catalog,
{
    Calendar::new(Mode::Range {
        value,
        on_change: Box::new(on_change),
    })
}

/// A calendar that picks any number of dates. Clicking a day reports it
/// through `on_toggle`; the application adds it to, or removes it from, the
/// set it passes back in.
pub fn multiple<'a, Message, Theme>(
    value: &'a [NaiveDate],
    on_toggle: impl Fn(NaiveDate) -> Message + 'a,
) -> Calendar<'a, Message, Theme>
where
    Theme: Catalog,
{
    Calendar::new(Mode::Multiple {
        value,
        on_toggle: Box::new(on_toggle),
    })
}

// ── Widget ───────────────────────────────────────────────────────────────────

enum Mode<'a, Message> {
    Single {
        value: Option<NaiveDate>,
        on_select: Box<dyn Fn(NaiveDate) -> Message + 'a>,
    },
    Range {
        value: Option<DateRange>,
        on_change: Box<dyn Fn(DateRange) -> Message + 'a>,
    },
    Multiple {
        value: &'a [NaiveDate],
        on_toggle: Box<dyn Fn(NaiveDate) -> Message + 'a>,
    },
}

impl<Message> Mode<'_, Message> {
    /// The day the calendar opens on when nothing else says where to look.
    fn anchor(&self) -> Option<NaiveDate> {
        match self {
            Self::Single { value, .. } => *value,
            Self::Range { value, .. } => value.map(|range| range.start),
            Self::Multiple { value, .. } => value.iter().min().copied(),
        }
    }

    fn is_selected(&self, date: NaiveDate) -> bool {
        match self {
            Self::Single { value, .. } => *value == Some(date),
            Self::Range { value, .. } => {
                value.is_some_and(|range| date == range.start || range.end == Some(date))
            }
            Self::Multiple { value, .. } => value.contains(&date),
        }
    }
}

/// An inline calendar. See the [module documentation](self).
pub struct Calendar<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    mode: Mode<'a, Message>,
    months: usize,
    first_weekday: Weekday,
    min: Option<NaiveDate>,
    max: Option<NaiveDate>,
    disabled: Option<Box<dyn Fn(NaiveDate) -> bool + 'a>>,
    min_days: Option<u32>,
    max_days: Option<u32>,
    show_outside_days: bool,
    cell_size: f32,
    text_size: Option<Pixels>,
    font: Option<Font>,
    today: Option<NaiveDate>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> Calendar<'a, Message, Theme>
where
    Theme: Catalog,
{
    fn new(mode: Mode<'a, Message>) -> Self {
        // The paging arrows are Lucide glyphs.
        lucide::ensure_loaded();

        Self {
            id: None,
            mode,
            months: 1,
            first_weekday: Weekday::Sun,
            min: None,
            max: None,
            disabled: None,
            min_days: None,
            max_days: None,
            show_outside_days: true,
            cell_size: 36.0,
            text_size: None,
            font: None,
            today: None,
            class: Theme::default(),
        }
    }

    /// Sets the [`widget::Id`], so operations can focus the calendar.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Shows `months` months side by side (1 to 12). Two suit a range picker.
    pub fn months(mut self, months: usize) -> Self {
        self.months = months.clamp(1, 12);
        self
    }

    /// Sets the day each week starts on. Sunday by default.
    pub fn first_weekday(mut self, weekday: Weekday) -> Self {
        self.first_weekday = weekday;
        self
    }

    /// Days before `min` cannot be picked.
    pub fn min(mut self, min: NaiveDate) -> Self {
        self.min = Some(min);
        self
    }

    /// Days after `max` cannot be picked.
    pub fn max(mut self, max: NaiveDate) -> Self {
        self.max = Some(max);
        self
    }

    /// Days for which `is_disabled` returns `true` cannot be picked — weekends,
    /// holidays, fully booked days.
    pub fn disabled(mut self, is_disabled: impl Fn(NaiveDate) -> bool + 'a) -> Self {
        self.disabled = Some(Box::new(is_disabled));
        self
    }

    /// For a [`range`] calendar: a completed range must span at least this many
    /// days, counting both ends. Ignored by the other modes.
    pub fn min_days(mut self, days: u32) -> Self {
        self.min_days = Some(days.max(1));
        self
    }

    /// For a [`range`] calendar: a completed range may span at most this many
    /// days, counting both ends. Ignored by the other modes.
    pub fn max_days(mut self, days: u32) -> Self {
        self.max_days = Some(days.max(1));
        self
    }

    /// Whether to draw the days of the previous and next month that fill out
    /// the first and last week. On by default.
    pub fn show_outside_days(mut self, show: bool) -> Self {
        self.show_outside_days = show;
        self
    }

    /// Sets the size of one day cell; the whole calendar scales with it.
    pub fn cell_size(mut self, size: f32) -> Self {
        self.cell_size = size.max(16.0);
        self
    }

    /// Sets the size of the day numbers. The title is a little larger.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the font of the calendar's text.
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Overrides the day marked as today, which is otherwise the local date.
    pub fn today(mut self, today: NaiveDate) -> Self {
        self.today = Some(today);
        self
    }

    /// Sets the style of the calendar.
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the calendar.
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn today_or_local(&self) -> NaiveDate {
        self.today.unwrap_or_else(|| Local::now().date_naive())
    }

    /// Whether `date` can be picked: inside `min`/`max` and not disabled.
    fn is_enabled(&self, date: NaiveDate) -> bool {
        self.min.is_none_or(|min| date >= min)
            && self.max.is_none_or(|max| date <= max)
            && !self.disabled.as_ref().is_some_and(|f| f(date))
    }

    /// The message for picking `date`, if any.
    fn pick(&self, date: NaiveDate) -> Option<Message> {
        if !self.is_enabled(date) {
            return None;
        }

        Some(match &self.mode {
            Mode::Single { on_select, .. } => on_select(date),
            Mode::Range { value, on_change } => {
                on_change(next_range(*value, date, self.min_days, self.max_days))
            }
            Mode::Multiple { on_toggle, .. } => on_toggle(date),
        })
    }

    fn geometry(&self, bounds: Rectangle, view: View) -> Geometry {
        Geometry::new(bounds, self.months, self.cell_size, view)
    }

    /// The size of the day numbers: the configured one, or the renderer's
    /// default capped to what fits a cell.
    fn number_size(&self, renderer: &impl text::Renderer) -> f32 {
        self.text_size
            .map(|size| size.0)
            .unwrap_or_else(|| renderer.text_size().0.min(self.cell_size * 0.42))
    }
}

// ── State ────────────────────────────────────────────────────────────────────

/// Which grid the calendar shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Days,
    Months,
    Years,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hit {
    Previous,
    Next,
    /// The title of the given month panel.
    Title(usize),
    Day(NaiveDate),
    Month(u32),
    Year(i32),
}

struct State {
    view: View,
    /// The first displayed month, as its first day.
    month: NaiveDate,
    /// The application's anchor date when `month` was last chosen for it.
    /// When the anchor changes and is not on screen, the calendar follows it.
    anchor: Option<NaiveDate>,
    hovered: Option<Hit>,
    /// The day the keyboard acts on.
    cursor: NaiveDate,
    is_focused: bool,
    /// Trackpad scrolling not yet turned into a page.
    scroll: f32,
}

impl State {
    fn new(anchor: Option<NaiveDate>, today: NaiveDate) -> Self {
        let day = anchor.unwrap_or(today);

        Self {
            view: View::Days,
            month: first_of_month(day),
            anchor,
            hovered: None,
            cursor: day,
            is_focused: false,
            scroll: 0.0,
        }
    }

    fn page(&mut self, months: i32) {
        self.month = add_months(self.month, months);
    }

    /// Moves the keyboard cursor, paging so it stays on screen.
    fn move_cursor(&mut self, to: NaiveDate, months_shown: usize) {
        self.cursor = to;
        self.reveal(to, months_shown);
    }

    /// Pages the fewest months needed to bring `date` on screen.
    fn reveal(&mut self, date: NaiveDate, months_shown: usize) {
        let target = first_of_month(date);
        let last = add_months(self.month, months_shown as i32 - 1);

        if target < self.month {
            self.month = target;
        } else if target > last {
            self.month = add_months(target, -(months_shown as i32 - 1));
        }
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

impl<Message, Theme> widget::Meta for Calendar<'_, Message, Theme> where Theme: Catalog {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Calendar<'_, Message, Theme>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.mode.anchor(), self.today_or_local()))
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        let anchor = self.mode.anchor();

        // Follow a value the application changed, but leave the user's paging
        // alone while the value stays put.
        if anchor != state.anchor {
            state.anchor = anchor;

            if let Some(day) = anchor {
                state.reveal(day, self.months);
                state.cursor = day;
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.resolve(
            Length::Shrink,
            Length::Shrink,
            Geometry::size(self.months, self.cell_size),
        );
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
        let geometry = self.geometry(bounds, state.view);

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Mouse(mouse::Event::CursorLeft) => {
                let hovered = cursor
                    .position()
                    .and_then(|position| geometry.hit(position, state.month, self.first_weekday));

                if hovered != state.hovered {
                    state.hovered = hovered;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    state.is_focused = false;
                    return;
                };

                state.is_focused = true;
                shell.capture_event();
                shell.request_redraw();

                match geometry.hit(position, state.month, self.first_weekday) {
                    Some(Hit::Previous) => state.page(-page_size(state.view, self.months)),
                    Some(Hit::Next) => state.page(page_size(state.view, self.months)),
                    Some(Hit::Title(_)) => {
                        state.view = match state.view {
                            View::Days => View::Months,
                            View::Months | View::Years => View::Years,
                        };
                    }
                    Some(Hit::Month(month)) => {
                        state.month = NaiveDate::from_ymd_opt(state.month.year(), month, 1)
                            .unwrap_or(state.month);
                        state.view = View::Days;
                    }
                    Some(Hit::Year(year)) => {
                        state.month = NaiveDate::from_ymd_opt(year, state.month.month(), 1)
                            .unwrap_or(state.month);
                        state.view = View::Months;
                    }
                    Some(Hit::Day(date)) => {
                        if let Some(message) = self.pick(date) {
                            state.cursor = date;
                            state.reveal(date, self.months);
                            shell.publish(message);
                        }
                    }
                    None => {}
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let pages = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => {
                        state.scroll = 0.0;
                        -y.signum() as i32
                    }
                    mouse::ScrollDelta::Pixels { y, .. } => {
                        state.scroll -= y;
                        let pages = (state.scroll / PIXELS_PER_PAGE).trunc();
                        state.scroll -= pages * PIXELS_PER_PAGE;
                        pages as i32
                    }
                };

                if pages != 0 {
                    state.page(pages * page_size(state.view, self.months).min(12));
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                if state.is_focused =>
            {
                let action = keyboard_action(key, *modifiers);

                match action {
                    Some(KeyAction::Move(to)) => {
                        let to = to.apply(state.cursor, self.first_weekday);
                        state.move_cursor(to, self.months);
                        state.view = View::Days;
                    }
                    Some(KeyAction::Pick) => {
                        if let Some(message) = self.pick(state.cursor) {
                            shell.publish(message);
                        }
                    }
                    Some(KeyAction::Back) => {
                        if state.view == View::Days {
                            state.is_focused = false;
                        } else {
                            state.view = View::Days;
                        }
                    }
                    None => return,
                }

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
        let Some(position) = cursor.position_over(layout.bounds()) else {
            return mouse::Interaction::None;
        };

        match self.geometry(layout.bounds(), state.view).hit(
            position,
            state.month,
            self.first_weekday,
        ) {
            Some(Hit::Day(date)) if !self.is_enabled(date) => mouse::Interaction::NotAllowed,
            Some(_) => mouse::Interaction::Pointer,
            None => mouse::Interaction::None,
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
        let style = theme.style(&self.class);
        let geometry = self.geometry(bounds, state.view);

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

        let painter = Painter {
            font: self.font.unwrap_or_else(|| renderer.font()),
            size: self.number_size(renderer),
            clip: *viewport,
        };
        let text_color = if style.text_color.a > 0.0 {
            style.text_color
        } else {
            defaults.text_color
        };

        // Paging arrows, on the outer panels only.
        for (hit, rect, icon) in [
            (Hit::Previous, geometry.previous, Icon::ChevronLeft),
            (Hit::Next, geometry.next, Icon::ChevronRight),
        ] {
            if state.hovered == Some(hit) {
                fill_circle(renderer, rect, style.hover_background);
            }

            painter.glyph(renderer, icon, rect, style.arrow_color);
        }

        match state.view {
            View::Days => self.draw_days(state, renderer, &style, text_color, &geometry, &painter),
            View::Months => {
                self.draw_months(state, renderer, &style, text_color, &geometry, &painter)
            }
            View::Years => {
                self.draw_years(state, renderer, &style, text_color, &geometry, &painter)
            }
        }
    }
}

impl<'a, Message, Theme> Calendar<'a, Message, Theme>
where
    Theme: Catalog,
{
    fn draw_days<Renderer>(
        &self,
        state: &State,
        renderer: &mut Renderer,
        style: &Style,
        text_color: Color,
        geometry: &Geometry,
        painter: &Painter,
    ) where
        Renderer: text::Renderer,
    {
        let today = self.today_or_local();
        let hovered_day = match state.hovered {
            Some(Hit::Day(day)) => Some(day),
            _ => None,
        };

        // The band behind a range: the picked one, or the preview from an open
        // range's start to the hovered day.
        let band = match &self.mode {
            Mode::Range {
                value: Some(range @ DateRange { end: Some(_), .. }),
                ..
            } => Some((*range, style.range_background)),
            Mode::Range {
                value: Some(DateRange { start, end: None }),
                ..
            } => hovered_day
                .filter(|day| self.is_enabled(*day))
                .map(|day| (DateRange::new(*start, day), style.range_preview_background)),
            _ => None,
        };

        for (index, panel) in geometry.panels.iter().enumerate() {
            let month = add_months(state.month, index as i32);

            // Title.
            if state.hovered == Some(Hit::Title(index)) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: panel.title,
                        border: Border::default().rounded(panel.title.height / 2.0),
                        ..renderer::Quad::default()
                    },
                    style.hover_background,
                );
            }

            painter.text(
                renderer,
                &month.format("%B %Y").to_string(),
                painter.size * 1.1,
                true,
                panel.title,
                style.header_text_color,
            );

            // Weekday labels.
            for column in 0..7 {
                let weekday = weekday_at(self.first_weekday, column);
                let label = &weekday.to_string()[..2];

                painter.text(
                    renderer,
                    label,
                    painter.size * 0.85,
                    false,
                    panel.weekday(column),
                    style.weekday_text_color,
                );
            }

            // Days.
            for (row, week) in month_grid(month, self.first_weekday).iter().enumerate() {
                for (column, &day) in week.iter().enumerate() {
                    let cell = panel.day(row, column);
                    let is_outside = day.month() != month.month();

                    if is_outside && !self.show_outside_days {
                        continue;
                    }

                    // Outside days belong to the neighbouring panel when one is
                    // shown, so only the panels at the ends draw them.
                    let shown_elsewhere = is_outside
                        && ((day < month && index > 0)
                            || (day > month && index + 1 < geometry.panels.len()));

                    if shown_elsewhere {
                        continue;
                    }

                    if let Some((range, color)) = band
                        && range.contains(day)
                        && range.days() > 1
                    {
                        let rect = band_rect(cell, day, range);

                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: rect,
                                border: Border::default()
                                    .rounded(band_radius(cell, day, range, column)),
                                ..renderer::Quad::default()
                            },
                            color,
                        );
                    }

                    let enabled = self.is_enabled(day);
                    let selected = self.mode.is_selected(day);
                    let disc = disc_rect(cell);

                    if selected {
                        fill_circle(renderer, disc, style.selected_background);
                    } else if enabled && hovered_day == Some(day) {
                        fill_circle(renderer, disc, style.hover_background);
                    }

                    if day == today && !selected {
                        stroke_circle(renderer, disc, style.today_border);
                    }

                    if state.is_focused && state.cursor == day {
                        stroke_circle(renderer, disc.expand(1.5), style.focus_border);
                    }

                    let color = if selected {
                        style.selected_text_color
                    } else if !enabled {
                        style.disabled_text_color
                    } else if is_outside {
                        style.outside_text_color
                    } else if band.is_some_and(|(range, _)| range.contains(day)) {
                        style.range_text_color
                    } else {
                        text_color
                    };

                    painter.text(
                        renderer,
                        &day.day().to_string(),
                        painter.size,
                        false,
                        cell,
                        color,
                    );
                }
            }
        }
    }

    fn draw_months<Renderer>(
        &self,
        state: &State,
        renderer: &mut Renderer,
        style: &Style,
        text_color: Color,
        geometry: &Geometry,
        painter: &Painter,
    ) where
        Renderer: text::Renderer,
    {
        let panel = &geometry.panels[0];

        self.draw_title(
            state,
            renderer,
            style,
            panel,
            painter,
            &state.month.format("%Y").to_string(),
        );

        for month in 1..=12u32 {
            let rect = geometry.option(month as usize - 1);
            let label = NaiveDate::from_ymd_opt(2000, month, 1)
                .map(|date| date.format("%b").to_string())
                .unwrap_or_default();
            let current = month == state.month.month();

            draw_option(
                renderer,
                painter,
                style,
                rect,
                &label,
                current,
                state.hovered == Some(Hit::Month(month)),
                text_color,
            );
        }
    }

    fn draw_years<Renderer>(
        &self,
        state: &State,
        renderer: &mut Renderer,
        style: &Style,
        text_color: Color,
        geometry: &Geometry,
        painter: &Painter,
    ) where
        Renderer: text::Renderer,
    {
        let panel = &geometry.panels[0];
        let first = year_page_start(state.month.year());

        self.draw_title(
            state,
            renderer,
            style,
            panel,
            painter,
            &format!("{} – {}", first, first + 11),
        );

        for offset in 0..12 {
            let year = first + offset;
            let current = year == state.month.year();

            draw_option(
                renderer,
                painter,
                style,
                geometry.option(offset as usize),
                &year.to_string(),
                current,
                state.hovered == Some(Hit::Year(year)),
                text_color,
            );
        }
    }

    fn draw_title<Renderer>(
        &self,
        state: &State,
        renderer: &mut Renderer,
        style: &Style,
        panel: &Panel,
        painter: &Painter,
        title: &str,
    ) where
        Renderer: text::Renderer,
    {
        if state.hovered == Some(Hit::Title(0)) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: panel.title,
                    border: Border::default().rounded(panel.title.height / 2.0),
                    ..renderer::Quad::default()
                },
                style.hover_background,
            );
        }

        painter.text(
            renderer,
            title,
            painter.size * 1.1,
            true,
            panel.title,
            style.header_text_color,
        );
    }
}

// ── Picking rules ────────────────────────────────────────────────────────────

/// The range that results from clicking `date` while the range is `current`.
///
/// With no range, or a complete one, the click starts a new range. With an
/// open range, it completes it — in either direction — unless the result would
/// break the length limits, in which case the click starts over from `date`.
fn next_range(
    current: Option<DateRange>,
    date: NaiveDate,
    min_days: Option<u32>,
    max_days: Option<u32>,
) -> DateRange {
    match current {
        Some(DateRange { start, end: None }) => {
            let range = DateRange::new(start, date);
            let days = range.days();

            let too_short = min_days.is_some_and(|min| days < i64::from(min));
            let too_long = max_days.is_some_and(|max| days > i64::from(max));

            if too_short || too_long {
                DateRange::starting(date)
            } else {
                range
            }
        }
        _ => DateRange::starting(date),
    }
}

/// A keyboard press, decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyAction {
    Move(Step),
    Pick,
    Back,
}

/// Where a keyboard move takes the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Days(i64),
    Months(i32),
    WeekStart,
    WeekEnd,
}

impl Step {
    fn apply(self, from: NaiveDate, first_weekday: Weekday) -> NaiveDate {
        match self {
            Self::Days(days) => from + Duration::days(days),
            Self::Months(months) => add_months_clamped(from, months),
            Self::WeekStart => from - Duration::days(days_into_week(from, first_weekday)),
            Self::WeekEnd => from + Duration::days(6 - days_into_week(from, first_weekday)),
        }
    }
}

fn keyboard_action(key: &keyboard::Key, modifiers: keyboard::Modifiers) -> Option<KeyAction> {
    use keyboard::key::Named;

    let keyboard::Key::Named(named) = key else {
        return None;
    };

    let years = if modifiers.shift() { 12 } else { 1 };

    Some(match named {
        Named::ArrowLeft => KeyAction::Move(Step::Days(-1)),
        Named::ArrowRight => KeyAction::Move(Step::Days(1)),
        Named::ArrowUp => KeyAction::Move(Step::Days(-7)),
        Named::ArrowDown => KeyAction::Move(Step::Days(7)),
        Named::PageUp => KeyAction::Move(Step::Months(-years)),
        Named::PageDown => KeyAction::Move(Step::Months(years)),
        Named::Home => KeyAction::Move(Step::WeekStart),
        Named::End => KeyAction::Move(Step::WeekEnd),
        Named::Enter | Named::Space => KeyAction::Pick,
        Named::Escape => KeyAction::Back,
        _ => return None,
    })
}

// ── Date arithmetic ──────────────────────────────────────────────────────────

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("every month has a first day")
}

/// The first day of the month `months` after the one `month` is in.
fn add_months(month: NaiveDate, months: i32) -> NaiveDate {
    let index = month.year() * 12 + month.month0() as i32 + months;

    NaiveDate::from_ymd_opt(index.div_euclid(12), index.rem_euclid(12) as u32 + 1, 1)
        .unwrap_or(month)
}

/// `date` moved by `months`, keeping its day where the target month has it
/// and the month's last day where it does not (31 January + 1 → 28 February).
fn add_months_clamped(date: NaiveDate, months: i32) -> NaiveDate {
    let month = add_months(date, months);
    let last = add_months(month, 1) - Duration::days(1);

    month.with_day(date.day().min(last.day())).unwrap_or(month)
}

fn days_into_week(date: NaiveDate, first_weekday: Weekday) -> i64 {
    (date.weekday().num_days_from_monday() as i64 - first_weekday.num_days_from_monday() as i64)
        .rem_euclid(7)
}

fn weekday_at(first_weekday: Weekday, column: usize) -> Weekday {
    let mut weekday = first_weekday;

    for _ in 0..column {
        weekday = weekday.succ();
    }

    weekday
}

/// The six weeks shown for `month`: the week holding its first day, and the
/// five after it.
fn month_grid(month: NaiveDate, first_weekday: Weekday) -> [[NaiveDate; 7]; 6] {
    let first = first_of_month(month);
    let start = first - Duration::days(days_into_week(first, first_weekday));

    std::array::from_fn(|row| {
        std::array::from_fn(|column| start + Duration::days((row * 7 + column) as i64))
    })
}

/// The first year of the twelve-year page holding `year`.
fn year_page_start(year: i32) -> i32 {
    year - year.rem_euclid(12)
}

/// How far the paging arrows move in the given view.
fn page_size(view: View, months_shown: usize) -> i32 {
    match view {
        View::Days => months_shown as i32,
        View::Months => 12,
        View::Years => 144,
    }
}

// ── Geometry ─────────────────────────────────────────────────────────────────

/// One month's rectangles.
#[derive(Debug, Clone, Copy)]
struct Panel {
    title: Rectangle,
    /// The weekday labels' row.
    weekdays: Rectangle,
    /// The 6 × 7 day grid.
    days: Rectangle,
    cell: f32,
}

impl Panel {
    fn weekday(&self, column: usize) -> Rectangle {
        Rectangle {
            x: self.weekdays.x + column as f32 * self.cell,
            y: self.weekdays.y,
            width: self.cell,
            height: self.weekdays.height,
        }
    }

    fn day(&self, row: usize, column: usize) -> Rectangle {
        Rectangle {
            x: self.days.x + column as f32 * self.cell,
            y: self.days.y + row as f32 * self.cell,
            width: self.cell,
            height: self.cell,
        }
    }
}

#[derive(Debug, Clone)]
struct Geometry {
    view: View,
    previous: Rectangle,
    next: Rectangle,
    panels: Vec<Panel>,
    /// The area the month and year grids fill.
    options: Rectangle,
}

impl Geometry {
    fn size(months: usize, cell: f32) -> Size {
        let panel_width = cell * 7.0;

        Size::new(
            PADDING * 2.0 + panel_width * months as f32 + MONTH_GAP * (months - 1) as f32,
            PADDING * 2.0 + HEADER_HEIGHT + cell * WEEKDAY_ROW_RATIO + cell * 6.0,
        )
    }

    fn new(bounds: Rectangle, months: usize, cell: f32, view: View) -> Self {
        let panel_width = cell * 7.0;
        let top = bounds.y + PADDING;
        let arrow = HEADER_HEIGHT - 8.0;
        let weekday_height = cell * WEEKDAY_ROW_RATIO;

        // The month and year grids replace every panel with one.
        let shown = if view == View::Days { months } else { 1 };
        let width = if view == View::Days {
            panel_width
        } else {
            bounds.width - PADDING * 2.0
        };

        let panels = (0..shown)
            .map(|index| {
                let x = bounds.x + PADDING + index as f32 * (panel_width + MONTH_GAP);
                let title_width = (width - arrow * 2.0 - 8.0).max(0.0);

                Panel {
                    title: Rectangle {
                        x: x + (width - title_width) / 2.0,
                        y: top + 4.0,
                        width: title_width,
                        height: HEADER_HEIGHT - 8.0,
                    },
                    weekdays: Rectangle {
                        x,
                        y: top + HEADER_HEIGHT,
                        width: panel_width,
                        height: weekday_height,
                    },
                    days: Rectangle {
                        x,
                        y: top + HEADER_HEIGHT + weekday_height,
                        width: panel_width,
                        height: cell * 6.0,
                    },
                    cell,
                }
            })
            .collect();

        let arrow_y = top + (HEADER_HEIGHT - arrow) / 2.0;

        Self {
            view,
            previous: Rectangle::new(
                Point::new(bounds.x + PADDING, arrow_y),
                Size::new(arrow, arrow),
            ),
            next: Rectangle::new(
                Point::new(bounds.x + bounds.width - PADDING - arrow, arrow_y),
                Size::new(arrow, arrow),
            ),
            panels,
            options: Rectangle {
                x: bounds.x + PADDING,
                y: top + HEADER_HEIGHT,
                width: bounds.width - PADDING * 2.0,
                height: bounds.height - PADDING * 2.0 - HEADER_HEIGHT,
            },
        }
    }

    /// The rectangle of the `index`th month or year option, row-major.
    fn option(&self, index: usize) -> Rectangle {
        let width = self.options.width / OPTION_COLUMNS as f32;
        let height = self.options.height / OPTION_ROWS as f32;
        let column = index % OPTION_COLUMNS;
        let row = index / OPTION_COLUMNS;

        Rectangle {
            x: self.options.x + column as f32 * width,
            y: self.options.y + row as f32 * height,
            width,
            height,
        }
        .shrink(4.0)
    }

    fn hit(&self, position: Point, month: NaiveDate, first_weekday: Weekday) -> Option<Hit> {
        if self.previous.contains(position) {
            return Some(Hit::Previous);
        }

        if self.next.contains(position) {
            return Some(Hit::Next);
        }

        for (index, panel) in self.panels.iter().enumerate() {
            if panel.title.contains(position) {
                return Some(Hit::Title(index));
            }
        }

        match self.view {
            View::Days => self.panels.iter().enumerate().find_map(|(index, panel)| {
                if !panel.days.contains(position) {
                    return None;
                }

                let column = ((position.x - panel.days.x) / panel.cell) as usize;
                let row = ((position.y - panel.days.y) / panel.cell) as usize;
                let shown = add_months(month, index as i32);
                let day = month_grid(shown, first_weekday)[row.min(5)][column.min(6)];

                // An outside day belongs to the neighbouring panel when one is
                // shown, and that panel does not draw it here.
                let is_outside = day.month() != shown.month();
                let shown_elsewhere = is_outside
                    && ((day < shown && index > 0)
                        || (day > shown && index + 1 < self.panels.len()));

                (!shown_elsewhere).then_some(Hit::Day(day))
            }),
            View::Months => (0..12)
                .find(|&index| self.option(index).contains(position))
                .map(|index| Hit::Month(index as u32 + 1)),
            View::Years => (0..12)
                .find(|&index| self.option(index).contains(position))
                .map(|index| Hit::Year(year_page_start(month.year()) + index as i32)),
        }
    }
}

/// The circle a day's number sits on.
fn disc_rect(cell: Rectangle) -> Rectangle {
    cell.shrink(cell.width * 0.08)
}

/// The slice of a range band behind `day`: from the centre outwards at the
/// ends of the range, the full cell width in between.
fn band_rect(cell: Rectangle, day: NaiveDate, range: DateRange) -> Rectangle {
    let disc = disc_rect(cell);
    let end = range.end.unwrap_or(range.start);

    let left = if day == range.start {
        cell.center_x()
    } else {
        cell.x
    };
    let right = if day == end {
        cell.center_x()
    } else {
        cell.x + cell.width
    };

    Rectangle {
        x: left,
        y: disc.y,
        width: right - left,
        height: disc.height,
    }
}

/// Rounds the band where it meets the edge of a week, so a range spanning
/// several weeks reads as one shape per row rather than as cut-off strips.
fn band_radius(
    cell: Rectangle,
    day: NaiveDate,
    range: DateRange,
    column: usize,
) -> iced::border::Radius {
    let radius = disc_rect(cell).height / 2.0;
    let end = range.end.unwrap_or(range.start);
    let round_left = column == 0 && day != range.start;
    let round_right = column == 6 && day != end;

    iced::border::Radius {
        top_left: if round_left { radius } else { 0.0 },
        bottom_left: if round_left { radius } else { 0.0 },
        top_right: if round_right { radius } else { 0.0 },
        bottom_right: if round_right { radius } else { 0.0 },
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

struct Painter {
    font: Font,
    size: f32,
    clip: Rectangle,
}

impl Painter {
    fn text<Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        size: f32,
        bold: bool,
        rect: Rectangle,
        color: Color,
    ) where
        Renderer: text::Renderer,
    {
        let font = if bold {
            Font {
                weight: iced::font::Weight::Semibold,
                ..self.font
            }
        } else {
            self.font
        };

        self.fill(renderer, content, font, size, rect, color);
    }

    fn glyph<Renderer>(&self, renderer: &mut Renderer, icon: Icon, rect: Rectangle, color: Color)
    where
        Renderer: text::Renderer,
    {
        let mut buffer = [0u8; 4];

        self.fill(
            renderer,
            icon.character().encode_utf8(&mut buffer),
            lucide::FONT,
            rect.height * 0.55,
            rect,
            color,
        );
    }

    fn fill<Renderer>(
        &self,
        renderer: &mut Renderer,
        content: &str,
        font: Font,
        size: f32,
        rect: Rectangle,
        color: Color,
    ) where
        Renderer: text::Renderer,
    {
        renderer.fill_text(
            iced::advanced::Text {
                content: content.to_string(),
                bounds: rect.size(),
                size: Pixels(size),
                font,
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::default(),
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: None,
            },
            rect.center(),
            color,
            self.clip,
        );
    }
}

fn fill_circle<Renderer: renderer::Renderer>(
    renderer: &mut Renderer,
    rect: Rectangle,
    color: Color,
) {
    if color.a == 0.0 {
        return;
    }

    renderer.fill_quad(
        renderer::Quad {
            bounds: rect,
            border: Border::default().rounded(rect.width.min(rect.height) / 2.0),
            ..renderer::Quad::default()
        },
        color,
    );
}

fn stroke_circle<Renderer: renderer::Renderer>(
    renderer: &mut Renderer,
    rect: Rectangle,
    color: Color,
) {
    if color.a == 0.0 {
        return;
    }

    renderer.fill_quad(
        renderer::Quad {
            bounds: rect,
            border: Border {
                color,
                width: 1.0,
                radius: (rect.width.min(rect.height) / 2.0).into(),
            },
            ..renderer::Quad::default()
        },
        Color::TRANSPARENT,
    );
}

fn draw_option<Renderer>(
    renderer: &mut Renderer,
    painter: &Painter,
    style: &Style,
    rect: Rectangle,
    label: &str,
    current: bool,
    hovered: bool,
    text_color: Color,
) where
    Renderer: text::Renderer,
{
    let background = if current {
        style.selected_background
    } else if hovered {
        style.hover_background
    } else {
        Color::TRANSPARENT
    };

    if background.a > 0.0 {
        renderer.fill_quad(
            renderer::Quad {
                bounds: rect,
                border: Border::default().rounded(rect.height / 2.0),
                ..renderer::Quad::default()
            },
            background,
        );
    }

    painter.text(
        renderer,
        label,
        painter.size,
        false,
        rect,
        if current {
            style.selected_text_color
        } else {
            text_color
        },
    );
}

// ── Style ────────────────────────────────────────────────────────────────────

/// The appearance of a [`Calendar`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The calendar's own background. `None` by default: inside a popover or
    /// dialog the surface around it already has one.
    pub background: Option<Background>,
    /// The border around the calendar.
    pub border: Border,
    /// The day numbers. A fully transparent color inherits the surrounding
    /// text color.
    pub text_color: Color,
    /// The month titles.
    pub header_text_color: Color,
    /// The weekday labels.
    pub weekday_text_color: Color,
    /// Days of the neighbouring months.
    pub outside_text_color: Color,
    /// Days that cannot be picked.
    pub disabled_text_color: Color,
    /// The paging arrows.
    pub arrow_color: Color,
    /// Behind a hovered day, arrow, title or option.
    pub hover_background: Color,
    /// Behind a picked day — both ends of a range.
    pub selected_background: Color,
    /// A picked day's number.
    pub selected_text_color: Color,
    /// The band behind the days inside a picked range.
    pub range_background: Color,
    /// The band previewing an open range up to the hovered day.
    pub range_preview_background: Color,
    /// The numbers of the days inside a range.
    pub range_text_color: Color,
    /// The ring around today.
    pub today_border: Color,
    /// The ring around the day the keyboard acts on.
    pub focus_border: Color,
}

/// A styling function for a [`Calendar`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`Calendar`].
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

/// The default style of a [`Calendar`], from the theme's palette.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();
    let text = palette.background.base.text;
    let primary = palette.primary.base.color;

    Style {
        background: None,
        border: Border::default(),
        text_color: text,
        header_text_color: text,
        weekday_text_color: text.scale_alpha(0.55),
        outside_text_color: text.scale_alpha(0.4),
        disabled_text_color: text.scale_alpha(0.25),
        arrow_color: text.scale_alpha(0.8),
        hover_background: text.scale_alpha(0.1),
        selected_background: primary,
        selected_text_color: palette.primary.base.text,
        range_background: primary.scale_alpha(0.22),
        range_preview_background: primary.scale_alpha(0.12),
        range_text_color: text,
        today_border: primary,
        focus_border: text.scale_alpha(0.7),
    }
}

/// A [`default`] calendar on a card of its own, for use inline on a page.
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

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn a_range_completes_in_either_order() {
        let open = Some(DateRange::starting(date(2026, 10, 20)));

        assert_eq!(
            next_range(open, date(2026, 10, 25), None, None),
            DateRange::new(date(2026, 10, 20), date(2026, 10, 25))
        );
        assert_eq!(
            next_range(open, date(2026, 10, 12), None, None),
            DateRange::new(date(2026, 10, 12), date(2026, 10, 20))
        );
    }

    #[test]
    fn a_click_after_a_complete_range_starts_a_new_one() {
        let done = Some(DateRange::new(date(2026, 1, 1), date(2026, 1, 3)));

        assert_eq!(
            next_range(done, date(2026, 2, 9), None, None),
            DateRange::starting(date(2026, 2, 9))
        );
        assert_eq!(
            next_range(None, date(2026, 2, 9), None, None),
            DateRange::starting(date(2026, 2, 9))
        );
    }

    #[test]
    fn the_same_day_twice_is_a_one_day_range() {
        let open = Some(DateRange::starting(date(2026, 5, 5)));
        let range = next_range(open, date(2026, 5, 5), None, None);

        assert!(range.is_complete());
        assert_eq!(range.days(), 1);
    }

    #[test]
    fn a_range_outside_the_length_limits_restarts_from_the_click() {
        let open = Some(DateRange::starting(date(2026, 3, 1)));

        // Three days, but at least five are needed.
        assert_eq!(
            next_range(open, date(2026, 3, 3), Some(5), None),
            DateRange::starting(date(2026, 3, 3))
        );
        // Eleven days, but at most seven are allowed.
        assert_eq!(
            next_range(open, date(2026, 3, 11), None, Some(7)),
            DateRange::starting(date(2026, 3, 11))
        );
        // Seven days fits both.
        assert_eq!(
            next_range(open, date(2026, 3, 7), Some(5), Some(7)),
            DateRange::new(date(2026, 3, 1), date(2026, 3, 7))
        );
    }

    #[test]
    fn the_grid_starts_on_the_configured_weekday() {
        // 1 October 2026 is a Thursday.
        let sunday = month_grid(date(2026, 10, 1), Weekday::Sun);
        let monday = month_grid(date(2026, 10, 1), Weekday::Mon);

        assert_eq!(sunday[0][0], date(2026, 9, 27));
        assert_eq!(sunday[0][4], date(2026, 10, 1));
        assert_eq!(monday[0][0], date(2026, 9, 28));
        assert_eq!(monday[0][3], date(2026, 10, 1));
        assert_eq!(sunday[5][6], date(2026, 11, 7));
    }

    #[test]
    fn months_add_across_years() {
        assert_eq!(add_months(date(2026, 11, 1), 3), date(2027, 2, 1));
        assert_eq!(add_months(date(2026, 1, 1), -1), date(2025, 12, 1));
        assert_eq!(add_months(date(2026, 1, 15), -25), date(2023, 12, 1));
    }

    #[test]
    fn moving_by_a_month_keeps_the_day_where_it_can() {
        assert_eq!(add_months_clamped(date(2026, 1, 31), 1), date(2026, 2, 28));
        assert_eq!(add_months_clamped(date(2028, 1, 31), 1), date(2028, 2, 29));
        assert_eq!(add_months_clamped(date(2026, 3, 15), -1), date(2026, 2, 15));
    }

    #[test]
    fn home_and_end_stay_within_the_week() {
        // Wednesday 21 October 2026, weeks starting on Sunday.
        let day = date(2026, 10, 21);

        assert_eq!(Step::WeekStart.apply(day, Weekday::Sun), date(2026, 10, 18));
        assert_eq!(Step::WeekEnd.apply(day, Weekday::Sun), date(2026, 10, 24));
        assert_eq!(Step::WeekStart.apply(day, Weekday::Mon), date(2026, 10, 19));
    }

    #[test]
    fn revealing_a_date_pages_the_fewest_months() {
        let mut state = State::new(Some(date(2026, 5, 10)), date(2026, 5, 10));

        // Two months shown: May and June. July needs one page forward.
        state.reveal(date(2026, 7, 2), 2);
        assert_eq!(state.month, date(2026, 6, 1));

        // Already on screen: nothing moves.
        state.reveal(date(2026, 6, 30), 2);
        assert_eq!(state.month, date(2026, 6, 1));

        state.reveal(date(2026, 1, 1), 2);
        assert_eq!(state.month, date(2026, 1, 1));
    }

    #[test]
    fn a_day_hit_lands_on_the_day_drawn_there() {
        let bounds = Rectangle::new(Point::ORIGIN, Geometry::size(1, 36.0));
        let geometry = Geometry::new(bounds, 1, 36.0, View::Days);
        let panel = geometry.panels[0];

        // October 2026, Sunday-first: row 0, column 4 is the 1st.
        let cell = panel.day(0, 4);

        assert_eq!(
            geometry.hit(cell.center(), date(2026, 10, 1), Weekday::Sun),
            Some(Hit::Day(date(2026, 10, 1)))
        );
        assert_eq!(
            geometry.hit(geometry.next.center(), date(2026, 10, 1), Weekday::Sun),
            Some(Hit::Next)
        );
    }

    #[test]
    fn side_by_side_panels_leave_shared_days_to_their_own_month() {
        let bounds = Rectangle::new(Point::ORIGIN, Geometry::size(2, 36.0));
        let geometry = Geometry::new(bounds, 2, 36.0, View::Days);

        // The October panel's last row holds 1–7 November, which the November
        // panel draws; the October panel must not report them.
        let last = geometry.panels[0].day(5, 6);

        assert_eq!(
            geometry.hit(last.center(), date(2026, 10, 1), Weekday::Sun),
            None
        );
    }
}
