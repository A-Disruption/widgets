//! Calendar / date-picker overlay widget for iced 0.14.
//!
//! The user controls open/close externally by passing `is_open: bool`.
//! Supports single date and date-range selection with an optional time picker.
//!
//! # Example
//! ```no_run
//! use widgets::date_picker::{date_picker, DateSelection};
//!
//! #[derive(Clone)]
//! enum Message {
//!     DateChanged(DateSelection),
//!     ClosePicker,
//! }
//!
//! let picker_open = true;
//! let selection = DateSelection::single();
//!
//! let _picker = date_picker::<Message, iced::Theme, iced::Renderer>(
//!     picker_open,
//!     selection,
//! )
//!     .on_change(Message::DateChanged)
//!     .on_close(|| Message::ClosePicker)
//!     .show_time();
//! ```

#![allow(clippy::too_many_arguments)]

use chrono::{Datelike, Local, NaiveDate};
use iced::{
    Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Vector,
    advanced::{
        Layout, Overlay, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{self, tree::Tree},
    },
    alignment::Vertical,
    keyboard, mouse, touch,
    widget::text,
};
use std::marker::PhantomData;

// ── Re-exports ────────────────────────────────────────────────────────────────
pub use chrono::NaiveDate as Date;
pub use chrono::NaiveDateTime as DateTime;
pub use chrono::NaiveTime as Time;

// ── Layout constants ─────────────────────────────────────────────────────────
const OVERLAY_WIDTH: f32 = 280.0;
const HEADER_HEIGHT: f32 = 44.0;
const DOW_HEIGHT: f32 = 28.0;
const GRID_HEIGHT: f32 = 180.0; // 6 rows × 30 px
const TIME_HEIGHT: f32 = 48.0;
const FOOTER_HEIGHT: f32 = 40.0;
const INNER_PAD: f32 = 8.0;
const ANCHOR_SIZE: f32 = 1.0;
const HEADER_TITLE_FONT_SIZE: f32 = 14.0;
const HEADER_TITLE_HIT_PADDING_X: f32 = 12.0;
const HEADER_TITLE_HIT_HEIGHT: f32 = 30.0;
const TIME_DRAG_STEP_PX: f32 = 8.0;

// ── Public types ──────────────────────────────────────────────────────────────

/// The user's date selection — either a single date or a date range.
#[derive(Debug, Clone, PartialEq)]
pub enum DateSelection {
    Single(Option<NaiveDate>),
    Range {
        start: Option<NaiveDate>,
        end: Option<NaiveDate>,
    },
}

impl DateSelection {
    /// Creates an empty single-date selection.
    pub fn single() -> Self {
        Self::Single(None)
    }

    /// Creates a single-date selection with the given date.
    pub fn with_date(date: NaiveDate) -> Self {
        Self::Single(Some(date))
    }

    /// Creates an empty range selection.
    pub fn range() -> Self {
        Self::Range {
            start: None,
            end: None,
        }
    }

    /// Creates a range selection with start and end dates (auto-sorted).
    pub fn with_range(a: NaiveDate, b: NaiveDate) -> Self {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        Self::Range {
            start: Some(start),
            end: Some(end),
        }
    }

    /// Returns the selected date (or range start) if any.
    pub fn date(&self) -> Option<NaiveDate> {
        match self {
            Self::Single(d) => *d,
            Self::Range { start, .. } => *start,
        }
    }

    /// Returns `true` when this is a range selection.
    pub fn is_range(&self) -> bool {
        matches!(self, Self::Range { .. })
    }

    /// Returns `true` when nothing is selected.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Single(d) => d.is_none(),
            Self::Range { start, end } => start.is_none() && end.is_none(),
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Single(d) => *d = None,
            Self::Range { start, end } => {
                *start = None;
                *end = None;
            }
        }
    }
}

/// Time selection stored as 24 h values, displayed as 12 h AM/PM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeSelection {
    /// Hour in the range 0..=23.
    pub hour: u32,
    /// Minute in the range 0..=59.
    pub minute: u32,
}

impl Default for TimeSelection {
    fn default() -> Self {
        Self { hour: 0, minute: 0 }
    }
}

impl TimeSelection {
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }
}

// ── Constructor ───────────────────────────────────────────────────────────────

/// Creates a [`DatePicker`] overlay widget.
///
/// - `is_open`: the caller decides when the overlay is visible.
/// - `selection`: the initial selected date or range.
pub fn date_picker<'a, Message, Theme, Renderer>(
    is_open: bool,
    selection: DateSelection,
) -> DatePicker<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    DatePicker::new(is_open, selection)
}

// ── Widget struct ─────────────────────────────────────────────────────────────

/// A tiny invisible anchor widget that shows a calendar overlay when `is_open`.
pub struct DatePicker<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    is_open: bool,
    selection: DateSelection,
    time: TimeSelection,
    show_time: bool,
    position: Option<Point>,
    on_change: Option<Box<dyn Fn(DateSelection) -> Message + 'a>>,
    on_change_with_time: Option<Box<dyn Fn(DateSelection, TimeSelection) -> Message + 'a>>,
    on_close: Option<Box<dyn Fn() -> Message + 'a>>,
    on_today: Option<Box<dyn Fn(NaiveDate) -> Message + 'a>>,
    class: Theme::Class<'a>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, Theme, Renderer> DatePicker<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    pub fn new(is_open: bool, selection: DateSelection) -> Self {
        Self {
            is_open,
            selection,
            time: TimeSelection::default(),
            show_time: false,
            position: None,
            on_change: None,
            on_change_with_time: None,
            on_close: None,
            on_today: None,
            class: Theme::default(),
            _renderer: PhantomData,
        }
    }

    /// Sets the initial time when `show_time()` is enabled.
    pub fn initial_time(mut self, time: TimeSelection) -> Self {
        self.time = time;
        self
    }

    /// Callback fired when the user selects a date (or completes a range).
    pub fn on_change(mut self, f: impl Fn(DateSelection) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Like [`on_change`] but also receives the current time selection.
    pub fn on_change_with_time(
        mut self,
        f: impl Fn(DateSelection, TimeSelection) -> Message + 'a,
    ) -> Self {
        self.on_change_with_time = Some(Box::new(f));
        self
    }

    /// Callback fired when the user clicks the **Close** button.
    pub fn on_close(mut self, f: impl Fn() -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    /// Callback fired when the user clicks the **Today** button after selecting today's date.
    pub fn on_today(mut self, f: impl Fn(NaiveDate) -> Message + 'a) -> Self {
        self.on_today = Some(Box::new(f));
        self
    }

    /// Adds an AM/PM + hour/minute row below the calendar grid.
    pub fn show_time(mut self) -> Self {
        self.show_time = true;
        self
    }

    /// Overrides the overlay's top-left position (default: viewport-centered).
    pub fn position(mut self, position: Point) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets the style via a closure.
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = Theme::Class::from(Box::new(style) as StyleFn<'a, Theme>);
        self
    }

    /// Sets the style via a class token.
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct State {
    /// Month currently shown (1..=12).
    view_month: u32,
    /// Year currently shown.
    view_year: i32,
    /// Active picker view.
    view_mode: PickerView,
    /// Day cell under the cursor (for hover highlight and range preview).
    hovered_date: Option<NaiveDate>,
    /// First click in range mode — the uncommitted start.
    range_anchor: Option<NaiveDate>,
    /// Current time value.
    time: TimeSelection,
    /// Focused time field for typed entry.
    time_focus: Option<TimeField>,
    /// Digits typed into the focused time field.
    time_input: String,
    /// Time field currently pressed and waiting to become a drag.
    time_press: Option<TimeField>,
    /// Which time spinner is being dragged.
    time_drag: Option<TimeField>,
    time_drag_start_y: f32,
    time_drag_start_value: u32,
    /// Overlay window drag.
    is_dragging: bool,
    drag_offset: Vector,
    /// Overlay top-left in viewport coordinates.
    overlay_position: Point,
    /// Last known viewport size (updated on window resize).
    viewport_size: Size,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeField {
    Hour,
    Minute,
}

impl TimeField {
    fn next(self) -> Self {
        match self {
            Self::Hour => Self::Minute,
            Self::Minute => Self::Hour,
        }
    }

    fn previous(self) -> Self {
        self.next()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PickerView {
    Days,
    Months,
    Years,
}

impl PickerView {
    fn next(self) -> Self {
        match self {
            Self::Days => Self::Months,
            Self::Months => Self::Years,
            Self::Years => Self::Days,
        }
    }
}

impl State {
    fn new(selection: &DateSelection, initial_time: TimeSelection) -> Self {
        let today = Local::now().date_naive();
        let nav = selection.date().unwrap_or(today);
        Self {
            view_month: nav.month(),
            view_year: nav.year(),
            view_mode: PickerView::Days,
            hovered_date: None,
            range_anchor: None,
            time: initial_time,
            time_focus: None,
            time_input: String::new(),
            time_press: None,
            time_drag: None,
            time_drag_start_y: 0.0,
            time_drag_start_value: 0,
            is_dragging: false,
            drag_offset: Vector::ZERO,
            overlay_position: Point::ORIGIN,
            viewport_size: Size::new(1920.0, 1080.0),
        }
    }
}

// ── Widget trait ──────────────────────────────────────────────────────────────

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DatePicker<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(&self.selection, self.time))
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        // Sync view to newly-passed selection when the widget is updated externally
        // (e.g. the parent sets a new date). Only when overlay is closed to avoid
        // disrupting in-progress navigation.
        if state.overlay_position == Point::ORIGIN {
            let today = Local::now().date_naive();
            let nav = self.selection.date().unwrap_or(today);
            state.view_month = nav.month();
            state.view_year = nav.year();
            state.view_mode = PickerView::Days;
            state.time = self.time;
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(ANCHOR_SIZE), Length::Fixed(ANCHOR_SIZE))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        Node::new(Size::new(ANCHOR_SIZE, ANCHOR_SIZE))
    }

    fn draw(
        &self,
        _state: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        // Invisible anchor — nothing drawn here.
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let s = state.state.downcast_mut::<State>();
        if let Event::Window(iced::window::Event::Opened { size, .. })
        | Event::Window(iced::window::Event::Resized(size)) = event
        {
            s.viewport_size = Size::new(size.width, size.height);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let s = state.state.downcast_mut::<State>();

        if !self.is_open {
            // Reset so the next open re-centers.
            s.overlay_position = Point::ORIGIN;
            s.range_anchor = None;
            s.time_focus = None;
            s.time_input.clear();
            s.time_press = None;
            s.time_drag = None;
            return None;
        }

        // First frame after opening: initialise position and view.
        if s.overlay_position == Point::ORIGIN {
            let today = Local::now().date_naive();
            let nav = self.selection.date().unwrap_or(today);
            s.view_month = nav.month();
            s.view_year = nav.year();
            s.view_mode = PickerView::Days;
            s.time = self.time;
            s.time_focus = None;
            s.time_input.clear();
            s.time_press = None;
            s.time_drag = None;

            let ov = overlay_size(self.show_time);
            s.overlay_position = self.position.unwrap_or_else(|| {
                Point::new(
                    ((viewport.width - ov.width) / 2.0).max(0.0),
                    ((viewport.height - ov.height) / 2.0).max(0.0),
                )
            });
        }

        let viewport_size = s.viewport_size;
        let show_time = self.show_time;

        Some(
            DatePickerOverlay {
                state: s,
                selection: &mut self.selection,
                on_change: &self.on_change,
                on_change_with_time: &self.on_change_with_time,
                on_close: &self.on_close,
                on_today: &self.on_today,
                show_time,
                class: &self.class,
                viewport_size,
                _renderer: PhantomData,
            }
            .into_overlay(),
        )
    }
}

impl<'a, Message, Theme, Renderer> From<DatePicker<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    fn from(picker: DatePicker<'a, Message, Theme, Renderer>) -> Self {
        Self::new(picker)
    }
}

// ── Overlay struct ────────────────────────────────────────────────────────────
//
// 'r = borrow/reference lifetime (the overlay element's lifetime)
// 'w = widget data lifetime (the 'a from DatePicker<'w, ...>)

struct DatePickerOverlay<'r, 'w, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    state: &'r mut State,
    selection: &'r mut DateSelection,
    on_change: &'r Option<Box<dyn Fn(DateSelection) -> Message + 'w>>,
    on_change_with_time: &'r Option<Box<dyn Fn(DateSelection, TimeSelection) -> Message + 'w>>,
    on_close: &'r Option<Box<dyn Fn() -> Message + 'w>>,
    on_today: &'r Option<Box<dyn Fn(NaiveDate) -> Message + 'w>>,
    show_time: bool,
    class: &'r Theme::Class<'w>,
    #[allow(dead_code)]
    viewport_size: Size,
    _renderer: PhantomData<Renderer>,
}

