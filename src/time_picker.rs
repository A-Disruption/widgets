//! An inline scroll-wheel time picker, for one time or a range of times.
//!
//! A [`TimePicker`] is an ordinary widget, not an overlay. Put it in a
//! [`popover`](crate::popover) to dock it under a field, in a
//! [`layer::dialog`](crate::layer) to make it modal, or leave it on the page.
//!
//! Each part of the time — hours, minutes, optionally seconds, and AM/PM on a
//! 12-hour clock — is a wheel. A wheel turns by dragging it, by the mouse
//! wheel, by clicking the item to bring to the middle, or, once the picker has
//! been clicked, by the arrow keys: Up and Down turn the focused wheel, Left
//! and Right move between wheels. Hours, minutes and seconds wrap around; the
//! AM/PM wheel does not.
//!
//! The application owns the value. A wheel reports when it comes to rest, not
//! on every frame of a drag, and if the application keeps the old value the
//! wheels turn back to it.
//!
//! - [`time_picker`] picks one [`NaiveTime`].
//! - [`range`] picks a [`TimeRange`]: a start and an end shown side by side.
//!   [`TimePicker::min_duration`] and [`TimePicker::max_duration`] keep it
//!   within limits by moving the end. A range whose end is earlier than its
//!   start runs past midnight.
//!
//! # Example
//! ```no_run
//! use chrono::NaiveTime;
//! use iced::{Element, Widget};
//! use widgets::time_picker::{self, TimeRange};
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     Alarm(NaiveTime),
//!     Shift(TimeRange),
//! }
//!
//! fn view(alarm: Option<NaiveTime>, shift: TimeRange) -> Element<'static, Message> {
//!     iced::widget::column![
//!         time_picker::time_picker(alarm, Message::Alarm).minute_step(5),
//!         time_picker::range(shift, Message::Shift)
//!             .min_duration(chrono::Duration::minutes(30)),
//!     ]
//!     .spacing(24)
//!     .boxed()
//! }
//! ```

use chrono::{Duration, NaiveTime, Timelike};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::operation::Focusable;
use iced::advanced::widget::{self, Operation, Tree, tree};
use iced::advanced::{Shell, Widget};
use iced::alignment::Vertical;
use iced::time::Instant;
use iced::{
    Background, Border, Color, Event, Font, Length, Pixels, Point, Rectangle, Size, Theme,
    keyboard, mouse, window,
};

/// Space around the whole picker.
const PADDING: f32 = 8.0;
/// The width of the `:` between two wheels.
const SEPARATOR_WIDTH: f32 = 16.0;
/// The gap before the AM/PM wheel.
const PERIOD_GAP: f32 = 6.0;
/// The gap between the start and end of a range, holding the dash.
const RANGE_GAP: f32 = 36.0;
/// The height of the start/end labels above a range.
const LABEL_HEIGHT: f32 = 24.0;
/// How far the pointer must travel before a press becomes a drag.
const DRAG_THRESHOLD: f32 = 3.0;
/// How long a wheel takes to settle.
const SETTLE: std::time::Duration = std::time::Duration::from_millis(180);
/// The time shown before the application has picked one.
const DEFAULT_TIME: NaiveTime = NaiveTime::from_hms_opt(12, 0, 0).expect("noon exists");

// ── Values ───────────────────────────────────────────────────────────────────

/// A span of time within a day, as picked by a [`range`] time picker.
///
/// An `end` earlier than `start` runs past midnight: 22:00 to 06:00 is eight
/// hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
    /// When the range starts.
    pub start: NaiveTime,
    /// When the range ends.
    pub end: NaiveTime,
}

impl TimeRange {
    /// A range from `start` to `end`.
    pub fn new(start: NaiveTime, end: NaiveTime) -> Self {
        Self { start, end }
    }

    /// How long the range lasts, from zero up to (but excluding) a day.
    pub fn duration(&self) -> Duration {
        let seconds = (self.end - self.start).num_seconds().rem_euclid(DAY);

        Duration::seconds(seconds)
    }

    /// Whether the range runs past midnight.
    pub fn crosses_midnight(&self) -> bool {
        self.end < self.start
    }
}

const DAY: i64 = 24 * 60 * 60;

/// Whether a time picker shows a 12-hour clock with AM/PM, or a 24-hour one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clock {
    /// Hours 12, 1, …, 11 and an AM/PM wheel.
    #[default]
    TwelveHour,
    /// Hours 00 to 23.
    TwentyFourHour,
}

// ── Constructors ─────────────────────────────────────────────────────────────

/// A time picker for one time, reporting it through `on_change`.
///
/// With no value it shows noon, and reports a time as soon as a wheel moves.
pub fn time_picker<'a, Message, Theme>(
    value: Option<NaiveTime>,
    on_change: impl Fn(NaiveTime) -> Message + 'a,
) -> TimePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    TimePicker::new(Mode::Single {
        value,
        on_change: Box::new(on_change),
    })
}

/// A time picker for a [`TimeRange`], reporting every change through
/// `on_change`.
pub fn range<'a, Message, Theme>(
    value: TimeRange,
    on_change: impl Fn(TimeRange) -> Message + 'a,
) -> TimePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    TimePicker::new(Mode::Range {
        value,
        on_change: Box::new(on_change),
    })
}

