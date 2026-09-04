//! A surface positioned against the viewport rather than against a widget.
//!
//! [`crate::menu`] and [`crate::popover`] both hang off the element that opens
//! them. A [`Layer`] does not: it anchors to a corner, an edge, or the centre
//! of the window, which is what modals, dialogs and toasts need. Nothing in the
//! view layout moves to make room for it — the host reports a zero size and
//! everything happens in an overlay.
//!
//! ```rust,ignore
//! use iced::widget::{button, column, text};
//!
//! layer(column![text("Delete this project?"), button(text("Delete"))])
//!     .open(self.confirming)
//!     .backdrop(0.4)
//!     .on_dismiss(Message::CancelDelete)
//! ```
//!
//! # Visibility is the application's
//!
//! A layer has no trigger, so there is nothing for it to toggle itself from.
//! [`Layer::open`] decides whether it shows, and [`Layer::on_dismiss`] reports
//! that the user asked to close it — by pressing the backdrop, hitting Escape,
//! or pressing the close button on its title bar. Acting on that is up to the
//! application, which is what lets a modal refuse to close while a save is in
//! flight.
//!
//! # Backdrop
//!
//! Without [`Layer::backdrop`] a layer is a floating surface: it draws over the
//! page but events reach whatever is underneath, which is what a toast wants.
//! With one, the layer is modal — the backdrop dims the page and swallows every
//! press that does not land on the surface.
//!
//! # Window chrome
//!
//! [`Layer::title`] turns the surface into a window: it gains a title bar, and
//! a close button that reports through [`Layer::on_dismiss`] like every other
//! dismissal. [`Layer::draggable`] then lets the user move it by that bar, and
//! [`Layer::resizable`] lets them drag any of its eight edges and corners.
//! [`modal`] switches all three on; [`dialog`] leaves them off, because a
//! confirmation prompt has nothing to be moved out of the way of.
//!
//! The moment the user grabs a window it stops following its [`Anchor`]: the
//! position and size are theirs, and stay theirs across closes and reopens.
//! Both are kept inside the viewport, so a window can never be dragged or
//! shrunk to somewhere it cannot be reached again.
//!
//! A surface clips its content to itself, which is what lets a window be
//! resized smaller than the content asked for without that content spilling
//! across the page. Child overlays — a menu opened from inside a window — are
//! drawn separately and are not clipped.
//!
//! # Motion
//!
//! The surface slides in from whichever viewport edge it is anchored to. It is
//! deliberately not faded: a layer holds arbitrary content, and widgets that
//! paint themselves ignore an inherited text color, so fading would leave
//! buttons at full strength over a half-drawn panel. The backdrop *is* faded —
//! it is a plain quad this widget draws itself, with no children to disagree
//! with it.

use iced::advanced::text;
use iced::advanced::widget::{self, Operation, tree::Tree};
use iced::advanced::{
    Layout, Shell, Widget,
    layout::{Limits, Node},
    overlay, renderer,
};
use iced::border::Radius;
use iced::time::Instant;
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Theme,
    Vector, alignment, keyboard, mouse, touch, window,
};

// Re-exported so `layer::Anchor` resolves for callers who never touch the
// base-relative placement module.
pub use crate::anchor::Anchor;
use crate::anchor::Side;
use crate::animation::{self, Motion, Transition};

/// The default margin between the surface and the viewport edge.
const DEFAULT_MARGIN: f32 = 16.0;

/// The default padding between the surface edge and its content.
const DEFAULT_PADDING: f32 = 16.0;

/// The default corner radius of the surface.
const DEFAULT_RADIUS: f32 = 10.0;

/// The height of the title bar.
const HEADER_HEIGHT: f32 = 34.0;

/// The size of the title text.
const TITLE_SIZE: f32 = 14.0;

/// The inset of the title from the leading edge of the title bar.
const TITLE_INSET: f32 = 12.0;

/// The width and height of the close button.
const CLOSE_SIZE: f32 = 22.0;

/// The inset of the close button from the trailing edge of the title bar.
const CLOSE_INSET: f32 = 6.0;

/// The glyph drawn on the close button.
///
/// A multiplication sign rather than a Lucide `X`, so a decorated layer does
/// not oblige the application to register an icon font.
const CLOSE_GLYPH: char = '×';

/// How far inside each edge of a resizable surface its handles reach.
const HANDLE_BAND: f32 = 6.0;

/// The narrowest a surface can be resized to, whatever [`Layer::min_width`]
/// says.
const HANDLE_MIN_WIDTH: f32 = 140.0;

/// The shortest a surface's content area can be resized to.
///
/// The title bar is added on top of this, so a decorated window always has room
/// for its own chrome.
const HANDLE_MIN_CONTENT_HEIGHT: f32 = 48.0;

/// Creates a [`Layer`] anchored to the centre of the viewport.
pub fn layer<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Layer::new(content)
}

/// Creates a dialog: centred, undecorated, with a dimmed backdrop.
///
/// The plain confirmation surface — no title bar, nothing to drag, nothing to
/// resize. Visibility is the application's, as with every layer. Reach for
/// [`modal`] instead when the surface holds work the user may want to move
/// aside or make room in.
pub fn dialog<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Layer::new(content).backdrop(0.4)
}

/// Creates a sheet: a panel that slides out of one edge and spans it.
///
/// `side` is the edge it comes from. A sheet from [`Anchor::Left`] or
/// [`Anchor::Right`] runs the full height of the viewport; one from
/// [`Anchor::Top`] or [`Anchor::Bottom`] runs the full width. Any other anchor
/// is treated as the nearest edge.
///
/// This lives here rather than in [`crate::toast`] despite both sliding out of
/// an edge: a sheet is a single surface with application-owned visibility,
/// which is exactly what a layer is. A toast is a list with per-item timers.
pub fn sheet<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    side: Anchor,
) -> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Layer::new(content)
        .anchor(side)
        .backdrop(0.4)
        .margin(0.0)
        .radius(0.0)
        .stretch(true)
}

/// Creates a modal window: centred and dimmed, with a title bar the user can
/// drag it by and edges they can resize it from.
///
/// This is [`dialog`] plus chrome, and the distinction is about what the
/// surface holds. A dialog asks a question and goes away; a modal window holds
/// something the user works in, so it is worth being able to shift it aside to
/// read the page underneath, or to make it bigger.
///
/// The close button reports through [`Layer::on_dismiss`] and is only drawn
/// when there is one to report to.
pub fn modal<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    title: impl Into<String>,
) -> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Layer::new(content)
        .backdrop(0.4)
        .title(title)
        .draggable(true)
        .resizable(true)
}

