//! A stack of transient notifications anchored to a viewport corner.
//!
//! [`crate::layer`] shows one surface. Toasts need more than that: several at
//! once, each with its own lifetime, and the ones below sliding up to close the
//! gap when one expires. That is a typed list with per-item behaviour, which is
//! why this is its own widget rather than a `layer` with a column in it — the
//! same reason [`crate::menu`] is not a [`crate::popover`] full of rows.
//!
//! ```rust,ignore
//! toasts(
//!     self.notices
//!         .iter()
//!         .map(|notice| {
//!             success(notice.id, text(&notice.text))
//!                 .timeout(Duration::from_secs(4))
//!                 .on_close(Message::Dismissed(notice.id))
//!         })
//!         .collect(),
//! )
//! .anchor(Anchor::BottomRight)
//! ```
//!
//! # Identity
//!
//! Every toast carries a caller-supplied `id`. Without one the widget could
//! only track toasts by position, and removing the second of four would look
//! like the third and fourth each becoming a different toast — the entering,
//! leaving and sliding would all be attributed to the wrong rows. The `id` is
//! what lets a toast keep its countdown and its place across a view rebuild.
//!
//! # Lifetime
//!
//! A toast never removes itself. When its timer runs out it publishes
//! [`Toast::on_close`] and keeps showing until the application drops it from
//! the list, exactly as if the close button had been pressed. The application
//! owns the list; the widget only ever reports.
//!
//! # Countdown
//!
//! A toast with a timeout draws a bar along its bottom edge that shrinks as the
//! time runs down. A bar rather than a ring around the close button: the
//! renderer available here can fill rounded rectangles and nothing else, so an
//! arc has to be faked with a circle of dots, and a single smooth quad reads
//! better than a coarse dotted ring at notification sizes.

use iced::advanced::widget::{self, Operation, tree::Tree};
use iced::advanced::{
    Layout, Shell, Widget,
    layout::{Limits, Node},
    overlay, renderer, text,
};
use iced::time::{Duration, Instant};
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Theme,
    Vector, mouse, touch, window,
};

use crate::anchor::Anchor;
use crate::animation::{self, Motion, Transition};
use crate::lucide;

/// The default margin between the stack and the viewport edge.
const DEFAULT_MARGIN: f32 = 16.0;

/// The default gap between stacked toasts.
const DEFAULT_SPACING: f32 = 10.0;

/// The default padding inside a toast.
const DEFAULT_PADDING: f32 = 14.0;

/// The default corner radius of a toast.
const DEFAULT_RADIUS: f32 = 8.0;

/// The default width of a toast.
const DEFAULT_WIDTH: f32 = 360.0;

/// The thickness of the countdown bar.
const COUNTDOWN_HEIGHT: f32 = 3.0;

/// The space between an icon or close button and the toast content.
const ICON_SPACING: f32 = 10.0;

/// Creates a stack of [`Toast`]s.
pub fn toasts<'a, Message, Theme, Renderer>(
    items: Vec<Toast<'a, Message, Theme, Renderer>>,
) -> Toasts<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Toasts::new(items)
}

/// Creates a neutral [`Toast`].
pub fn toast<'a, Message, Theme, Renderer>(
    id: u64,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Toast<'a, Message, Theme, Renderer> {
    Toast::new(id, content)
}

/// Creates an informational [`Toast`].
pub fn info<'a, Message, Theme, Renderer>(
    id: u64,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Toast<'a, Message, Theme, Renderer> {
    Toast::new(id, content).variant(Variant::Info)
}

/// Creates a success [`Toast`].
pub fn success<'a, Message, Theme, Renderer>(
    id: u64,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Toast<'a, Message, Theme, Renderer> {
    Toast::new(id, content).variant(Variant::Success)
}

/// Creates a warning [`Toast`].
pub fn warning<'a, Message, Theme, Renderer>(
    id: u64,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Toast<'a, Message, Theme, Renderer> {
    Toast::new(id, content).variant(Variant::Warning)
}