// ── Widget ───────────────────────────────────────────────────────────────────

enum Mode<'a, Message> {
    Single {
        value: Option<NaiveTime>,
        on_change: Box<dyn Fn(NaiveTime) -> Message + 'a>,
    },
    Range {
        value: TimeRange,
        on_change: Box<dyn Fn(TimeRange) -> Message + 'a>,
    },
}

impl<Message> Mode<'_, Message> {
    /// The time each group of wheels shows.
    fn times(&self) -> Vec<NaiveTime> {
        match self {
            Self::Single { value, .. } => vec![value.unwrap_or(DEFAULT_TIME)],
            Self::Range { value, .. } => vec![value.start, value.end],
        }
    }
}

/// An inline scroll-wheel time picker. See the [module documentation](self).
pub struct TimePicker<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    mode: Mode<'a, Message>,
    clock: Clock,
    minute_step: u32,
    seconds: bool,
    min: Option<NaiveTime>,
    max: Option<NaiveTime>,
    disabled: Option<Box<dyn Fn(NaiveTime) -> bool + 'a>>,
    min_duration: Option<Duration>,
    max_duration: Option<Duration>,
    labels: [String; 2],
    rows: usize,
    row_height: f32,
    column_width: f32,
    text_size: Option<Pixels>,
    font: Option<Font>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> TimePicker<'a, Message, Theme>
where
    Theme: Catalog,
{
    fn new(mode: Mode<'a, Message>) -> Self {
        Self {
            id: None,
            mode,
            clock: Clock::default(),
            minute_step: 1,
            seconds: false,
            min: None,
            max: None,
            disabled: None,
            min_duration: None,
            max_duration: None,
            labels: ["Start".to_owned(), "End".to_owned()],
            rows: 5,
            row_height: 36.0,
            column_width: 56.0,
            text_size: None,
            font: None,
            class: Theme::default(),
        }
    }

    /// Sets the [`widget::Id`], so operations can focus the picker.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the [`Clock`]: 12-hour with AM/PM (the default) or 24-hour.
    pub fn clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        self
    }

    /// Offers minutes in steps of `step` — 5 or 15, say. A value between steps
    /// is shown at the step before it.
    pub fn minute_step(mut self, step: u32) -> Self {
        self.minute_step = step.clamp(1, 30);
        self
    }

    /// Adds a seconds wheel.
    pub fn seconds(mut self, show: bool) -> Self {
        self.seconds = show;
        self
    }

    /// Times before `min` cannot be picked.
    pub fn min(mut self, min: NaiveTime) -> Self {
        self.min = Some(min);
        self
    }

    /// Times after `max` cannot be picked.
    pub fn max(mut self, max: NaiveTime) -> Self {
        self.max = Some(max);
        self
    }

    /// Times for which `is_disabled` returns `true` cannot be picked — a lunch
    /// break, the hours a shop is closed.
    pub fn disabled(mut self, is_disabled: impl Fn(NaiveTime) -> bool + 'a) -> Self {
        self.disabled = Some(Box::new(is_disabled));
        self
    }

    /// For a [`range`] picker: the range lasts at least this long. When a
    /// change would make it shorter, the end moves to make up the difference.
    pub fn min_duration(mut self, duration: Duration) -> Self {
        self.min_duration = Some(duration);
        self
    }

    /// For a [`range`] picker: the range lasts at most this long. When a
    /// change would make it longer, the end moves to cut it down.
    pub fn max_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// For a [`range`] picker: the labels above the start and the end.
    /// Empty strings hide the label row.
    pub fn labels(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
        self.labels = [start.into(), end.into()];
        self
    }

    /// How many rows each wheel shows: the picked one in the middle and the
    /// rest around it. Odd numbers from 3 to 9; 5 by default.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = (rows.clamp(3, 9) / 2) * 2 + 1;
        self
    }

    /// Sets the height of one row of a wheel.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = height.max(16.0);
        self
    }

    /// Sets the width of one wheel.
    pub fn column_width(mut self, width: f32) -> Self {
        self.column_width = width.max(24.0);
        self
    }

    /// Sets the size of the numbers.
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

    fn wheels(&self) -> Wheels {
        Wheels {
            clock: self.clock,
            minute_step: self.minute_step,
            seconds: self.seconds,
        }
    }

    fn groups(&self) -> usize {
        match self.mode {
            Mode::Single { .. } => 1,
            Mode::Range { .. } => 2,
        }
    }

    fn show_labels(&self) -> bool {
        self.groups() == 2 && self.labels.iter().any(|label| !label.is_empty())
    }

    fn geometry(&self, bounds: Rectangle) -> Geometry {
        Geometry::new(
            bounds,
            &self.wheels(),
            self.groups(),
            self.show_labels(),
            self.rows,
            self.row_height,
            self.column_width,
        )
    }

    fn is_enabled(&self, time: NaiveTime) -> bool {
        self.min.is_none_or(|min| time >= min)
            && self.max.is_none_or(|max| time <= max)
            && !self.disabled.as_ref().is_some_and(|f| f(time))
    }

    /// Settles wheel `column` of `group` on `target`, steering it to the
    /// nearest enabled time, and returns the message for the new value, if
    /// it changed.
    fn settle(
        &self,
        state: &mut State,
        group: usize,
        column: usize,
        target: i64,
        now: Instant,
    ) -> Option<Message> {
        let wheels = self.wheels();
        let columns = wheels.columns();
        let kind = columns[column];
        let len = wheels.len(kind) as i64;
        let target = if kind.wraps() {
            target
        } else {
            target.clamp(0, len - 1)
        };

        let previous = state.groups[group][column].target;
        let period = columns.iter().position(|kind| *kind == Kind::Period);

        // On a 12-hour clock the hour wheel carries into AM/PM, as a clock
        // face does: 11 AM turned on by one is 12 PM. Every full turn of the
        // hour wheel past 11 flips the period once.
        let flips = |target: i64| {
            kind == Kind::Hour
                && period.is_some()
                && (target.div_euclid(len) - previous.div_euclid(len)).rem_euclid(2) == 1
        };

        let compose = |state: &State, target: i64| {
            let mut indices = state.indices(group, &wheels);
            indices[column] = wheels.index(kind, target);

            if let Some(period) = period
                && flips(target)
            {
                indices[period] = 1 - indices[period];
            }

            wheels.compose(&indices)
        };

        // Prefer the target; failing that, the nearest enabled item, trying
        // the way the wheel was turning first; failing that, stay put.
        let forward = target >= previous;
        let steer = (0..len).flat_map(|distance| {
            if forward {
                [target + distance, target - distance]
            } else {
                [target - distance, target + distance]
            }
        });
        let target = steer
            .filter(|candidate| kind.wraps() || (0..len).contains(candidate))
            .find(|&candidate| self.is_enabled(compose(state, candidate)))
            .unwrap_or(previous);

        let time = compose(state, target);

        state.groups[group][column].turn_to(target, now);

        if let Some(period) = period
            && flips(target)
        {
            let wheel = &mut state.groups[group][period];
            let flipped = 1 - wheels.index(Kind::Period, wheel.target) as i64;

            wheel.turn_to(flipped, now);
        }

        match &self.mode {
            Mode::Single { value, on_change } => (*value != Some(time)).then(|| on_change(time)),
            Mode::Range { value, on_change } => {
                let mut range = *value;

                if group == 0 {
                    range.start = time;
                } else {
                    range.end = time;
                }

                let range = keep_duration(range, self.min_duration, self.max_duration);

                (range != *value).then(|| on_change(range))
            }
        }
    }

    /// The size of the numbers: the configured one, or half a row.
    fn number_size(&self) -> f32 {
        self.text_size
            .map(|size| size.0)
            .unwrap_or(self.row_height * 0.5)
    }
}