/// A surface positioned against the viewport.
#[allow(missing_debug_implementations)]
pub struct Layer<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    content: Element<'a, Message, Theme, Renderer>,
    anchor: Anchor,
    margin: f32,
    is_open: bool,
    padding: f32,
    radius: f32,
    min_width: f32,
    min_height: f32,
    max_width: Option<f32>,
    stretch: bool,
    title: Option<String>,
    close_button: bool,
    draggable: bool,
    resizable: bool,
    motion: Motion,
    backdrop: Option<f32>,
    dismiss_on_backdrop: bool,
    dismiss_on_escape: bool,
    on_dismiss: Option<Message>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Creates a new [`Layer`], open and centred.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            id: None,
            content: content.into(),
            anchor: Anchor::default(),
            margin: DEFAULT_MARGIN,
            is_open: true,
            padding: DEFAULT_PADDING,
            radius: DEFAULT_RADIUS,
            min_width: 0.0,
            min_height: 0.0,
            max_width: None,
            stretch: false,
            title: None,
            close_button: true,
            draggable: false,
            resizable: false,
            motion: Motion::SMOOTH,
            backdrop: None,
            dismiss_on_backdrop: true,
            dismiss_on_escape: true,
            on_dismiss: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the [`widget::Id`] of the [`Layer`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets whether the layer is showing.
    pub fn open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Sets where in the viewport the layer sits.
    ///
    /// A window the user has dragged or resized no longer follows this — see
    /// [`Layer::draggable`].
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets the margin between the surface and the viewport edge.
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Sets the padding between the surface edge and its content.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the corner radius of the surface.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Sets a lower bound on the width of the surface.
    ///
    /// This bounds a [`Layer::resizable`] surface too: the user cannot drag it
    /// narrower than this.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Sets a lower bound on the height of the surface.
    ///
    /// This bounds a [`Layer::resizable`] surface too: the user cannot drag it
    /// shorter than this.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }

    /// Sets an upper bound on the width the surface takes from its content.
    ///
    /// This caps the width the layer *chooses*. It is not a limit on the user:
    /// someone dragging the edge of a [`Layer::resizable`] window is asking for
    /// a specific size and gets it.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Makes the surface span the edge it is anchored to.
    ///
    /// A stretched layer anchored to a vertical edge fills the height of the
    /// viewport and vice versa, which is what turns a panel into a [`sheet`].
    /// A centred layer has no edge to span, so this does nothing to one.
    pub fn stretch(mut self, stretch: bool) -> Self {
        self.stretch = stretch;
        self
    }

    /// Gives the surface a title bar, turning it into a window.
    ///
    /// The bar carries the title and, unless [`Layer::close_button`] says
    /// otherwise, a close button that reports through [`Layer::on_dismiss`].
    /// Pass an empty string for a bare bar to drag a window by.
    ///
    /// Everything else about chrome hangs off this: without a title bar there
    /// is nothing to drag and no close button to draw.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets whether the title bar carries a close button.
    ///
    /// Defaults to `true`, but the button is only drawn when there is both a
    /// [`Layer::title`] bar to put it on and an [`Layer::on_dismiss`] for it to
    /// report to — a button that cannot do anything is worse than none.
    pub fn close_button(mut self, close_button: bool) -> Self {
        self.close_button = close_button;
        self
    }

    /// Lets the user move the surface by dragging its title bar.
    ///
    /// Needs a [`Layer::title`] bar to drag by. Once moved, the surface keeps
    /// the position the user gave it — [`Layer::anchor`] no longer applies to
    /// it, and closing and reopening the layer brings it back where they left
    /// it. It is always kept inside the viewport.
    pub fn draggable(mut self, draggable: bool) -> Self {
        self.draggable = draggable;
        self
    }

    /// Lets the user resize the surface by dragging its edges and corners.
    ///
    /// All eight handles are live, bounded by [`Layer::min_width`],
    /// [`Layer::min_height`] and the viewport. As with [`Layer::draggable`], a
    /// resized surface keeps the size the user gave it and stops taking one
    /// from its content.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Dims the page behind the layer and makes it modal.
    ///
    /// `alpha` is the opacity of the dimming, from `0.0` to `1.0`. Any press
    /// that misses the surface is swallowed rather than reaching the page.
    pub fn backdrop(mut self, alpha: f32) -> Self {
        self.backdrop = Some(alpha.clamp(0.0, 1.0));
        self
    }

    /// Sets whether pressing the backdrop asks the layer to close.
    ///
    /// Has no effect without a [`Layer::backdrop`]. A press still never reaches
    /// the page; this only decides whether [`Layer::on_dismiss`] fires.
    pub fn dismiss_on_backdrop(mut self, dismiss: bool) -> Self {
        self.dismiss_on_backdrop = dismiss;
        self
    }

    /// Sets whether Escape asks the layer to close.
    pub fn dismiss_on_escape(mut self, dismiss: bool) -> Self {
        self.dismiss_on_escape = dismiss;
        self
    }

    /// Sets the message published when the user asks to close the layer.
    ///
    /// The layer does not close itself — it reports, and the application
    /// decides. Without this, backdrop presses, Escape and the close button all
    /// do nothing.
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }

    /// Sets how the layer animates in and out.
    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// Shows and hides the layer instantly, with no transition.
    pub fn no_animation(mut self) -> Self {
        self.motion = Motion::NONE;
        self
    }

    /// Sets the style of the [`Layer`].
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Layer`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

/// A surface whose position and size the user has taken over.
///
/// Present only once someone has actually dragged or resized the layer. Until
/// then the [`Anchor`] and the content decide, which is what keeps an
/// undecorated dialog behaving exactly as it did before chrome existed.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Window {
    position: Point,
    size: Size,
}

/// A resize handle on the border of a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Edge {
    /// Returns the handle under `cursor`, if any.
    ///
    /// The band is capped at a third of each side, so the handles of a surface
    /// dragged down to its minimum never meet in the middle and leave no way to
    /// grow it again.
    fn at(cursor: Point, bounds: Rectangle) -> Option<Self> {
        if !bounds.contains(cursor) {
            return None;
        }

        let band = HANDLE_BAND.min(bounds.width / 3.0).min(bounds.height / 3.0);

        let left = cursor.x <= bounds.x + band;
        let right = cursor.x >= bounds.x + bounds.width - band;
        let top = cursor.y <= bounds.y + band;
        let bottom = cursor.y >= bounds.y + bounds.height - band;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(Self::TopLeft),
            (_, true, true, _) => Some(Self::TopRight),
            (true, _, _, true) => Some(Self::BottomLeft),
            (_, true, _, true) => Some(Self::BottomRight),
            (true, ..) => Some(Self::Left),
            (_, true, ..) => Some(Self::Right),
            (_, _, true, _) => Some(Self::Top),
            (_, _, _, true) => Some(Self::Bottom),
            _ => None,
        }
    }

    /// Returns the cursor shown while this handle is under the pointer.
    fn interaction(self) -> mouse::Interaction {
        match self {
            Self::Top | Self::Bottom => mouse::Interaction::ResizingVertically,
            Self::Left | Self::Right => mouse::Interaction::ResizingHorizontally,
            Self::TopRight | Self::BottomLeft => mouse::Interaction::ResizingDiagonallyUp,
            Self::TopLeft | Self::BottomRight => mouse::Interaction::ResizingDiagonallyDown,
        }
    }

    fn moves_left(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    fn moves_right(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    fn moves_top(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    fn moves_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

/// What the user is doing to the surface with the pointer held down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grab {
    /// Moving it by its title bar.
    Move,
    /// Resizing it by one of its handles.
    Resize(Edge),
}

/// A [`Grab`] in flight, with everything needed to resolve it from scratch.
///
/// The window is remembered as it was when the grab began rather than being
/// nudged event by event: accumulating deltas drifts, and a batched or dropped
/// pointer event would leave the surface permanently offset from the cursor.
#[derive(Debug, Clone, Copy)]
struct Held {
    grab: Grab,
    origin: Point,
    window: Window,
}

/// The persistent state of a [`Layer`].
#[derive(Debug, Default)]
struct State {
    /// Whether the layer was showing as of the last event.
    ///
    /// Compared against the value the application passed in, to notice the
    /// moment it changes and start the transition.
    is_open: bool,
    transition: Transition,
    /// Where the user put the surface, once they have moved or resized it.
    window: Option<Window>,
    /// The drag or resize in flight, if any.
    held: Option<Held>,
    /// The instant of the frame the layer was last updated in.
    ///
    /// Whether the surface is showing, and how far through its transition it
    /// is, are both read from here rather than from the clock. A frame is not
    /// an instant: `UserInterface` lays the overlay out once during `update`,
    /// caches that layout, then rebuilds the overlay tree in `draw` and hands
    /// it the cached one. Deciding visibility from `Instant::now()` lets a
    /// transition finish *between* the two, at which point the surface leaves
    /// the tree but not the layout — and `overlay::Group` pairs the two up by
    /// position, so every sibling overlay is then drawn against a layout node
    /// belonging to something else, which panics as soon as that node has the
    /// wrong shape.
    ///
    /// `None` until the first frame, when nothing has opened yet.
    frame: Option<Instant>,
}

impl State {
    /// Returns `true` when the surface should still be produced and drawn.
    ///
    /// Answered as of the frame the layer was last updated in, so that one
    /// frame gets one answer however many passes it takes. See
    /// [`State::frame`].
    ///
    /// `is_open` is the application's rather than [`State::is_open`], which has
    /// not necessarily caught up with it: the view is rebuilt before `update`
    /// runs.
    fn is_showing(&self, is_open: bool) -> bool {
        is_open
            || self
                .frame
                .is_some_and(|frame| self.transition.is_visible(frame))
    }

    /// How far open the surface is, as of that same frame.
    fn progress(&self) -> f32 {
        self.frame
            .map_or(0.0, |frame| self.transition.progress(frame))
    }
}

/// A pointer event, without the distinction between mouse and touch.
///
/// Dragging a window and dragging it with a finger are the same gesture, and
/// the two event families carry the same information for it.
#[derive(Debug, Clone, Copy)]
enum Pointer {
    Pressed(Point),
    Moved(Point),
    Released,
}

impl Pointer {
    /// Reads `event` as a pointer gesture, if it is one.
    ///
    /// `cursor` supplies the position for mouse presses, which — unlike every
    /// other event here — do not carry one.
    fn read(event: &Event, cursor: mouse::Cursor) -> Option<Self> {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor.position().map(Self::Pressed)
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => Some(Self::Moved(*position)),
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => Some(Self::Released),
            Event::Touch(touch::Event::FingerPressed { position, .. }) => {
                Some(Self::Pressed(*position))
            }
            Event::Touch(touch::Event::FingerMoved { position, .. }) => {
                Some(Self::Moved(*position))
            }
            Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                Some(Self::Released)
            }
            _ => None,
        }
    }
}

/// [`f32::clamp`] that tolerates an inverted range instead of panicking.
///
/// The ranges below come from a minimum size and a viewport, and a viewport
/// smaller than the minimum inverts them. Preferring the lower bound keeps the
/// surface at its minimum rather than crashing on a window nobody can use
/// anyway.
fn clamped(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max.max(min))
}

/// Slides a surface of `size` at `position` back inside `viewport`.
///
/// A window parked near an edge would otherwise be left hanging outside — or
/// out of reach entirely — when the window it lives in shrinks under it.
fn clamp_into(position: Point, size: Size, viewport: Rectangle) -> Point {
    let free_x = (viewport.width - size.width).max(0.0);
    let free_y = (viewport.height - size.height).max(0.0);

    Point::new(
        clamped(position.x, viewport.x, viewport.x + free_x),
        clamped(position.y, viewport.y, viewport.y + free_y),
    )
}

/// Applies a pointer drag to the window the grab started on.
///
/// `delta` is the cursor's travel since the press, and `start` the window as it
/// was then. The result is always at least `min` and always inside `viewport`.
fn dragged(grab: Grab, start: Window, delta: Vector, min: Size, viewport: Rectangle) -> Window {
    match grab {
        Grab::Move => Window {
            position: clamp_into(
                Point::new(start.position.x + delta.x, start.position.y + delta.y),
                start.size,
                viewport,
            ),
            size: start.size,
        },
        Grab::Resize(edge) => {
            let mut left = start.position.x;
            let mut top = start.position.y;
            let mut right = left + start.size.width;
            let mut bottom = top + start.size.height;

            // Each handle moves only the edges it lies on, and each edge is
            // clamped in its own right. Bounding the edge rather than the
            // resulting size is what nails the opposite edge down when the drag
            // runs into the minimum size or the viewport, instead of dragging
            // the whole surface along with it.
            if edge.moves_left() {
                left = clamped(left + delta.x, viewport.x, right - min.width);
            }

            if edge.moves_right() {
                right = clamped(
                    right + delta.x,
                    left + min.width,
                    viewport.x + viewport.width,
                );
            }

            if edge.moves_top() {
                top = clamped(top + delta.y, viewport.y, bottom - min.height);
            }

            if edge.moves_bottom() {
                bottom = clamped(
                    bottom + delta.y,
                    top + min.height,
                    viewport.y + viewport.height,
                );
            }

            Window {
                position: Point::new(left, top),
                size: Size::new(right - left, bottom - top),
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Layer<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 1 {
            tree.children = vec![Tree::new(self.content.as_widget())];
        }

        tree.children[0].diff(self.content.as_widget_mut());
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(0.0), Length::Fixed(0.0))
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) -> Node {
        // The host holds a place in the tree so it can produce an overlay, and
        // takes up no room doing it.
        //
        // It deliberately does not report `is_void`: iced drops void children
        // before they get the chance to produce an overlay. A `Column` or `Row`
        // with spacing will therefore still count this as a child — put it in a
        // `Stack` where that matters.
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
        // Everything is drawn by the overlay.
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
        let state = tree.state.downcast_mut::<State>();
        state.transition.sync(self.motion);

        // The one place the transition is allowed to advance. Everything else
        // reads the answer back off the state, so that a frame gets a single
        // answer however many passes it takes. See [`State::frame`].
        if let Event::Window(window::Event::RedrawRequested(now)) = event {
            let was_showing = state.is_showing(self.is_open);

            state.frame = Some(*now);

            if state.transition.is_animating(*now) {
                shell.request_redraw();
            }

            // The frame the surface stops showing on is a frame the overlay has
            // to be laid out on again: it is leaving the tree, and the layout
            // cached for it would otherwise be handed to whichever overlay takes
            // its place.
            if was_showing != state.is_showing(self.is_open) {
                shell.invalidate_layout();
                shell.request_redraw();
            }
        }

        if self.is_open != state.is_open {
            let now = Instant::now();

            state.is_open = self.is_open;
            state.frame = Some(now);

            if self.is_open {
                state.transition.open(now);
            } else {
                state.transition.close(now);

                // A grab cannot outlive the surface it was on: the closing
                // surface stops handling events, so the release that would have
                // ended it never arrives.
                state.held = None;
            }

            shell.invalidate_layout();
            shell.request_redraw();
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let _ = tree;
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
        let state = tree.state.downcast_mut::<State>();
        state.transition.sync(self.motion);

        // The surface outlives the close so it can animate out.
        //
        // Deliberately the frame's instant and not the clock's: `overlay` is
        // called once to lay the surface out and again to draw it, and the two
        // have to agree. See [`State::frame`].
        if !state.is_showing(self.is_open) {
            return None;
        }

        // Read the transition out before `state` is handed to the overlay.
        let progress = state.progress();
        let is_closing = !self.is_open;

        // The close button is chrome the user can press, so drawing one with
        // nothing to report to would be a lie.
        let close_button = self.close_button && self.on_dismiss.is_some();

        Some(overlay::Element::new(Box::new(Surface {
            content: &mut self.content,
            tree: &mut tree.children[0],
            state,
            anchor: self.anchor,
            margin: self.margin,
            padding: self.padding,
            radius: self.radius,
            min_width: self.min_width,
            min_height: self.min_height,
            max_width: self.max_width,
            stretch: self.stretch,
            title: self.title.as_deref(),
            close_button,
            draggable: self.draggable,
            resizable: self.resizable,
            motion: self.motion,
            progress,
            is_closing,
            backdrop: self.backdrop,
            dismiss_on_backdrop: self.dismiss_on_backdrop,
            dismiss_on_escape: self.dismiss_on_escape,
            on_dismiss: self.on_dismiss.as_ref(),
            class: &self.class,
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Layer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer + 'a,
{
    fn from(layer: Layer<'a, Message, Theme, Renderer>) -> Self {
        Element::new(layer)
    }
}

/// The viewport-anchored surface, as an iced overlay.
struct Surface<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    content: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    /// The layer's own state, so a drag or resize can outlive the event that
    /// moved it.
    state: &'a mut State,
    anchor: Anchor,
    margin: f32,
    padding: f32,
    radius: f32,
    min_width: f32,
    min_height: f32,
    max_width: Option<f32>,
    stretch: bool,
    title: Option<&'a str>,
    close_button: bool,
    draggable: bool,
    resizable: bool,
    motion: Motion,
    progress: f32,
    is_closing: bool,
    backdrop: Option<f32>,
    dismiss_on_backdrop: bool,
    dismiss_on_escape: bool,
    on_dismiss: Option<&'a Message>,
    class: &'a Theme::Class<'b>,
}

impl<Message, Theme, Renderer> Surface<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer,
{
    /// Reports that the user asked to close the layer.
    fn dismiss(&self, shell: &mut Shell<'_, Message>) {
        if let Some(message) = self.on_dismiss {
            shell.publish(message.clone());
        }
    }

    /// The height the title bar takes out of the surface.
    fn header_height(&self) -> f32 {
        if self.title.is_some() {
            HEADER_HEIGHT
        } else {
            0.0
        }
    }

    /// The room the surface takes for itself, before any content.
    fn chrome(&self) -> Size {
        Size::new(
            self.padding * 2.0,
            self.padding * 2.0 + self.header_height(),
        )
    }

    /// The title bar of the surface, if it has one.
    fn header_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        self.title.map(|_| Rectangle {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: HEADER_HEIGHT.min(bounds.height),
        })
    }

    /// The close button on the title bar, if there is one to press.
    fn close_bounds(&self, bounds: Rectangle) -> Option<Rectangle> {
        if !self.close_button {
            return None;
        }

        let header = self.header_bounds(bounds)?;

        Some(Rectangle {
            x: header.x + header.width - CLOSE_INSET - CLOSE_SIZE,
            y: header.y + (header.height - CLOSE_SIZE) / 2.0,
            width: CLOSE_SIZE,
            height: CLOSE_SIZE,
        })
    }

    /// The smallest the user may leave the surface at.
    fn min_size(&self) -> Size {
        Size::new(
            self.min_width.max(HANDLE_MIN_WIDTH),
            self.min_height
                .max(self.header_height() + HANDLE_MIN_CONTENT_HEIGHT),
        )
    }

    /// Returns what a press at `position` grabs, if anything.
    ///
    /// A resize handle wins over the title bar where they overlap, which is what
    /// makes the corners of a window's own bar resize it rather than move it.
    fn grab_at(&self, position: Point, bounds: Rectangle) -> Option<Grab> {
        if self.resizable
            && let Some(edge) = Edge::at(position, bounds)
        {
            return Some(Grab::Resize(edge));
        }

        if self.draggable
            && self
                .header_bounds(bounds)
                .is_some_and(|header| header.contains(position))
        {
            return Some(Grab::Move);
        }

        None
    }

    /// The surface as it stands, ready to be handed to the user.
    ///
    /// Taken from the layout it is currently drawn at, so the first grab of an
    /// anchored surface picks up exactly where the anchor left it instead of
    /// jumping. It is brought up to the minimum size here rather than on the
    /// first move, so a content-sized surface smaller than that minimum does not
    /// start the drag out of bounds.
    fn pinned_window(&self, bounds: Rectangle, viewport: Rectangle) -> Window {
        let min = self.min_size();

        let size = Size::new(
            clamped(bounds.width, min.width, viewport.width),
            clamped(bounds.height, min.height, viewport.height),
        );

        Window {
            position: clamp_into(bounds.position(), size, viewport),
            size,
        }
    }

    /// The content area left inside a surface of `size`.
    fn inner_size(&self, size: Size) -> Size {
        let chrome = self.chrome();

        Size::new(
            (size.width - chrome.width).max(0.0),
            (size.height - chrome.height).max(0.0),
        )
    }

    /// Lays the content out against a content area that has been settled on.
    ///
    /// The width is pinned exactly; the height is only capped, so content that
    /// wants to fill does, and content that does not keeps its own height and
    /// leaves the rest of a resized window empty.
    fn layout_content(&mut self, renderer: &Renderer, inner: Size) -> Node {
        let limits = Limits::new(
            Size::new(inner.width, 0.0),
            Size::new(inner.width, inner.height),
        );

        self.content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits)
    }

    /// Takes the surface size from its content, and lays that content out.
    fn measure(&mut self, renderer: &Renderer, bounds: Size) -> (Size, Node) {
        let chrome = self.chrome();

        let available = Size::new(
            (bounds.width - self.margin * 2.0 - chrome.width).max(0.0),
            (bounds.height - self.margin * 2.0 - chrome.height).max(0.0),
        );

        // Measured compressed so fluid content reports its intrinsic width
        // rather than swallowing the viewport.
        let limits = Limits::new(Size::ZERO, available).width(Length::Shrink);
        let measured = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);

        // A stretched surface fills the edge it hugs, which is what makes a
        // sheet read as a panel sliding out rather than a floating card. Which
        // axis that is decides how much of the work below is still open.
        let side = self.anchor.slide_from();
        let spans_width = self.stretch && !side.is_horizontal();
        let spans_height = self.stretch && side.is_horizontal();

        // The width is settled first, because the content's height depends on
        // it. A sheet across the top or bottom takes the whole viewport;
        // everything else takes what its content asked for.
        let width = if spans_width {
            bounds.width
        } else {
            let mut width = measured.size().width.max(self.min_width - chrome.width);

            if let Some(max) = self.max_width {
                width = width.min(max - chrome.width);
            }

            width.min(available.width) + chrome.width
        };

        let inner_width = (width - chrome.width).max(0.0);

        // Nothing moved, so the measuring pass stands and its height is the
        // surface's.
        if inner_width == measured.size().width && !spans_height {
            return (
                Size::new(
                    width,
                    (measured.size().height + chrome.height).max(self.min_height),
                ),
                measured,
            );
        }

        // Otherwise the content was measured against the wrong width and has to
        // be laid out again against the one the surface actually took.
        let inner_height = if spans_height {
            (bounds.height - chrome.height).max(0.0)
        } else {
            available.height
        };

        let content = self.layout_content(renderer, Size::new(inner_width, inner_height));

        let height = if spans_height {
            bounds.height
        } else {
            (content.size().height + chrome.height).max(self.min_height)
        };

        (Size::new(width, height), content)
    }
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Surface<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let viewport = Rectangle::with_size(bounds);

        // A surface the user has taken over is placed outright; only one still
        // owned by the widget consults its anchor and its content.
        let (size, position, content) = match self.state.window {
            Some(window) => {
                // Only shrunk to fit, never remembered smaller: the size the
                // user chose comes back when the window it lives in grows again.
                let size = Size::new(
                    window.size.width.min(bounds.width),
                    window.size.height.min(bounds.height),
                );

                let inner = self.inner_size(size);
                let content = self.layout_content(renderer, inner);

                (size, clamp_into(window.position, size, viewport), content)
            }
            None => {
                let (size, content) = self.measure(renderer, bounds);

                (
                    size,
                    self.anchor.position(size, viewport, self.margin),
                    content,
                )
            }
        };

        // A sheet travels its own extent, so it rolls completely out of the
        // window edge and back into it. Anything else just eases a few pixels.
        let side = self.anchor.slide_from();
        let travel = if self.stretch {
            match side {
                Side::Left | Side::Right => size.width,
                Side::Top | Side::Bottom => size.height,
            }
        } else {
            self.motion.slide
        };

        let offset = animation::slide_from_edge(side, travel, self.progress);

        let surface = Node::with_children(
            size,
            vec![content.move_to(Point::new(
                self.padding,
                self.padding + self.header_height(),
            ))],
        )
        .move_to(position + offset);

        // The root spans the viewport so the backdrop has something to fill and
        // presses outside the surface are still inside this overlay.
        Node::with_children(bounds, vec![surface])
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let surface = layout.children().next().expect("surface layout");
        let bounds = surface.bounds();
        let style = theme.style(self.class);

        // The backdrop is the one thing here with no children of its own, so it
        // is the one thing that can honestly be faded.
        if let Some(alpha) = self.backdrop {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds(),
                    ..renderer::Quad::default()
                },
                animation::fade(
                    Color {
                        a: alpha,
                        ..style.backdrop_color
                    },
                    self.progress,
                ),
            );
        }

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

        self.draw_header(renderer, &style, bounds, cursor);

        let content = surface.children().next().expect("content layout");

        // Clipped to the surface: a window resized smaller than its content
        // asked for has to cut that content off rather than let it paint across
        // the page.
        renderer.with_layer(bounds, |renderer| {
            self.content.as_widget().draw(
                self.tree,
                renderer,
                theme,
                &renderer::Style {
                    text_color: style.text_color,
                },
                content,
                cursor,
                &bounds,
            );
        });
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let surface = layout.children().next().expect("surface layout");
        let bounds = surface.bounds();
        let viewport = layout.bounds();

        // A surface on its way out is a picture, not a control.
        if self.is_closing {
            return;
        }

        let pointer = Pointer::read(event, cursor);

        // A drag in flight owns the pointer outright: the content underneath
        // must not read the sweep as a hover, and the release that ends the drag
        // is not a click on anything.
        if let Some(held) = self.state.held {
            match pointer {
                Some(Pointer::Moved(position)) => {
                    let delta = Vector::new(position.x - held.origin.x, position.y - held.origin.y);
                    let window = dragged(held.grab, held.window, delta, self.min_size(), viewport);

                    if Some(window) != self.state.window {
                        self.state.window = Some(window);
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }

                    shell.capture_event();
                }
                Some(Pointer::Released) => {
                    self.state.held = None;
                    shell.capture_event();
                }
                // Losing focus mid-drag means the release will be delivered
                // somewhere else, so the grab has to end here or it would still
                // be live when the window comes back.
                _ if matches!(event, Event::Window(window::Event::Unfocused)) => {
                    self.state.held = None;
                }
                _ => {}
            }

            return;
        }

        // Chrome is checked before the content, so a press on a resize handle or
        // the title bar never reaches whatever is painted under it.
        if let Some(Pointer::Pressed(position)) = pointer {
            if self
                .close_bounds(bounds)
                .is_some_and(|close| close.contains(position))
            {
                self.dismiss(shell);
                shell.capture_event();
                return;
            }

            if let Some(grab) = self.grab_at(position, bounds) {
                let window = self.pinned_window(bounds, viewport);

                self.state.window = Some(window);
                self.state.held = Some(Held {
                    grab,
                    origin: position,
                    window,
                });

                shell.invalidate_layout();
                shell.capture_event();
                return;
            }
        }

        self.content.as_widget_mut().update(
            self.tree,
            event,
            surface.children().next().expect("content layout"),
            cursor,
            renderer,
            shell,
            &bounds,
        );

        if shell.is_event_captured() {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(_))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) || self.backdrop.is_none() {
                    return;
                }

                if self.dismiss_on_backdrop {
                    self.dismiss(shell);
                }

                // Swallowed either way: a modal that let presses through to the
                // page it is covering would not be modal.
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if self.dismiss_on_escape => {
                self.dismiss(shell);
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.is_closing {
            return mouse::Interaction::None;
        }

        let surface = layout.children().next().expect("surface layout");
        let bounds = surface.bounds();

        // A drag in flight keeps its cursor wherever the pointer wanders, which
        // is what stops the shape flickering as it crosses the edge it is
        // dragging.
        if let Some(held) = self.state.held {
            return match held.grab {
                Grab::Move => mouse::Interaction::Grabbing,
                Grab::Resize(edge) => edge.interaction(),
            };
        }

        if let Some(position) = cursor.position() {
            if self
                .close_bounds(bounds)
                .is_some_and(|close| close.contains(position))
            {
                return mouse::Interaction::Pointer;
            }

            if self.resizable
                && let Some(edge) = Edge::at(position, bounds)
            {
                return edge.interaction();
            }

            if self.draggable
                && self
                    .header_bounds(bounds)
                    .is_some_and(|header| header.contains(position))
            {
                return mouse::Interaction::Grab;
            }
        }

        self.content.as_widget().mouse_interaction(
            self.tree,
            surface.children().next().expect("content layout"),
            cursor,
            &bounds,
            renderer,
        )
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        let surface = layout.children().next().expect("surface layout");

        self.content.as_widget_mut().operate(
            self.tree,
            surface.children().next().expect("content layout"),
            renderer,
            operation,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let surface = layout.children().next()?;
        let bounds = surface.bounds();

        self.content.as_widget_mut().overlay(
            self.tree,
            surface.children().next()?,
            renderer,
            &bounds,
            Vector::ZERO,
        )
    }
}

impl<Message, Theme, Renderer> Surface<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + text::Renderer,
{
    /// Draws the title bar over a surface of `bounds`: its background, the
    /// title, and the close button. Does nothing to an undecorated surface.
    fn draw_header(
        &self,
        renderer: &mut Renderer,
        style: &Style,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        let Some(header) = self.header_bounds(bounds) else {
            return;
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: header,
                border: Border {
                    // Only the top corners follow the surface: the bottom of the
                    // bar is a seam against the content, not an outside edge.
                    radius: Radius {
                        top_left: self.radius,
                        top_right: self.radius,
                        bottom_left: 0.0,
                        bottom_right: 0.0,
                    },
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            style.header_background,
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: header.x,
                    y: header.y + header.height - 1.0,
                    width: header.width,
                    height: 1.0,
                },
                ..renderer::Quad::default()
            },
            style.border.color,
        );

        let close = self.close_bounds(bounds);

        if let Some(close) = close {
            if cursor.is_over(close) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: close,
                        border: Border {
                            radius: (CLOSE_SIZE / 2.0).into(),
                            ..Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    style.close_background,
                );
            }

            renderer.fill_text(
                text::Text {
                    content: CLOSE_GLYPH.to_string(),
                    bounds: close.size(),
                    size: iced::Pixels(CLOSE_SIZE * 0.8),
                    line_height: text::LineHeight::default(),
                    font: renderer.default_font(),
                    align_x: text::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: None,
                },
                close.center(),
                style.title_color,
                close,
            );
        }

        let Some(title) = self.title.filter(|title| !title.is_empty()) else {
            return;
        };

        // The title stops where the close button starts, so a long one is
        // ellipsized rather than drawn underneath it.
        let end = close.map_or(header.x + header.width - TITLE_INSET, |close| {
            close.x - CLOSE_INSET
        });

        let bounds = Rectangle {
            x: header.x + TITLE_INSET,
            y: header.y,
            width: (end - header.x - TITLE_INSET).max(0.0),
            height: header.height,
        };

        renderer.fill_text(
            text::Text {
                content: title.to_string(),
                bounds: bounds.size(),
                size: iced::Pixels(TITLE_SIZE),
                line_height: text::LineHeight::default(),
                font: renderer.default_font(),
                // `Default` rather than `Left`, so a right-to-left title lands
                // on the side of the bar it belongs on.
                align_x: text::Alignment::Default,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Advanced,
                wrapping: text::Wrapping::None,
                ellipsis: text::Ellipsis::End,
                hint_factor: None,
            },
            Point::new(bounds.x, bounds.center_y()),
            style.title_color,
            bounds,
        );
    }
}

/// The appearance of a [`Layer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the surface.
    pub background: Background,
    /// The [`Border`] of the surface. Its radius is overridden by
    /// [`Layer::radius`].
    ///
    /// Its color also draws the seam under a [`Layer::title`] bar.
    pub border: Border,
    /// The [`Shadow`] cast by the surface.
    pub shadow: Shadow,
    /// The text [`Color`] inherited by the content.
    pub text_color: Color,
    /// The [`Color`] the backdrop dims the page with.
    ///
    /// Its alpha is replaced by the value given to [`Layer::backdrop`].
    pub backdrop_color: Color,
    /// The [`Background`] of the [`Layer::title`] bar.
    pub header_background: Background,
    /// The [`Color`] of the title and of the close glyph.
    pub title_color: Color,
    /// The [`Color`] highlighting the close button under the cursor.
    pub close_background: Color,
}