/// Creates a danger [`Toast`].
pub fn danger<'a, Message, Theme, Renderer>(
    id: u64,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Toast<'a, Message, Theme, Renderer> {
    Toast::new(id, content).variant(Variant::Danger)
}

/// The semantic appearance of a [`Toast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// Neutral: uses the theme's ordinary foreground.
    #[default]
    Neutral,
    /// Informational.
    Info,
    /// Something needs attention.
    Warning,
    /// Something worked.
    Success,
    /// Something failed.
    Danger,
}

impl Variant {
    /// Returns the Lucide glyph drawn alongside a toast of this variant.
    fn icon(self) -> Option<lucide::Icon> {
        match self {
            Self::Neutral => None,
            Self::Info => Some(lucide::Icon::Info),
            Self::Warning => Some(lucide::Icon::TriangleAlert),
            Self::Success => Some(lucide::Icon::CircleCheck),
            Self::Danger => Some(lucide::Icon::CircleX),
        }
    }
}

/// A single transient notification.
#[allow(missing_debug_implementations)]
pub struct Toast<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    id: u64,
    content: Element<'a, Message, Theme, Renderer>,
    variant: Variant,
    show_icon: bool,
    timeout: Option<Duration>,
    on_close: Option<Message>,
}

impl<'a, Message, Theme, Renderer> Toast<'a, Message, Theme, Renderer> {
    /// Creates a new [`Toast`] with the given stable `id`.
    ///
    /// See the [module documentation](self) on why the id is required.
    pub fn new(id: u64, content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        // So the variant icon renders even if the application never registered
        // `lucide::FONT_BYTES` itself.
        lucide::ensure_loaded();

        Self {
            id,
            content: content.into(),
            variant: Variant::default(),
            show_icon: true,
            timeout: None,
            on_close: None,
        }
    }

    /// Sets the semantic appearance of the [`Toast`].
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Hides the variant icon.
    pub fn without_icon(mut self) -> Self {
        self.show_icon = false;
        self
    }

    /// Sets how long the toast shows before asking to be closed.
    ///
    /// Also turns on the countdown bar. Without a timeout the toast stays until
    /// the application removes it.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the message published when the toast asks to be closed.
    ///
    /// Published both when the timer runs out and when the close button is
    /// pressed. The toast does not remove itself — see the
    /// [module documentation](self).
    pub fn on_close(mut self, message: Message) -> Self {
        self.on_close = Some(message);
        self
    }
}