impl<'r, 'w, Message, Theme, Renderer> DatePickerOverlay<'r, 'w, Message, Theme, Renderer>
where
    Message: Clone + 'r,
    Theme: Catalog + 'r,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'r,
{
    fn into_overlay(self) -> overlay::Element<'r, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    fn publish_change(&self, shell: &mut Shell<'_, Message>) {
        if let Some(cb) = self.on_change_with_time {
            shell.publish(cb(self.selection.clone(), self.state.time));
        } else if let Some(cb) = self.on_change {
            shell.publish(cb(self.selection.clone()));
        }
    }

    fn navigate(&mut self, delta: i32) {
        match self.state.view_mode {
            PickerView::Days => {
                let (year, month) = add_months(self.state.view_year, self.state.view_month, delta);
                self.state.view_year = year;
                self.state.view_month = month;
            }
            PickerView::Months => {
                self.state.view_year += delta;
            }
            PickerView::Years => {
                self.state.view_year += delta * 12;
            }
        }
    }

    fn select_today(&mut self, shell: &mut Shell<'_, Message>) {
        let today = Local::now().date_naive();

        self.state.view_month = today.month();
        self.state.view_year = today.year();
        self.state.view_mode = PickerView::Days;
        self.state.hovered_date = Some(today);
        self.state.range_anchor = None;

        match self.selection {
            DateSelection::Single(selection) => {
                *selection = Some(today);
            }
            DateSelection::Range { start, end } => {
                *start = Some(today);
                *end = Some(today);
            }
        }

        self.publish_change(shell);

        if let Some(cb) = self.on_today {
            shell.publish(cb(today));
        }

        shell.request_redraw();
    }

    fn handle_day_click(&mut self, date: NaiveDate, shell: &mut Shell<'_, Message>) {
        match self.selection {
            DateSelection::Single(sel) => {
                *sel = Some(date);
                self.publish_change(shell);
            }
            DateSelection::Range { start, end } => {
                if self.state.range_anchor.is_none() {
                    // First click: set anchor
                    self.state.range_anchor = Some(date);
                    *start = Some(date);
                    *end = None;
                } else {
                    // Second click: commit range
                    let anchor = self.state.range_anchor.take().unwrap();
                    let (s, e) = if date <= anchor {
                        (date, anchor)
                    } else {
                        (anchor, date)
                    };
                    *start = Some(s);
                    *end = Some(e);
                    self.publish_change(shell);
                }
            }
        }
        shell.request_redraw();
    }

    /// Returns the effective (start, end) for rendering.
    ///
    /// When the user has clicked the first range point but not yet committed,
    /// the preview uses the anchor + hovered date.
    fn range_preview(&self) -> (Option<NaiveDate>, Option<NaiveDate>) {
        match &self.selection {
            DateSelection::Range { start, end } => {
                if end.is_none() {
                    // Show uncommitted preview
                    if let (Some(anchor), Some(hover)) =
                        (self.state.range_anchor, self.state.hovered_date)
                    {
                        let (s, e) = if hover <= anchor {
                            (hover, anchor)
                        } else {
                            (anchor, hover)
                        };
                        return (Some(s), Some(e));
                    }
                }
                (*start, *end)
            }
            DateSelection::Single(_) => (None, None),
        }
    }

    fn handle_month_click(&mut self, month: u32) {
        self.state.view_month = month;
        self.state.view_mode = PickerView::Days;
        self.state.hovered_date = None;
    }

    fn handle_year_click(&mut self, year: i32) {
        self.state.view_year = year;
        self.state.view_mode = PickerView::Months;
        self.state.hovered_date = None;
    }

    fn focus_time_field(&mut self, field: Option<TimeField>) {
        self.state.time_focus = field;
        self.state.time_input.clear();
    }

    fn clear_time_interaction(&mut self, preserve_focus: bool) {
        if !preserve_focus {
            self.focus_time_field(None);
        } else {
            self.state.time_input.clear();
        }
        self.state.time_press = None;
        self.state.time_drag = None;
    }

    fn step_time_field(&mut self, field: TimeField, steps: i32, shell: &mut Shell<'_, Message>) {
        if steps == 0 {
            return;
        }

        self.focus_time_field(Some(field));

        match field {
            TimeField::Hour => {
                self.state.time.hour = (self.state.time.hour as i32 + steps).rem_euclid(24) as u32;
            }
            TimeField::Minute => {
                self.state.time.minute =
                    (self.state.time.minute as i32 + steps).rem_euclid(60) as u32;
            }
        }

        self.publish_change(shell);
        shell.request_redraw();
    }

    fn apply_time_field_value(&mut self, field: TimeField, value: u32) -> bool {
        match field {
            TimeField::Hour => {
                if !(1..=12).contains(&value) {
                    return false;
                }

                let is_pm = self.state.time.hour >= 12;
                let new_hour = if value == 12 {
                    if is_pm { 12 } else { 0 }
                } else if is_pm {
                    value + 12
                } else {
                    value
                };

                if self.state.time.hour != new_hour {
                    self.state.time.hour = new_hour;
                    return true;
                }
            }
            TimeField::Minute => {
                if value > 59 {
                    return false;
                }

                if self.state.time.minute != value {
                    self.state.time.minute = value;
                    return true;
                }
            }
        }

        false
    }

    fn parse_time_input(field: TimeField, digits: &str) -> Option<u32> {
        if digits.is_empty() {
            return None;
        }

        let value = digits.parse::<u32>().ok()?;

        match field {
            TimeField::Hour => {
                if digits.len() == 1 && value == 0 {
                    None
                } else if (1..=12).contains(&value) {
                    Some(value)
                } else {
                    None
                }
            }
            TimeField::Minute => (value <= 59).then_some(value),
        }
    }

    fn push_time_digit(&mut self, field: TimeField, digit: char, shell: &mut Shell<'_, Message>) {
        if self.state.time_focus != Some(field) {
            self.focus_time_field(Some(field));
        }

        let mut digits = if self.state.time_input.len() >= 2 {
            String::new()
        } else {
            self.state.time_input.clone()
        };
        digits.push(digit);

        if Self::parse_time_input(field, &digits).is_none() && digits.len() == 2 {
            digits = digit.to_string();
        }

        let changed = Self::parse_time_input(field, &digits)
            .is_some_and(|value| self.apply_time_field_value(field, value));

        self.state.time_input = digits;

        if changed {
            self.publish_change(shell);
        }

        shell.request_redraw();
    }

    fn backspace_time_input(&mut self, field: TimeField, shell: &mut Shell<'_, Message>) {
        if self.state.time_focus != Some(field) {
            return;
        }

        if self.state.time_input.pop().is_none() {
            return;
        }

        let changed = Self::parse_time_input(field, &self.state.time_input)
            .is_some_and(|value| self.apply_time_field_value(field, value));

        if changed {
            self.publish_change(shell);
        }

        shell.request_redraw();
    }

    fn time_field_text(&self, field: TimeField) -> String {
        if self.state.time_focus == Some(field) && !self.state.time_input.is_empty() {
            return format!("{:0>2}", self.state.time_input);
        }

        match field {
            TimeField::Hour => format!("{:02}", to_12h(self.state.time.hour).0),
            TimeField::Minute => format!("{:02}", self.state.time.minute),
        }
    }
}