/// Moves the end of `range` so it lasts between `min` and `max`.
fn keep_duration(range: TimeRange, min: Option<Duration>, max: Option<Duration>) -> TimeRange {
    let duration = range.duration();
    let mut wanted = duration;

    if let Some(min) = min {
        wanted = wanted.max(min);
    }

    if let Some(max) = max {
        wanted = wanted.min(max);
    }

    if wanted == duration {
        range
    } else {
        TimeRange {
            start: range.start,
            end: range.start.overflowing_add_signed(wanted).0,
        }
    }
}

// ── Wheels ───────────────────────────────────────────────────────────────────

/// What a wheel picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Hour,
    Minute,
    Second,
    Period,
}

impl Kind {
    fn wraps(self) -> bool {
        self != Self::Period
    }
}

/// The wheels a configuration shows, and how indices map to times.
#[derive(Debug, Clone, Copy)]
struct Wheels {
    clock: Clock,
    minute_step: u32,
    seconds: bool,
}

impl Wheels {
    fn columns(&self) -> Vec<Kind> {
        let mut columns = vec![Kind::Hour, Kind::Minute];

        if self.seconds {
            columns.push(Kind::Second);
        }

        if self.clock == Clock::TwelveHour {
            columns.push(Kind::Period);
        }

        columns
    }

    fn len(&self, kind: Kind) -> usize {
        match kind {
            Kind::Hour => match self.clock {
                Clock::TwelveHour => 12,
                Clock::TwentyFourHour => 24,
            },
            Kind::Minute => 60usize.div_ceil(self.minute_step as usize),
            Kind::Second => 60,
            Kind::Period => 2,
        }
    }

    /// The item an unbounded wheel position lands on.
    fn index(&self, kind: Kind, position: i64) -> usize {
        let len = self.len(kind) as i64;

        if kind.wraps() {
            position.rem_euclid(len) as usize
        } else {
            position.clamp(0, len - 1) as usize
        }
    }

    /// The item of wheel `kind` that shows `time`.
    fn index_of(&self, kind: Kind, time: NaiveTime) -> usize {
        match kind {
            Kind::Hour => match self.clock {
                Clock::TwelveHour => time.hour() as usize % 12,
                Clock::TwentyFourHour => time.hour() as usize,
            },
            Kind::Minute => (time.minute() / self.minute_step) as usize,
            Kind::Second => time.second() as usize,
            Kind::Period => usize::from(time.hour() >= 12),
        }
    }