/// A stack of [`Toast`]s anchored to a viewport corner.
#[allow(missing_debug_implementations)]
pub struct Toasts<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    items: Vec<Toast<'a, Message, Theme, Renderer>>,
    anchor: Anchor,
    margin: f32,
    spacing: f32,
    padding: f32,
    radius: f32,
    width: f32,
    motion: Motion,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Toasts<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Creates a new stack of toasts.
    pub fn new(items: Vec<Toast<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            id: None,
            items,
            anchor: Anchor::BottomRight,
            margin: DEFAULT_MARGIN,
            spacing: DEFAULT_SPACING,
            padding: DEFAULT_PADDING,
            radius: DEFAULT_RADIUS,
            width: DEFAULT_WIDTH,
            motion: Motion::SMOOTH,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the [`widget::Id`] of the stack.
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the viewport corner the stack grows from.
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets the margin between the stack and the viewport edge.
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Sets the gap between stacked toasts.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Sets the padding inside each toast.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the corner radius of each toast.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Sets the width of each toast.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Sets how toasts animate in, out, and into their new places.
    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// Shows and hides toasts instantly, with no transitions.
    pub fn no_animation(mut self) -> Self {
        self.motion = Motion::NONE;
        self
    }

    /// Sets the style of the toasts.
    pub fn style(mut self, style: impl Fn(&Theme, Variant) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the toasts.
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Returns `true` when the stack grows upwards from its anchor.
    ///
    /// A stack at the bottom of the screen puts its newest toast lowest and
    /// pushes older ones up; at the top it does the reverse.
    fn grows_upward(&self) -> bool {
        matches!(
            self.anchor,
            Anchor::BottomLeft | Anchor::Bottom | Anchor::BottomRight
        )
    }
}

/// What the widget remembers about one live toast.
#[derive(Debug)]
struct Entry {
    /// The caller's id, which is what ties this to a toast across rebuilds.
    id: u64,
    /// The show/hide transition.
    transition: Transition,
    /// When the toast first appeared, for the countdown.
    born: Instant,
    /// Where the toast currently sits along the stack, animating towards its
    /// place as the toasts around it come and go.
    offset: Reflow,
    /// Whether the timeout has already been reported, so it fires once.
    expired: bool,
}

/// A one-dimensional slide towards a target, used for stack reflow.
///
/// `Transition` animates between two fixed ends; this has to chase a moving
/// target, since the place a toast belongs changes whenever one above it goes.
#[derive(Debug)]
struct Reflow {
    from: f32,
    to: f32,
    started: Instant,
    duration: Duration,
}

impl Reflow {
    /// Creates a [`Reflow`] already settled at `value`.
    fn settled(value: f32, now: Instant) -> Self {
        Self {
            from: value,
            to: value,
            started: now,
            duration: Duration::ZERO,
        }
    }

    /// Points the slide at a new target, starting from wherever it is now.
    fn retarget(&mut self, to: f32, now: Instant, duration: Duration) {
        if (self.to - to).abs() < f32::EPSILON {
            return;
        }

        self.from = self.value(now);
        self.to = to;
        self.started = now;
        self.duration = duration;
    }

    /// Returns the current position.
    fn value(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return self.to;
        }

        let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
        let progress = (elapsed / self.duration.as_secs_f32()).clamp(0.0, 1.0);

        // Ease out, to match the surface transitions.
        let eased = 1.0 - (1.0 - progress).powi(3);

        self.from + (self.to - self.from) * eased
    }

    /// Returns `true` while the slide is still moving.
    fn is_animating(&self, now: Instant) -> bool {
        !self.duration.is_zero()
            && now.saturating_duration_since(self.started) < self.duration
            && (self.to - self.from).abs() > f32::EPSILON
    }
}

/// The persistent state of a [`Toasts`] stack.
#[derive(Debug, Default)]
struct State {
    entries: Vec<Entry>,
}