// ── Overlay trait ─────────────────────────────────────────────────────────────

impl<'r, Message, Theme, Renderer> Overlay<Message, Theme, Renderer>
    for DatePickerOverlay<'r, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'r,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'r,
{
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> Node {
        self.state.viewport_size = bounds;
        let size = overlay_size(self.show_time);
        Node::new(size).move_to(self.state.overlay_position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let style = theme.style(self.class, Status::Active);
        let bounds = layout.bounds();
        let header = header_rect(bounds);

        // Panel background + shadow
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                shadow: style.shadow,
                snap: true,
            },
            style.background,
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: header,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: iced::border::Radius {
                        top_left: 10.0,
                        top_right: 10.0,
                        bottom_right: 0.0,
                        bottom_left: 0.0,
                    },
                },
                shadow: Shadow::default(),
                snap: true,
            },
            style.header_background,
        );

        let dow = dow_header_rect(bounds);
        let grid = grid_rect(bounds);
        let footer = footer_rect(bounds, self.show_time);

        self.draw_header(renderer, &style, header, cursor);
        self.draw_dow_header(renderer, &style, dow);
        self.draw_grid(renderer, &style, grid, cursor);

        if self.show_time {
            let tr = time_rect(bounds);
            self.draw_time_row(renderer, &style, tr, cursor);
        }

        self.draw_footer(renderer, &style, footer, cursor);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();

        match event {
            // ── Left click ───────────────────────────────────────────────
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let cursor_pos = cursor.position().unwrap_or(Point::ORIGIN);
                let header = header_rect(bounds);
                let grid = grid_rect(bounds);
                let footer = footer_rect(bounds, self.show_time);

                // Header area: arrows or start-drag
                if cursor.is_over(header) {
                    self.clear_time_interaction(false);
                    let prev = prev_arrow_rect(header);
                    let next = next_arrow_rect(header);
                    let title = header_title_hit_rect(
                        header,
                        self.state.view_mode,
                        self.state.view_year,
                        self.state.view_month,
                    );
                    if cursor.is_over(prev) {
                        self.navigate(-1);
                        shell.request_redraw();
                        return;
                    }
                    if cursor.is_over(next) {
                        self.navigate(1);
                        shell.request_redraw();
                        return;
                    }
                    if cursor.is_over(title) {
                        self.state.view_mode = self.state.view_mode.next();
                        self.state.hovered_date = None;
                        shell.request_redraw();
                        return;
                    }
                    // Begin overlay drag
                    self.state.is_dragging = true;
                    self.state.drag_offset =
                        Vector::new(cursor_pos.x - bounds.x, cursor_pos.y - bounds.y);
                    return;
                }

                // Grid cell click
                if cursor.is_over(grid) {
                    self.clear_time_interaction(false);
                    match self.state.view_mode {
                        PickerView::Days => {
                            if let Some(date) = hit_test_grid(
                                grid,
                                cursor_pos,
                                self.state.view_year,
                                self.state.view_month,
                            ) {
                                self.handle_day_click(date, shell);
                                return;
                            }
                        }
                        PickerView::Months => {
                            if let Some(month) = hit_test_month_grid(grid, cursor_pos) {
                                self.handle_month_click(month);
                                shell.request_redraw();
                                return;
                            }
                        }
                        PickerView::Years => {
                            if let Some(year) =
                                hit_test_year_grid(grid, cursor_pos, self.state.view_year)
                            {
                                self.handle_year_click(year);
                                shell.request_redraw();
                                return;
                            }
                        }
                    }
                }

                // Time row
                if self.show_time {
                    let tr = time_rect(bounds);
                    if cursor.is_over(tr) {
                        self.handle_time_click(tr, cursor_pos, shell);
                        return;
                    }
                }

                // Footer buttons
                self.clear_time_interaction(false);
                let [today_btn, clear_btn, close_btn] = footer_button_rects(footer);
                if cursor.is_over(today_btn) {
                    self.select_today(shell);
                    return;
                }
                if cursor.is_over(clear_btn) {
                    self.selection.clear();
                    self.state.range_anchor = None;
                    self.publish_change(shell);
                    shell.request_redraw();
                    return;
                }
                if cursor.is_over(close_btn) {
                    if let Some(cb) = self.on_close {
                        shell.publish(cb());
                    }
                    return;
                }
            }

            // ── Cursor moved ─────────────────────────────────────────────
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let pos = *position;

                // Overlay drag
                if self.state.is_dragging {
                    let new_pos = Point::new(
                        pos.x - self.state.drag_offset.x,
                        pos.y - self.state.drag_offset.y,
                    );
                    let ov = overlay_size(self.show_time);
                    self.state.overlay_position = Point::new(
                        new_pos
                            .x
                            .max(0.0)
                            .min(self.state.viewport_size.width - ov.width),
                        new_pos
                            .y
                            .max(0.0)
                            .min(self.state.viewport_size.height - ov.height),
                    );
                    shell.invalidate_layout();
                    shell.request_redraw();
                    return;
                }

                if let Some(field) = self.state.time_press {
                    let delta = self.state.time_drag_start_y - pos.y;

                    if delta.abs() >= TIME_DRAG_STEP_PX {
                        self.state.time_drag = Some(field);
                        self.state.time_press = None;
                    }
                }

                // Time spinner drag
                if let Some(target) = self.state.time_drag {
                    let delta = self.state.time_drag_start_y - pos.y;
                    let steps = (delta / TIME_DRAG_STEP_PX) as i32;
                    match target {
                        TimeField::Hour => {
                            self.state.time.hour = (self.state.time_drag_start_value as i32 + steps)
                                .rem_euclid(24)
                                as u32;
                        }
                        TimeField::Minute => {
                            self.state.time.minute =
                                (self.state.time_drag_start_value as i32 + steps).rem_euclid(60)
                                    as u32;
                        }
                    }
                    shell.request_redraw();
                    return;
                }

                // Hover highlight
                let grid = grid_rect(bounds);
                let new_hover = if self.state.view_mode == PickerView::Days && bounds.contains(pos)
                {
                    hit_test_grid(grid, pos, self.state.view_year, self.state.view_month)
                } else {
                    None
                };
                if new_hover != self.state.hovered_date {
                    self.state.hovered_date = new_hover;
                    shell.request_redraw();
                }
            }

            // ── Button released ──────────────────────────────────────────
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                self.state.is_dragging = false;
                self.state.time_press = None;
                if self.state.time_drag.is_some() {
                    self.state.time_drag = None;
                    self.publish_change(shell);
                    shell.request_redraw();
                }
            }

            // ── Scroll on time spinners ──────────────────────────────────
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if !self.show_time {
                    return;
                }

                let Some(field) = self.state.time_focus else {
                    return;
                };

                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        self.backspace_time_input(field, shell);
                        shell.capture_event();
                    }
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        self.clear_time_interaction(false);
                        shell.request_redraw();
                        shell.capture_event();
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        let next = if modifiers.shift() {
                            field.previous()
                        } else {
                            field.next()
                        };
                        self.focus_time_field(Some(next));
                        self.state.time_press = None;
                        self.state.time_drag = None;
                        shell.request_redraw();
                        shell.capture_event();
                    }
                    keyboard::Key::Character(chars)
                        if chars.chars().count() == 1
                            && chars.chars().all(|digit| digit.is_ascii_digit()) =>
                    {
                        self.push_time_digit(field, chars.chars().next().unwrap(), shell);
                        shell.capture_event();
                    }
                    _ => {}
                }
            }

            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !self.show_time {
                    return;
                }
                let tr = time_rect(bounds);
                if !cursor.is_over(tr) {
                    return;
                }
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 20.0,
                };
                let steps = lines.round() as i32;
                let hr = hour_rect(tr);
                let mr = minute_rect(tr);
                if cursor.is_over(hr) {
                    self.step_time_field(TimeField::Hour, steps, shell);
                } else if cursor.is_over(mr) {
                    self.step_time_field(TimeField::Minute, steps, shell);
                }
            }

            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        if !cursor.is_over(bounds) {
            return mouse::Interaction::default();
        }

        if self.state.is_dragging {
            return mouse::Interaction::Grabbing;
        }

        let header = header_rect(bounds);
        if cursor.is_over(prev_arrow_rect(header))
            || cursor.is_over(next_arrow_rect(header))
            || cursor.is_over(header_title_hit_rect(
                header,
                self.state.view_mode,
                self.state.view_year,
                self.state.view_month,
            ))
        {
            return mouse::Interaction::Pointer;
        }
        if cursor.is_over(header) {
            return mouse::Interaction::Grab;
        }

        let grid = grid_rect(bounds);
        let cursor_pos = cursor.position().unwrap_or(Point::ORIGIN);
        let grid_hit = match self.state.view_mode {
            PickerView::Days => hit_test_grid(
                grid,
                cursor_pos,
                self.state.view_year,
                self.state.view_month,
            )
            .is_some(),
            PickerView::Months => hit_test_month_grid(grid, cursor_pos).is_some(),
            PickerView::Years => {
                hit_test_year_grid(grid, cursor_pos, self.state.view_year).is_some()
            }
        };

        if grid_hit {
            return mouse::Interaction::Pointer;
        }

        let footer = footer_rect(bounds, self.show_time);
        for btn in footer_button_rects(footer) {
            if cursor.is_over(btn) {
                return mouse::Interaction::Pointer;
            }
        }

        if self.show_time {
            let tr = time_rect(bounds);

            if self.state.time_drag.is_some() {
                return mouse::Interaction::ResizingVertically;
            }

            if cursor.is_over(hour_rect(tr)) || cursor.is_over(minute_rect(tr)) {
                return mouse::Interaction::Pointer;
            }

            if cursor.is_over(ampm_rect(tr)) {
                return mouse::Interaction::Pointer;
            }
        }

        mouse::Interaction::default()
    }
}

