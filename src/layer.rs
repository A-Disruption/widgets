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
//! that the user asked to close it — by pressing the backdrop or hitting
//! Escape. Acting on that is up to the application, which is what lets a modal
//! refuse to close while a save is in flight.
//!
//! # Backdrop
//!
//! Without [`Layer::backdrop`] a layer is a floating surface: it draws over the
//! page but events reach whatever is underneath, which is what a toast wants.
//! With one, the layer is modal — the backdrop dims the page and swallows every
//! press that does not land on the surface.
//!
//! # Motion
//!
//! The surface slides in from whichever viewport edge it is anchored to. It is
//! deliberately not faded: a layer holds arbitrary content, and widgets that
//! paint themselves ignore an inherited text color, so fading would leave
//! buttons at full strength over a half-drawn panel. The backdrop *is* faded —
//! it is a plain quad this widget draws itself, with no children to disagree
//! with it.

use iced::advanced::widget::{self, Operation, tree::Tree};
use iced::advanced::{
    Layout, Shell, Widget,
    layout::{Limits, Node},
    overlay, renderer,
};
use iced::time::Instant;
use iced::{
    Background, Border, Color, Element, Event, Length, Point, Rectangle, Shadow, Size, Theme,
    Vector, keyboard, mouse, touch, window,
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
/// resize. Visibility is the application's, as with every layer.
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

/// Creates a modal [`Layer`]: centred, with a dimmed backdrop.
pub fn modal<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Layer<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    Layer::new(content).backdrop(0.4)
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
    max_width: Option<f32>,
    stretch: bool,
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
            max_width: None,
            stretch: false,
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
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Sets an upper bound on the width of the surface.
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
    /// decides. Without this, backdrop presses and Escape do nothing.
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

/// The persistent state of a [`Layer`].
#[derive(Debug, Default)]
struct State {
    /// Whether the layer was showing as of the last event.
    ///
    /// Compared against the value the application passed in, to notice the
    /// moment it changes and start the transition.
    is_open: bool,
    transition: Transition,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Layer<'a, Message, Theme, Renderer>
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

        if self.is_open != state.is_open {
            let now = Instant::now();

            state.is_open = self.is_open;

            if self.is_open {
                state.transition.open(now);
            } else {
                state.transition.close(now);
            }

            shell.invalidate_layout();
            shell.request_redraw();
        }

        if let Event::Window(window::Event::RedrawRequested(now)) = event
            && state.transition.is_animating(*now)
        {
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

        let now = Instant::now();

        // The surface outlives the close so it can animate out.
        if !self.is_open && !state.transition.is_visible(now) {
            return None;
        }

        let progress = state.transition.progress(now);

        Some(overlay::Element::new(Box::new(Surface {
            content: &mut self.content,
            tree: &mut tree.children[0],
            anchor: self.anchor,
            margin: self.margin,
            padding: self.padding,
            radius: self.radius,
            min_width: self.min_width,
            max_width: self.max_width,
            stretch: self.stretch,
            motion: self.motion,
            progress,
            is_closing: !self.is_open,
            backdrop: self.backdrop,
            dismiss_on_backdrop: self.dismiss_on_backdrop,
            dismiss_on_escape: self.dismiss_on_escape,
            on_dismiss: self.on_dismiss.as_ref(),
            class: &self.class,
            viewport: Rectangle::default(),
        })))
    }
}

impl<'a, Message, Theme, Renderer> From<Layer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + 'a,
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
    anchor: Anchor,
    margin: f32,
    padding: f32,
    radius: f32,
    min_width: f32,
    max_width: Option<f32>,
    stretch: bool,
    motion: Motion,
    progress: f32,
    is_closing: bool,
    backdrop: Option<f32>,
    dismiss_on_backdrop: bool,
    dismiss_on_escape: bool,
    on_dismiss: Option<&'a Message>,
    class: &'a Theme::Class<'b>,
    /// The viewport, recorded during layout so `draw` knows how far the
    /// backdrop has to reach.
    viewport: Rectangle,
}

