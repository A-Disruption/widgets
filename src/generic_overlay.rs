use iced::animation::{Animation, Easing};
use iced::time::{Duration, Instant};
use iced::window;
use iced::{
    Alignment, Background, Border, Color, Element, Event, Length, Padding, Pixels, Point,
    Rectangle, Shadow, Size, Vector,
    advanced::{
        Layout, Overlay as _, Renderer as _, Shell, Widget,
        layout::{self, Limits, Node},
        overlay, renderer, text,
        text::Renderer as _,
        widget::operation::Operation,
        widget::{self, tree::Tree},
    },
    alignment::Vertical,
    border::Radius,
    keyboard, mouse, touch,
    widget::button,
};
use iced::advanced::graphics::core::length::{Bounds, Constraint, Sizing};

const HEADER_HEIGHT: f32 = 32.0;
const CLOSE_BUTTON_SIZE: f32 = 30.0;
const CLOSE_BUTTON_OFFSET: f32 = 1.0;
const CONTENT_PADDING: f32 = 15.0;
const RESIZE_HANDLE_SIZE: f32 = 8.0; // Size of resize hit areas
const MIN_OVERLAY_SIZE: f32 = 100.0; // Minimum overlay dimensions

/// Collapses a [`Length`] to either `Fill` or `Shrink`, depending on whether it
/// grows with the available space.
fn fluid(length: Length) -> Length {
    if length.is_fill() {
        Length::Fill
    } else {
        Length::Shrink
    }
}

/// Returns `true` when a [`Length`] resolves to the intrinsic size of its contents.
fn is_intrinsic(length: Length) -> bool {
    matches!(length, Length::Shrink | Length::Fit)
        || matches!(
            length,
            Length::Bounded {
                sizing: Sizing::Shrink | Sizing::Fit,
                ..
            }
        )
}

/// Resolves a [`Length`] to a concrete amount of pixels, given the space available
/// to fill and the intrinsic size of the contents.
fn resolve_length(length: Length, available: f32, intrinsic: impl FnOnce() -> f32) -> f32 {
    match length {
        Length::Fixed(amount) => amount,
        Length::Fill | Length::FillPortion(_) => available,
        Length::Shrink | Length::Fit => intrinsic(),
        Length::Fluid(Constraint::Min(min)) => available.max(min),
        Length::Fluid(Constraint::Max) => available.min(intrinsic()),
        Length::Bounded { bounds, sizing } => {
            let amount = match sizing {
                Sizing::Fill(_) => available,
                Sizing::Fit | Sizing::Shrink => intrinsic(),
            };

            match bounds {
                Bounds::Min(min) => amount.max(min),
                Bounds::Max(max) => amount.min(max),
                Bounds::Both { min, max } => amount.clamp(min, max.max(min)),
            }
        }
    }
}

/// Helper function to create an overlay button
pub fn overlay_button<'a, Message, Theme, Renderer>(
    button_label: impl Into<Element<'a, Message, Theme, Renderer>>,
    header_title: impl Into<String>,
    overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    OverlayButton::new(button_label, header_title, overlay_content)
}

/// Helper function to create an interactive tooltip ( hover button to open overlay )
pub fn interactive_tooltip<'a, Message, Theme, Renderer>(
    button_label: impl Into<Element<'a, Message, Theme, Renderer>>,
    overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    OverlayButton::new(button_label, "", overlay_content)
        .hide_header()
        .close_on_click_outside()
        .on_hover()
}

/// Helper function to create a dropdown menu overlay
pub fn dropdown_menu<'a, Message, Theme, Renderer>(
    button_label: impl Into<Element<'a, Message, Theme, Renderer>>,
    overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    OverlayButton::new(button_label, "", overlay_content)
        .hide_header()
        .close_on_click_outside()
        .overlay_width(Length::Fixed(150.0))
        .overlay_padding(1.0)
        .overlay_radius(0.0)
        .on_hover()
        .hover_gap(0.0)
        .hover_alignment(Alignment::Start)
        .width(Length::Fill)
}

/// Helper function to create a dropdown menu overlay
pub fn dropdown_root<'a, Message, Theme, Renderer>(
    button_label: impl Into<Element<'a, Message, Theme, Renderer>>,
    overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    OverlayButton::new(button_label, "", overlay_content)
        .hide_header()
        .close_on_click_outside()
        .overlay_width(Length::Fixed(150.0))
        .overlay_padding(1.0)
        .overlay_radius(0.0)
        .hover_positions_on_click()
        .hover_position(Position::Bottom)
        .hover_gap(0.0)
        .hover_alignment(Alignment::Start)
}

/// A button that opens a draggable overlay with custom content
#[allow(missing_debug_implementations)]
pub struct OverlayButton<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog + button::Catalog,
    Renderer: text::Renderer,
{
    /// Widget Id for Operations
    id: Option<widget::Id>,
    /// The button label
    button_content: Element<'a, Message, Theme, Renderer>,
    /// The overlay title
    title: String,
    /// text size for title text
    title_text_size: Option<Pixels>,
    /// font for title text
    title_font: Option<Renderer::Font>,
    /// Function to create the overlay content (called each time)
    content: Element<'a, Message, Theme, Renderer>,
    /// Sets the radius of the overlay
    overlay_radius: f32,
    /// Optional width for the overlay (defaults to 400px)
    overlay_width: Option<SizeStrategy<'a>>,
    /// Optional height for the overlay (defaults to content height)
    overlay_height: Option<SizeStrategy<'a>>,
    /// Optional padding for the overlay (defaults to CONTENT_PADDING)
    overlay_padding: f32,
    /// Button width
    width: Length,
    /// Button height
    height: Length,
    /// Button padding
    padding: Padding,
    /// Should button clip content
    clip: bool,
    /// Callback when the overlay is opened
    on_open: Option<Box<dyn Fn(Point, Size) -> Message + 'a>>,
    /// Callback when the overlay is closed
    on_close: Option<Box<dyn Fn() -> Message + 'a>>,
    /// Callback when the overlay is opened/closed
    on_toggle: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    /// Hover Config
    hover: Hover,
    /// Use Hover layout with click to open.
    hover_positions_on_click: bool,
    /// Class of the Overlay
    class: <Theme as Catalog>::Class<'a>,
    /// Status from button widget to match style
    status: Option<button::Status>,
    /// Button class
    button_class: <Theme as button::Catalog>::Class<'a>,
    /// is_press to match button status
    is_pressed: bool,
    /// If true, blocks interaction with content behind overlay
    opaque: bool,
    /// Alpha value for the opaque backdrop (0.1 to 1.0)
    opaque_alpha: f32,
    /// If true, clicking outside the overlay closes it
    close_on_click_outside: bool,
    /// If true, hides the header completely (no title bar or close button)
    hide_header: bool,
    /// If true, removes the X button from header
    hide_close_button: bool,
    /// If true, prevents the overlay from being dragged via the header or Ctrl+drag
    block_dragging: bool,
    /// Resize mode for the overlay
    resizable: ResizeMode,
    /// reset size and position on overlay closure
    reset_on_close: bool,
    /// Externally controlled open state
    external_is_open: Option<bool>,
    /// Forward all updates to base Element
    interactive_base: bool,
    /// Whether to animate open/close transitions
    animate: bool,
    /// Duration for open/close animation (None = 200ms)
    animation_duration: Option<Duration>,
    /// Easing for open/close animation
    animation_easing: Easing,
    /// Whether to use safe triangle to keep hover overlays open during diagonal cursor movement
    safe_triangle: bool,
}