// ── Draw helpers ──────────────────────────────────────────────────────────────

impl<'r, Message, Theme, Renderer> DatePickerOverlay<'r, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'r,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'r,
{
    fn draw_header(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        let prev = prev_arrow_rect(bounds);
        let next = next_arrow_rect(bounds);

        // Arrow hover backgrounds
        for arrow in [prev, next] {
            if cursor.is_over(arrow) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: arrow,
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 4.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    style.arrow_hover_background,
                );
            }
        }

        // Prev arrow ‹
        fill_text_centered(renderer, "‹", 18.0, style.arrow_color, prev, bounds);

        // Month/year label
        let label_rect = header_title_band_rect(bounds);
        fill_text_bold_centered(
            renderer,
            &header_title(
                self.state.view_mode,
                self.state.view_year,
                self.state.view_month,
            ),
            14.0,
            style.header_text_color,
            label_rect,
            bounds,
        );

        // Next arrow ›
        fill_text_centered(renderer, "›", 18.0, style.arrow_color, next, bounds);
    }

    fn draw_dow_header(&self, renderer: &mut Renderer, style: &Style, bounds: Rectangle) {
        match self.state.view_mode {
            PickerView::Days => {
                const DAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
                let col_w = bounds.width / 7.0;
                for (i, &label) in DAYS.iter().enumerate() {
                    let cell = Rectangle {
                        x: bounds.x + i as f32 * col_w,
                        y: bounds.y,
                        width: col_w,
                        height: bounds.height,
                    };
                    fill_text_centered(renderer, label, 11.0, style.dim_text_color, cell, bounds);
                }
            }
            PickerView::Months => {
                fill_text_centered(
                    renderer,
                    "Select month",
                    11.0,
                    style.dim_text_color,
                    bounds,
                    bounds,
                );
            }
            PickerView::Years => {
                fill_text_centered(
                    renderer,
                    "Select year",
                    11.0,
                    style.dim_text_color,
                    bounds,
                    bounds,
                );
            }
        }
    }

    fn draw_grid(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        match self.state.view_mode {
            PickerView::Days => self.draw_day_grid(renderer, style, bounds),
            PickerView::Months => self.draw_month_grid(renderer, style, bounds, cursor),
            PickerView::Years => self.draw_year_grid(renderer, style, bounds, cursor),
        }
    }

    fn draw_day_grid(&self, renderer: &mut Renderer, style: &Style, bounds: Rectangle) {
        let today = Local::now().date_naive();
        let grid = month_grid(self.state.view_year, self.state.view_month);
        let cell_w = bounds.width / 7.0;
        let cell_h = bounds.height / 6.0;

        let (preview_start, preview_end) = self.range_preview();
        let single_selected = match &self.selection {
            DateSelection::Single(Some(d)) => Some(*d),
            _ => None,
        };

        for (row, week) in grid.iter().enumerate() {
            for (col, gd) in week.iter().enumerate() {
                let cell = Rectangle {
                    x: bounds.x + col as f32 * cell_w,
                    y: bounds.y + row as f32 * cell_h,
                    width: cell_w,
                    height: cell_h,
                };
                let date = gd.date;
                let is_current = gd.is_current_month;
                let is_today = date == today;
                let is_hovered = self.state.hovered_date == Some(date);

                // Classification
                let is_single_sel = single_selected == Some(date);
                let is_range_start = preview_start == Some(date) && preview_end.is_some();
                let is_range_end = preview_end == Some(date) && preview_start.is_some();
                let is_anchor_only = preview_start == Some(date) && preview_end.is_none();
                let is_interior = match (preview_start, preview_end) {
                    (Some(s), Some(e)) => date > s && date < e,
                    _ => false,
                };
                let is_endpoint = is_range_start || is_range_end || is_anchor_only;
                let is_highlighted = is_single_sel || is_endpoint;

                // Background colour
                let bg: Option<Color> = if is_highlighted {
                    Some(style.selected_background)
                } else if is_interior {
                    Some(style.range_background)
                } else {
                    None
                };

                // Background quad sizing: interior cells span full width, others are inset
                let bg_rect = if is_interior {
                    Rectangle {
                        x: cell.x,
                        y: cell.y + 2.0,
                        width: cell.width,
                        height: cell.height - 4.0,
                    }
                } else {
                    Rectangle {
                        x: cell.x + 2.0,
                        y: cell.y + 2.0,
                        width: cell.width - 4.0,
                        height: cell.height - 4.0,
                    }
                };

                // Corner radius
                let radius: iced::border::Radius = if is_range_start && !is_range_end {
                    iced::border::Radius {
                        top_left: 4.0,
                        bottom_left: 4.0,
                        top_right: 0.0,
                        bottom_right: 0.0,
                    }
                } else if is_range_end && !is_range_start {
                    iced::border::Radius {
                        top_left: 0.0,
                        bottom_left: 0.0,
                        top_right: 4.0,
                        bottom_right: 4.0,
                    }
                } else {
                    4.0.into()
                };

                if let Some(color) = bg {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: bg_rect,
                            border: Border {
                                color: Color::TRANSPARENT,
                                width: 0.0,
                                radius,
                            },
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        color,
                    );
                }

                // Hover outline (non-selected cells only)
                if is_hovered && !is_highlighted && !is_interior {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: bg_rect,
                            border: style.hover_border,
                            shadow: Shadow::default(),
                            snap: true,
                        },
                        Color::TRANSPARENT,
                    );
                }

                // Day number
                let text_color = if is_highlighted {
                    style.selected_text_color
                } else if is_today {
                    style.today_text_color
                } else if !is_current {
                    style.dim_text_color
                } else {
                    style.text_color
                };

                fill_text_centered(
                    renderer,
                    &date.day().to_string(),
                    13.0,
                    text_color,
                    cell,
                    bounds,
                );
            }
        }
    }

    fn draw_month_grid(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        let today = Local::now().date_naive();

        for month in 1..=12 {
            let cell = grid_option_rect(bounds, (month - 1) as usize, 3, 4);
            let is_selected = self.state.view_month == month;
            let is_today = self.state.view_year == today.year() && today.month() == month;
            let is_hovered = cursor.is_over(cell);

            let bg = if is_selected {
                Some(style.selected_background)
            } else {
                None
            };

            let bg_rect = Rectangle {
                x: cell.x + 4.0,
                y: cell.y + 4.0,
                width: cell.width - 8.0,
                height: cell.height - 8.0,
            };

            if let Some(color) = bg {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: bg_rect,
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 6.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    color,
                );
            }

            if is_hovered && !is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: bg_rect,
                        border: style.hover_border,
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Color::TRANSPARENT,
                );
            }

            let text_color = if is_selected {
                style.selected_text_color
            } else if is_today {
                style.today_text_color
            } else {
                style.text_color
            };

            fill_text_centered(
                renderer,
                month_abbrev(month),
                13.0,
                text_color,
                cell,
                bounds,
            );
        }
    }

    fn draw_year_grid(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        let today = Local::now().date_naive();
        let start_year = year_page_start(self.state.view_year);

        for index in 0..12 {
            let year = start_year + index as i32;
            let cell = grid_option_rect(bounds, index, 3, 4);
            let is_selected = self.state.view_year == year;
            let is_today = today.year() == year;
            let is_hovered = cursor.is_over(cell);

            let bg = if is_selected {
                Some(style.selected_background)
            } else {
                None
            };

            let bg_rect = Rectangle {
                x: cell.x + 4.0,
                y: cell.y + 4.0,
                width: cell.width - 8.0,
                height: cell.height - 8.0,
            };

            if let Some(color) = bg {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: bg_rect,
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 6.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    color,
                );
            }

            if is_hovered && !is_selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: bg_rect,
                        border: style.hover_border,
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Color::TRANSPARENT,
                );
            }

            let text_color = if is_selected {
                style.selected_text_color
            } else if is_today {
                style.today_text_color
            } else {
                style.text_color
            };

            fill_text_centered(renderer, &year.to_string(), 13.0, text_color, cell, bounds);
        }
    }

    fn draw_time_row(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) {
        let hr = hour_rect(bounds);
        let colon = colon_rect(bounds);
        let mr = minute_rect(bounds);
        let ampm = ampm_rect(bounds);
        let is_pm = self.state.time.hour >= 12;
        let hour_border = if self.state.time_focus == Some(TimeField::Hour) {
            style.hover_border
        } else {
            style.time_border
        };
        let minute_border = if self.state.time_focus == Some(TimeField::Minute) {
            style.hover_border
        } else {
            style.time_border
        };

        // Hour box
        renderer.fill_quad(
            renderer::Quad {
                bounds: hr,
                border: hour_border,
                shadow: Shadow::default(),
                snap: true,
            },
            style.time_background,
        );
        fill_text_centered(
            renderer,
            &self.time_field_text(TimeField::Hour),
            14.0,
            style.text_color,
            hr,
            bounds,
        );

        // Colon
        fill_text_centered(renderer, ":", 14.0, style.text_color, colon, bounds);

        // Minute box
        renderer.fill_quad(
            renderer::Quad {
                bounds: mr,
                border: minute_border,
                shadow: Shadow::default(),
                snap: true,
            },
            style.time_background,
        );
        fill_text_centered(
            renderer,
            &self.time_field_text(TimeField::Minute),
            14.0,
            style.text_color,
            mr,
            bounds,
        );

        // AM / PM toggle
        let am_rect = Rectangle {
            x: ampm.x,
            y: ampm.y,
            width: ampm.width / 2.0,
            height: ampm.height,
        };
        let pm_rect = Rectangle {
            x: ampm.x + ampm.width / 2.0,
            y: ampm.y,
            width: ampm.width / 2.0,
            height: ampm.height,
        };

        let am_bg = if !is_pm {
            style.time_active_background
        } else {
            style.time_background
        };
        let pm_bg = if is_pm {
            style.time_active_background
        } else {
            style.time_background
        };
        let am_text = if !is_pm {
            style.time_active_text_color
        } else {
            style.text_color
        };
        let pm_text = if is_pm {
            style.time_active_text_color
        } else {
            style.text_color
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: am_rect,
                border: Border {
                    color: style.time_border.color,
                    width: 1.0,
                    radius: iced::border::Radius {
                        top_left: 4.0,
                        bottom_left: 4.0,
                        top_right: 0.0,
                        bottom_right: 0.0,
                    },
                },
                shadow: Shadow::default(),
                snap: true,
            },
            am_bg,
        );
        fill_text_centered(renderer, "AM", 12.0, am_text, am_rect, bounds);

        renderer.fill_quad(
            renderer::Quad {
                bounds: pm_rect,
                border: Border {
                    color: style.time_border.color,
                    width: 1.0,
                    radius: iced::border::Radius {
                        top_left: 0.0,
                        bottom_left: 0.0,
                        top_right: 4.0,
                        bottom_right: 4.0,
                    },
                },
                shadow: Shadow::default(),
                snap: true,
            },
            pm_bg,
        );
        fill_text_centered(renderer, "PM", 12.0, pm_text, pm_rect, bounds);
    }

    fn draw_footer(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        // Top separator line
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: bounds.x + INNER_PAD,
                    y: bounds.y,
                    width: bounds.width - 2.0 * INNER_PAD,
                    height: 1.0,
                },
                border: Border::default(),
                shadow: Shadow::default(),
                snap: true,
            },
            style.dim_text_color,
        );

        let [today_btn, clear_btn, close_btn] = footer_button_rects(bounds);
        let labels = [
            ("Today", today_btn),
            ("Clear", clear_btn),
            ("Close", close_btn),
        ];

        for (label, btn) in labels {
            if cursor.is_over(btn) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: btn.x + 4.0,
                            y: btn.y + 4.0,
                            width: btn.width - 8.0,
                            height: btn.height - 8.0,
                        },
                        border: Border {
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: 4.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    style.footer_button_hover_background,
                );
            }
            fill_text_centered(
                renderer,
                label,
                13.0,
                style.footer_button_text_color,
                btn,
                bounds,
            );
        }
    }

    fn handle_time_click(&mut self, tr: Rectangle, pos: Point, shell: &mut Shell<'_, Message>) {
        let hr = hour_rect(tr);
        let mr = minute_rect(tr);
        let ampm = ampm_rect(tr);
        let am_rect = Rectangle {
            x: ampm.x,
            y: ampm.y,
            width: ampm.width / 2.0,
            height: ampm.height,
        };
        let pm_rect = Rectangle {
            x: ampm.x + ampm.width / 2.0,
            y: ampm.y,
            width: ampm.width / 2.0,
            height: ampm.height,
        };

        if hr.contains(pos) {
            self.focus_time_field(Some(TimeField::Hour));
            self.state.time_press = Some(TimeField::Hour);
            self.state.time_drag_start_y = pos.y;
            self.state.time_drag_start_value = self.state.time.hour;
            shell.request_redraw();
        } else if mr.contains(pos) {
            self.focus_time_field(Some(TimeField::Minute));
            self.state.time_press = Some(TimeField::Minute);
            self.state.time_drag_start_y = pos.y;
            self.state.time_drag_start_value = self.state.time.minute;
            shell.request_redraw();
        } else if am_rect.contains(pos) {
            self.clear_time_interaction(true);
            if self.state.time.hour >= 12 {
                self.state.time.hour -= 12;
            }
            self.publish_change(shell);
            shell.request_redraw();
        } else if pm_rect.contains(pos) {
            self.clear_time_interaction(true);
            if self.state.time.hour < 12 {
                self.state.time.hour += 12;
            }
            self.publish_change(shell);
            shell.request_redraw();
        } else {
            self.clear_time_interaction(false);
            shell.request_redraw();
        }
    }
}

