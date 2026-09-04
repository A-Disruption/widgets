//! A surface anchored to the element that opens it.
//!
//! A [`Popover`] wraps a trigger and shows arbitrary content beside it. Unlike
//! [`crate::menu`], it makes no assumptions about what that content is: there
//! are no rows, no gutters and no selection. Use it for rich hover cards,
//! pickers, filter panels — anything that belongs *next to* the thing that
//! opened it.
//!
//! ```rust,ignore
//! use iced::widget::{button, column, text};
//!
//! popover(
//!     button(text("Filters")),
//!     column![text("Status"), text("Assignee")].spacing(8),
//! )
//! .side(Side::Bottom)
//! .on_toggle(Message::FiltersToggled)
//! ```
//!
//! # Triggers
//!
//! A popover opens on press by default. [`Popover::on_hover`] switches it to
//! opening on hover, which is what makes an interactive tooltip: content the
//! user can move the cursor into and click.
//!
//! That only works because of the safe corridor. A hover popover sits a few
//! pixels off its trigger, and the cursor has to cross that gap — over neither
//! the trigger nor the popover — to reach it. Closing the moment the cursor
//! leaves the trigger would make the content unreachable. Instead, the cursor
//! is treated as still inside while it stays within the triangle swept from
//! where it left the trigger to the two near corners of the popover, so a
//! diagonal path works. See [`crate::anchor::in_safe_corridor`].
//!
//! # Sizing
//!
//! A popover is as big as its content. [`Popover::min_width`] and
//! [`Popover::max_width`] bound it; the height always follows the content, up
//! to the viewport.
//!
//! # Placement and motion
//!
//! Placement comes from [`crate::anchor`], so a popover that will not fit on
//! its preferred side flips before it is clamped. Transitions come from
//! [`crate::animation`], and default to [`Motion::QUICK`].

use iced::advanced::widget::{self, Operation, tree::Tree};
use iced::advanced::{
    Layout, Shell, Widget,
    layout::{Limits, Node},
    overlay, renderer,
};
use iced::time::Instant;
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Point, Rectangle, Shadow, Size,
    Theme, Vector, keyboard, mouse, touch, window,
};

use crate::anchor::{self, Align, Placement, Side};
use crate::animation::{self, Motion, Transition};

/// The default gap between a trigger and its popover.
const DEFAULT_GAP: f32 = 6.0;

/// The default padding between the popover edge and its content.
const DEFAULT_PADDING: f32 = 12.0;

/// The default corner radius of a popover.
const DEFAULT_RADIUS: f32 = 8.0;

/// How far the safe corridor is widened at the popover end.
///
/// Without this, a popover aligned flush with a narrow trigger leaves an almost
/// degenerate triangle and the corridor is nearly useless.
const CORRIDOR_EXTEND: f32 = 12.0;

/// Creates a [`Popover`] that opens when its trigger is pressed.
pub fn popover<'a, Message, Theme, Renderer>(
    trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Popover<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Popover::new(trigger, content)
}

/// Creates a [`Popover`] that opens on hover.
///
/// This is the interactive tooltip: unlike a plain tooltip, the cursor can
/// travel into the content and interact with it.
pub fn hover_popover<'a, Message, Theme, Renderer>(
    trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Popover<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Popover::new(trigger, content).on_hover()
}

/// What opens a [`Popover`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Trigger {
    /// A press on the trigger toggles the popover.
    #[default]
    Press,
    /// Moving the cursor onto the trigger opens the popover, and leaving both
    /// it and the popover closes it.
    Hover,
}