    /// The time the given items of [`Wheels::columns`] spell out.
    fn compose(&self, indices: &[usize]) -> NaiveTime {
        let mut hour = 0;
        let mut minute = 0;
        let mut second = 0;
        let mut afternoon = false;

        for (kind, &index) in self.columns().into_iter().zip(indices) {
            match kind {
                Kind::Hour => hour = index as u32,
                Kind::Minute => minute = (index as u32 * self.minute_step).min(59),
                Kind::Second => second = index as u32,
                Kind::Period => afternoon = index == 1,
            }
        }

        if self.clock == Clock::TwelveHour && afternoon {
            hour += 12;
        }

        NaiveTime::from_hms_opt(hour, minute, second).unwrap_or(DEFAULT_TIME)
    }

    fn label(&self, kind: Kind, index: usize) -> String {
        match kind {
            Kind::Hour => match self.clock {
                Clock::TwelveHour if index == 0 => "12".to_owned(),
                Clock::TwelveHour => index.to_string(),
                Clock::TwentyFourHour => format!("{index:02}"),
            },
            Kind::Minute => format!("{:02}", index as u32 * self.minute_step),
            Kind::Second => format!("{index:02}"),
            Kind::Period => if index == 0 { "AM" } else { "PM" }.to_owned(),
        }
    }
}

// ── State ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct Tween {
    from: f32,
    started: Instant,
}

/// One wheel's position, in items. A wrapping wheel's position is unbounded:
/// turning past 59 minutes goes on to 60, which shows as 00.
#[derive(Debug, Clone, Copy)]
struct Wheel {
    /// Where the wheel is drawn.
    position: f32,
    /// Where the wheel is going, or rests.
    target: i64,
    tween: Option<Tween>,
}

impl Wheel {
    fn at(target: i64) -> Self {
        Self {
            position: target as f32,
            target,
            tween: None,
        }
    }

    fn turn_to(&mut self, target: i64, now: Instant) {
        self.target = target;
        self.tween = Some(Tween {
            from: self.position,
            started: now,
        });
    }

    /// Advances the settling animation; returns whether it is still running.
    fn tick(&mut self, now: Instant) -> bool {
        let Some(tween) = self.tween else {
            return false;
        };

        let progress = (now.saturating_duration_since(tween.started).as_secs_f32()
            / SETTLE.as_secs_f32())
        .min(1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);

        self.position = tween.from + (self.target as f32 - tween.from) * eased;

        if progress >= 1.0 {
            self.position = self.target as f32;
            self.tween = None;
        }

        self.tween.is_some()
    }
}

/// The nearest unbounded position of a wrapping wheel at `current` that shows
/// item `index` of `len`.
fn nearest_turn(current: i64, index: usize, len: usize) -> i64 {
    let len = len as i64;
    let base = current - current.rem_euclid(len) + index as i64;

    [base - len, base, base + len]
        .into_iter()
        .min_by_key(|candidate| (candidate - current).abs())
        .unwrap_or(base)
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    group: usize,
    column: usize,
    origin: Point,
    from: f32,
    moved: bool,
}

struct State {
    groups: Vec<Vec<Wheel>>,
    drag: Option<Drag>,
    /// The wheel the keyboard turns: `(group, column)`.
    focus: (usize, usize),
    is_focused: bool,
    /// Trackpad scrolling not yet turned into a step.
    scroll: f32,
    /// The configuration the wheels were laid out for.
    wheels: Option<(Clock, u32, bool, usize)>,
}

impl State {
    fn indices(&self, group: usize, wheels: &Wheels) -> Vec<usize> {
        wheels
            .columns()
            .into_iter()
            .zip(&self.groups[group])
            .map(|(kind, wheel)| wheels.index(kind, wheel.target))
            .collect()
    }