impl<Message, Theme, Renderer> Surface<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer,
{
    /// Reports that the user asked to close the layer.
    fn dismiss(&self, shell: &mut Shell<'_, Message>) {
        if let Some(message) = self.on_dismiss {
            shell.publish(message.clone());
        }
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
        self.viewport = viewport;
        let chrome = self.padding * 2.0;

        let available = Size::new(
            (bounds.width - self.margin * 2.0 - chrome).max(0.0),
            (bounds.height - self.margin * 2.0 - chrome).max(0.0),
        );

        // Measured compressed so fluid content reports its intrinsic width
        // rather than swallowing the viewport.
        let limits = Limits::new(Size::ZERO, available).width(Length::Shrink);
        let mut content = self.content.as_widget_mut().layout(self.tree, renderer, &limits);

        let mut width = content.size().width.max(self.min_width - chrome);

        if let Some(max) = self.max_width {
            width = width.min(max - chrome);
        }

        width = width.min(available.width);

        if width != content.size().width {
            let pinned = Limits::new(Size::new(width, 0.0), Size::new(width, available.height));
            content = self.content.as_widget_mut().layout(self.tree, renderer, &pinned);
        }

        let mut size = Size::new(
            content.size().width + chrome,
            content.size().height + chrome,
        );

        // A stretched surface fills the edge it hugs: the cross axis of its
        // anchor becomes the viewport extent, which is what makes a sheet read
        // as a panel sliding out rather than a floating card.
        if self.stretch {
            match self.anchor.slide_from() {
                Side::Left | Side::Right => size.height = bounds.height,
                Side::Top | Side::Bottom => {
                    size.width = bounds.width;

                    // The measuring pass compressed fluid widths, so content
                    // that wants to fill the sheet has to be laid out again
                    // against the width the sheet actually took.
                    let filled = (size.width - chrome).max(0.0);
                    let pinned =
                        Limits::new(Size::new(filled, 0.0), Size::new(filled, available.height));

                    content = self.content.as_widget_mut().layout(self.tree, renderer, &pinned);
                    size.height = content.size().height + chrome;
                }
            }
        }

        let position = self.anchor.position(size, viewport, self.margin);

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

        Node::with_children(
            size,
            vec![content.move_to(Point::new(self.padding, self.padding))],
        )
        .move_to(position + offset)
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
        let style = theme.style(self.class);

        // The backdrop is the one thing here with no children of its own, so it
        // is the one thing that can honestly be faded.
        if let Some(alpha) = self.backdrop {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: self.viewport,
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

        self.content.as_widget().mouse_interaction(
            self.tree,
            layout.children().next().expect("content layout"),
            cursor,
            &layout.bounds(),
            renderer,
        )
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
}

/// The appearance of a [`Layer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the surface.
    pub background: Background,
    /// The [`Border`] of the surface. Its radius is overridden by
    /// [`Layer::radius`].
    pub border: Border,
    /// The [`Shadow`] cast by the surface.
    pub shadow: Shadow,
    /// The text [`Color`] inherited by the content.
    pub text_color: Color,
    /// The [`Color`] the backdrop dims the page with.
    ///
    /// Its alpha is replaced by the value given to [`Layer::backdrop`].
    pub backdrop_color: Color,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anchor::Side;

    const VIEWPORT: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn size() -> Size {
        Size::new(200.0, 100.0)
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

    #[test]
    fn a_modal_gets_a_backdrop_and_a_plain_layer_does_not() {
        let plain: Layer<'_, ()> = layer(iced::widget::text("Hi"));
        let dialog: Layer<'_, ()> = modal(iced::widget::text("Hi"));

        assert_eq!(plain.backdrop, None);
        assert_eq!(dialog.backdrop, Some(0.4));
    }

}