// ── Rendering utilities ───────────────────────────────────────────────────────

fn fill_text_centered<R>(
    renderer: &mut R,
    content: &str,
    size: f32,
    color: Color,
    cell: Rectangle,
    clip: Rectangle,
) where
    R: iced::advanced::text::Renderer<Font = iced::Font>,
{
    renderer.fill_text(
        iced::advanced::Text {
            content: content.to_string(),
            bounds: Size::new(cell.width, cell.height),
            size: iced::Pixels(size),
            font: iced::Font::default(),
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: iced::advanced::text::LineHeight::default(),
            shaping: iced::advanced::text::Shaping::Basic,
            wrapping: iced::widget::text::Wrapping::None,
            ellipsis: iced::advanced::text::Ellipsis::None,
            hint_factor: None,
        },
        Point::new(cell.center_x(), cell.center_y()),
        color,
        clip,
    );
}

fn fill_text_bold_centered<R>(
    renderer: &mut R,
    content: &str,
    size: f32,
    color: Color,
    cell: Rectangle,
    clip: Rectangle,
) where
    R: iced::advanced::text::Renderer<Font = iced::Font>,
{
    renderer.fill_text(
        iced::advanced::Text {
            content: content.to_string(),
            bounds: Size::new(cell.width, cell.height),
            size: iced::Pixels(size),
            font: iced::Font {
                weight: iced::font::Weight::Bold,
                ..iced::Font::default()
            },
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: iced::advanced::text::LineHeight::default(),
            shaping: iced::advanced::text::Shaping::Basic,
            wrapping: iced::widget::text::Wrapping::None,
            ellipsis: iced::advanced::text::Ellipsis::None,
            hint_factor: None,
        },
        Point::new(cell.center_x(), cell.center_y()),
        color,
        clip,
    );
}

// ── Layout helpers ────────────────────────────────────────────────────────────

fn overlay_size(show_time: bool) -> Size {
    let h = HEADER_HEIGHT
        + DOW_HEIGHT
        + GRID_HEIGHT
        + FOOTER_HEIGHT
        + if show_time { TIME_HEIGHT } else { 0.0 };
    Size::new(OVERLAY_WIDTH, h)
}

fn header_rect(b: Rectangle) -> Rectangle {
    Rectangle {
        x: b.x,
        y: b.y,
        width: b.width,
        height: HEADER_HEIGHT,
    }
}

fn dow_header_rect(b: Rectangle) -> Rectangle {
    Rectangle {
        x: b.x + INNER_PAD,
        y: b.y + HEADER_HEIGHT,
        width: b.width - 2.0 * INNER_PAD,
        height: DOW_HEIGHT,
    }
}

fn grid_rect(b: Rectangle) -> Rectangle {
    Rectangle {
        x: b.x + INNER_PAD,
        y: b.y + HEADER_HEIGHT + DOW_HEIGHT,
        width: b.width - 2.0 * INNER_PAD,
        height: GRID_HEIGHT,
    }
}

fn time_rect(b: Rectangle) -> Rectangle {
    Rectangle {
        x: b.x + INNER_PAD,
        y: b.y + HEADER_HEIGHT + DOW_HEIGHT + GRID_HEIGHT,
        width: b.width - 2.0 * INNER_PAD,
        height: TIME_HEIGHT,
    }
}