impl State {
    /// Returns the entry for `id`, if the widget already knows about it.
    fn entry(&self, id: u64) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Returns `true` while anything in the stack is still moving.
    fn is_animating(&self, now: Instant) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.transition.is_animating(now) || entry.offset.is_animating(now))
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Toasts<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != self.items.len() {
            tree.children = self
                .items
                .iter()
                .map(|item| Tree::new(item.content.as_widget()))
                .collect();
        }

        for (item, child) in self.items.iter_mut().zip(tree.children.iter_mut()) {
            child.diff(item.content.as_widget_mut());
        }

        // Reconcile the entries against the list the application just handed
        // us: adopt new ids, and start fading out any the application dropped.
        let state = tree.state.downcast_mut::<State>();
        let now = Instant::now();

        for item in &self.items {
            if state.entry(item.id).is_none() {
                let mut transition = Transition::new();
                transition.sync(self.motion);
                transition.open(now);

                state.entries.push(Entry {
                    id: item.id,
                    transition,
                    born: now,
                    offset: Reflow::settled(0.0, now),
                    expired: false,
                });
            }
        }

        for entry in &mut state.entries {
            if !self.items.iter().any(|item| item.id == entry.id) {
                entry.transition.close(now);
            }
        }

        // Drop the ones that have finished fading out.
        state
            .entries
            .retain(|entry| entry.transition.is_visible(now));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        // The stack lives entirely in an overlay, so the host takes no room.
        Node::new(Size::ZERO)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Window(window::Event::RedrawRequested(now)) = event else {
            return;
        };

        let now = *now;
        let state = tree.state.downcast_mut::<State>();

        // Report every toast whose time is up, once each. The application
        // removes it; the widget only ever asks.
        let mut expired = Vec::new();

        for item in &self.items {
            let Some(timeout) = item.timeout else {
                continue;
            };

            if let Some(entry) = state.entries.iter_mut().find(|entry| entry.id == item.id)
                && !entry.expired
                && now.saturating_duration_since(entry.born) >= timeout
            {
                entry.expired = true;
                expired.push(item.id);
            }
        }

        for id in expired {
            if let Some(message) = self
                .items
                .iter()
                .find(|item| item.id == id)
                .and_then(|item| item.on_close.clone())
            {
                shell.publish(message);
            }
        }

        // A running countdown needs a frame to move on, and so does a reflow.
        if state.is_animating(now) || self.items.iter().any(|item| item.timeout.is_some()) {
            shell.request_redraw();
        }
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(self.id.as_ref(), layout.bounds());
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        _offset: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        // Read before `self` is split apart for the overlay.
        let grows_upward = self.grows_upward();

        let state = tree.state.downcast_mut::<State>();

        if state.entries.is_empty() {
            return None;
        }

        Some(overlay::Element::new(Box::new(Stack {
            state,
            items: &mut self.items,
            trees: &mut tree.children,
            anchor: self.anchor,
            margin: self.margin,
            spacing: self.spacing,
            padding: self.padding,
            radius: self.radius,
            width: self.width,
            motion: self.motion,
            grows_upward,
            class: &self.class,
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Toasts<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font> + 'a,
{
    fn from(toasts: Toasts<'a, Message, Theme, Renderer>) -> Self {
        Element::new(toasts)
    }
}

/// The stack of toasts, as an iced overlay.
struct Stack<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    state: &'a mut State,
    items: &'a mut Vec<Toast<'b, Message, Theme, Renderer>>,
    trees: &'a mut Vec<Tree>,
    anchor: Anchor,
    margin: f32,
    spacing: f32,
    padding: f32,
    radius: f32,
    width: f32,
    motion: Motion,
    grows_upward: bool,
    class: &'a Theme::Class<'b>,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Stack<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = iced::Font>,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let viewport = Rectangle::with_size(bounds);
        let now = Instant::now();

        let width = self.width.min(bounds.width - self.margin * 2.0);
        let glyph = glyph_size(renderer);

        // Each toast is laid out at the stack width, then given its place along
        // the stack. The offsets animate, so a toast that loses a neighbour
        // slides into the gap instead of jumping.
        let mut nodes = Vec::with_capacity(self.items.len());
        let mut extent = 0.0_f32;

        for (item, tree) in self.items.iter_mut().zip(self.trees.iter_mut()) {
            let indent = icon_indent(item, glyph);
            let content_width = (width - self.padding * 2.0 - indent).max(0.0);

            let limits = Limits::new(
                Size::new(content_width, 0.0),
                Size::new(content_width, bounds.height),
            );

            let content = item.content.as_widget_mut().layout(tree, renderer, &limits);
            let height = content.size().height + self.padding * 2.0 + COUNTDOWN_HEIGHT;

            nodes.push((item.id, height, content.move_to(Point::new(indent, 0.0))));
            extent += height + self.spacing;
        }

        // Position along the stack axis, measured from the anchored edge.
        let mut cursor = 0.0_f32;
        let mut placed = Vec::with_capacity(nodes.len());

        for (id, height, content) in nodes {
            let target = cursor;
            cursor += height + self.spacing;

            let offset = if let Some(entry) = self
                .state
                .entries
                .iter_mut()
                .find(|entry| entry.id == id)
            {
                entry
                    .offset
                    .retarget(target, now, self.motion.duration);
                entry.offset.value(now)
            } else {
                target
            };

            placed.push((height, offset, content));
        }

        let total = Size::new(width, (extent - self.spacing).max(0.0));
        let origin = self.anchor.position(total, viewport, self.margin);

        let children = placed
            .into_iter()
            .map(|(height, offset, content)| {
                // Growing upward means the first toast sits at the bottom of
                // the stack, so offsets run backwards from the far edge.
                let y = if self.grows_upward {
                    total.height - offset - height
                } else {
                    offset
                };

                Node::with_children(
                    Size::new(width, height),
                    vec![content.translate(Vector::new(self.padding, self.padding))],
                )
                .move_to(Point::new(origin.x, origin.y + y))
            })
            .collect();

        Node::with_children(total, children).move_to(Point::ORIGIN)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let now = Instant::now();

        for ((item, tree), toast_layout) in
            self.items.iter().zip(self.trees.iter()).zip(layout.children())
        {
            let Some(entry) = self.state.entry(item.id) else {
                continue;
            };

            let progress = entry.transition.progress(now);
            let style = theme.style(self.class, item.variant);
            let bounds = toast_layout.bounds();

            // Slide only, as everywhere else: a toast holds arbitrary content,
            // and children that paint themselves would not fade with it.
            let slide = animation::slide(self.anchor.slide_from(), self.motion.slide, progress);
            let bounds = Rectangle {
                x: bounds.x + slide.x,
                y: bounds.y + slide.y,
                ..bounds
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: self.radius.into(),
                        ..style.border
                    },
                    shadow: style.shadow,
                    ..renderer::Quad::default()
                },
                style.background,
            );

            // The countdown, along the bottom edge.
            if let Some(timeout) = item.timeout {
                let elapsed = now.saturating_duration_since(entry.born).as_secs_f32();
                let remaining =
                    (1.0 - elapsed / timeout.as_secs_f32().max(f32::EPSILON)).clamp(0.0, 1.0);

                let track = countdown_track(bounds, self.radius);

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: track.x,
                            y: bounds.y + bounds.height - COUNTDOWN_HEIGHT,
                            width: track.width * remaining,
                            height: COUNTDOWN_HEIGHT,
                        },
                        border: Border {
                            radius: (COUNTDOWN_HEIGHT / 2.0).into(),
                            ..Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    style.countdown,
                );
            }

            if let Some(icon) = item.variant.icon().filter(|_| item.show_icon) {
                let glyph = glyph_size(renderer);

                let slot = Rectangle {
                    x: bounds.x + self.padding,
                    y: bounds.y + self.padding,
                    width: glyph,
                    height: glyph,
                };

                renderer.fill_text(
                    text::Text {
                        content: icon.character().to_string(),
                        font: lucide::FONT,
                        size: iced::Pixels(glyph),
                        line_height: text::LineHeight::default(),
                        bounds: slot.size(),
                        align_x: text::Alignment::Center,
                        align_y: iced::alignment::Vertical::Center,
                        shaping: text::Shaping::Basic,
                        wrapping: text::Wrapping::None,
                        ellipsis: text::Ellipsis::default(),
                        hint_factor: None,
                    },
                    slot.center(),
                    style.countdown_color(),
                    bounds,
                );
            }

            let content_layout = toast_layout.children().next().expect("toast content");

            renderer.with_translation(slide, |renderer| {
                item.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    &renderer::Style {
                        text_color: style.text_color,
                    },
                    content_layout,
                    cursor,
                    &bounds,
                );
            });
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        for ((item, tree), toast_layout) in self
            .items
            .iter_mut()
            .zip(self.trees.iter_mut())
            .zip(layout.children())
        {
            let bounds = toast_layout.bounds();

            item.content.as_widget_mut().update(
                tree,
                event,
                toast_layout.children().next().expect("toast content"),
                cursor,
                renderer,
                shell,
                &bounds,
            );

            if shell.is_event_captured() {
                return;
            }
        }

        // A press on a toast is swallowed so it cannot fall through to the page
        // the stack is floating over.
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(_))
                | Event::Touch(touch::Event::FingerPressed { .. })
        ) && layout.children().any(|toast| cursor.is_over(toast.bounds()))
        {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.items
            .iter()
            .zip(self.trees.iter())
            .zip(layout.children())
            .map(|((item, tree), toast_layout)| {
                item.content.as_widget().mouse_interaction(
                    tree,
                    toast_layout.children().next().expect("toast content"),
                    cursor,
                    &toast_layout.bounds(),
                    renderer,
                )
            })
            .max()
            .unwrap_or_default()
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        for ((item, tree), toast_layout) in self
            .items
            .iter_mut()
            .zip(self.trees.iter_mut())
            .zip(layout.children())
        {
            item.content.as_widget_mut().operate(
                tree,
                toast_layout.children().next().expect("toast content"),
                renderer,
                operation,
            );
        }
    }
}