    fn is_animating(&self) -> bool {
        self.groups
            .iter()
            .flatten()
            .any(|wheel| wheel.tween.is_some())
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

impl<Message, Theme> widget::Meta for TimePicker<'_, Message, Theme> where Theme: Catalog {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for TimePicker<'_, Message, Theme>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            groups: Vec::new(),
            drag: None,
            focus: (0, 0),
            is_focused: false,
            scroll: 0.0,
            wheels: None,
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        let wheels = self.wheels();
        let columns = wheels.columns();
        let times = self.mode.times();
        let config = (self.clock, self.minute_step, self.seconds, times.len());

        // A different set of wheels starts over, at the value.
        if state.wheels != Some(config) {
            state.wheels = Some(config);
            state.groups = times
                .iter()
                .map(|&time| {
                    columns
                        .iter()
                        .map(|&kind| Wheel::at(wheels.index_of(kind, time) as i64))
                        .collect()
                })
                .collect();
            state.focus = (0, 0);
            return;
        }

        // Otherwise turn each wheel to the value, the short way round — except
        // the one being dragged, which the user is holding.
        let now = Instant::now();

        for (group, &time) in times.iter().enumerate() {
            for (column, &kind) in columns.iter().enumerate() {
                let held = state
                    .drag
                    .is_some_and(|drag| drag.group == group && drag.column == column);

                if held {
                    continue;
                }

                let wheel = &mut state.groups[group][column];
                let index = wheels.index_of(kind, time);

                if wheels.index(kind, wheel.target) != index {
                    let target = if kind.wraps() {
                        nearest_turn(wheel.target, index, wheels.len(kind))
                    } else {
                        index as i64
                    };

                    wheel.turn_to(target, now);
                }
            }
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        let size = Geometry::size(
            &self.wheels(),
            self.groups(),
            self.show_labels(),
            self.rows,
            self.row_height,
            self.column_width,
        );

        tree.size = limits.resolve(Length::Shrink, Length::Shrink, size);
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

        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                let mut running = false;

                for wheel in state.groups.iter_mut().flatten() {
                    running |= wheel.tick(*now);
                }

                if running {
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

                if let Some((group, column)) = geometry.wheel_at(position) {
                    state.focus = (group, column);
                    state.drag = Some(Drag {
                        group,
                        column,
                        origin: position,
                        from: state.groups[group][column].position,
                        moved: false,
                    });
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let Some(drag) = state.drag.as_mut() else {
                    return;
                };

                let travel = position.y - drag.origin.y;

                if travel.abs() > DRAG_THRESHOLD {
                    drag.moved = true;
                }

                if drag.moved {
                    let wheels = self.wheels();
                    let kind = wheels.columns()[drag.column];
                    let wheel = &mut state.groups[drag.group][drag.column];
                    let mut position = drag.from - travel / self.row_height;

                    if !kind.wraps() {
                        position = position.clamp(0.0, (wheels.len(kind) - 1) as f32);
                    }

                    wheel.position = position;
                    wheel.tween = None;
                    shell.request_redraw();
                }

                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let Some(drag) = state.drag.take() else {
                    return;
                };

                let wheel = state.groups[drag.group][drag.column];
                let target = if drag.moved {
                    wheel.position.round() as i64
                } else {
                    // A click brings the clicked item to the middle.
                    let centre = geometry.wheel(drag.group, drag.column).center_y();
                    let rows = ((drag.origin.y - centre) / self.row_height).round() as i64;

                    wheel.target + rows
                };

                if let Some(message) =
                    self.settle(state, drag.group, drag.column, target, Instant::now())
                {
                    shell.publish(message);
                }

                shell.capture_event();
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let Some((group, column)) = cursor
                    .position_over(bounds)
                    .and_then(|position| geometry.wheel_at(position))
                else {
                    return;
                };

                let steps = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => {
                        state.scroll = 0.0;

                        if y.abs() < 1.0 {
                            -y.signum() as i64
                        } else {
                            -y.round() as i64
                        }
                    }
                    mouse::ScrollDelta::Pixels { y, .. } => {
                        state.scroll -= y;
                        let steps = (state.scroll / self.row_height).trunc();
                        state.scroll -= steps * self.row_height;
                        steps as i64
                    }
                };

                if steps != 0 {
                    let target = state.groups[group][column].target + steps;

                    state.focus = (group, column);

                    if let Some(message) = self.settle(state, group, column, target, Instant::now())
                    {
                        shell.publish(message);
                    }

                    shell.request_redraw();
                }

                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(named),
                ..
            }) if state.is_focused => {
                use keyboard::key::Named;

                let (group, column) = state.focus;
                let columns = self.wheels().columns().len();

                match named {
                    Named::ArrowUp | Named::ArrowDown => {
                        let step = if *named == Named::ArrowUp { -1 } else { 1 };
                        let target = state.groups[group][column].target + step;

                        if let Some(message) =
                            self.settle(state, group, column, target, Instant::now())
                        {
                            shell.publish(message);
                        }
                    }
                    Named::ArrowLeft | Named::ArrowRight => {
                        let flat = group * columns + column;
                        let total = self.groups() * columns;
                        let flat = if *named == Named::ArrowLeft {
                            (flat + total - 1) % total
                        } else {
                            (flat + 1) % total
                        };

                        state.focus = (flat / columns, flat % columns);
                    }
                    Named::Escape => state.is_focused = false,
                    _ => return,
                }

                shell.capture_event();
                shell.request_redraw();
            }
            _ => {}
        }