/// A surface anchored to the element that opens it.
#[allow(missing_debug_implementations)]
pub struct Popover<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    id: Option<widget::Id>,
    trigger: Element<'a, Message, Theme, Renderer>,
    content: Element<'a, Message, Theme, Renderer>,
    trigger_mode: Trigger,
    controlled: Option<bool>,
    placement: Placement,
    padding: f32,
    radius: f32,
    min_width: f32,
    max_width: Option<f32>,
    motion: Motion,
    safe_corridor: bool,
    dismiss_on_outside_press: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme, Renderer> Popover<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Creates a new [`Popover`].
    pub fn new(
        trigger: impl Into<Element<'a, Message, Theme, Renderer>>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            id: None,
            trigger: trigger.into(),
            content: content.into(),
            trigger_mode: Trigger::default(),
            controlled: None,
            placement: Placement::new(Side::Bottom)
                .align(Align::Center)
                .gap(DEFAULT_GAP),
            padding: DEFAULT_PADDING,
            radius: DEFAULT_RADIUS,
            min_width: 0.0,
            max_width: None,
            motion: Motion::QUICK,
            safe_corridor: true,
            dismiss_on_outside_press: true,
            on_toggle: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the [`widget::Id`] of the [`Popover`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Opens the popover on hover instead of on press.
    pub fn on_hover(mut self) -> Self {
        self.trigger_mode = Trigger::Hover;
        self
    }

    /// Hands control of whether the popover is open to the application.
    ///
    /// The popover stops opening and closing itself; it shows exactly what it
    /// is told. Interactions still report through [`Popover::on_toggle`], so
    /// the application can decide what to do with them.
    pub fn open(mut self, is_open: bool) -> Self {
        self.controlled = Some(is_open);
        self
    }

    /// Sets the [`Side`] of the trigger the popover opens on.
    pub fn side(mut self, side: Side) -> Self {
        self.placement.side = side;
        self
    }

    /// Sets how the popover aligns to its trigger along the cross axis.
    pub fn align(mut self, align: Align) -> Self {
        self.placement.align = align;
        self
    }

    /// Sets the gap between the trigger and the popover.
    pub fn gap(mut self, gap: f32) -> Self {
        self.placement.gap = gap;
        self
    }

    /// Sets whether the popover may flip to the opposite side to fit.
    pub fn flip(mut self, flip: bool) -> Self {
        self.placement.flip = flip;
        self
    }

    /// Sets the padding between the popover edge and its content.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Sets the corner radius of the popover.
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Sets a lower bound on the width of the popover.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Sets an upper bound on the width of the popover.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Sets how the popover animates in and out.
    pub fn motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// Opens and closes the popover instantly, with no transition.
    pub fn no_animation(mut self) -> Self {
        self.motion = Motion::NONE;
        self
    }

    /// Sets whether a hover popover keeps itself open while the cursor crosses
    /// the gap towards it.
    ///
    /// Only meaningful with [`Popover::on_hover`]. Turning it off makes the
    /// popover close as soon as the cursor leaves the trigger, which is only
    /// usable when the gap is zero.
    pub fn safe_corridor(mut self, enabled: bool) -> Self {
        self.safe_corridor = enabled;
        self
    }

    /// Sets whether a press outside the popover dismisses it.
    pub fn dismiss_on_outside_press(mut self, dismiss: bool) -> Self {
        self.dismiss_on_outside_press = dismiss;
        self
    }

    /// Sets a callback for when the popover opens or closes.
    pub fn on_toggle(mut self, callback: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(callback));
        self
    }

    /// Sets the style of the [`Popover`].
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Popover`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

/// The index of the trigger within the widget's state tree and layout.
const TRIGGER: usize = 0;

/// The index of the content within the widget's state tree.
const CONTENT: usize = 1;

/// The persistent state of a [`Popover`].
#[derive(Debug, Default)]
struct State {
    /// Whether the popover is logically open.
    is_open: bool,
    /// The open/close transition.
    transition: Transition,
    /// Whether the cursor is over the trigger.
    is_trigger_hovered: bool,
    /// The last cursor position known to be over the trigger.
    ///
    /// The apex of the safe corridor. Without it there is nothing to sweep the
    /// triangle from.
    left_trigger_at: Option<Point>,
    /// The side the popover was last placed on.
    ///
    /// Recorded during layout so that hit-testing the corridor and sliding the
    /// open transition both use the side actually in effect, not the requested
    /// one — they differ whenever the popover has flipped.
    side: Side,
    /// The instant of the frame the popover was last updated in.
    ///
    /// Whether the surface is showing, and how far through its transition it
    /// is, are both read from here rather than from the clock. A frame is not
    /// an instant: `UserInterface` lays the overlay out once during `update`,
    /// caches that layout, then rebuilds the overlay tree in `draw` and hands
    /// it the cached one. Deciding visibility from `Instant::now()` lets a
    /// transition finish *between* the two, at which point the surface leaves
    /// the tree but not the layout — and `overlay::Group` pairs the two up by
    /// position, so every sibling overlay is then drawn against a layout node
    /// belonging to something else. A tooltip beside a closing popover panics
    /// on the mismatch.
    ///
    /// `None` until the first frame, when nothing has opened yet.
    frame: Option<Instant>,
}

impl State {
    /// Opens the popover.
    fn open(&mut self, now: Instant) {
        self.is_open = true;
        self.frame = Some(now);
        self.transition.open(now);
    }

    /// Closes the popover, leaving it visible until the transition finishes.
    fn close(&mut self, now: Instant) {
        self.is_open = false;
        self.left_trigger_at = None;
        self.frame = Some(now);
        self.transition.close(now);
    }

    /// Returns `true` when the popover should still be produced and drawn.
    ///
    /// Answered as of the frame the popover was last updated in, so that one
    /// frame gets one answer however many passes it takes. See
    /// [`State::frame`].
    fn is_showing(&self) -> bool {
        self.is_open
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

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Popover<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        if tree.children.len() != 2 {
            tree.children = vec![
                Tree::new(self.trigger.as_widget()),
                Tree::new(self.content.as_widget()),
            ];
        }

        let (trigger, content) = tree.children.split_at_mut(CONTENT);
        trigger[TRIGGER].diff(self.trigger.as_widget_mut());
        content[0].diff(self.content.as_widget_mut());
    }

    fn size(&self) -> Size<Length> {
        self.trigger.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        // Only the trigger takes part in the view layout; the popover is an
        // overlay and so never shifts the surrounding page.
        let trigger = self
            .trigger
            .as_widget_mut()
            .layout(&mut tree.children[TRIGGER], renderer, limits);

        let size = trigger.size();

        Node::with_children(size, vec![trigger])
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.trigger.as_widget().draw(
            &tree.children[TRIGGER],
            renderer,
            theme,
            style,
            layout.children().next().expect("trigger layout"),
            cursor,
            viewport,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let trigger_layout = layout.children().next().expect("trigger layout");

        {
            let state = tree.state.downcast_mut::<State>();
            state.transition.sync(self.motion);

            // The one place the transition is allowed to advance. Everything
            // else reads the answer back off the state, so that a frame gets a
            // single answer however many passes it takes. See [`State::frame`].
            if let Event::Window(window::Event::RedrawRequested(now)) = event {
                let was_showing = state.is_showing();

                state.frame = Some(*now);

                if state.transition.is_animating(*now) {
                    shell.request_redraw();
                }

                // The frame a surface stops showing on is a frame the overlay
                // has to be laid out on again: it is leaving the tree, and the
                // layout cached for it would otherwise be handed to whichever
                // overlay takes its place.
                if was_showing != state.is_showing() {
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }

            // Application-owned visibility wins over anything the widget
            // decided for itself.
            if let Some(is_open) = self.controlled
                && is_open != state.is_open
            {
                let now = Instant::now();

                if is_open {
                    state.open(now);
                } else {
                    state.close(now);
                }

                shell.invalidate_layout();
                shell.request_redraw();
            }
        }

        let pressed = matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. })
        );

        // The trigger stays a fully live widget: it sees every event first and
        // keeps whatever behaviour it already had. A `button` fires its
        // `on_press`, a `text_input` takes focus and typing, and so on.
        self.trigger.as_widget_mut().update(
            &mut tree.children[TRIGGER],
            event,
            trigger_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );

        // Toggling happens *in addition*, not instead. Deciding on the strength
        // of `is_event_captured` would mean a trigger that handles its own
        // presses — which is most of them — could never open the popover.
        if self.trigger_mode == Trigger::Press && pressed && cursor.is_over(bounds) {
            let state = tree.state.downcast_mut::<State>();
            let was_open = state.is_open;
            let now = Instant::now();

            if self.controlled.is_none() {
                if was_open {
                    state.close(now);
                } else {
                    state.open(now);
                }
            }

            if let Some(on_toggle) = &self.on_toggle {
                shell.publish(on_toggle(!was_open));
            }

            // Deliberately not captured: the trigger has already had the event,
            // and swallowing it here would only hide it from anything else that
            // legitimately wants it.
            shell.invalidate_layout();
            shell.request_redraw();

            return;
        }

        if shell.is_event_captured() {
            return;
        }

        if matches!(
            event,
            Event::Mouse(mouse::Event::CursorMoved { .. }) | Event::Window(window::Event::Unfocused)
        ) {
            let state = tree.state.downcast_mut::<State>();
            let is_hovered = cursor.is_over(bounds);

            if is_hovered != state.is_trigger_hovered {
                state.is_trigger_hovered = is_hovered;
                shell.request_redraw();
            }

            // Remember where the cursor last sat on the trigger; that point is
            // the apex of the safe corridor once it leaves.
            if is_hovered {
                state.left_trigger_at = cursor.position();
            }

            if self.trigger_mode == Trigger::Hover
                && is_hovered
                && !state.is_open
                && self.controlled.is_none()
            {
                state.open(Instant::now());

                if let Some(on_toggle) = &self.on_toggle {
                    shell.publish(on_toggle(true));
                }

                shell.invalidate_layout();
                shell.request_redraw();
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.trigger.as_widget().mouse_interaction(
            &tree.children[TRIGGER],
            layout.children().next().expect("trigger layout"),
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(self.id.as_ref(), layout.bounds());

        operation.traverse(&mut |operation| {
            self.trigger.as_widget_mut().operate(
                &mut tree.children[TRIGGER],
                layout.children().next().expect("trigger layout"),
                renderer,
                operation,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();
        state.transition.sync(self.motion);

        // Deliberately the frame's instant and not the clock's: `overlay` is
        // called once to lay the surface out and again to draw it, and the two
        // have to agree. See [`State::frame`].
        if !state.is_showing() {
            return None;
        }

        let progress = state.progress();
        let is_closing = !state.is_open;

        let mut trigger_bounds = layout.bounds();
        trigger_bounds.x += offset.x;
        trigger_bounds.y += offset.y;

        let (_, content_tree) = tree.children.split_at_mut(CONTENT);

        Some(overlay::Element::new(Box::new(Surface {
            state,
            content: &mut self.content,
            tree: &mut content_tree[0],
            trigger_bounds,
            placement: self.placement,
            padding: self.padding,
            radius: self.radius,
            min_width: self.min_width,
            max_width: self.max_width,
            motion: self.motion,
            progress,
            is_closing,
            trigger_mode: self.trigger_mode,
            safe_corridor: self.safe_corridor,
            dismiss_on_outside_press: self.dismiss_on_outside_press,
            controlled: self.controlled.is_some(),
            on_toggle: self.on_toggle.as_deref(),
            class: &self.class,
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Popover<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(popover: Popover<'a, Message, Theme, Renderer>) -> Self {
        Element::new(popover)
    }
}

/// The anchored surface, as an iced overlay.
struct Surface<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    state: &'a mut State,
    content: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    trigger_bounds: Rectangle,
    placement: Placement,
    padding: f32,
    radius: f32,
    min_width: f32,
    max_width: Option<f32>,
    motion: Motion,
    progress: f32,
    is_closing: bool,
    trigger_mode: Trigger,
    safe_corridor: bool,
    dismiss_on_outside_press: bool,
    controlled: bool,
    on_toggle: Option<&'a dyn Fn(bool) -> Message>,
    class: &'a Theme::Class<'b>,
}

impl<Message, Theme, Renderer> Surface<'_, '_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Returns `true` when the cursor should keep a hover popover open.
    ///
    /// Being over the trigger or the surface itself is obvious. The third case
    /// is the corridor: the cursor is in the gap between them, travelling
    /// towards the surface.
    fn keeps_hover_alive(&self, bounds: Rectangle, cursor: mouse::Cursor) -> bool {
        if cursor.is_over(self.trigger_bounds) || cursor.is_over(bounds) {
            return true;
        }

        if !self.safe_corridor {
            return false;
        }

        let (Some(position), Some(from)) = (cursor.position(), self.state.left_trigger_at) else {
            return false;
        };

        anchor::in_safe_corridor(position, from, bounds, self.state.side, CORRIDOR_EXTEND)
    }

    /// Closes the popover and reports it.
    fn dismiss(&mut self, shell: &mut Shell<'_, Message>) {
        if !self.controlled {
            self.state.close(Instant::now());
        }

        if let Some(on_toggle) = self.on_toggle {
            shell.publish(on_toggle(false));
        }

        shell.invalidate_layout();
        shell.request_redraw();
    }

}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Surface<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        let viewport = Rectangle::with_size(bounds);
        let padding = Padding::from(self.padding);

        let available = Size::new(
            (bounds.width - self.padding * 2.0).max(0.0),
            (bounds.height - self.padding * 2.0).max(0.0),
        );

        // Measured with the width compressed so fluid content reports its
        // intrinsic size rather than swallowing the viewport, exactly as a menu
        // row is measured.
        let limits = Limits::new(Size::ZERO, available).width(Length::Shrink);

        let mut content = self.content.as_widget_mut().layout(self.tree, renderer, &limits);
        let mut width = content.size().width.max(self.min_width - self.padding * 2.0);

        if let Some(max) = self.max_width {
            width = width.min(max - self.padding * 2.0);
        }

        width = width.min(available.width);

        // Re-laid out at the settled width so fluid content fills it.
        if width != content.size().width {
            let pinned = Limits::new(
                Size::new(width, 0.0),
                Size::new(width, available.height),
            );

            content = self.content.as_widget_mut().layout(self.tree, renderer, &pinned);
        }

        let size = Size::new(
            content.size().width + self.padding * 2.0,
            content.size().height + self.padding * 2.0,
        );

        let placed = anchor::place(self.trigger_bounds, size, viewport, self.placement);

        // Recorded for the corridor and the slide, both of which need the side
        // the surface actually landed on rather than the one requested.
        self.state.side = placed.side;

        let offset = animation::slide(placed.side, self.motion.slide, self.progress);

        Node::with_children(
            size,
            vec![content.move_to(Point::new(padding.left, padding.top))],
        )
        .move_to(placed.position + offset)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();

        // Deliberately not faded. A popover holds arbitrary content, and
        // widgets that paint themselves — `button`, `checkbox`, anything with
        // its own `Catalog` — ignore the inherited text color and so would stay
        // at full strength while the surface behind them faded in. Half-faded
        // reads as broken; sliding the whole thing reads as deliberate. See
        // `crate::animation` for why real opacity is not available here.
        let style = theme.style(self.class);

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

        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            &renderer::Style {
                text_color: style.text_color,
            },
            layout.children().next().expect("content layout"),
            cursor,
            &bounds,
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();

        // A surface on its way out is a picture, not a control.
        if self.is_closing {
            return;
        }

        // The content is live and gets first refusal on everything, so a button
        // inside a popover behaves like a button anywhere else.
        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout.children().next().expect("content layout"),
            cursor,
            renderer,
            shell,
            &bounds,
        );

        if shell.is_event_captured() {
            return;
        }

        // Over the surface, the surface owns the pointer.
        //
        // Set before the branches below rather than after them, because several
        // of them return early — and this is not about what the popover does
        // with the event, it is about the page underneath not also getting it.
        // Without it a move across an open popover hovered rows behind it, a
        // wheel over a gap between its items scrolled them, and a press on its
        // own padding landed on whatever it was covering.
        //
        // A press *outside* is deliberately still let through: a popover is not
        // modal, and the trigger has to see the press that toggles it. Callers
        // wanting a press outside swallowed want a `layer` with a backdrop.
        if cursor.is_over(bounds) && is_pointer_event(event) {
            shell.capture_event();
        }

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if self.trigger_mode == Trigger::Hover && !self.keeps_hover_alive(bounds, cursor) {
                    self.dismiss(shell);
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if !self.dismiss_on_outside_press || cursor.is_over(bounds) {
                    return;
                }

                // A press on the trigger is left for the trigger to toggle,
                // otherwise the widget would re-open what was just closed.
                if cursor.is_over(self.trigger_bounds) {
                    return;
                }

                self.dismiss(shell);
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
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

        let interaction = self.content.as_widget().mouse_interaction(
            self.tree,
            layout.children().next().expect("content layout"),
            cursor,
            &layout.bounds(),
            renderer,
        );

        // The shape over the surface is the surface's to decide even when its
        // content has no opinion, or the widget underneath supplies one — an
        // I-beam over a popover laid across a text field, for text nobody can
        // reach.
        if interaction == mouse::Interaction::None && cursor.is_over(layout.bounds()) {
            return mouse::Interaction::Idle;
        }

        interaction
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content.as_widget_mut().operate(
            self.tree,
            layout.children().next().expect("content layout"),
            renderer,
            operation,
        );
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'a>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let bounds = layout.bounds();

        self.content.as_widget_mut().overlay(
            self.tree,
            layout.children().next()?,
            renderer,
            &bounds,
            Vector::ZERO,
        )
    }
}

/// Is this an event a surface should keep to itself when the pointer is over
/// it?
///
/// Every mouse and touch event except `CursorLeft`: the cursor leaving the
/// window is not over anything, and swallowing it is how the page underneath
/// gets stuck hovered. Keyboard events are not included — focus decides those,
/// not position.
fn is_pointer_event(event: &Event) -> bool {
    match event {
        Event::Mouse(mouse::Event::CursorLeft) => false,
        Event::Mouse(_) | Event::Touch(_) => true,
        _ => false,
    }
}

/// The appearance of a [`Popover`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the surface.
    pub background: Background,
    /// The [`Border`] of the surface. Its radius is overridden by
    /// [`Popover::radius`].
    pub border: Border,
    /// The [`Shadow`] cast by the surface.
    pub shadow: Shadow,
    /// The text [`Color`] inherited by the content.
    pub text_color: Color,
}

/// A boxed [`Popover`] style function.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// The theme catalog of a [`Popover`].
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

/// The default style of a [`Popover`], drawn from the palette of the theme.
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
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
        text_color: palette.background.base.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::time::Duration;

    #[derive(Debug, Clone, PartialEq)]
    enum Message {}

    fn state_open_at(now: Instant) -> State {
        let mut state = State::default();
        state.open(now);
        state
    }

    #[test]
    fn a_closing_popover_stays_showing_until_its_transition_ends() {
        let start = Instant::now();
        let mut state = state_open_at(start);

        let settled = start + Duration::from_millis(300);
        state.close(settled);

        assert!(state.is_showing(), "still fading out");

        // Frames are what carry the transition forward, and `update` is where
        // they land. Until one lands past the end of it, the surface shows.
        state.frame = Some(settled + Motion::QUICK.duration + Duration::from_millis(1));

        assert!(!state.is_showing());
    }

    /// Closing must forget where the cursor left the trigger, or the next open
    /// would sweep its corridor from a stale point.
    #[test]
    fn closing_forgets_the_corridor_apex() {
        let start = Instant::now();
        let mut state = state_open_at(start);

        state.left_trigger_at = Some(Point::new(10.0, 10.0));
        state.close(start);

        assert_eq!(state.left_trigger_at, None);
    }

    #[test]
    fn the_default_popover_opens_below_its_trigger_on_a_press() {
        let popover: Popover<'_, Message> =
            Popover::new(iced::widget::text("Trigger"), iced::widget::text("Content"));

        assert_eq!(popover.trigger_mode, Trigger::Press);
        assert_eq!(popover.placement.side, Side::Bottom);
        assert_eq!(popover.placement.align, Align::Center);
    }

    /// A closing popover must not vanish from the overlay tree *between* the
    /// layout pass and the draw pass.
    ///
    /// `UserInterface` lays the overlay out once, during `update`, and caches
    /// that layout. `draw` then rebuilds the overlay tree and hands it the
    /// cached layout, and `overlay::Group` pairs the two up by position. If a
    /// popover's transition happens to finish in the gap between the two, the
    /// group loses a child and every sibling overlay — a tooltip, say — is
    /// drawn against a layout node belonging to something else, which panics
    /// as soon as that node has the wrong shape.
    ///
    /// So visibility has to be decided from the frame the widget was last
    /// updated in, not from the wall clock.
    #[test]
    fn a_transition_finishing_mid_frame_does_not_desync_sibling_overlays() {
        use iced::advanced::shell;
        use iced::widget::{column, container, text, tooltip};
        use iced::{Length, Point, Size};
        use iced_runtime::user_interface::{Cache, UserInterface};

        #[derive(Debug, Clone, PartialEq)]
        enum Msg {}

        fn view(is_open: bool) -> Element<'static, Msg, Theme, ()> {
            column![
                Element::from(
                    popover(
                        container(text("Trigger"))
                            .width(Length::Fixed(120.0))
                            .height(Length::Fixed(30.0)),
                        // A leaf: its layout node has no children, so a
                        // sibling handed it by mistake panics rather than
                        // quietly drawing the wrong thing.
                        text("Panel"),
                    )
                    .open(is_open)
                ),
                Element::from(tooltip(
                    container(text("Cell"))
                        .width(Length::Fixed(120.0))
                        .height(Length::Fixed(30.0)),
                    container(text("Tip")).padding(5),
                    tooltip::Position::Bottom,
                )),
            ]
            .into()
        }

        fn drive(
            ui: &mut UserInterface<'_, Msg, Theme, ()>,
            event: Event,
            cursor: mouse::Cursor,
            renderer: &mut (),
        ) {
            let mut bus = shell::Bus::new();

            let _ = ui.update(
                &iced::window::Headless,
                &shell::Waker::noop(),
                &[event],
                cursor,
                renderer,
                &mut bus,
            );
        }

        let bounds = Size::new(400.0, 400.0);
        let mut renderer = ();

        // Over the tooltip's trigger, which sits under the popover's.
        let cursor = mouse::Cursor::Available(Point::new(60.0, 45.0));

        let redraw = |now| Event::Window(window::Event::RedrawRequested(now));

        // Open the popover, and hover the tooltip, so both overlays are live.
        let mut cache = Cache::new();

        for _ in 0..2 {
            let mut ui = UserInterface::build(view(true), bounds, cache, &mut renderer);
            drive(&mut ui, redraw(Instant::now()), cursor, &mut renderer);
            cache = ui.into_cache();
        }

        // Close it. The surface is still showing — it is animating out — so
        // the overlay laid out here holds both it and the tooltip.
        let mut ui = UserInterface::build(view(false), bounds, cache, &mut renderer);
        drive(&mut ui, redraw(Instant::now()), cursor, &mut renderer);

        // The transition ends before the frame is drawn.
        std::thread::sleep(Motion::QUICK.duration + Duration::from_millis(50));

        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &iced::advanced::renderer::Style::default(),
            cursor,
        );
    }

    #[test]
    fn hover_popover_opens_on_hover() {
        let popover: Popover<'_, Message> =
            hover_popover(iced::widget::text("Trigger"), iced::widget::text("Content"));

        assert_eq!(popover.trigger_mode, Trigger::Hover);
    }
}