/// Returns the horizontal span the countdown bar runs along.
///
/// A rounded toast only has a straight bottom edge *between* its corner arcs,
/// so the bar is inset by the corner radius at each end and stops where the
/// rounding starts. With square corners the inset is zero and the bar runs the
/// full width, corner to corner.
///
/// Only `x` and `width` of the result are meaningful.
fn countdown_track(bounds: Rectangle, radius: f32) -> Rectangle {
    let inset = radius.clamp(0.0, bounds.width / 2.0);

    Rectangle {
        x: bounds.x + inset,
        width: (bounds.width - inset * 2.0).max(0.0),
        ..bounds
    }
}

/// Returns the size of a toast's variant glyph.
fn glyph_size<Renderer>(renderer: &Renderer) -> f32
where
    Renderer: text::Renderer,
{
    renderer.default_size().0 * 1.15
}

/// Returns how far a toast's content is indented past its variant glyph.
fn icon_indent<Message, Theme, Renderer>(
    item: &Toast<'_, Message, Theme, Renderer>,
    glyph: f32,
) -> f32 {
    if item.show_icon && item.variant.icon().is_some() {
        glyph + ICON_SPACING
    } else {
        0.0
    }
}

/// The appearance of a [`Toast`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the toast.
    pub background: Background,
    /// The [`Border`] of the toast. Its radius is overridden by
    /// [`Toasts::radius`].
    pub border: Border,
    /// The [`Shadow`] cast by the toast.
    pub shadow: Shadow,
    /// The text [`Color`] inherited by the content.
    pub text_color: Color,
    /// The [`Background`] of the countdown bar.
    pub countdown: Background,
}