fn footer_rect(b: Rectangle, show_time: bool) -> Rectangle {
    let y_off =
        HEADER_HEIGHT + DOW_HEIGHT + GRID_HEIGHT + if show_time { TIME_HEIGHT } else { 0.0 };
    Rectangle {
        x: b.x,
        y: b.y + y_off,
        width: b.width,
        height: FOOTER_HEIGHT,
    }
}

fn prev_arrow_rect(header: Rectangle) -> Rectangle {
    let sz = 32.0;
    Rectangle {
        x: header.x + 6.0,
        y: header.y + (header.height - sz) / 2.0,
        width: sz,
        height: sz,
    }
}

fn next_arrow_rect(header: Rectangle) -> Rectangle {
    let sz = 32.0;
    Rectangle {
        x: header.x + header.width - sz - 6.0,
        y: header.y + (header.height - sz) / 2.0,
        width: sz,
        height: sz,
    }
}

fn header_title_band_rect(header: Rectangle) -> Rectangle {
    let prev = prev_arrow_rect(header);
    let next = next_arrow_rect(header);
    let x = prev.x + prev.width;

    Rectangle {
        x,
        y: header.y,
        width: next.x - x,
        height: header.height,
    }
}

fn header_title_hit_rect(
    header: Rectangle,
    view_mode: PickerView,
    year: i32,
    month: u32,
) -> Rectangle {
    let band = header_title_band_rect(header);
    let label = header_title(view_mode, year, month);
    let estimated_width = (label.chars().count() as f32 * (HEADER_TITLE_FONT_SIZE * 0.58)
        + HEADER_TITLE_HIT_PADDING_X * 2.0)
        .min(band.width);
    let height = HEADER_TITLE_HIT_HEIGHT.min(band.height);

    Rectangle {
        x: band.x + (band.width - estimated_width) / 2.0,
        y: band.y + (band.height - height) / 2.0,
        width: estimated_width,
        height,
    }
}

fn footer_button_rects(footer: Rectangle) -> [Rectangle; 3] {
    let w = footer.width / 3.0;
    [
        Rectangle {
            x: footer.x,
            y: footer.y,
            width: w,
            height: footer.height,
        },
        Rectangle {
            x: footer.x + w,
            y: footer.y,
            width: w,
            height: footer.height,
        },
        Rectangle {
            x: footer.x + 2.0 * w,
            y: footer.y,
            width: w,
            height: footer.height,
        },
    ]
}

// Time-row sub-rects
// Layout (centred): [hour:40] [colon:14] [min:40] [gap:8] [ampm:72]  total = 174
fn time_row_start(time_row: Rectangle) -> f32 {
    let total_w = 174.0;
    time_row.x + (time_row.width - total_w) / 2.0
}