        if state.is_animating() {
            shell.request_redraw();
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

        if state.drag.is_some_and(|drag| drag.moved) {
            return mouse::Interaction::Grabbing;
        }

        let over_wheel = cursor
            .position_over(layout.bounds())
            .and_then(|position| self.geometry(layout.bounds()).wheel_at(position))
            .is_some();

        if over_wheel {
            mouse::Interaction::Grab
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
        let style = theme.style(&self.class);
        let geometry = self.geometry(bounds);
        let wheels = self.wheels();
        let columns = wheels.columns();
        let font = self.font.unwrap_or_else(|| renderer.font());
        let size = self.number_size();
        let text_color = if style.text_color.a > 0.0 {
            style.text_color
        } else {
            defaults.text_color
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

        for (group, group_geometry) in geometry.groups.iter().enumerate() {
            let Some(wheel_states) = state.groups.get(group) else {
                continue;
            };

            if let Some(label) = group_geometry.label {
                fill_text(
                    renderer,
                    &self.labels[group],
                    font,
                    size * 0.7,
                    label,
                    style.label_text_color,
                    *viewport,
                );
            }

            // The band marking the picked row, behind the wheels.
            renderer.fill_quad(
                renderer::Quad {
                    bounds: group_geometry.band,
                    border: style.highlight_border,
                    ..renderer::Quad::default()
                },
                style.highlight_background,
            );

            for separator in &group_geometry.separators {
                fill_text(
                    renderer,
                    ":",
                    font,
                    size,
                    *separator,
                    style.separator_color,
                    *viewport,
                );
            }

            let indices = state.indices(group, &wheels);

            // Rows that turn out of the wheel area are cut off at its edge.
            renderer.with_layer(group_geometry.area, |renderer| {
                for (column, (&kind, wheel)) in columns.iter().zip(wheel_states).enumerate() {
                    let rect = group_geometry.wheels[column];
                    let centre = rect.center_y();
                    let reach = self.rows as i64 / 2 + 1;
                    let nearest = wheel.position.round() as i64;

                    for item in nearest - reach..=nearest + reach {
                        if !kind.wraps() && !(0..wheels.len(kind) as i64).contains(&item) {
                            continue;
                        }

                        let offset = item as f32 - wheel.position;
                        let distance = offset.abs();
                        let cell = Rectangle {
                            x: rect.x,
                            y: centre - self.row_height / 2.0 + offset * self.row_height,
                            width: rect.width,
                            height: self.row_height,
                        };

                        let index = wheels.index(kind, item);
                        let mut candidate = indices.clone();
                        candidate[column] = index;
                        let enabled = self.is_enabled(wheels.compose(&candidate));

                        let color = if !enabled {
                            style.disabled_text_color
                        } else if distance < 0.5 {
                            text_color
                        } else {
                            style.muted_text_color
                        };

                        // Rows fade towards the edges, like a drum turning away.
                        let fade =
                            (1.0 - distance / (self.rows as f32 / 2.0 + 0.5)).clamp(0.0, 1.0);
                        let color = color.scale_alpha(0.25 + 0.75 * fade);

                        fill_text(
                            renderer,
                            &wheels.label(kind, index),
                            font,
                            size,
                            cell,
                            color,
                            *viewport,
                        );
                    }

                    if state.is_focused && state.focus == (group, column) {
                        let band = Rectangle {
                            x: rect.x + 2.0,
                            width: rect.width - 4.0,
                            ..group_geometry.band
                        };

                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: band,
                                border: Border {
                                    color: style.focus_border,
                                    width: 1.0,
                                    radius: style.highlight_border.radius,
                                },
                                ..renderer::Quad::default()
                            },
                            Color::TRANSPARENT,
                        );
                    }
                }
            });
        }

        if let Some(dash) = geometry.dash {
            fill_text(
                renderer,
                "–",
                font,
                size,
                dash,
                style.separator_color,
                *viewport,
            );
        }
    }
}

fn fill_text<Renderer: text::Renderer>(
    renderer: &mut Renderer,
    content: &str,
    font: Font,
    size: f32,
    rect: Rectangle,
    color: Color,
    clip: Rectangle,
) {
    if color.a == 0.0 {
        return;
    }

    renderer.fill_text(
        iced::advanced::Text {
            content: content.to_owned(),
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
        clip,
    );
}

// ── Geometry ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct GroupGeometry {
    label: Option<Rectangle>,
    /// Every wheel and separator of the group: the clip for its rows.
    area: Rectangle,
    wheels: Vec<Rectangle>,
    separators: Vec<Rectangle>,
    /// The band behind the middle row.
    band: Rectangle,
}

#[derive(Debug, Clone)]
struct Geometry {
    groups: Vec<GroupGeometry>,
    /// Where the dash between a range's start and end goes.
    dash: Option<Rectangle>,
}

impl Geometry {
    fn group_width(wheels: &Wheels, column_width: f32) -> f32 {
        wheels
            .columns()
            .iter()
            .map(|kind| match kind {
                Kind::Hour => column_width,
                Kind::Minute | Kind::Second => SEPARATOR_WIDTH + column_width,
                Kind::Period => PERIOD_GAP + column_width,
            })
            .sum()
    }

    fn size(
        wheels: &Wheels,
        groups: usize,
        labels: bool,
        rows: usize,
        row_height: f32,
        column_width: f32,
    ) -> Size {
        let group = Self::group_width(wheels, column_width);

        Size::new(
            PADDING * 2.0 + group * groups as f32 + RANGE_GAP * (groups - 1) as f32,
            PADDING * 2.0 + if labels { LABEL_HEIGHT } else { 0.0 } + rows as f32 * row_height,
        )
    }