/// A boxed [`Layer`] style function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`Layer`].
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

/// The default style of a [`Layer`], drawn from the palette of the theme.
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Background::Color(palette.background.weak.color),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            radius: DEFAULT_RADIUS.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 32.0,
        },
        text_color: palette.background.base.text,
        backdrop_color: Color::BLACK,
        // One step further from the page than the surface, so the bar reads as
        // chrome without becoming a second accent.
        header_background: Background::Color(palette.background.strong.color),
        title_color: palette.background.base.text,
        close_background: palette.background.stronger.color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Side;
    use iced::advanced::shell;
    use iced::time::Duration;

    const VIEWPORT: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn size() -> Size {
        Size::new(200.0, 100.0)
    }

    /// The bounds the hosted layer is laid out in.
    const HOST_BOUNDS: Size = Size::new(400.0, 400.0);

    /// Drives one layer through frames the way `UserInterface` does.
    ///
    /// Deliberately not a `UserInterface`: what is under test is whether the
    /// widget gives a frame one answer, which is a question about the widget
    /// alone.
    struct Host<'a> {
        element: Element<'a, (), Theme, ()>,
        tree: Tree,
        node: Node,
    }

    impl<'a> Host<'a> {
        /// Hosts a layer, opened or closed, reusing the state of the last one.
        ///
        /// Visibility is the application's, so changing it means a new view —
        /// which is what the real thing does between frames.
        fn show(&mut self, is_open: bool) {
            let mut element = Element::from(Self::layer(is_open));
            element.as_widget_mut().diff(&mut self.tree);

            self.node = element.as_widget_mut().layout(
                &mut self.tree,
                &(),
                &Limits::new(Size::ZERO, HOST_BOUNDS),
            );

            self.element = element;
        }

        fn layer(is_open: bool) -> Layer<'a, (), Theme, ()> {
            dialog(iced::widget::text("Sure?")).open(is_open)
        }

        fn new(is_open: bool) -> Self {
            let mut element = Element::from(Self::layer(is_open));
            let mut tree = Tree::new(element.as_widget());
            element.as_widget_mut().diff(&mut tree);

            let node = element.as_widget_mut().layout(
                &mut tree,
                &(),
                &Limits::new(Size::ZERO, HOST_BOUNDS),
            );

            Self {
                element,
                tree,
                node,
            }
        }

        fn redraw(&mut self) {
            let mut bus = shell::Bus::new();
            let mut shell = Shell::new(&iced::window::Headless, shell::Waker::noop(), &mut bus);

            self.element.as_widget_mut().update(
                &mut self.tree,
                &Event::Window(window::Event::RedrawRequested(Instant::now())),
                Layout::new(&self.node),
                mouse::Cursor::Unavailable,
                &(),
                &mut shell,
                &Rectangle::with_size(HOST_BOUNDS),
            );
        }

        /// Whether the layer would put a surface in the overlay tree right now.
        fn has_overlay(&mut self) -> bool {
            self.element
                .as_widget_mut()
                .overlay(
                    &mut self.tree,
                    Layout::new(&self.node),
                    &(),
                    &Rectangle::with_size(HOST_BOUNDS),
                    Vector::ZERO,
                )
                .is_some()
        }
    }

    /// A frame asks for the overlay twice, and the two answers have to match.
    ///
    /// `UserInterface` lays the overlay out once during `update` and caches
    /// that layout; `draw` then rebuilds the overlay tree and hands it the
    /// cached one, and `overlay::Group` pairs the two up by position. A layer
    /// that decided it was showing for the layout and not for the draw takes
    /// its layout node with it, and every sibling overlay — a `tooltip`, say —
    /// is left drawing against a node belonging to something else. So the
    /// answer comes from the frame rather than the clock. See [`State::frame`].
    #[test]
    fn a_closing_layer_answers_the_same_twice_within_one_frame() {
        let mut host = Host::new(true);

        host.redraw();

        // Closed by the application. The surface is animating out, and still
        // showing.
        host.show(false);
        host.redraw();

        let showing_for_the_layout = host.has_overlay();
        assert!(showing_for_the_layout, "still sliding out");

        // However long the frame takes to get from its layout to its draw.
        std::thread::sleep(Motion::SMOOTH.duration + Duration::from_millis(50));

        assert_eq!(
            host.has_overlay(),
            showing_for_the_layout,
            "the surface left the overlay tree without the layout being rebuilt"
        );

        // And the next frame does retire it, rather than pinning it open.
        host.redraw();

        assert!(!host.has_overlay());
    }

    /// A 200×100 window at (100, 100), the starting point for the drag tests.
    fn window() -> Window {
        Window {
            position: Point::new(100.0, 100.0),
            size: size(),
        }
    }

    fn min() -> Size {
        Size::new(140.0, 80.0)
    }

    #[test]
    fn center_sits_in_the_middle_regardless_of_margin() {
        let position = Anchor::Center.position(size(), VIEWPORT, 16.0);

        assert_eq!(position, Point::new(300.0, 250.0));
    }

    #[test]
    fn corners_inset_by_the_margin() {
        assert_eq!(
            Anchor::TopLeft.position(size(), VIEWPORT, 16.0),
            Point::new(16.0, 16.0)
        );
        assert_eq!(
            Anchor::BottomRight.position(size(), VIEWPORT, 16.0),
            Point::new(584.0, 484.0)
        );
    }

    #[test]
    fn edges_center_on_their_free_axis() {
        assert_eq!(
            Anchor::Top.position(size(), VIEWPORT, 16.0),
            Point::new(300.0, 16.0)
        );
        assert_eq!(
            Anchor::Left.position(size(), VIEWPORT, 16.0),
            Point::new(16.0, 250.0)
        );
    }

    /// A surface too large for the viewport must not be pushed off the top-left
    /// by a margin there is no room for.
    #[test]
    fn an_oversized_surface_stays_at_the_origin() {
        let huge = Size::new(1000.0, 900.0);

        assert_eq!(
            Anchor::BottomRight.position(huge, VIEWPORT, 16.0),
            Point::ORIGIN
        );
        assert_eq!(Anchor::Center.position(huge, VIEWPORT, 16.0), Point::ORIGIN);
    }

    #[test]
    fn each_anchor_slides_from_the_edge_it_hugs() {
        assert_eq!(Anchor::Top.slide_from(), Side::Top);
        assert_eq!(Anchor::TopRight.slide_from(), Side::Top);
        assert_eq!(Anchor::Left.slide_from(), Side::Left);
        assert_eq!(Anchor::Right.slide_from(), Side::Right);
        assert_eq!(Anchor::BottomLeft.slide_from(), Side::Bottom);
        // A centred surface has no edge of its own, so it rises.
        assert_eq!(Anchor::Center.slide_from(), Side::Bottom);
    }

    #[test]
    fn a_sheet_stretches_along_the_edge_it_comes_from_and_a_dialog_does_not() {
        let panel: Layer<'_, ()> = sheet(iced::widget::text("Filters"), Anchor::Right);
        let confirm: Layer<'_, ()> = dialog(iced::widget::text("Sure?"));

        assert!(panel.stretch);
        assert_eq!(panel.anchor, Anchor::Right);
        // Flush against the edge: a sheet with a margin would float.
        assert_eq!(panel.margin, 0.0);
        assert_eq!(panel.radius, 0.0);

        assert!(!confirm.stretch);
        assert_eq!(confirm.anchor, Anchor::Center);
    }

    #[test]
    fn a_dialog_and_a_modal_are_both_backed_by_a_dimmed_page() {
        let confirm: Layer<'_, ()> = dialog(iced::widget::text("Sure?"));
        let panel: Layer<'_, ()> = sheet(iced::widget::text("Filters"), Anchor::Left);

        assert_eq!(confirm.backdrop, Some(0.4));
        assert_eq!(panel.backdrop, Some(0.4));
    }

    /// The line between the two centred surfaces: a modal is a window the user
    /// can move and resize, a dialog is a prompt they cannot.
    #[test]
    fn a_modal_is_a_window_and_a_dialog_is_not() {
        let window: Layer<'_, ()> = modal(iced::widget::text("Hi"), "Settings");
        let confirm: Layer<'_, ()> = dialog(iced::widget::text("Hi"));

        assert_eq!(window.backdrop, Some(0.4));
        assert_eq!(window.title.as_deref(), Some("Settings"));
        assert!(window.draggable);
        assert!(window.resizable);

        assert_eq!(confirm.title, None);
        assert!(!confirm.draggable);
        assert!(!confirm.resizable);
    }

    #[test]
    fn a_plain_layer_has_neither_a_backdrop_nor_chrome() {
        let plain: Layer<'_, ()> = layer(iced::widget::text("Hi"));

        assert_eq!(plain.backdrop, None);
        assert_eq!(plain.title, None);
        assert!(!plain.draggable);
        assert!(!plain.resizable);
    }

    #[test]
    fn each_handle_covers_the_edge_it_is_named_for() {
        let bounds = Rectangle {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 100.0,
        };

        assert_eq!(
            Edge::at(Point::new(102.0, 102.0), bounds),
            Some(Edge::TopLeft)
        );
        assert_eq!(
            Edge::at(Point::new(298.0, 102.0), bounds),
            Some(Edge::TopRight)
        );
        assert_eq!(
            Edge::at(Point::new(102.0, 198.0), bounds),
            Some(Edge::BottomLeft)
        );
        assert_eq!(
            Edge::at(Point::new(298.0, 198.0), bounds),
            Some(Edge::BottomRight)
        );

        assert_eq!(Edge::at(Point::new(102.0, 150.0), bounds), Some(Edge::Left));
        assert_eq!(
            Edge::at(Point::new(298.0, 150.0), bounds),
            Some(Edge::Right)
        );
        assert_eq!(Edge::at(Point::new(200.0, 102.0), bounds), Some(Edge::Top));
        assert_eq!(
            Edge::at(Point::new(200.0, 198.0), bounds),
            Some(Edge::Bottom)
        );

        // The middle is content, not a handle.
        assert_eq!(Edge::at(Point::new(200.0, 150.0), bounds), None);
        // Nor is anywhere outside the surface.
        assert_eq!(Edge::at(Point::new(50.0, 150.0), bounds), None);
    }

    /// A surface small enough for the bands to meet must still have a middle,
    /// or opposite handles would overlap and one of them would be unreachable.
    #[test]
    fn handles_never_swallow_a_tiny_surface() {
        let tiny = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 12.0,
            height: 12.0,
        };

        assert_eq!(Edge::at(Point::new(1.0, 6.0), tiny), Some(Edge::Left));
        assert_eq!(Edge::at(Point::new(11.0, 6.0), tiny), Some(Edge::Right));
        assert_eq!(Edge::at(Point::new(6.0, 6.0), tiny), None);
    }

    #[test]
    fn an_inverted_range_settles_on_its_lower_bound() {
        assert_eq!(clamped(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamped(-5.0, 0.0, 10.0), 0.0);
        assert_eq!(clamped(50.0, 0.0, 10.0), 10.0);
        // A viewport narrower than the minimum size inverts the range.
        assert_eq!(clamped(5.0, 140.0, 100.0), 140.0);
    }

    #[test]
    fn a_window_hanging_outside_the_viewport_is_brought_back_in() {
        assert_eq!(
            clamp_into(Point::new(700.0, 550.0), size(), VIEWPORT),
            Point::new(600.0, 500.0)
        );
        assert_eq!(
            clamp_into(Point::new(-40.0, -40.0), size(), VIEWPORT),
            Point::ORIGIN
        );
        // A window bigger than the viewport has nowhere to go but the origin.
        assert_eq!(
            clamp_into(Point::new(100.0, 100.0), Size::new(900.0, 700.0), VIEWPORT),
            Point::ORIGIN
        );
    }

    #[test]
    fn moving_a_window_keeps_its_size() {
        let moved = dragged(
            Grab::Move,
            window(),
            Vector::new(40.0, -30.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(moved.position, Point::new(140.0, 70.0));
        assert_eq!(moved.size, size());
    }

    #[test]
    fn a_window_cannot_be_dragged_out_of_the_viewport() {
        let moved = dragged(
            Grab::Move,
            window(),
            Vector::new(900.0, 900.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(moved.position, Point::new(600.0, 500.0));
        assert_eq!(moved.size, size());
    }

    #[test]
    fn a_trailing_handle_resizes_without_moving_the_window() {
        let resized = dragged(
            Grab::Resize(Edge::Right),
            window(),
            Vector::new(60.0, 0.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, window().position);
        assert_eq!(resized.size, Size::new(260.0, 100.0));
    }

    /// The half that is easy to get wrong: dragging a leading edge has to move
    /// the window as well as resize it, leaving the opposite edge where it was.
    #[test]
    fn a_leading_handle_moves_the_edge_it_is_on_and_pins_the_other() {
        let resized = dragged(
            Grab::Resize(Edge::Left),
            window(),
            Vector::new(-40.0, 0.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, Point::new(60.0, 100.0));
        assert_eq!(resized.size, Size::new(240.0, 100.0));
        // The right edge has not budged.
        assert_eq!(resized.position.x + resized.size.width, 300.0);
    }

    #[test]
    fn a_corner_handle_works_on_both_axes_at_once() {
        let resized = dragged(
            Grab::Resize(Edge::TopRight),
            window(),
            Vector::new(50.0, -25.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, Point::new(100.0, 75.0));
        assert_eq!(resized.size, Size::new(250.0, 125.0));
    }

    /// Running into the minimum size must stop the edge being dragged, not
    /// start pushing the opposite one along.
    #[test]
    fn the_minimum_size_stops_a_leading_edge_rather_than_dragging_the_window() {
        let resized = dragged(
            Grab::Resize(Edge::Left),
            window(),
            Vector::new(500.0, 0.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.size.width, min().width);
        // Pinned right edge, so the window's left lands exactly min() short of it.
        assert_eq!(resized.position.x, 300.0 - min().width);
        assert_eq!(resized.position.x + resized.size.width, 300.0);
    }

    #[test]
    fn the_minimum_size_stops_a_trailing_edge_too() {
        let resized = dragged(
            Grab::Resize(Edge::Bottom),
            window(),
            Vector::new(0.0, -500.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, window().position);
        assert_eq!(resized.size.height, min().height);
    }

    #[test]
    fn a_window_cannot_be_resized_past_the_viewport() {
        let resized = dragged(
            Grab::Resize(Edge::BottomRight),
            window(),
            Vector::new(900.0, 900.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, window().position);
        assert_eq!(resized.size, Size::new(700.0, 500.0));

        let resized = dragged(
            Grab::Resize(Edge::TopLeft),
            window(),
            Vector::new(-900.0, -900.0),
            min(),
            VIEWPORT,
        );

        assert_eq!(resized.position, Point::ORIGIN);
        assert_eq!(resized.size, Size::new(300.0, 200.0));
    }
}