fn hour_rect(tr: Rectangle) -> Rectangle {
    let bh = 30.0;
    Rectangle {
        x: time_row_start(tr),
        y: tr.y + (tr.height - bh) / 2.0,
        width: 40.0,
        height: bh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::advanced::renderer;
    use iced::advanced::shell;
    use iced::advanced::text::{self, Text};
    use iced::{
        Background, Color, Font, Length, Pixels, Point, Rectangle, Size, Transformation, keyboard,
        mouse,
        widget::{column, container},
    };
    use iced_runtime::user_interface::{Cache, State as UiState, UserInterface};

    #[derive(Debug, Clone, PartialEq)]
    enum TestMessage {
        Closed,
        Selection(DateSelection),
        SelectionWithTime(DateSelection, TimeSelection),
        Today(NaiveDate),
    }

    /// Drives one `UserInterface::update` pass, collecting what it published.
    ///
    /// `update` now takes the window and waker a `Shell` carries, and publishes
    /// into a `shell::Bus` rather than a `&mut Vec`; the clipboard it used to
    /// borrow is built internally. Draining the bus into a `Vec` here keeps the
    /// accumulate-and-assert style the tests below are written in.
    fn update<Message, Theme, Renderer>(
        ui: &mut UserInterface<'_, Message, Theme, Renderer>,
        events: &[Event],
        cursor: mouse::Cursor,
        renderer: &mut Renderer,
        messages: &mut Vec<Message>,
    ) -> (UiState, Vec<iced::event::Status>)
    where
        Renderer: iced::advanced::Renderer,
    {
        let mut bus = shell::Bus::new();

        let result = ui.update(
            &iced::window::Headless,
            &shell::Waker::noop(),
            events,
            cursor,
            renderer,
            &mut bus,
        );

        messages.extend(bus.drain());

        result
    }

    #[derive(Default)]
    struct RecordingRenderer {
        quads: usize,
        texts: usize,
        scale: Option<renderer::Scale>,
    }

    impl iced::advanced::Renderer for RecordingRenderer {
        fn start_layer(&mut self, _bounds: Rectangle) {}

        fn end_layer(&mut self) {}

        fn start_transformation(&mut self, _transformation: Transformation) {}

        fn end_transformation(&mut self) {}

        fn fill_quad(&mut self, _quad: renderer::Quad, _background: impl Into<Background>) {
            self.quads += 1;
        }

        fn reset(&mut self, _new_bounds: Rectangle) {}

        fn hint(&mut self, scale: renderer::Scale) {
            self.scale = Some(scale);
        }

        fn scale(&self) -> Option<renderer::Scale> {
            self.scale
        }

        fn settings(&self) -> renderer::Settings {
            renderer::Settings::default()
        }

        fn allocate_image(
            &mut self,
            handle: &iced::advanced::image::Handle,
            callback: impl FnOnce(
                Result<iced::advanced::image::Allocation, iced::advanced::image::Error>,
            ) + Send
            + 'static,
        ) {
            #[allow(unsafe_code)]
            callback(Ok(unsafe {
                iced::advanced::image::allocate(handle, Size::new(1, 1))
            }));
        }
    }

    impl text::Renderer for RecordingRenderer {
        type Font = Font;
        type Paragraph = ();
        type Editor = ();

        const ICON_FONT: Font = Font::DEFAULT;
        const CHECKMARK_ICON: char = '0';
        const ARROW_DOWN_ICON: char = '0';
        const SCROLL_UP_ICON: char = '0';
        const SCROLL_DOWN_ICON: char = '0';
        const SCROLL_LEFT_ICON: char = '0';
        const SCROLL_RIGHT_ICON: char = '0';
        const ICED_LOGO: char = '0';

        fn default_font(&self) -> Self::Font {
            Font::default()
        }

        fn default_size(&self) -> Pixels {
            Pixels(16.0)
        }

        fn fill_paragraph(
            &mut self,
            _paragraph: &Self::Paragraph,
            _position: Point,
            _color: Color,
            _clip_bounds: Rectangle,
        ) {
        }

        fn fill_editor(
            &mut self,
            _editor: &Self::Editor,
            _position: Point,
            _color: Color,
            _clip_bounds: Rectangle,
        ) {
        }

        fn fill_text(
            &mut self,
            _text: Text,
            _position: Point,
            _color: Color,
            _clip_bounds: Rectangle,
        ) {
            self.texts += 1;
        }
    }

    fn overlay_bounds(show_time: bool) -> Rectangle {
        let overlay = overlay_size(show_time);
        Rectangle {
            x: (800.0 - overlay.width) / 2.0,
            y: (600.0 - overlay.height) / 2.0,
            width: overlay.width,
            height: overlay.height,
        }
    }

    fn date_cell_center(bounds: Rectangle, date: NaiveDate) -> Point {
        let grid = grid_rect(bounds);
        let grid_days = month_grid(date.year(), date.month());

        for (row, week) in grid_days.iter().enumerate() {
            for (col, grid_day) in week.iter().enumerate() {
                if grid_day.date == date {
                    let cell = Rectangle {
                        x: grid.x + col as f32 * (grid.width / 7.0),
                        y: grid.y + row as f32 * (grid.height / 6.0),
                        width: grid.width / 7.0,
                        height: grid.height / 6.0,
                    };

                    return Point::new(cell.center_x(), cell.center_y());
                }
            }
        }

        panic!("date {date} not found in month grid");
    }

    fn month_cell_center(bounds: Rectangle, month: u32) -> Point {
        let cell = grid_option_rect(grid_rect(bounds), (month - 1) as usize, 3, 4);
        Point::new(cell.center_x(), cell.center_y())
    }

    fn year_cell_center(bounds: Rectangle, base_year: i32, year: i32) -> Point {
        let start = year_page_start(base_year);
        let index = (year - start) as usize;
        let cell = grid_option_rect(grid_rect(bounds), index, 3, 4);
        Point::new(cell.center_x(), cell.center_y())
    }

    fn key_pressed(
        key: keyboard::Key,
        physical_key: keyboard::key::Physical,
        text: Option<&str>,
    ) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key,
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::default(),
            text: text.map(Into::into),
            repeat: false,
        })
    }

    fn digit_key_pressed(digit: char) -> Event {
        let code = match digit {
            '0' => keyboard::key::Code::Digit0,
            '1' => keyboard::key::Code::Digit1,
            '2' => keyboard::key::Code::Digit2,
            '3' => keyboard::key::Code::Digit3,
            '4' => keyboard::key::Code::Digit4,
            '5' => keyboard::key::Code::Digit5,
            '6' => keyboard::key::Code::Digit6,
            '7' => keyboard::key::Code::Digit7,
            '8' => keyboard::key::Code::Digit8,
            '9' => keyboard::key::Code::Digit9,
            _ => panic!("unsupported digit"),
        };

        key_pressed(
            keyboard::Key::Character(digit.to_string().into()),
            keyboard::key::Physical::Code(code),
            Some(&digit.to_string()),
        )
    }

    #[test]
    fn open_picker_exposes_overlay_interaction() {
        let mut renderer = ();
        let mut ui = UserInterface::build(
            date_picker::<(), iced::Theme, ()>(true, DateSelection::single()),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let overlay = overlay_size(false);
        let cursor = mouse::Cursor::Available(Point::new(
            400.0,
            (600.0 - overlay.height) / 2.0 + HEADER_HEIGHT / 2.0,
        ));

        let (state, statuses) =
            update(&mut ui, &[], cursor, &mut renderer, &mut messages);

        assert!(statuses.is_empty());
        assert!(messages.is_empty());

        match state {
            UiState::Updated {
                mouse_interaction, ..
            } => {
                assert_eq!(mouse_interaction, mouse::Interaction::Pointer);
            }
            UiState::Outdated => panic!("date picker UI unexpectedly became outdated"),
        }
    }

    #[test]
    fn open_picker_draws_overlay_content() {
        let mut renderer = RecordingRenderer::default();
        let mut ui = UserInterface::build(
            date_picker::<(), iced::Theme, RecordingRenderer>(true, DateSelection::single()),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let overlay = overlay_size(false);
        let cursor = mouse::Cursor::Available(Point::new(
            400.0,
            (600.0 - overlay.height) / 2.0 + HEADER_HEIGHT / 2.0,
        ));

        let _ = update(&mut ui, &[], cursor, &mut renderer, &mut messages);
        ui.draw(
            &mut renderer,
            &iced::Theme::TokyoNight,
            &renderer::Style::default(),
            cursor,
        );

        assert!(
            renderer.quads > 0,
            "expected the picker overlay to draw quads"
        );
        assert!(
            renderer.texts > 0,
            "expected the picker overlay to draw text"
        );
    }

    #[test]
    fn nested_open_picker_exposes_overlay_interaction() {
        let mut renderer = ();
        let mut ui = UserInterface::build(
            container(column![
                date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::single(),)
                    .on_close(|| TestMessage::Closed),
            ])
            .width(Length::Fill)
            .height(Length::Fill),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let overlay = overlay_size(false);
        let cursor = mouse::Cursor::Available(Point::new(
            400.0,
            (600.0 - overlay.height) / 2.0 + HEADER_HEIGHT / 2.0,
        ));

        let (state, statuses) =
            update(&mut ui, &[], cursor, &mut renderer, &mut messages);

        assert!(statuses.is_empty());
        assert!(messages.is_empty());

        match state {
            UiState::Updated {
                mouse_interaction, ..
            } => {
                assert_eq!(mouse_interaction, mouse::Interaction::Pointer);
            }
            UiState::Outdated => panic!("nested date picker UI unexpectedly became outdated"),
        }
    }

    #[test]
    fn nested_open_picker_close_button_publishes_message() {
        let mut renderer = ();
        let mut ui = UserInterface::build(
            container(column![
                date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::single(),)
                    .on_close(|| TestMessage::Closed),
            ])
            .width(Length::Fill)
            .height(Length::Fill),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let overlay = overlay_size(false);
        let bounds = Rectangle {
            x: (800.0 - overlay.width) / 2.0,
            y: (600.0 - overlay.height) / 2.0,
            width: overlay.width,
            height: overlay.height,
        };
        let close_button = footer_button_rects(footer_rect(bounds, false))[2];
        let cursor =
            mouse::Cursor::Available(Point::new(close_button.center_x(), close_button.center_y()));

        let _ = update(&mut ui, &[], cursor, &mut renderer, &mut messages);
        messages.clear();

        let (state, statuses) = update(&mut ui,
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            cursor,
            &mut renderer,
            &mut messages,
        );

        assert_eq!(statuses, vec![iced::event::Status::Ignored]);
        assert_eq!(messages, vec![TestMessage::Closed]);

        match state {
            UiState::Updated {
                mouse_interaction, ..
            } => {
                assert_eq!(mouse_interaction, mouse::Interaction::Pointer);
            }
            UiState::Outdated => panic!("nested date picker UI unexpectedly became outdated"),
        }
    }

    #[test]
    fn today_button_selects_today_and_publishes_callback() {
        let mut renderer = ();
        let mut ui = UserInterface::build(
            date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::single())
                .on_change(TestMessage::Selection)
                .on_today(TestMessage::Today),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let bounds = overlay_bounds(false);
        let today_button = footer_button_rects(footer_rect(bounds, false))[0];
        let cursor =
            mouse::Cursor::Available(Point::new(today_button.center_x(), today_button.center_y()));

        let _ = update(&mut ui, &[], cursor, &mut renderer, &mut messages);
        messages.clear();

        let (_state, _statuses) = update(&mut ui,
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            cursor,
            &mut renderer,
            &mut messages,
        );

        let today = Local::now().date_naive();
        assert_eq!(
            messages,
            vec![
                TestMessage::Selection(DateSelection::Single(Some(today))),
                TestMessage::Today(today),
            ]
        );
    }

    #[test]
    fn title_navigation_allows_selecting_different_year_and_month() {
        let initial = NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid initial date");
        let target = NaiveDate::from_ymd_opt(2026, 7, 10).expect("valid target date");
        let mut renderer = ();
        let mut ui = UserInterface::build(
            date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::with_date(initial))
                .on_change(TestMessage::Selection),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let bounds = overlay_bounds(false);
        let title_rect = header_title_hit_rect(
            header_rect(bounds),
            PickerView::Days,
            initial.year(),
            initial.month(),
        );
        let title_cursor =
            mouse::Cursor::Available(Point::new(title_rect.center_x(), title_rect.center_y()));

        let _ = update(&mut ui,
            &[],
            title_cursor,
            &mut renderer,
            &mut messages,
        );

        for _ in 0..2 {
            let (_state, _statuses) = update(&mut ui,
                &[Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Left,
                ))],
                title_cursor,
                &mut renderer,
                &mut messages,
            );
        }

        assert!(
            messages.is_empty(),
            "view switching should not publish selection messages"
        );

        let year_cursor =
            mouse::Cursor::Available(year_cell_center(bounds, initial.year(), target.year()));
        let (_state, _statuses) = update(&mut ui,
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            year_cursor,
            &mut renderer,
            &mut messages,
        );

        assert!(
            messages.is_empty(),
            "year selection should only change navigation state"
        );

        let month_cursor = mouse::Cursor::Available(month_cell_center(bounds, target.month()));
        let (_state, _statuses) = update(&mut ui,
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            month_cursor,
            &mut renderer,
            &mut messages,
        );

        assert!(
            messages.is_empty(),
            "month selection should only change navigation state"
        );

        let day_cursor = mouse::Cursor::Available(date_cell_center(bounds, target));
        let (_state, _statuses) = update(&mut ui,
            &[Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left,
            ))],
            day_cursor,
            &mut renderer,
            &mut messages,
        );

        assert_eq!(
            messages,
            vec![TestMessage::Selection(DateSelection::Single(Some(target)))],
        );
    }

    #[test]
    fn header_padding_keeps_overlay_draggable() {
        let initial = NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid initial date");
        let mut renderer = ();
        let mut ui = UserInterface::build(
            date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::with_date(initial)),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let bounds = overlay_bounds(false);
        let header = header_rect(bounds);
        let prev = prev_arrow_rect(header);
        let title =
            header_title_hit_rect(header, PickerView::Days, initial.year(), initial.month());
        let cursor = mouse::Cursor::Available(Point::new(
            (prev.x + prev.width + title.x) / 2.0,
            title.center_y(),
        ));

        let (state, statuses) =
            update(&mut ui, &[], cursor, &mut renderer, &mut messages);

        assert!(statuses.is_empty());
        assert!(messages.is_empty());

        match state {
            UiState::Updated {
                mouse_interaction, ..
            } => {
                assert_eq!(mouse_interaction, mouse::Interaction::Grab);
            }
            UiState::Outdated => panic!("date picker UI unexpectedly became outdated"),
        }
    }

    #[test]
    fn time_fields_accept_typed_digits() {
        let selected = NaiveDate::from_ymd_opt(2024, 1, 15).expect("valid selected date");
        let mut renderer = ();
        let mut ui = UserInterface::build(
            date_picker::<TestMessage, iced::Theme, ()>(true, DateSelection::with_date(selected))
                .initial_time(TimeSelection::new(15, 42))
                .show_time()
                .on_change_with_time(TestMessage::SelectionWithTime),
            Size::new(800.0, 600.0),
            Cache::default(),
            &mut renderer,
        );

        let mut messages = Vec::new();
        let bounds = overlay_bounds(true);
        let time = time_rect(bounds);
        let hour_cursor = mouse::Cursor::Available(Point::new(
            hour_rect(time).center_x(),
            hour_rect(time).center_y(),
        ));
        let minute_cursor = mouse::Cursor::Available(Point::new(
            minute_rect(time).center_x(),
            minute_rect(time).center_y(),
        ));

        let _ = update(&mut ui,
            &[],
            hour_cursor,
            &mut renderer,
            &mut messages,
        );
        let _ = update(&mut ui,
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            hour_cursor,
            &mut renderer,
            &mut messages,
        );
        messages.clear();

        let _ = update(&mut ui,
            &[digit_key_pressed('0')],
            hour_cursor,
            &mut renderer,
            &mut messages,
        );
        assert!(messages.is_empty());

        let _ = update(&mut ui,
            &[digit_key_pressed('7')],
            hour_cursor,
            &mut renderer,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![TestMessage::SelectionWithTime(
                DateSelection::Single(Some(selected)),
                TimeSelection::new(19, 42),
            )],
        );

        let _ = update(&mut ui,
            &[
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            minute_cursor,
            &mut renderer,
            &mut messages,
        );
        messages.clear();

        let _ = update(&mut ui,
            &[digit_key_pressed('5')],
            minute_cursor,
            &mut renderer,
            &mut messages,
        );
        assert_eq!(
            messages,
            vec![TestMessage::SelectionWithTime(
                DateSelection::Single(Some(selected)),
                TimeSelection::new(19, 5),
            )],
        );
    }

    #[test]
    fn default_style_uses_primary_color_for_today_text() {
        let style = default_style(&iced::Theme::TokyoNight, Status::Active);

        assert_eq!(style.today_text_color, style.selected_background);
    }
}