    fn new(
        bounds: Rectangle,
        wheels: &Wheels,
        groups: usize,
        labels: bool,
        rows: usize,
        row_height: f32,
        column_width: f32,
    ) -> Self {
        let group_width = Self::group_width(wheels, column_width);
        let content = Self::size(wheels, groups, labels, rows, row_height, column_width);
        // Centre the content in whatever space the layout gave.
        let left = bounds.x + (bounds.width - content.width) / 2.0 + PADDING;
        let top = bounds.y + (bounds.height - content.height) / 2.0 + PADDING;
        let label_height = if labels { LABEL_HEIGHT } else { 0.0 };
        let wheel_top = top + label_height;
        let wheel_height = rows as f32 * row_height;

        let groups_geometry = (0..groups)
            .map(|group| {
                let x0 = left + group as f32 * (group_width + RANGE_GAP);
                let mut x = x0;
                let mut wheel_rects = Vec::new();
                let mut separators = Vec::new();

                for kind in wheels.columns() {
                    match kind {
                        Kind::Hour => {}
                        Kind::Minute | Kind::Second => {
                            separators.push(Rectangle {
                                x,
                                y: wheel_top + (wheel_height - row_height) / 2.0,
                                width: SEPARATOR_WIDTH,
                                height: row_height,
                            });
                            x += SEPARATOR_WIDTH;
                        }
                        Kind::Period => x += PERIOD_GAP,
                    }

                    wheel_rects.push(Rectangle {
                        x,
                        y: wheel_top,
                        width: column_width,
                        height: wheel_height,
                    });
                    x += column_width;
                }

                GroupGeometry {
                    label: labels.then_some(Rectangle {
                        x: x0,
                        y: top,
                        width: group_width,
                        height: LABEL_HEIGHT,
                    }),
                    area: Rectangle {
                        x: x0,
                        y: wheel_top,
                        width: group_width,
                        height: wheel_height,
                    },
                    wheels: wheel_rects,
                    separators,
                    band: Rectangle {
                        x: x0,
                        y: wheel_top + (wheel_height - row_height) / 2.0,
                        width: group_width,
                        height: row_height,
                    },
                }
            })
            .collect();

        Self {
            groups: groups_geometry,
            dash: (groups > 1).then(|| Rectangle {
                x: left + group_width,
                y: wheel_top + (wheel_height - row_height) / 2.0,
                width: RANGE_GAP,
                height: row_height,
            }),
        }
    }

    fn wheel(&self, group: usize, column: usize) -> Rectangle {
        self.groups[group].wheels[column]
    }

    fn wheel_at(&self, position: Point) -> Option<(usize, usize)> {
        self.groups
            .iter()
            .enumerate()
            .find_map(|(group, geometry)| {
                geometry
                    .wheels
                    .iter()
                    .position(|wheel| wheel.contains(position))
                    .map(|column| (group, column))
            })
    }
}

// ── Style ────────────────────────────────────────────────────────────────────

/// The appearance of a [`TimePicker`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The picker's own background. `None` by default: inside a popover or
    /// dialog the surface around it already has one.
    pub background: Option<Background>,
    /// The border around the picker.
    pub border: Border,
    /// The picked row. A fully transparent color inherits the surrounding
    /// text color.
    pub text_color: Color,
    /// The rows around the picked one.
    pub muted_text_color: Color,
    /// Rows that cannot be picked.
    pub disabled_text_color: Color,
    /// The band behind the picked row.
    pub highlight_background: Color,
    /// The border of that band; its radius also rounds the focus ring.
    pub highlight_border: Border,
    /// The `:` between wheels and the dash in a range.
    pub separator_color: Color,
    /// The start and end labels of a range.
    pub label_text_color: Color,
    /// The ring around the wheel the keyboard turns.
    pub focus_border: Color,
}

/// A styling function for a [`TimePicker`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`TimePicker`].
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

/// The default style of a [`TimePicker`], from the theme's palette.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();
    let text = palette.background.base.text;

    Style {
        background: None,
        border: Border::default(),
        text_color: text,
        muted_text_color: text.scale_alpha(0.55),
        disabled_text_color: text.scale_alpha(0.2),
        highlight_background: text.scale_alpha(0.08),
        highlight_border: Border::default().rounded(8.0),
        separator_color: text.scale_alpha(0.7),
        label_text_color: text.scale_alpha(0.6),
        focus_border: palette.primary.base.color,
    }
}