impl<'a, Message, Theme, Renderer> OverlayButton<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    /// Creates a new overlay button with the given label and content function
    pub fn new(
        label: impl Into<Element<'a, Message, Theme, Renderer>>,
        title: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let button_content = label.into();
        let size = button_content.as_widget().size();

        Self {
            // Overlay Button
            id: None,
            button_content,
            width: fluid(size.width),
            height: fluid(size.height),
            padding: DEFAULT_PADDING,
            button_class: <Theme as button::Catalog>::default(),

            // Overlay Header
            title: title.into(),
            title_text_size: None,
            title_font: None,

            // Overlay Content
            content: content.into(),
            overlay_radius: 12.0,
            overlay_width: None,
            overlay_height: None,
            overlay_padding: CONTENT_PADDING,
            clip: false,
            class: <Theme as Catalog>::default(),
            status: None,

            // Callbacks
            on_open: None,
            on_close: None,
            on_toggle: None,

            // Overlay behavior options
            hover: Hover::default(),
            hover_positions_on_click: false,
            is_pressed: false,
            opaque: false,
            opaque_alpha: 0.3,
            close_on_click_outside: false,
            hide_header: false,
            hide_close_button: false,
            block_dragging: false,
            resizable: ResizeMode::None,
            reset_on_close: false,
            external_is_open: None,
            interactive_base: false,
            animate: false,
            animation_duration: None,
            animation_easing: Easing::EaseOutCubic,
            safe_triangle: true,
        }
    }

    /// Sets the [`widget::Id`] of the [`Generic Overlay`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Manually control whether the overlay is open or closed.
    ///
    /// If this is set, the widget will sync its internal state to this value.
    /// You should use this in conjunction with `on_toggle` to update your application state with existing internal open/close messages.
    pub fn is_open(mut self, is_open: bool) -> Self {
        self.external_is_open = Some(is_open);
        self
    }

    /// Sets the overlay width
    pub fn overlay_width(mut self, width: impl Into<SizeStrategy<'a>>) -> Self {
        self.overlay_width = Some(width.into());
        self
    }

    /// Sets the overlay height
    pub fn overlay_height(mut self, height: impl Into<SizeStrategy<'a>>) -> Self {
        self.overlay_height = Some(height.into());
        self
    }

    // "Rule Style" convenience method for dynamic width
    // Usage: .overlay_width_dynamic(|available| Length::Fixed(available * 0.8))
    pub fn overlay_width_dynamic(mut self, calc: impl Fn(f32) -> Length + 'a) -> Self {
        self.overlay_width = Some(SizeStrategy::Dynamic(Box::new(calc)));
        self
    }

    // "Rule Style" convenience method for dynamic height
    // Usage: .overlay_height_dynamic(|available| Length::Fixed(available * 0.8))
    pub fn overlay_height_dynamic(mut self, calc: impl Fn(f32) -> Length + 'a) -> Self {
        self.overlay_height = Some(SizeStrategy::Dynamic(Box::new(calc)));
        self
    }

    /// Sets the overlay padding
    pub fn overlay_padding(mut self, padding: f32) -> Self {
        self.overlay_padding = padding;
        self
    }

    /// Sets the overlay radius
    pub fn overlay_radius(mut self, radius: f32) -> Self {
        self.overlay_radius = radius;
        self
    }

    /// Sets the button width
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the button height
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the button padding
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets a callback for when the overlay is opened
    pub fn on_open(mut self, callback: impl Fn(Point, Size) -> Message + 'a) -> Self {
        self.on_open = Some(Box::new(callback));
        self
    }

    /// Sets a callback for when the overlay is closed
    pub fn on_close(mut self, callback: impl Fn() -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(callback));
        self
    }

    /// Sets a callback for when the overlay is opened/closed
    pub fn on_toggle(mut self, toggled: impl Fn(bool) -> Message + 'a) -> Self {
        self.on_toggle = Some(Box::new(toggled));
        self
    }

    /// Enable hover positions on_click - to use in menus :D
    #[must_use]
    pub fn hover_positions_on_click(mut self) -> Self {
        self.hover_positions_on_click = true;
        self
    }

    /// Enable hover-to-open mode
    #[must_use]
    pub fn on_hover(mut self) -> Self {
        self.hover.enabled = true;
        self
    }

    #[must_use]
    pub fn hover_position(mut self, position: Position) -> Self {
        self.hover.config.position = position;
        self
    }

    #[must_use]
    pub fn hover_gap(mut self, gap: f32) -> Self {
        self.hover.config.gap = gap;
        self
    }

    #[must_use]
    pub fn hover_alignment(mut self, alignment: Alignment) -> Self {
        self.hover.config.alignment = alignment;
        self
    }

    #[must_use]
    pub fn hover_mode(mut self, mode: PositionMode) -> Self {
        self.hover.config.mode = mode;
        self
    }

    #[must_use]
    pub fn hover_snap(mut self, snap: bool) -> Self {
        self.hover.config.snap_within_viewport = snap;
        self
    }

    /// Sets whether the contents of the [`Button`] should be clipped on
    /// overflow.
    pub fn button_clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Sets the style of the button using button's styling system
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, button::Status) -> button::Style + 'a) -> Self
    where
        <Theme as button::Catalog>::Class<'a>: From<button::StyleFn<'a, Theme>>,
    {
        self.button_class = (Box::new(style) as button::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the button class directly
    #[must_use]
    pub fn button_class(mut self, class: impl Into<<Theme as button::Catalog>::Class<'a>>) -> Self {
        self.button_class = class.into();
        self
    }

    /// Sets the overlay style
    #[must_use]
    pub fn overlay_style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the class of the Overlay
    #[must_use]
    pub fn overlay_class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// If true, clicking outside the overlay will close it
    #[must_use]
    pub fn close_on_click_outside(mut self) -> Self {
        self.close_on_click_outside = true;
        self
    }

    /// Makes the overlay opaque, blocking interaction with content behind it
    #[must_use]
    pub fn opaque(mut self, opaque: bool) -> Self {
        self.opaque = opaque;
        self
    }

    /// Sets the alpha/darkness of the opaque backdrop (clamped to 0.1â€“1.0)
    #[must_use]
    pub fn opaque_alpha(mut self, alpha: f32) -> Self {
        self.opaque_alpha = alpha.clamp(0.1, 1.0);
        self
    }

    /// If true, hides the header (no title bar or close button)
    #[must_use]
    pub fn hide_header(mut self) -> Self {
        self.hide_header = true;
        self
    }

    /// If true, hides the close button in header
    #[must_use]
    pub fn hide_close_button(mut self) -> Self {
        self.hide_close_button = true;
        self
    }

    /// Prevents the overlay from being dragged via the header or Ctrl+drag
    #[must_use]
    pub fn block_dragging(mut self) -> Self {
        self.block_dragging = true;
        self
    }

    /// Sets the resize mode for the overlay
    #[must_use]
    pub fn resizable(mut self, mode: ResizeMode) -> Self {
        self.resizable = mode;
        self
    }

    /// Reset the position and size of the [`Generic Overlay`] each time it's closed.
    pub fn reset_on_close(mut self) -> Self {
        self.reset_on_close = true;
        self
    }

    pub fn interactive_base(mut self, interactive: bool) -> Self {
        self.interactive_base = interactive;
        self
    }

    /// Enables or disables smooth open/close animation. Default is `false`.
    #[must_use]
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }

    /// Enables animation with a quick (200ms) duration.
    #[must_use]
    pub fn quick_animation(mut self) -> Self {
        self.animate = true;
        self.animation_duration = Some(Duration::from_millis(200));
        self
    }

    /// Enables animation with a slow (400ms) duration.
    #[must_use]
    pub fn slow_animation(mut self) -> Self {
        self.animate = true;
        self.animation_duration = Some(Duration::from_millis(400));
        self
    }

    /// Enables animation with a custom duration.
    #[must_use]
    pub fn animation_duration(mut self, duration: Duration) -> Self {
        self.animate = true;
        self.animation_duration = Some(duration);
        self
    }

    /// Sets the easing function for the animation. Also enables animation.
    #[must_use]
    pub fn animation_easing(mut self, easing: Easing) -> Self {
        self.animate = true;
        self.animation_easing = easing;
        self
    }

    /// Enables or disables the safe triangle for hover mode. Default is `true`.
    ///
    /// The safe triangle keeps the overlay open while the cursor moves diagonally
    /// from the trigger widget toward the overlay, preventing accidental dismissal.
    #[must_use]
    pub fn safe_triangle(mut self, enabled: bool) -> Self {
        self.safe_triangle = enabled;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Left,
    Right,
}

impl Position {
    pub const ALL: &'static [Self] = &[Self::Top, Self::Right, Self::Bottom, Self::Left];
}

impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Position::Top => write!(f, "Top"),
            Position::Right => write!(f, "Right"),
            Position::Bottom => write!(f, "Bottom"),
            Position::Left => write!(f, "Left"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Hover {
    pub enabled: bool,
    pub config: HoverConfig,
}

#[derive(Debug, Clone)]
pub struct HoverConfig {
    position: Position,
    gap: f32,
    snap_within_viewport: bool,
    alignment: Alignment,
    buffer: f32,
    mode: PositionMode,
}

impl Default for HoverConfig {
    fn default() -> Self {
        Self {
            position: Position::Right,
            gap: 5.0,
            snap_within_viewport: true,
            alignment: Alignment::Center,
            buffer: 10.0,
            mode: PositionMode::Outside,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionMode {
    /// Overlay appears outside/adjacent to the button (default)
    Outside,
    /// Overlay appears inside/overlapping the button bounds
    Inside,
}

impl Default for PositionMode {
    fn default() -> Self {
        Self::Outside
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResizeMode {
    /// Not resizable
    None,
    /// Always resizable
    Always,
    /// Resizable only when Ctrl is pressed
    WithCtrl,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ResizeEdge {
    None,
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn from_position(cursor_pos: Point, bounds: Rectangle) -> Self {
        let handle = RESIZE_HANDLE_SIZE;

        let on_left = cursor_pos.x >= bounds.x && cursor_pos.x <= bounds.x + handle;
        let on_right = cursor_pos.x >= bounds.x + bounds.width - handle
            && cursor_pos.x <= bounds.x + bounds.width;
        let on_top = cursor_pos.y >= bounds.y && cursor_pos.y <= bounds.y + handle;
        let on_bottom = cursor_pos.y >= bounds.y + bounds.height - handle
            && cursor_pos.y <= bounds.y + bounds.height;

        match (on_left, on_right, on_top, on_bottom) {
            (true, false, true, false) => Self::TopLeft,
            (false, true, true, false) => Self::TopRight,
            (true, false, false, true) => Self::BottomLeft,
            (false, true, false, true) => Self::BottomRight,
            (true, false, false, false) => Self::Left,
            (false, true, false, false) => Self::Right,
            (false, false, true, false) => Self::Top,
            (false, false, false, true) => Self::Bottom,
            _ => Self::None,
        }
    }

    fn cursor_icon(&self) -> mouse::Interaction {
        match self {
            Self::None => mouse::Interaction::default(),
            Self::Top | Self::Bottom => mouse::Interaction::ResizingVertically,
            Self::Left | Self::Right => mouse::Interaction::ResizingHorizontally,
            Self::TopRight | Self::BottomLeft => mouse::Interaction::ResizingDiagonallyUp,
            Self::TopLeft | Self::BottomRight => mouse::Interaction::ResizingDiagonallyDown,
        }
    }

    fn affects_height(&self) -> bool {
        matches!(
            self,
            Self::Top
                | Self::Bottom
                | Self::TopLeft
                | Self::TopRight
                | Self::BottomLeft
                | Self::BottomRight
        )
    }
}

/// Helper function to check if any descendant OverlayButton has an open overlay.
/// This enables parent overlays to stay open while nested (child) overlays are active.
fn has_open_descendant_overlays<P>(tree: &Tree) -> bool
where
    P: iced::advanced::text::Paragraph + 'static,
{
    let overlay_tag = widget::tree::Tag::of::<State<P>>();
    if tree.tag == overlay_tag {
        let state = tree.state.downcast_ref::<State<P>>();
        if state.is_open {
            return true;
        }
    }
    // Recurse into children
    tree.children.iter().any(has_open_descendant_overlays::<P>)
}

/// Returns true if the point `p` is inside the triangle formed by vertices `a`, `b`, `c`.
/// Uses the sign-of-cross-product method.
fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let sign = |p1: Point, p2: Point, p3: Point| -> f32 {
        (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
    };
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// Returns the two overlay corners closest to the trigger button, given the hover position.
/// `extend` widens the base of the triangle perpendicularly (above/below for Left/Right
/// overlays, left/right for Top/Bottom overlays) so diagonal cursor movement has a
/// generous safe zone even when the overlay and button are closely aligned.
fn overlay_near_corners(b: Rectangle, pos: &Position, extend: f32) -> (Point, Point) {
    match pos {
        Position::Right => (
            Point::new(b.x, b.y - extend),
            Point::new(b.x, b.y + b.height + extend),
        ),
        Position::Left => (
            Point::new(b.x + b.width, b.y - extend),
            Point::new(b.x + b.width, b.y + b.height + extend),
        ),
        Position::Bottom => (
            Point::new(b.x - extend, b.y),
            Point::new(b.x + b.width + extend, b.y),
        ),
        Position::Top => (
            Point::new(b.x - extend, b.y + b.height),
            Point::new(b.x + b.width + extend, b.y + b.height),
        ),
    }
}

#[derive(Debug, Clone)]
struct State<P>
where
    P: iced::advanced::text::Paragraph,
{
    is_open: bool,
    position: Point,
    is_dragging: bool,
    drag_offset: Vector,
    window_bounds: Rectangle,
    ctrl_pressed: bool,
    is_resizing: bool,
    cursor_over_button: bool,
    cursor_over_overlay: bool,
    resize_edge: ResizeEdge,
    resize_start_size: Size,
    resize_start_position: Point,
    resize_start_cursor: Point,
    current_width: f32,
    current_height: f32,
    height_auto: bool,
    title_text: widget::text::State<P>,
    suppress_hover_reopen: bool,
    reset_on_close: bool,
    external_is_open: Option<bool>,
    // Animation fields
    open_animation: Animation<bool>,
    open_progress: f32,
    was_animating: bool,
    is_closing: bool,
    pending_close: bool,
    // Safe triangle fields
    last_button_cursor_pos: Option<Point>,
    in_safe_triangle: bool,
}

impl<P: iced::advanced::text::Paragraph> State<P> {
    /// Resets the state to default values, effectively closing the overlay
    /// and forcing a recalculation of size/position on the next open.
    fn reset(&mut self) {
        self.is_open = false;
        self.is_closing = false;
        self.pending_close = false;
        self.open_progress = 0.0;
        self.last_button_cursor_pos = None;
        self.in_safe_triangle = false;
        self.ctrl_pressed = false;

        if self.reset_on_close {
            // Resetting position to ORIGIN triggers the centering logic in `overlay::layout`
            self.position = Point::ORIGIN;

            // Resetting dimensions to 0.0 triggers the size calculation logic in `overlay()`
            self.current_width = 0.0;
            self.current_height = 0.0;

            // Clear interaction states
            self.is_dragging = false;
            self.is_resizing = false;
            self.resize_edge = ResizeEdge::None;
        }
    }

    /// Initiates a close animation if `animate` is true, or resets immediately if false.
    /// Callers must also call `shell.request_redraw()` and `shell.invalidate_layout()`.
    fn start_close_animation(&mut self, animate: bool) {
        if animate && !self.is_closing {
            self.is_closing = true;
            self.open_animation.go_mut(false, Instant::now());
        } else if !animate {
            self.reset();
        }
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for OverlayButton<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::container::Catalog
        + Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> widget::tree::State {
        let open_animation = {
            let anim = Animation::new(false).easing(self.animation_easing);
            if let Some(dur) = self.animation_duration {
                anim.duration(dur)
            } else {
                anim.quick()
            }
        };
        widget::tree::State::new(State {
            is_open: false,
            position: Point::new(0.0, 0.0),
            is_dragging: false,
            drag_offset: Vector::new(0.0, 0.0),
            window_bounds: Rectangle::with_size(Size::ZERO),
            ctrl_pressed: false,
            is_resizing: false,
            cursor_over_button: false,
            cursor_over_overlay: false,
            resize_edge: ResizeEdge::None,
            resize_start_size: Size::ZERO,
            resize_start_position: Point::ORIGIN,
            resize_start_cursor: Point::ORIGIN,
            current_width: 0.0,
            current_height: 0.0,
            height_auto: false,
            title_text: widget::text::State::<Renderer::Paragraph>::default(),
            suppress_hover_reopen: false,
            reset_on_close: self.reset_on_close,
            external_is_open: self.external_is_open,
            open_animation,
            open_progress: 0.0,
            was_animating: false,
            is_closing: false,
            pending_close: false,
            last_button_cursor_pos: None,
            in_safe_triangle: false,
        })
    }

    fn diff(&mut self, tree: &mut Tree) {
        // Sync external is_open state with internal state
        if let Some(external_open) = self.external_is_open {
            let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
            if state.is_open != external_open {
                if !external_open {
                    // Use pending_close so update() can start the animation with shell access
                    if self.animate {
                        state.pending_close = true;
                        // Don't call reset() yet â€” deferred to update() via pending_close
                    } else {
                        state.reset();
                    }
                } else {
                    state.is_open = true;
                    state.is_closing = false;
                    state.pending_close = false;
                    state.open_progress = if self.animate { 0.0 } else { 1.0 };
                }
            }
        }

        tree.diff_children(&mut [&mut self.content, &mut self.button_content]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        layout::padded(limits, self.width, self.height, self.padding, |limits| {
            self.button_content
                .as_widget_mut()
                .layout(&mut tree.children[1], renderer, limits)
        })
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) where
        Theme: Catalog + button::Catalog,
    {
        let bounds = layout.bounds();
        let button_content_layout = layout.children().next().unwrap();
        let style = <Theme as button::Catalog>::style(
            theme,
            &self.button_class,
            self.status.unwrap_or(button::Status::Active),
        );

        if style.background.is_some() || style.border.width > 0.0 || style.shadow.color.a > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    shadow: style.shadow,
                    snap: style.snap,
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        let viewport = if self.clip {
            bounds.intersection(viewport).unwrap_or(*viewport)
        } else {
            *viewport
        };

        self.button_content.as_widget().draw(
            &tree.children[1],
            renderer,
            theme,
            &renderer::Style {
                text_color: style.text_color,
            },
            button_content_layout,
            cursor,
            &viewport,
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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();

        // Handle pending_close set by diff() when external is_open(false) changes
        if state.pending_close {
            state.pending_close = false;
            state.start_close_animation(self.animate);
            shell.request_redraw();
            shell.invalidate_layout();
            if !self.animate {
                if let Some(on_close) = &self.on_close {
                    shell.publish(on_close());
                }
                if let Some(on_toggle) = &self.on_toggle {
                    shell.publish(on_toggle(false));
                }
            }
        }

        if self.interactive_base {
            self.button_content.as_widget_mut().update(
                &mut tree.children[1],
                event,
                layout.children().next().unwrap(),
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                if self.is_pressed && self.interactive_base == false {
                    self.is_pressed = false;
                    self.status = Some(button::Status::Active);
                }
            }

            Event::Mouse(mouse::Event::CursorMoved { position: _ }) => {
                if cursor.is_over(layout.bounds()) {
                    self.status = Some(button::Status::Hovered);
                    // Track cursor position while over button as safe triangle apex
                    if self.hover.enabled {
                        if let Some(pos) = cursor.position() {
                            state.last_button_cursor_pos = Some(pos);
                        }
                        state.in_safe_triangle = false;
                    }
                    shell.invalidate_layout();
                } else {
                    self.status = Some(button::Status::Active);
                    if state.suppress_hover_reopen && self.hover.enabled {
                        state.suppress_hover_reopen = !state.suppress_hover_reopen
                    }
                    shell.invalidate_layout();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if self.animate && (state.is_open || state.is_closing) {
                    state.open_progress = state.open_animation.interpolate(0.0, 1.0, *now);
                    // When closing and the progress is visually imperceptible (sub-pixel
                    // position, <3% backdrop opacity), complete immediately rather than
                    // leaving the overlay alive for the rest of the animation timer.
                    let close_done = state.is_closing && state.open_progress < 0.03;
                    if !close_done && state.open_animation.is_animating(*now) {
                        state.was_animating = true;
                        shell.invalidate_layout();
                        shell.request_redraw();
                    } else if state.was_animating || close_done {
                        state.was_animating = false;
                        shell.invalidate_layout();
                        if state.is_closing {
                            state.reset();
                            if let Some(on_close) = &self.on_close {
                                shell.publish(on_close());
                            }
                            if let Some(on_toggle) = &self.on_toggle {
                                shell.publish(on_toggle(false));
                            }
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) {
                    self.status = Some(button::Status::Pressed);

                    let should_open = if !self.hover.enabled {
                        // Normal click mode - open, close is handled in overlay
                        true
                    } else if !state.suppress_hover_reopen
                        && !(self.hover.config.mode == PositionMode::Inside)
                    {
                        // First hover click - close
                        state.suppress_hover_reopen = true;
                        false
                    } else {
                        state.suppress_hover_reopen = false; // Second hover click - reopen
                        true
                    };

                    state.is_open = should_open;

                    if should_open {
                        if self.animate {
                            state.is_closing = false;
                            state.open_animation.go_mut(true, Instant::now());
                            state.open_progress =
                                state.open_animation.interpolate(0.0, 1.0, Instant::now());
                        } else {
                            state.open_progress = 1.0;
                        }
                        if let Some(on_open) = &self.on_open {
                            shell.publish(on_open(
                                state.position,
                                Size::new(state.current_width, state.current_height),
                            ));
                        }
                        if let Some(on_toggle) = &self.on_toggle {
                            shell.publish(on_toggle(true))
                        }
                    }

                    if !(self.hover.config.mode == PositionMode::Inside)
                        || self.external_is_open.is_none()
                    {
                        self.is_pressed = true;
                        shell.capture_event();
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }
            }
            _ => {}
        }

        if self.hover.config.mode == PositionMode::Inside {
            self.button_content.as_widget_mut().update(
                &mut tree.children[1],
                event,
                layout.children().next().unwrap(),
                cursor,
                renderer,
                shell,
                viewport,
            );
        }

        if state.is_open {
            return;
        }

        if self.hover.enabled {
            let cursor_over_button = cursor.is_over(bounds);
            state.cursor_over_button = cursor_over_button;

            // Open on hover
            if cursor_over_button && !state.is_open && !state.suppress_hover_reopen {
                state.is_open = true;
                if self.animate {
                    state.is_closing = false;
                    state.open_animation.go_mut(true, Instant::now());
                    state.open_progress =
                        state.open_animation.interpolate(0.0, 1.0, Instant::now());
                } else {
                    state.open_progress = 1.0;
                }
                if let Some(on_open) = &self.on_open {
                    shell.publish(on_open(
                        state.position,
                        Size::new(state.current_width, state.current_height),
                    ));
                }
                if let Some(on_toggle) = &self.on_toggle {
                    shell.publish(on_toggle(true))
                }
                shell.invalidate_layout();
                shell.request_redraw();
            }

            // Close when cursor exits both button and overlay (fallback; primary close is in Overlay::update)
            if !state.cursor_over_button
                && !state.cursor_over_overlay
                && state.is_open
                && !state.in_safe_triangle
            {
                state.start_close_animation(self.animate);
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
        self.button_content.as_widget().mouse_interaction(
            &tree.children[1],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        viewport: &Rectangle,
        offset: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        if !state.is_open {
            return None;
        }

        let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
        let padding = self.overlay_padding * 2.0;
        let content_tree = &mut tree.children[0];

        let mut content_node: Node;
        let mut computed_content_h: f32;

        // Helper to resolve the strategy into a concrete Length
        let resolve_strategy = |strategy: &Option<SizeStrategy<'a>>,
                                available_space: f32|
         -> Length {
            match strategy {
                Some(SizeStrategy::Static(l)) => *l,
                Some(SizeStrategy::Dynamic(f)) => f(available_space),
                None => Length::Fixed(if strategy.is_none() && available_space == viewport.width {
                    400.0 // Default width
                } else {
                    300.0 // Default height
                }),
            }
        };

        // Resolve width and height using the viewport size
        let width_strategy = resolve_strategy(&self.overlay_width, viewport.width);
        let height_strategy = resolve_strategy(&self.overlay_height, viewport.height);

        // Initialize sizes if needed
        if state.current_width == 0.0 {
            let width_limits = Limits::new(Size::ZERO, Size::new(f32::INFINITY, f32::INFINITY));
            let resolved_width = resolve_length(width_strategy, viewport.width, || {
                let measure_limits =
                    Limits::new(Size::ZERO, Size::new(viewport.width, f32::INFINITY));
                let temp_node =
                    self.content
                        .as_widget_mut()
                        .layout(content_tree, renderer, &measure_limits);
                temp_node.size().width + padding
            });

            state.current_width = resolved_width;
            let init_auto = self.overlay_height.is_none() || is_intrinsic(height_strategy);
            state.height_auto = init_auto;

            // First layout with resolved width to measure natural content height
            let init_limits = Limits::new(
                Size::ZERO,
                Size::new(state.current_width - padding, f32::INFINITY),
            );
            content_node =
                self.content
                    .as_widget_mut()
                    .layout(content_tree, renderer, &init_limits);
            computed_content_h = content_node.size().height;

            if init_auto {
                state.current_height = header_height + computed_content_h + padding;
            } else {
                let resolved_height =
                    resolve_length(height_strategy, width_limits.max().height, || {
                        header_height + computed_content_h + padding
                    });

                state.current_height = resolved_height;

                let constrained_h = state.current_height - header_height - padding;
                let constrained_limits = Limits::new(
                    Size::ZERO,
                    Size::new(state.current_width - padding, constrained_h),
                );
                content_node = self.content.as_widget_mut().layout(
                    content_tree,
                    renderer,
                    &constrained_limits,
                );
                computed_content_h = content_node.size().height;
            }
        } else if state.height_auto {
            let auto_limits = Limits::new(
                Size::ZERO,
                Size::new(state.current_width - padding, f32::INFINITY),
            );
            content_node =
                self.content
                    .as_widget_mut()
                    .layout(content_tree, renderer, &auto_limits);
            computed_content_h = content_node.size().height;
            state.current_height = header_height + computed_content_h + padding;
        } else {
            let fixed_h = state.current_height - header_height - padding;
            let fixed_limits = Limits::new(
                Size::ZERO,
                Size::new(state.current_width - padding, fixed_h),
            );
            content_node =
                self.content
                    .as_widget_mut()
                    .layout(content_tree, renderer, &fixed_limits);
            computed_content_h = content_node.size().height;
        }

        let total_w = state.current_width;
        let total_h = state.current_height;

        let mut button_bounds = layout.bounds();
        button_bounds.x += offset.x;
        button_bounds.y += offset.y;

        Some(overlay::Element::new(Box::new(Overlay {
            state,
            title: &self.title,
            class: &self.class,
            content: &mut self.content,
            radius: self.overlay_radius,
            tree: content_tree,
            width: total_w,
            height: total_h,
            padding: self.overlay_padding,
            on_close: self.on_close.as_deref(),
            on_toggle: self.on_toggle.as_deref(),
            button_bounds,
            button_padding: self.padding,
            hover: &self.hover,
            hover_positions_on_click: self.hover_positions_on_click,
            content_layout: content_node,
            opaque: self.opaque,
            opaque_alpha: self.opaque_alpha,
            close_on_click_outside: self.close_on_click_outside,
            hide_header: self.hide_header,
            hide_close_button: self.hide_close_button,
            resizable: self.resizable,
            block_dragging: self.block_dragging,
            animate: self.animate,
            safe_triangle: self.safe_triangle,
        })))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        operation.custom(self.id.as_ref(), layout.bounds(), state);
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.button_content.as_widget_mut().operate(
                &mut tree.children[1],
                layout.children().next().unwrap(),
                renderer,
                operation,
            );
        });
    }
}

/// The default [`Padding`] of a [`Button`]. Using for Overlay Button to match iced::widget::button
pub(crate) const DEFAULT_PADDING: Padding = Padding {
    top: 5.0,
    bottom: 5.0,
    right: 10.0,
    left: 10.0,
};

struct Overlay<'a, 'b, Message, Theme, Renderer>
where
    Renderer: text::Renderer,
    Theme: Catalog,
{
    state: &'a mut State<Renderer::Paragraph>,
    class: &'a Theme::Class<'b>,
    title: &'a str,
    content: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    width: f32,
    height: f32,
    padding: f32,
    radius: f32,
    on_close: Option<&'a dyn Fn() -> Message>,
    on_toggle: Option<&'a dyn Fn(bool) -> Message>,
    button_bounds: Rectangle,
    button_padding: Padding,
    hover: &'a Hover,
    hover_positions_on_click: bool,
    content_layout: Node,
    opaque: bool,
    opaque_alpha: f32,
    close_on_click_outside: bool,
    hide_header: bool,
    hide_close_button: bool,
    resizable: ResizeMode,
    block_dragging: bool,
    animate: bool,
    safe_triangle: bool,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: iced::widget::container::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> Node {
        self.state.window_bounds = Rectangle::with_size(bounds);
        let size = Size::new(self.width, self.height);

        if self.state.position == Point::ORIGIN {
            self.state.position = Point::new(
                (bounds.width - size.width) / 2.0,
                (bounds.height - size.height) / 2.0,
            );
        }

        if self.hover.enabled || self.hover_positions_on_click {
            let overlay_width = self.state.current_width;
            let overlay_height = self.state.current_height;

            // Calculate position based on Position enum and mode
            let mut calculated_position = match self.hover.config.mode {
                PositionMode::Outside => {
                    // Current behavior - overlay adjacent to button
                    match self.hover.config.position {
                        Position::Top | Position::Bottom => {
                            let x = match self.hover.config.alignment {
                                Alignment::Start => self.button_bounds.x,
                                Alignment::Center => {
                                    self.button_bounds.x
                                        + (self.button_bounds.width - overlay_width) / 2.0
                                }
                                Alignment::End => {
                                    self.button_bounds.x + self.button_bounds.width - overlay_width
                                }
                            };

                            let y = if self.hover.config.position == Position::Top {
                                self.button_bounds.y - overlay_height - self.hover.config.gap
                            } else {
                                self.button_bounds.y
                                    + self.button_bounds.height
                                    + self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                        Position::Left | Position::Right => {
                            let y = match self.hover.config.alignment {
                                Alignment::Start => self.button_bounds.y,
                                Alignment::Center => {
                                    self.button_bounds.y
                                        + (self.button_bounds.height - overlay_height) / 2.0
                                }
                                Alignment::End => {
                                    self.button_bounds.y + self.button_bounds.height
                                        - overlay_height
                                }
                            };

                            let x = if self.hover.config.position == Position::Left {
                                self.button_bounds.x - overlay_width - self.hover.config.gap
                            } else {
                                self.button_bounds.x
                                    + self.button_bounds.width
                                    + self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                    }
                }
                PositionMode::Inside => {
                    let content_bounds = Rectangle {
                        x: self.button_bounds.x + self.button_padding.left,
                        y: self.button_bounds.y + self.button_padding.top,
                        width: self.button_bounds.width
                            - self.button_padding.left
                            - self.button_padding.right,
                        height: self.button_bounds.height
                            - self.button_padding.top
                            - self.button_padding.bottom,
                    };

                    // New behavior - overlay anchored inside button bounds
                    match self.hover.config.position {
                        Position::Top | Position::Bottom => {
                            // Horizontal positioning from content edges
                            let x = match self.hover.config.alignment {
                                Alignment::Start => content_bounds.x + self.hover.config.gap,
                                Alignment::Center => {
                                    content_bounds.x + (content_bounds.width - overlay_width) / 2.0
                                }
                                Alignment::End => {
                                    content_bounds.x + content_bounds.width
                                        - overlay_width
                                        - self.hover.config.gap
                                }
                            };

                            // Vertical positioning from content edges (inward)
                            let y = if self.hover.config.position == Position::Top {
                                content_bounds.y + self.hover.config.gap
                            } else {
                                content_bounds.y + content_bounds.height
                                    - overlay_height
                                    - self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                        Position::Left | Position::Right => {
                            // Vertical positioning from content edges
                            let y = match self.hover.config.alignment {
                                Alignment::Start => content_bounds.y + self.hover.config.gap,
                                Alignment::Center => {
                                    content_bounds.y
                                        + (content_bounds.height - overlay_height) / 2.0
                                }
                                Alignment::End => {
                                    content_bounds.y + content_bounds.height
                                        - overlay_height
                                        - self.hover.config.gap
                                }
                            };

                            // Horizontal positioning from content edges (inward)
                            let x = if self.hover.config.position == Position::Left {
                                content_bounds.x + self.hover.config.gap
                            } else {
                                content_bounds.x + content_bounds.width
                                    - overlay_width
                                    - self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                    }
                }
            };

            // Snap within viewport if enabled
            if self.hover.config.snap_within_viewport {
                // Horizontal bounds checking
                if calculated_position.x < self.state.window_bounds.x {
                    calculated_position.x = self.state.window_bounds.x;
                } else if calculated_position.x + overlay_width
                    > self.state.window_bounds.x + self.state.window_bounds.width
                {
                    calculated_position.x =
                        self.state.window_bounds.x + self.state.window_bounds.width - overlay_width;
                }

                // Vertical bounds checking
                if calculated_position.y < self.state.window_bounds.y {
                    calculated_position.y = self.state.window_bounds.y;
                } else if calculated_position.y + overlay_height
                    > self.state.window_bounds.y + self.state.window_bounds.height
                {
                    calculated_position.y = self.state.window_bounds.y
                        + self.state.window_bounds.height
                        - overlay_height;
                }
            }

            // Override the state position with calculated position
            self.state.position = calculated_position;
        }

        // Apply slide offset during open/close animation
        let final_position = if self.animate {
            let slide_dist = 18.0 * (1.0 - self.state.open_progress);
            let offset = if self.hover.enabled || self.hover_positions_on_click {
                match self.hover.config.position {
                    Position::Right => Vector::new(-slide_dist, 0.0),
                    Position::Left => Vector::new(slide_dist, 0.0),
                    Position::Bottom => Vector::new(0.0, -slide_dist),
                    Position::Top => Vector::new(0.0, slide_dist),
                }
            } else {
                // Centered/click-open overlays: slide up from slightly below
                Vector::new(0.0, slide_dist)
            };
            Point::new(
                self.state.position.x + offset.x,
                self.state.position.y + offset.y,
            )
        } else {
            self.state.position
        };

        Node::new(size).move_to(final_position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        let draw_style = <Theme as Catalog>::style(theme, self.class);

        // Alpha used only for backdrop animation â€” dialog draws at full opacity
        let alpha = self.state.open_progress;

        // Use layer rendering for proper overlay isolation
        renderer.with_layer(self.state.window_bounds, |renderer| {
            // Draw opaque backdrop if requested
            if self.opaque {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: self.state.window_bounds,
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: false,
                    },
                    Color::from_rgba(0.0, 0.0, 0.0, self.opaque_alpha * alpha),
                );
            }

            // Draw background with shadow
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        color: draw_style.border_color,
                        width: 1.0,
                        radius: self.radius.into(),
                    },
                    shadow: Shadow {
                        color: draw_style.shadow.color,
                        ..draw_style.shadow
                    },
                    snap: true,
                },
                Background::Color(draw_style.background),
            );

            // Draw header only if not hidden
            if !self.hide_header {
                // Draw header background
                let header_bounds = Rectangle {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: HEADER_HEIGHT,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: header_bounds,
                        border: Border {
                            color: draw_style.border_color,
                            width: 1.0,
                            radius: Radius {
                                top_left: self.radius,
                                top_right: self.radius,
                                bottom_left: 0.0,
                                bottom_right: 0.0,
                            },
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    Background::Color(draw_style.header_background),
                );

                // Draw title
                renderer.fill_text(
                    iced::advanced::Text {
                        content: self.title.to_string(),
                        bounds: Size::new(
                            header_bounds.width - CLOSE_BUTTON_SIZE - 20.0,
                            header_bounds.height,
                        ),
                        size: iced::Pixels(16.0),
                        font: iced::Font::default(),
                        align_x: iced::advanced::text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Advanced,
                        wrapping: iced::advanced::text::Wrapping::default(),
                        ellipsis: iced::advanced::text::Ellipsis::None,
                        hint_factor: None,
                    },
                    Point::new(
                        header_bounds.center_x() - (CLOSE_BUTTON_SIZE / 2.0),
                        header_bounds.center_y(),
                    ),
                    draw_style.text_color,
                    header_bounds,
                );

                if !self.hide_close_button {
                    // Draw close button - centered vertically in header
                    let close_bounds = Rectangle {
                        x: bounds.x + bounds.width - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_OFFSET * 2.0,
                        y: bounds.y + (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2.0,
                        width: CLOSE_BUTTON_SIZE,
                        height: CLOSE_BUTTON_SIZE,
                    };

                    if cursor.is_over(close_bounds) {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: close_bounds,
                                border: Border {
                                    radius: (CLOSE_BUTTON_SIZE / 2.0).into(),
                                    ..Default::default()
                                },
                                shadow: Shadow::default(),
                                snap: true,
                            },
                            Color::from_rgba(0.0, 0.0, 0.0, 0.1),
                        );
                    }

                    renderer.fill_text(
                        iced::advanced::Text {
                            content: "Ã—".to_string(),
                            bounds: Size::new(close_bounds.width, close_bounds.height),
                            size: iced::Pixels(24.0),
                            font: iced::Font::default(),
                            align_x: iced::advanced::text::Alignment::Center,
                            align_y: Vertical::Center,
                            line_height: iced::advanced::text::LineHeight::default(),
                            shaping: iced::advanced::text::Shaping::Basic,
                            wrapping: iced::advanced::text::Wrapping::default(),
                            ellipsis: iced::advanced::text::Ellipsis::None,
                            hint_factor: None,
                        },
                        Point::new(close_bounds.center_x(), close_bounds.center_y()),
                        draw_style.text_color,
                        close_bounds,
                    );
                }
            }

            // Draw content
            let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
            let content_bounds = Rectangle {
                x: bounds.x + self.padding,
                y: bounds.y + header_height + self.padding,
                width: bounds.width - self.padding * 2.0,
                height: bounds.height - header_height - self.padding * 2.0,
            };

            // Adjust cursor to content coordinate space (computed outside closures)
            let adjusted_cursor = cursor
                .position()
                .map(|position| {
                    mouse::Cursor::Available(Point::new(
                        position.x - content_bounds.x,
                        position.y - content_bounds.y,
                    ))
                })
                .unwrap_or(mouse::Cursor::Unavailable);

            // with_layer creates a compositing boundary. Any with_layer calls inside child
            // widgets are isolated within this layer, so drawing after it closes is guaranteed
            // to composite on top of all child content.
            renderer.with_layer(content_bounds, |renderer| {
                renderer.with_translation(
                    Vector::new(content_bounds.x, content_bounds.y),
                    |renderer| {
                        self.content.as_widget().draw(
                            self.tree,
                            renderer,
                            theme,
                            &renderer::Style {
                                text_color: draw_style.text_color,
                            },
                            Layout::new(&self.content_layout),
                            adjusted_cursor,
                            &Rectangle::new(Point::ORIGIN, content_bounds.size()),
                        );
                    },
                );
            });

            // Debug: draw safe triangle outline when Ctrl is held (window coordinate space).
            // Each edge is rendered as a series of 2Ã—2 pixel squares spaced 1.5px apart,
            // producing a solid-looking line at typical display densities.
            if self.state.ctrl_pressed
                && self.safe_triangle
                && (self.hover.enabled || self.hover_positions_on_click)
            {
                if let Some(last_pos) = self.state.last_button_cursor_pos {
                    let extend = self.hover.config.gap.max(5.0);
                    let (corner_a, corner_b) =
                        overlay_near_corners(bounds, &self.hover.config.position, extend);
                    let line_color = Color::from_rgba(0.0, 0.8, 1.0, 0.8);
                    let px = 2.0_f32; // square side length
                    let step = 1.5_f32; // center-to-center spacing

                    for edge in [
                        (last_pos, corner_a),
                        (last_pos, corner_b),
                        (corner_a, corner_b),
                    ] {
                        let (p1, p2) = edge;
                        let dx = p2.x - p1.x;
                        let dy = p2.y - p1.y;
                        let len = (dx * dx + dy * dy).sqrt();
                        if len < 1.0 {
                            continue;
                        }
                        let steps = (len / step).ceil() as usize + 1;
                        for i in 0..=steps {
                            let t = (i as f32 * step / len).min(1.0);
                            let x = p1.x + dx * t;
                            let y = p1.y + dy * t;
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle {
                                        x: x - px / 2.0,
                                        y: y - px / 2.0,
                                        width: px,
                                        height: px,
                                    },
                                    border: Border::default(),
                                    shadow: Shadow::default(),
                                    snap: false,
                                },
                                line_color,
                            );
                        }
                    }
                }
            }
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
        let bounds = layout.bounds();

        // Track Ctrl key state
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Control),
                ..
            }) => {
                self.state.ctrl_pressed = true;
                shell.request_redraw();
                shell.invalidate_layout();
                return;
            }
            Event::Keyboard(keyboard::Event::KeyReleased {
                key: keyboard::Key::Named(keyboard::key::Named::Control),
                ..
            }) => {
                self.state.ctrl_pressed = false;
                shell.request_redraw();
                shell.invalidate_layout();
                return;
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                let was_pressed = self.state.ctrl_pressed;
                self.state.ctrl_pressed = modifiers.control();
                if self.state.ctrl_pressed != was_pressed {
                    shell.request_redraw();
                    shell.invalidate_layout();
                }
            }
            _ => {}
        }

        let can_resize = match self.resizable {
            ResizeMode::None => false,
            ResizeMode::Always => true,
            ResizeMode::WithCtrl => self.state.ctrl_pressed,
        };

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let cursor_over_overlay = cursor.is_over(bounds);
                if cursor.is_over(self.button_bounds)
                    && self.state.is_open
                    && !(self.hover.config.mode == PositionMode::Inside)
                    && self.state.external_is_open.is_none()
                {
                    self.state.start_close_animation(self.animate);
                    shell.invalidate_layout();
                    shell.request_redraw();
                    shell.capture_event();
                    return;
                }

                if self.close_on_click_outside && !cursor_over_overlay && self.state.is_open {
                    self.state.start_close_animation(self.animate);
                    if !self.animate {
                        if let Some(on_close) = self.on_close {
                            shell.publish(on_close());
                        }
                        if let Some(on_toggle) = &self.on_toggle {
                            shell.publish(on_toggle(false));
                        }
                    }
                    shell.invalidate_layout();
                    shell.request_redraw();
                    if self.opaque {
                        shell.capture_event();
                    }
                    return;
                }

                // If opaque and clicking outside, consume the event without forwarding
                if self.opaque && !cursor_over_overlay {
                    shell.capture_event();
                    return;
                }

                if let Some(position) = cursor.position() {
                    if can_resize && cursor_over_overlay {
                        let resize_edge = ResizeEdge::from_position(position, bounds);
                        if resize_edge != ResizeEdge::None {
                            self.state.is_resizing = true;
                            self.state.resize_edge = resize_edge;
                            self.state.resize_start_size = bounds.size();
                            self.state.resize_start_position = self.state.position;
                            self.state.resize_start_cursor = position;
                            self.state.drag_offset =
                                Vector::new(position.x - bounds.x, position.y - bounds.y);
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }
                    }

                    // Handle close button
                    if !self.hide_header {
                        if !self.hide_close_button {
                            let close_bounds = Rectangle {
                                x: bounds.x + bounds.width
                                    - CLOSE_BUTTON_SIZE
                                    - CLOSE_BUTTON_OFFSET * 2.0,
                                y: bounds.y + (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2.0,
                                width: CLOSE_BUTTON_SIZE,
                                height: CLOSE_BUTTON_SIZE,
                            };

                            if cursor.is_over(close_bounds) {
                                self.state.start_close_animation(self.animate);
                                if !self.animate {
                                    if let Some(on_close) = self.on_close {
                                        shell.publish(on_close());
                                    }
                                    if let Some(on_toggle) = &self.on_toggle {
                                        shell.publish(on_toggle(false));
                                    }
                                }
                                shell.invalidate_layout();
                                shell.request_redraw();
                                return;
                            }
                        }

                        // Handle header dragging
                        let header_bounds = Rectangle {
                            x: bounds.x,
                            y: bounds.y,
                            width: bounds.width,
                            height: HEADER_HEIGHT,
                        };

                        if cursor.is_over(header_bounds) && !self.block_dragging {
                            self.state.is_dragging = true;
                            self.state.drag_offset =
                                Vector::new(position.x - bounds.x, position.y - bounds.y);
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }
                    }

                    // Handle Ctrl+drag from anywhere in the overlay
                    if self.state.ctrl_pressed && cursor_over_overlay && !self.block_dragging {
                        self.state.is_dragging = true;
                        self.state.drag_offset =
                            Vector::new(position.x - bounds.x, position.y - bounds.y);
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let cursor_over_overlay = cursor.is_over(bounds);
                self.state.is_dragging = false;
                self.state.is_resizing = false;
                self.state.resize_edge = ResizeEdge::None;
                shell.invalidate_layout();
                shell.request_redraw();

                // If opaque, consume the event
                if self.opaque && !cursor_over_overlay {
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // handle hover first
                if self.hover.enabled || self.hover_positions_on_click {
                    let overlay_bounds = layout.bounds();
                    // Buffered bounds used for the close/keep-open decision
                    self.state.cursor_over_overlay =
                        cursor.is_over(overlay_bounds.expand(self.hover.config.buffer));
                    self.state.cursor_over_button =
                        cursor.is_over(self.button_bounds.expand(self.hover.config.buffer));

                    // Safe triangle tracking uses RAW (non-buffered) bounds.
                    // The buffer can be larger than the gap, making the buffered zones overlap
                    // and leaving no "gap" region where the safe triangle would apply.
                    let raw_over_button = cursor.is_over(self.button_bounds);
                    let raw_over_overlay = cursor.is_over(overlay_bounds);

                    if raw_over_button {
                        // Track cursor position on button as the safe triangle apex
                        if let Some(pos) = cursor.position() {
                            self.state.last_button_cursor_pos = Some(pos);
                        }
                        self.state.in_safe_triangle = false;
                    } else if raw_over_overlay {
                        // Cursor is inside the overlay â€” safe zone no longer needed.
                        // last_button_cursor_pos is intentionally NOT updated here: the safe
                        // triangle for this overlay stays anchored to where the cursor last
                        // left the trigger button, keeping it frozen while the user navigates
                        // inside the overlay (including through nested child overlay buttons).
                        self.state.in_safe_triangle = false;
                    } else if !self.state.is_closing {
                        // Cursor is in the gap between button and overlay â€” check safe triangle
                        if self.safe_triangle {
                            if let (Some(last_pos), Some(cur_pos)) =
                                (self.state.last_button_cursor_pos, cursor.position())
                            {
                                let (corner_a, corner_b) = overlay_near_corners(
                                    overlay_bounds,
                                    &self.hover.config.position,
                                    self.hover.config.gap.max(5.0),
                                );
                                self.state.in_safe_triangle =
                                    point_in_triangle(cur_pos, last_pos, corner_a, corner_b);
                            }
                        }
                    }

                    // Close if cursor over neither button nor overlay (buffered), not in safe triangle, not already closing
                    let should_close = !self.state.cursor_over_button
                        && !self.state.cursor_over_overlay
                        && !self.state.in_safe_triangle
                        && !self.state.is_closing
                        && !has_open_descendant_overlays::<Renderer::Paragraph>(self.tree);

                    if should_close {
                        if self.state.external_is_open.is_none() {
                            self.state.start_close_animation(self.animate);
                        }
                        if !self.animate {
                            if let Some(on_close) = self.on_close {
                                shell.publish(on_close());
                            }
                            if let Some(on_toggle) = &self.on_toggle {
                                shell.publish(on_toggle(false));
                            }
                        }
                        shell.invalidate_layout();
                        shell.request_redraw();
                    }
                }
                let can_drag = !self.hover.enabled; // Block drag if on_hover is enabled

                if let Some(position) = cursor.position() {
                    // Handle resizing
                    if self.state.is_resizing && can_drag {
                        let delta_x = position.x - self.state.resize_start_cursor.x;
                        let delta_y = position.y - self.state.resize_start_cursor.y;

                        let mut new_width = self.state.resize_start_size.width;
                        let mut new_height = self.state.resize_start_size.height;
                        let mut new_x = self.state.resize_start_position.x;
                        let mut new_y = self.state.resize_start_position.y;

                        // Width and x position
                        match self.state.resize_edge {
                            ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft => {
                                new_width = (self.state.resize_start_size.width - delta_x)
                                    .max(MIN_OVERLAY_SIZE);
                                new_x = self.state.resize_start_position.x + delta_x;
                            }
                            ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight => {
                                new_width = (self.state.resize_start_size.width + delta_x)
                                    .max(MIN_OVERLAY_SIZE);
                                // x unchanged
                            }
                            _ => {}
                        }

                        // Height and y position
                        match self.state.resize_edge {
                            ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight => {
                                new_height = (self.state.resize_start_size.height - delta_y)
                                    .max(MIN_OVERLAY_SIZE);
                                new_y = self.state.resize_start_position.y + delta_y;
                            }
                            ResizeEdge::Bottom
                            | ResizeEdge::BottomLeft
                            | ResizeEdge::BottomRight => {
                                new_height = (self.state.resize_start_size.height + delta_y)
                                    .max(MIN_OVERLAY_SIZE);
                                // y unchanged
                            }
                            _ => {}
                        }

                        // Store in state
                        self.state.current_width = new_width;
                        self.state.current_height = new_height;

                        // Fix height if this edge affects it
                        if self.state.resize_edge.affects_height() {
                            self.state.height_auto = false;
                        }

                        // Clamp position to viewport
                        new_x = new_x
                            .max(0.0)
                            .min(self.state.window_bounds.width - new_width);
                        new_y = new_y
                            .max(0.0)
                            .min(self.state.window_bounds.height - new_height);
                        self.state.position = Point::new(new_x, new_y);

                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }

                    // Handle dragging
                    if self.state.is_dragging && can_drag {
                        let new_x = position.x - self.state.drag_offset.x;
                        let new_y = position.y - self.state.drag_offset.y;

                        self.state.position.x = new_x
                            .max(0.0)
                            .min(self.state.window_bounds.width - self.state.current_width);
                        self.state.position.y = new_y
                            .max(0.0)
                            .min(self.state.window_bounds.height - self.state.current_height);

                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }
                }

                if self.opaque && !cursor.is_over(bounds) {
                    shell.capture_event();
                    return;
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                self.state.start_close_animation(self.animate);
                if !self.animate {
                    if let Some(on_close) = self.on_close {
                        shell.publish(on_close());
                    }
                    if let Some(on_toggle) = &self.on_toggle {
                        shell.publish(on_toggle(false));
                    }
                }
                shell.invalidate_layout();
                shell.request_redraw();
                return;
            }
            _ => {}
        }

        // If opaque, consume ALL mouse/touch events that are outside the overlay
        if self.opaque {
            match event {
                Event::Mouse(_) | Event::Touch(_) => {
                    if !cursor.is_over(bounds) {
                        shell.capture_event();
                        return;
                    }
                }
                _ => {}
            }
        }

        // Forward events to content
        let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
        let content_bounds = Rectangle {
            x: bounds.x + self.padding,
            y: bounds.y + header_height + self.padding,
            width: bounds.width - self.padding * 2.0,
            height: bounds.height - header_height - self.padding * 2.0,
        };

        let content_layout_node = self
            .content_layout
            .clone()
            .move_to(Point::new(content_bounds.x, content_bounds.y));
        let content_layout = Layout::new(&content_layout_node);

        // Only forward events to content if not dragging and if cursor is in content area
        if !self.state.is_dragging && !self.state.is_resizing {
            self.content.as_widget_mut().update(
                self.tree,
                event,
                content_layout,
                cursor,
                renderer,
                shell,
                &layout.bounds(),
            );
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();

        if cursor.is_over(bounds) {
            // Determine if we should be resizable
            let can_resize = match self.resizable {
                ResizeMode::None => false,
                ResizeMode::Always => true,
                ResizeMode::WithCtrl => self.state.ctrl_pressed,
            };

            // Show resize cursors if resizable
            if can_resize && let Some(position) = cursor.position() {
                let resize_edge = ResizeEdge::from_position(position, bounds);
                if resize_edge != ResizeEdge::None {
                    return resize_edge.cursor_icon();
                }
            }

            // Show pointer when over close button (if header is visible)
            if !self.hide_header {
                if !self.hide_close_button {
                    let close_bounds = Rectangle {
                        x: bounds.x + bounds.width - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_OFFSET * 2.0,
                        y: bounds.y + (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2.0,
                        width: CLOSE_BUTTON_SIZE,
                        height: CLOSE_BUTTON_SIZE,
                    };

                    if cursor.is_over(close_bounds) {
                        return mouse::Interaction::Pointer;
                    }
                }

                // Show grab cursor when over header
                let header_bounds = Rectangle {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: HEADER_HEIGHT,
                };

                if cursor.is_over(header_bounds) && !self.block_dragging {
                    return if self.state.is_dragging {
                        mouse::Interaction::Grabbing
                    } else {
                        mouse::Interaction::Grab
                    };
                }
            }

            // Show grab/grabbing when Ctrl is pressed
            if self.state.ctrl_pressed && !self.block_dragging {
                return if self.state.is_dragging {
                    mouse::Interaction::Grabbing
                } else {
                    mouse::Interaction::Grab
                };
            }

            // Forward to content with adjusted cursor
            let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
            let content_bounds = Rectangle {
                x: bounds.x + self.padding,
                y: bounds.y + header_height + self.padding,
                width: bounds.width - self.padding * 2.0,
                height: bounds.height - header_height - self.padding * 2.0,
            };

            let adjusted_cursor = if let Some(position) = cursor.position() {
                if content_bounds.contains(position) {
                    mouse::Cursor::Available(Point::new(
                        position.x - content_bounds.x,
                        position.y - content_bounds.y,
                    ))
                } else {
                    mouse::Cursor::Unavailable
                }
            } else {
                mouse::Cursor::Unavailable
            };

            let content_interaction = self.content.as_widget().mouse_interaction(
                self.tree,
                Layout::new(&self.content_layout),
                adjusted_cursor,
                &Rectangle::new(Point::ORIGIN, content_bounds.size()),
                renderer,
            );

            // If content doesn't want a specific interaction, return default to block passthrough
            if content_interaction == mouse::Interaction::default() {
                return mouse::Interaction::Idle;
            }

            content_interaction
        } else {
            // Cursor not over overlay, don't block
            mouse::Interaction::default()
        }
    }

    fn overlay<'a>(
        &'a mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        let bounds = layout.bounds();

        let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };

        let content_bounds = Rectangle {
            x: bounds.x + self.padding,
            y: bounds.y + header_height + self.padding,
            width: bounds.width - self.padding * 2.0,
            height: bounds.height - header_height - self.padding * 2.0,
        };

        self.content.as_widget_mut().overlay(
            self.tree,
            Layout::new(&self.content_layout),
            renderer,
            &content_bounds,
            Vector::new(content_bounds.x, content_bounds.y),
        )
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }
}

impl<'a, Message, Theme, Renderer> From<OverlayButton<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::container::Catalog
        + Catalog
        + 'a,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    fn from(button: OverlayButton<'a, Message, Theme, Renderer>) -> Self {
        Self::new(button)
    }
}

/// Closes an overlay button with the given Id
pub fn close<T>(id: widget::Id) -> impl Operation<T> {
    struct Close {
        id: widget::Id,
    }

    impl<T> Operation<T> for Close {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            // Continue traversing the tree
            operate(self);
        }

        fn custom(
            &mut self,
            widget_id: Option<&widget::Id>,
            _bounds: Rectangle,
            state: &mut dyn std::any::Any,
        ) {
            if widget_id == Some(&self.id) {
                type DefaultParagraph =
                    <iced::Renderer as iced::advanced::text::Renderer>::Paragraph;

                if let Some(state) = state.downcast_mut::<State<DefaultParagraph>>() {
                    state.reset();
                }
            }
        }
    }

    Close { id }
}

/// Strategy for sizing the overlay
pub enum SizeStrategy<'a> {
    /// A static (normal Iced) length (Fixed, Fill, Shrink, etc.)
    Static(Length),
    /// A dynamic calculation based on available space (viewport size)
    /// Returns a Length, so you can still return Fixed(v * 0.8) or Shrink
    Dynamic(Box<dyn Fn(f32) -> Length + 'a>),
}

// From length impl to allow passing a raw Length directly
impl<'a> From<Length> for SizeStrategy<'a> {
    fn from(length: Length) -> Self {
        Self::Static(length)
    }
}

// from f32 impl to allow passing a float directly as Fixed pixels
impl<'a> From<f32> for SizeStrategy<'a> {
    fn from(pixels: f32) -> Self {
        Self::Static(Length::Fixed(pixels))
    }
}

/// The theme catalog of a draggable overlay
pub trait Catalog {
    /// The style class
    type Class<'a>;

    /// Default style
    fn default<'a>() -> Self::Class<'a>;

    /// Get the style for a class
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// Style for the overlay
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Background color
    pub background: Color,
    /// Header background color  
    pub header_background: Color,
    /// Border color
    pub border_color: Color,
    /// Text color
    pub text_color: Color,
    /// Shadow
    pub shadow: Shadow,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Color::from_rgb8(245, 245, 245),
            header_background: Color::from_rgb8(230, 230, 230),
            border_color: Color::from_rgb8(200, 200, 200),
            text_color: Color::BLACK,
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
        }
    }
}

/// Styling function
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme| {
            let palette = theme.palette();
            Style {
                background: palette.background.base.color,
                header_background: palette.background.weak.color,
                border_color: palette.background.strong.color,
                text_color: palette.background.base.text,
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
            }
        })
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

pub fn primary(theme: &iced::Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: palette.primary.base.color,
        header_background: palette.primary.weak.color,
        border_color: palette.primary.strong.color,
        text_color: palette.primary.base.text,
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub fn success(theme: &iced::Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: palette.success.base.color,
        header_background: palette.success.weak.color,
        border_color: palette.success.strong.color,
        text_color: palette.success.base.text,
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub fn danger(theme: &iced::Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: palette.danger.base.color,
        header_background: palette.danger.weak.color,
        border_color: palette.danger.strong.color,
        text_color: palette.danger.base.text,
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub fn warning(theme: &iced::Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: palette.warning.base.color,
        header_background: palette.warning.weak.color,
        border_color: palette.warning.strong.color,
        text_color: palette.warning.base.text,
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 16.0,
        },
    }
}

pub fn blank(theme: &iced::Theme) -> Style {
    let palette = theme.palette();

    Style {
        background: Color::TRANSPARENT,
        header_background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        text_color: Color::TRANSPARENT,
        shadow: Shadow::default(),
    }
}