fn colon_rect(tr: Rectangle) -> Rectangle {
    let bh = 30.0;
    Rectangle {
        x: time_row_start(tr) + 40.0,
        y: tr.y + (tr.height - bh) / 2.0,
        width: 14.0,
        height: bh,
    }
}

fn minute_rect(tr: Rectangle) -> Rectangle {
    let bh = 30.0;
    Rectangle {
        x: time_row_start(tr) + 54.0,
        y: tr.y + (tr.height - bh) / 2.0,
        width: 40.0,
        height: bh,
    }
}

fn ampm_rect(tr: Rectangle) -> Rectangle {
    let bh = 30.0;
    Rectangle {
        x: time_row_start(tr) + 102.0,
        y: tr.y + (tr.height - bh) / 2.0,
        width: 72.0,
        height: bh,
    }
}

// ── Date helpers ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct GridDay {
    date: NaiveDate,
    is_current_month: bool,
}

/// Builds a Sunday-anchored 6×7 grid for the given year/month.
fn month_grid(year: i32, month: u32) -> [[GridDay; 7]; 6] {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("valid date");
    // num_days_from_monday(): Mon=0 … Sun=6. Convert to Sun=0 … Sat=6:
    let start_col = (first.weekday().num_days_from_monday() + 1) % 7;

    let dummy = GridDay {
        date: first,
        is_current_month: true,
    };
    let mut grid = [[dummy; 7]; 6];

    // Walk backwards from the first day of the month
    let start_date = first - chrono::Duration::days(start_col as i64);
    let mut current = start_date;

    for row in &mut grid {
        for cell in row {
            let is_current = current.month() == month && current.year() == year;
            *cell = GridDay {
                date: current,
                is_current_month: is_current,
            };
            current = current.succ_opt().unwrap_or(current);
        }
    }
    grid
}

/// Returns the `NaiveDate` for the grid cell at `pos`, or `None` if outside.
fn hit_test_grid(grid: Rectangle, pos: Point, year: i32, month: u32) -> Option<NaiveDate> {
    if !grid.contains(pos) {
        return None;
    }
    let col = ((pos.x - grid.x) / (grid.width / 7.0)) as usize;
    let row = ((pos.y - grid.y) / (grid.height / 6.0)) as usize;
    if col >= 7 || row >= 6 {
        return None;
    }
    Some(month_grid(year, month)[row][col].date)
}

fn hit_test_option_grid(grid: Rectangle, pos: Point, columns: usize, rows: usize) -> Option<usize> {
    if !grid.contains(pos) {
        return None;
    }

    let col = ((pos.x - grid.x) / (grid.width / columns as f32)) as usize;
    let row = ((pos.y - grid.y) / (grid.height / rows as f32)) as usize;

    if col >= columns || row >= rows {
        return None;
    }

    Some(row * columns + col)
}

fn hit_test_month_grid(grid: Rectangle, pos: Point) -> Option<u32> {
    let index = hit_test_option_grid(grid, pos, 3, 4)?;
    Some(index as u32 + 1)
}

fn hit_test_year_grid(grid: Rectangle, pos: Point, view_year: i32) -> Option<i32> {
    let index = hit_test_option_grid(grid, pos, 3, 4)?;
    Some(year_page_start(view_year) + index as i32)
}

/// Adds `delta` months to (year, month), wrapping the year as needed.
fn add_months(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + month as i32 - 1 + delta;
    (total.div_euclid(12), (total.rem_euclid(12) + 1) as u32)
}

fn grid_option_rect(bounds: Rectangle, index: usize, columns: usize, rows: usize) -> Rectangle {
    let col = index % columns;
    let row = index / columns;
    let cell_w = bounds.width / columns as f32;
    let cell_h = bounds.height / rows as f32;

    Rectangle {
        x: bounds.x + col as f32 * cell_w,
        y: bounds.y + row as f32 * cell_h,
        width: cell_w,
        height: cell_h,
    }
}

fn year_page_start(view_year: i32) -> i32 {
    view_year - 4
}

fn header_title(view_mode: PickerView, year: i32, month: u32) -> String {
    match view_mode {
        PickerView::Days => format_month_year(year, month),
        PickerView::Months => year.to_string(),
        PickerView::Years => {
            let start = year_page_start(year);
            format!("{start} - {}", start + 11)
        }
    }
}

fn format_month_year(year: i32, month: u32) -> String {
    const NAMES: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    format!("{} {year}", NAMES[(month - 1) as usize])
}

fn month_abbrev(month: u32) -> &'static str {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    MONTHS[(month - 1) as usize]
}

/// Converts a 24-hour value to `(12h, is_pm)`.
fn to_12h(hour: u32) -> (u32, bool) {
    let is_pm = hour >= 12;
    let h = match hour % 12 {
        0 => 12,
        h => h,
    };
    (h, is_pm)
}

// ── Style / Catalog ───────────────────────────────────────────────────────────

/// Interaction status for style queries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Status {
    Active,
    Hovered,
}

/// Visual style for the date picker.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Panel background colour.
    pub background: Color,
    /// Panel border.
    pub border: Border,
    /// Drop shadow.
    pub shadow: Shadow,
    /// Header background colour.
    pub header_background: Color,
    /// Default text colour.
    pub text_color: Color,
    /// Dimmed text colour (other-month days, day-of-week labels).
    pub dim_text_color: Color,
    /// Background for today's cell.
    pub today_background: Color,
    /// Text colour for today's cell.
    pub today_text_color: Color,
    /// Background for selected / endpoint cells.
    pub selected_background: Color,
    /// Text colour for selected / endpoint cells.
    pub selected_text_color: Color,
    /// Background for cells inside a range (between endpoints).
    pub range_background: Color,
    /// Border drawn around hovered (unselected) cells.
    pub hover_border: Border,
    /// Header month/year label colour.
    pub header_text_color: Color,
    /// Arrow glyph colour.
    pub arrow_color: Color,
    /// Background behind an arrow on hover.
    pub arrow_hover_background: Color,
    /// Footer button text colour.
    pub footer_button_text_color: Color,
    /// Footer button hover background.
    pub footer_button_hover_background: Color,
    /// Background of hour/minute spinner boxes.
    pub time_background: Color,
    /// Border of hour/minute spinner boxes.
    pub time_border: Border,
    /// Background of the active AM or PM button.
    pub time_active_background: Color,
    /// Text colour of the active AM or PM button.
    pub time_active_text_color: Color,
}

/// Theme catalog trait — mirrors the pattern used by [`collapsible`].
pub trait Catalog {
    type Class<'a>;
    fn default<'a>() -> Self::Class<'a>;
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// Type alias for a style closure.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default_style)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// Default style derived from the current iced [`Theme`].
pub fn default_style(theme: &iced::Theme, _status: Status) -> Style {
    let p = theme.palette();
    Style {
        background: p.background.weak.color,
        border: Border {
            color: p.background.strong.color,
            width: 1.5,
            radius: 10.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.32),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 22.0,
        },
        header_background: p.background.strong.color,
        text_color: p.background.weak.text,
        dim_text_color: Color {
            a: 0.48,
            ..p.background.weak.text
        },
        today_background: Color::from_rgb8(0xCC, 0xE5, 0xFF),
        today_text_color: p.primary.base.color,
        selected_background: p.primary.base.color,
        selected_text_color: p.primary.base.text,
        range_background: Color::from_rgba(
            p.primary.base.color.r,
            p.primary.base.color.g,
            p.primary.base.color.b,
            0.18,
        ),
        hover_border: Border {
            color: p.primary.base.color,
            width: 1.5,
            radius: 4.0.into(),
        },
        header_text_color: p.background.strong.text,
        arrow_color: p.background.strong.text,
        arrow_hover_background: p.background.base.color,
        footer_button_text_color: p.primary.base.color,
        footer_button_hover_background: p.background.strong.color,
        time_background: p.background.base.color,
        time_border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 4.0.into(),
        },
        time_active_background: p.primary.base.color,
        time_active_text_color: p.primary.base.text,
    }
}