impl Style {
    /// Returns the accent color, used for the variant glyph.
    fn countdown_color(&self) -> Color {
        match self.countdown {
            Background::Color(color) => color,
            _ => self.text_color,
        }
    }
}

/// A boxed [`Toast`] style function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Variant) -> Style + 'a>;

/// The theme catalog of a [`Toast`].
pub trait Catalog {
    /// The style class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class and a [`Variant`] into a [`Style`].
    fn style(&self, class: &Self::Class<'_>, variant: Variant) -> Style;
}

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, variant: Variant) -> Style {
        class(self, variant)
    }
}

/// The default style of a [`Toast`], drawn from the palette of the theme.
pub fn default(theme: &Theme, variant: Variant) -> Style {
    let palette = theme.palette();

    let accent = match variant {
        Variant::Neutral => palette.background.strongest.color,
        Variant::Info | Variant::Success if variant == Variant::Info => palette.primary.base.color,
        Variant::Success => palette.success.base.color,
        Variant::Warning => palette.warning.base.color,
        Variant::Danger => palette.danger.base.color,
        Variant::Info => palette.primary.base.color,
    };

    Style {
        background: Background::Color(palette.background.weak.color),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: DEFAULT_RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        text_color: palette.background.base.text,
        countdown: Background::Color(accent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reflow_settles_at_its_target() {
        let now = Instant::now();
        let mut reflow = Reflow::settled(0.0, now);

        reflow.retarget(100.0, now, Duration::from_millis(200));

        assert_eq!(reflow.value(now), 0.0, "starts where it was");
        assert_eq!(
            reflow.value(now + Duration::from_millis(200)),
            100.0,
            "arrives at the target"
        );
        assert!(!reflow.is_animating(now + Duration::from_millis(200)));
    }

    /// The point of chasing a moving target: a toast whose neighbour vanishes
    /// mid-slide must continue from where it is, not snap back.
    #[test]
    fn retargeting_mid_slide_continues_from_the_current_position() {
        let start = Instant::now();
        let mut reflow = Reflow::settled(0.0, start);

        reflow.retarget(100.0, start, Duration::from_millis(200));

        let midway = start + Duration::from_millis(100);
        let caught = reflow.value(midway);

        assert!(caught > 0.0 && caught < 100.0);

        reflow.retarget(50.0, midway, Duration::from_millis(200));

        assert_eq!(reflow.value(midway), caught, "did not jump on retarget");
    }

    #[test]
    fn retargeting_to_the_same_place_does_not_restart_the_slide() {
        let start = Instant::now();
        let mut reflow = Reflow::settled(0.0, start);

        reflow.retarget(100.0, start, Duration::from_millis(200));
        let midway = start + Duration::from_millis(100);
        let caught = reflow.value(midway);

        reflow.retarget(100.0, midway, Duration::from_millis(200));

        assert_eq!(reflow.value(midway), caught);
    }

    /// The bar has to stop where the corner rounding begins, or its ends run
    /// out past the curve and poke through the corner.
    #[test]
    fn the_countdown_track_is_inset_by_the_corner_radius() {
        let bounds = Rectangle {
            x: 100.0,
            y: 0.0,
            width: 360.0,
            height: 60.0,
        };

        let track = countdown_track(bounds, 8.0);

        assert_eq!(track.x, 108.0);
        assert_eq!(track.width, 344.0);
        assert_eq!(track.x + track.width, bounds.x + bounds.width - 8.0);
    }

    #[test]
    fn square_corners_let_the_countdown_run_the_full_width() {
        let bounds = Rectangle {
            x: 100.0,
            y: 0.0,
            width: 360.0,
            height: 60.0,
        };

        let track = countdown_track(bounds, 0.0);

        assert_eq!(track.x, bounds.x);
        assert_eq!(track.width, bounds.width);
    }

    /// A radius larger than the toast is half its width at most, so the track
    /// collapses to nothing rather than inverting.
    #[test]
    fn an_absurd_radius_collapses_the_track_instead_of_inverting_it() {
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 60.0,
        };

        let track = countdown_track(bounds, 999.0);

        assert_eq!(track.x, 20.0);
        assert_eq!(track.width, 0.0);
    }

    #[test]
    fn each_variant_maps_to_its_own_icon_except_the_neutral_one() {
        assert_eq!(Variant::Neutral.icon(), None);
        assert_eq!(Variant::Success.icon(), Some(lucide::Icon::CircleCheck));
        assert_eq!(Variant::Danger.icon(), Some(lucide::Icon::CircleX));
    }

    #[test]
    fn a_stack_at_the_bottom_grows_upward_and_one_at_the_top_does_not() {
        let up: Toasts<'_, ()> = toasts(vec![]).anchor(Anchor::BottomRight);
        let down: Toasts<'_, ()> = toasts(vec![]).anchor(Anchor::TopRight);

        assert!(up.grows_upward());
        assert!(!down.grows_upward());
    }
}