/// A [`default`] time picker on a card of its own, for use inline on a page.
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

    fn time(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    const TWELVE: Wheels = Wheels {
        clock: Clock::TwelveHour,
        minute_step: 1,
        seconds: false,
    };

    const TWENTY_FOUR: Wheels = Wheels {
        clock: Clock::TwentyFourHour,
        minute_step: 15,
        seconds: true,
    };

    fn roundtrip(wheels: &Wheels, t: NaiveTime) -> NaiveTime {
        let indices: Vec<usize> = wheels
            .columns()
            .into_iter()
            .map(|kind| wheels.index_of(kind, t))
            .collect();

        wheels.compose(&indices)
    }

    #[test]
    fn twelve_hour_wheels_round_trip_midnight_and_noon() {
        for t in [
            time(0, 5),
            time(12, 0),
            time(23, 59),
            time(9, 30),
            time(13, 1),
        ] {
            assert_eq!(roundtrip(&TWELVE, t), t);
        }

        assert_eq!(
            TWELVE.label(Kind::Hour, TWELVE.index_of(Kind::Hour, time(0, 0))),
            "12"
        );
        assert_eq!(
            TWELVE.label(Kind::Period, TWELVE.index_of(Kind::Period, time(0, 0))),
            "AM"
        );
        assert_eq!(
            TWELVE.label(Kind::Period, TWELVE.index_of(Kind::Period, time(12, 0))),
            "PM"
        );
    }

    #[test]
    fn minute_steps_show_the_step_before() {
        assert_eq!(TWENTY_FOUR.len(Kind::Minute), 4);
        assert_eq!(roundtrip(&TWENTY_FOUR, time(10, 44)), time(10, 30));
        assert_eq!(TWENTY_FOUR.label(Kind::Minute, 3), "45");
        assert_eq!(
            TWENTY_FOUR.columns(),
            vec![Kind::Hour, Kind::Minute, Kind::Second]
        );
    }

    #[test]
    fn wrapping_wheels_turn_the_short_way_round() {
        // At 59 minutes, 00 is one step on, not 59 back.
        assert_eq!(nearest_turn(59, 0, 60), 60);
        assert_eq!(nearest_turn(60, 59, 60), 59);
        assert_eq!(nearest_turn(-1, 3, 12), 3);
        assert_eq!(nearest_turn(130, 10, 60), 130);
    }

    #[test]
    fn a_range_shorter_than_the_minimum_pushes_its_end() {
        let range = TimeRange::new(time(9, 0), time(9, 10));
        let kept = keep_duration(range, Some(Duration::minutes(30)), None);

        assert_eq!(kept, TimeRange::new(time(9, 0), time(9, 30)));
    }

    #[test]
    fn a_range_longer_than_the_maximum_pulls_its_end() {
        let range = TimeRange::new(time(9, 0), time(18, 0));
        let kept = keep_duration(range, None, Some(Duration::hours(4)));

        assert_eq!(kept, TimeRange::new(time(9, 0), time(13, 0)));
    }

    #[test]
    fn ranges_run_past_midnight() {
        let night = TimeRange::new(time(22, 0), time(6, 0));

        assert!(night.crosses_midnight());
        assert_eq!(night.duration(), Duration::hours(8));
        assert_eq!(
            keep_duration(night, Some(Duration::hours(1)), Some(Duration::hours(9))),
            night
        );

        let pushed = keep_duration(
            TimeRange::new(time(23, 0), time(23, 30)),
            Some(Duration::hours(2)),
            None,
        );
        assert_eq!(pushed.end, time(1, 0));
    }

    /// Settles wheel `column` of a single picker showing `value` on
    /// `target`, through the widget itself, and returns what it reports.
    fn turn(picker: TimePicker<'_, NaiveTime>, column: usize, steps: i64) -> Option<NaiveTime> {
        let mut picker = picker;
        let mut tree = Tree::new::<NaiveTime, Theme, ()>(&picker);
        <TimePicker<'_, NaiveTime> as Widget<NaiveTime, Theme, ()>>::diff(&mut picker, &mut tree);

        let state = tree.state.downcast_mut::<State>();
        let target = state.groups[0][column].target + steps;

        picker.settle(state, 0, column, target, Instant::now())
    }

    #[test]
    fn the_hour_wheel_carries_into_am_and_pm() {
        // 11 AM on by one is noon; noon back by one is 11 AM.
        assert_eq!(
            turn(time_picker(Some(time(11, 0)), |t| t), 0, 1),
            Some(time(12, 0))
        );
        assert_eq!(
            turn(time_picker(Some(time(12, 0)), |t| t), 0, -1),
            Some(time(11, 0))
        );
        // 11 PM on by one is midnight.
        assert_eq!(
            turn(time_picker(Some(time(23, 0)), |t| t), 0, 1),
            Some(time(0, 0))
        );
        // Within a half-day nothing flips.
        assert_eq!(
            turn(time_picker(Some(time(13, 0)), |t| t), 0, 2),
            Some(time(15, 0))
        );
    }

    #[test]
    fn a_disabled_time_steers_the_way_the_wheel_was_turning() {
        let lunch = |t: NaiveTime| t.hour() == 12;

        assert_eq!(
            turn(time_picker(Some(time(9, 0)), |t| t).disabled(lunch), 0, 3),
            Some(time(13, 0))
        );
        assert_eq!(
            turn(time_picker(Some(time(15, 0)), |t| t).disabled(lunch), 0, -3),
            Some(time(11, 0))
        );
    }

    #[test]
    fn a_click_lands_on_the_wheel_under_it() {
        let size = Geometry::size(&TWELVE, 2, true, 5, 36.0, 56.0);
        let geometry = Geometry::new(
            Rectangle::new(Point::ORIGIN, size),
            &TWELVE,
            2,
            true,
            5,
            36.0,
            56.0,
        );

        let minute = geometry.wheel(1, 1);

        assert_eq!(geometry.wheel_at(minute.center()), Some((1, 1)));
        assert_eq!(geometry.wheel_at(geometry.dash.unwrap().center()), None);
    }
}
