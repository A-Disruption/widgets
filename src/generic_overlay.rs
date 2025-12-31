use iced::{
    advanced::{
        layout::{self, padded, Limits, Node},
        overlay,
        renderer,
        text::Renderer as _,
        text,
        widget::{self, tree::Tree},
        widget::operation::{self, Operation, Outcome},
        Clipboard, Layout, Overlay as _, Renderer as _, Shell, Widget,
    }, alignment::Vertical, border::Radius, event, keyboard, mouse, touch, widget::button, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow, Size, Theme, Vector, Background, Alignment
};


const HEADER_HEIGHT: f32 = 32.0;
const CLOSE_BUTTON_SIZE: f32 = 30.0;
const CLOSE_BUTTON_OFFSET: f32 = 1.0;
const CONTENT_PADDING: f32 = 15.0;
const RESIZE_HANDLE_SIZE: f32 = 8.0;  // Size of resize hit areas
const MIN_OVERLAY_SIZE: f32 = 100.0;   // Minimum overlay dimensions


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
    /// If true, clicking outside the overlay closes it
    close_on_click_outside: bool,
    /// If true, hides the header completely (no title bar or close button)
    hide_header: bool,
    /// Resize mode for the overlay
    resizable: ResizeMode,
    /// reset size and position on overlay closure
    reset_on_close: bool,
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
        let size = button_content.as_widget().size_hint();

        Self {
            // Overlay Button
            id: None,
            button_content,
            width: size.width.fluid(),
            height: size.height.fluid(),
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
            
            // Overlay behavior options
            hover: Hover::default(),
            hover_positions_on_click: false,
            is_pressed: false,
            opaque: false,
            close_on_click_outside: false,
            hide_header: false,
            resizable: ResizeMode::None,
            reset_on_close: false,
        }
    }

    /// Sets the [`widget::Id`] of the [`Generic Overlay`].
    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
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
    pub fn on_open(
        mut self, 
        callback: impl Fn(Point, Size) -> Message + 'a)
    -> Self {
        self.on_open = Some(Box::new(callback));
        self
    }

    /// Sets a callback for when the overlay is closed
    pub fn on_close(mut self, callback: impl Fn() -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(callback));
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

    /// If true, hides the header (no title bar or close button)
    #[must_use]
    pub fn hide_header(mut self) -> Self {
        self.hide_header = true;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    Top,
    Bottom,
    Left,
    Right,
}

impl Position {
    pub const ALL: &'static [Self] = &[
        Self::Top,
        Self::Right,
        Self::Bottom,   
        Self::Left  
    ];
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

#[derive(Debug, Clone)]
pub struct Hover {
    pub enabled: bool,
    pub config: HoverConfig,
}

impl Default for Hover {
    fn default() -> Self {
        Self {
            enabled: false,
            config: HoverConfig::default(),
        }
    }
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
        let on_right = cursor_pos.x >= bounds.x + bounds.width - handle && cursor_pos.x <= bounds.x + bounds.width;
        let on_top = cursor_pos.y >= bounds.y && cursor_pos.y <= bounds.y + handle;
        let on_bottom = cursor_pos.y >= bounds.y + bounds.height - handle && cursor_pos.y <= bounds.y + bounds.height;
        
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
        matches!(self, Self::Top | Self::Bottom | Self::TopLeft | Self::TopRight | Self::BottomLeft | Self::BottomRight)
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

#[derive(Debug, Clone)]
struct State<P>
where 
    P: iced::advanced::text::Paragraph
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
}

impl<P: iced::advanced::text::Paragraph> State<P> {
    /// Resets the state to default values, effectively closing the overlay
    /// and forcing a recalculation of size/position on the next open.
    fn reset(&mut self) {
        self.is_open = false;
        
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
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer> 
    for OverlayButton<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: iced::widget::button::Catalog + iced::widget::text::Catalog + iced::widget::container::Catalog + Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(
            State {
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
            }
        )
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&(self.content)), Tree::new(&(self.button_content))]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content, &self.button_content]);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self, 
        tree: &mut Tree, 
        renderer: &Renderer, 
        limits: &Limits
    ) -> Node {
        layout::padded(
            limits,
            self.width,
            self.height,
            self.padding,
            |limits| {
                self.button_content.as_widget_mut().layout(
                    &mut tree.children[1],
                    renderer,
                    limits,
                )
            },
        )
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
    ) 
    where 
        Theme: Catalog + button::Catalog,
    {
        let bounds = layout.bounds();
        let button_content_layout = layout.children().next().unwrap();
        let style = <Theme as button::Catalog>::style(theme, &self.button_class, self.status.unwrap_or(button::Status::Active));

        if style.background.is_some()
            || style.border.width > 0.0
            || style.shadow.color.a > 0.0
        {
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
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                if self.is_pressed {
                        self.is_pressed = false;
                        self.status = Some(button::Status::Active);
                    }
            }

            Event::Mouse(mouse::Event::CursorMoved { position: _ }) => {
                if cursor.is_over(layout.bounds()) {
                    self.status = Some(button::Status::Hovered);
                    shell.invalidate_layout();
                } else {
                    self.status = Some(button::Status::Active);
                    if state.suppress_hover_reopen && self.hover.enabled { state.suppress_hover_reopen = !state.suppress_hover_reopen }
                    shell.invalidate_layout();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) {
                    self.status = Some(button::Status::Pressed);
                    
                    let should_open = if !self.hover.enabled { // Normal click mode - open, close is handled in overlay
                        true 
                    } else if !state.suppress_hover_reopen { // First hover click - close
                        state.suppress_hover_reopen = true;
                        false
                    } else {
                        state.suppress_hover_reopen = false; // Second hover click - reopen
                        true
                    };
                    
                    state.is_open = should_open;
                    
                    if should_open {
                        if let Some(on_open) = &self.on_open {
                            shell.publish(on_open(state.position, Size::new(state.current_width, state.current_height)));
                        }
                    }
                    
                    self.is_pressed = true;
                    shell.capture_event();
                    shell.invalidate_layout();
                    shell.request_redraw();
                    return;
                }
            }
            _ => {}
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
                if let Some(on_open) = &self.on_open {
                    shell.publish(on_open(state.position, Size::new(state.current_width, state.current_height)));
                }
                shell.invalidate_layout();
                shell.request_redraw();
            }

            // Close when cursor exits both button and overlay
            if !state.cursor_over_button && !state.cursor_over_overlay && state.is_open {
                state.reset();
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
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();
        let bounds = layout.bounds().expand(self.padding);
        
        // Only show interaction when overlay is closed
        if state.is_open {
            return mouse::Interaction::None;
        }

        if cursor.is_over(bounds) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
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
        let resolve_strategy = |strategy: &Option<SizeStrategy<'a>>, available_space: f32| -> Length {
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
            let resolved_width = match width_strategy {
                Length::Fixed(w) => w,
                Length::Fill => viewport.width,
                Length::FillPortion(_) => viewport.width,
                Length::Shrink => {
                    let measure_limits = Limits::new(Size::ZERO, Size::new(viewport.width, f32::INFINITY));
                    let temp_node = self.content
                        .as_widget_mut()
                        .layout(content_tree, renderer, &measure_limits);
                    temp_node.size().width + padding
                }
            };
            
            state.current_width = resolved_width;
            let init_auto = self.overlay_height.is_none() || matches!(height_strategy, Length::Shrink);
            state.height_auto = init_auto;

            // First layout with resolved width to measure natural content height
            let init_limits = Limits::new(
                Size::ZERO,
                Size::new(state.current_width - padding, f32::INFINITY),
            );
            content_node = self.content
                .as_widget_mut()
                .layout(content_tree, renderer, &init_limits);
            computed_content_h = content_node.size().height;

            if init_auto {
                state.current_height = header_height + computed_content_h + padding;
            } else {
                let resolved_height = match height_strategy {
                    Length::Fixed(h) => h,
                    Length::Fill => width_limits.max().height,
                    Length::FillPortion(_) => width_limits.max().height,
                    Length::Shrink => header_height + computed_content_h + padding,
                };
                
                state.current_height = resolved_height;

                let constrained_h = state.current_height - header_height - padding;
                let constrained_limits = Limits::new(
                    Size::ZERO,
                    Size::new(state.current_width - padding, constrained_h),
                );
                content_node = self.content
                    .as_widget_mut()
                    .layout(content_tree, renderer, &constrained_limits);
                computed_content_h = content_node.size().height;
            }
        } else if state.height_auto {
            let auto_limits = Limits::new(
                Size::ZERO,
                Size::new(state.current_width - padding, f32::INFINITY),
            );
            content_node = self.content
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
            content_node = self.content
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
            class: <Theme as Catalog>::default(),
            content: &mut self.content,
            radius: self.overlay_radius,
            tree: content_tree,
            width: total_w,
            height: total_h,
            padding: self.overlay_padding,
            on_close: self.on_close.as_deref(),
            button_bounds,
            button_padding: self.padding,
            hover: &self.hover,
            hover_positions_on_click: self.hover_positions_on_click,
            content_layout: content_node,
            opaque: self.opaque,
            close_on_click_outside: self.close_on_click_outside,
            hide_header: self.hide_header,
            resizable: self.resizable,
        })))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        
        operation.custom(self.id.as_ref(), layout.bounds(), state);
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
    class: Theme::Class<'a>,
    title: &'a str,
    content: &'a mut Element<'b, Message, Theme, Renderer>,
    tree: &'a mut Tree,
    width: f32,
    height: f32,
    padding: f32,
    radius: f32,
    on_close: Option<&'a dyn Fn() -> Message>,
    button_bounds: Rectangle,
    button_padding: Padding,
    hover: &'a Hover,
    hover_positions_on_click: bool,
    content_layout: Node,
    opaque: bool,
    close_on_click_outside: bool,
    hide_header: bool,
    resizable: ResizeMode,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: iced::widget::container::Catalog 
        + iced::widget::button::Catalog 
        + iced::widget::text::Catalog
        + Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>
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

        if self.hover.enabled  || self.hover_positions_on_click {
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
                                Alignment::Center => self.button_bounds.x 
                                    + (self.button_bounds.width - overlay_width) / 2.0,
                                Alignment::End => self.button_bounds.x 
                                    + self.button_bounds.width - overlay_width,
                            };
                            
                            let y = if self.hover.config.position == Position::Top {
                                self.button_bounds.y - overlay_height - self.hover.config.gap
                            } else {
                                self.button_bounds.y + self.button_bounds.height + self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                        Position::Left | Position::Right => {
                            let y = match self.hover.config.alignment {
                                Alignment::Start => self.button_bounds.y,
                                Alignment::Center => self.button_bounds.y 
                                    + (self.button_bounds.height - overlay_height) / 2.0,
                                Alignment::End => self.button_bounds.y 
                                    + self.button_bounds.height - overlay_height,
                            };
                            
                            let x = if self.hover.config.position == Position::Left {
                                self.button_bounds.x - overlay_width - self.hover.config.gap
                            } else {
                                self.button_bounds.x + self.button_bounds.width + self.hover.config.gap
                            };
                            
                            Point::new(x, y)
                        }
                    }
                }
                PositionMode::Inside => {
                    let content_bounds = Rectangle {
                        x: self.button_bounds.x + self.button_padding.left,
                        y: self.button_bounds.y + self.button_padding.top,
                        width: self.button_bounds.width - self.button_padding.left - self.button_padding.right,
                        height: self.button_bounds.height - self.button_padding.top - self.button_padding.bottom,
                    };

                    // New behavior - overlay anchored inside button bounds
                    match self.hover.config.position {
                        Position::Top | Position::Bottom => {
                            // Horizontal positioning from content edges
                            let x = match self.hover.config.alignment {
                                Alignment::Start => content_bounds.x + self.hover.config.gap,
                                Alignment::Center => content_bounds.x 
                                    + (content_bounds.width - overlay_width) / 2.0,
                                Alignment::End => content_bounds.x 
                                    + content_bounds.width - overlay_width - self.hover.config.gap,
                            };
                            
                            // Vertical positioning from content edges (inward)
                            let y = if self.hover.config.position == Position::Top {
                                content_bounds.y + self.hover.config.gap
                            } else {
                                content_bounds.y + content_bounds.height - overlay_height - self.hover.config.gap
                            };

                            Point::new(x, y)
                        }
                        Position::Left | Position::Right => {
                            // Vertical positioning from content edges
                            let y = match self.hover.config.alignment {
                                Alignment::Start => content_bounds.y + self.hover.config.gap,
                                Alignment::Center => content_bounds.y 
                                    + (content_bounds.height - overlay_height) / 2.0,
                                Alignment::End => content_bounds.y 
                                    + content_bounds.height - overlay_height - self.hover.config.gap,
                            };
                            
                            // Horizontal positioning from content edges (inward)
                            let x = if self.hover.config.position == Position::Left {
                                content_bounds.x + self.hover.config.gap
                            } else {
                                content_bounds.x + content_bounds.width - overlay_width - self.hover.config.gap
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
                } else if calculated_position.x + overlay_width > self.state.window_bounds.x + self.state.window_bounds.width {
                    calculated_position.x = self.state.window_bounds.x + self.state.window_bounds.width - overlay_width;
                }
                
                // Vertical bounds checking
                if calculated_position.y < self.state.window_bounds.y {
                    calculated_position.y = self.state.window_bounds.y;
                } else if calculated_position.y + overlay_height > self.state.window_bounds.y + self.state.window_bounds.height {
                    calculated_position.y = self.state.window_bounds.y + self.state.window_bounds.height - overlay_height;
                }
            }
            
            // Override the state position with calculated position
            self.state.position = calculated_position;
        }

        Node::new(size).move_to(self.state.position)
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
        let draw_style = <Theme as Catalog>::style(&theme, &self.class);

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
                    Color::from_rgba(0.0, 0.0, 0.0, 0.3),
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
                    shadow: draw_style.shadow,
                    snap: true,
                },
                draw_style.background,
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
                    draw_style.header_background,
                );

                // Draw title
                renderer.fill_text(
                    iced::advanced::Text {
                        content: self.title.to_string(),
                        bounds: Size::new(header_bounds.width - CLOSE_BUTTON_SIZE - 20.0, header_bounds.height),
                        size: iced::Pixels(16.0),
                        font: iced::Font::default(),
                        align_x: iced::advanced::text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Advanced,
                        wrapping: iced::advanced::text::Wrapping::default(),
                    },
                    Point::new(header_bounds.center_x() - (CLOSE_BUTTON_SIZE / 2.0), header_bounds.center_y()),
                    draw_style.text_color,
                    header_bounds,
                );

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
                        content: "×".to_string(),
                        bounds: Size::new(close_bounds.width, close_bounds.height),
                        size: iced::Pixels(24.0),
                        font: iced::Font::default(),
                        align_x: iced::advanced::text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Basic,
                        wrapping: iced::advanced::text::Wrapping::default(),
                    },
                    Point::new(close_bounds.center_x(), close_bounds.center_y()),
                    draw_style.text_color,
                    close_bounds,
                );
            }

            // Draw content
            let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
            let content_bounds = Rectangle {
                x: bounds.x + self.padding,
                y: bounds.y + header_height + self.padding,
                width: bounds.width - self.padding * 2.0,
                height: bounds.height - header_height - self.padding * 2.0,
            };

            renderer.with_translation(
                Vector::new(content_bounds.x, content_bounds.y),
                |renderer| {
                    // Adjust cursor to content coordinate space
                    let adjusted_cursor = cursor.position().map(|position| {
                        mouse::Cursor::Available(Point::new(
                            position.x - content_bounds.x,
                            position.y - content_bounds.y,
                        ))
                    }).unwrap_or(mouse::Cursor::Unavailable);

                    self.content.as_widget().draw(
                        self.tree,
                        renderer,
                        theme,
                        style,
                        Layout::new(&self.content_layout),
                        adjusted_cursor,
                        &Rectangle::new(Point::ORIGIN, content_bounds.size()),
                    );
                },
            );
        });
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
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
                return;
            }
            Event::Keyboard(keyboard::Event::KeyReleased { 
                key: keyboard::Key::Named(keyboard::key::Named::Control),
                ..
            }) => {
                self.state.ctrl_pressed = false;
                return;
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.state.ctrl_pressed = modifiers.control();
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
                if cursor.is_over(self.button_bounds) && self.state.is_open {
                    self.state.reset();
                    shell.invalidate_layout();
                    shell.request_redraw();
                    shell.capture_event();
                    return
                }

                if self.close_on_click_outside && !cursor_over_overlay && self.state.is_open {
                    self.state.reset();
                    if let Some(on_close) = self.on_close {
                        shell.publish(on_close());
                    }
                    shell.invalidate_layout();
                    shell.request_redraw();
                    return;
                }
                
                // If opaque and clicking outside, consume the event without forwarding
                if self.opaque && !cursor_over_overlay {
                    return;  // Block event from reaching widgets below
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
                            self.state.drag_offset = Vector::new(
                                position.x - bounds.x,
                                position.y - bounds.y,
                            );
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }
                    }

                    // Handle close button
                    if !self.hide_header {
                        let close_bounds = Rectangle {
                            x: bounds.x + bounds.width - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_OFFSET * 2.0,
                            y: bounds.y + (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2.0,
                            width: CLOSE_BUTTON_SIZE,
                            height: CLOSE_BUTTON_SIZE,
                        };

                        if cursor.is_over(close_bounds) {
                            self.state.reset();
                            if let Some(on_close) = self.on_close {
                                shell.publish(on_close());
                            }
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }

                        // Handle header dragging
                        let header_bounds = Rectangle {
                            x: bounds.x,
                            y: bounds.y,
                            width: bounds.width,
                            height: HEADER_HEIGHT,
                        };

                        if cursor.is_over(header_bounds) {
                            self.state.is_dragging = true;
                            self.state.drag_offset = Vector::new(
                                position.x - bounds.x,
                                position.y - bounds.y,
                            );
                            shell.invalidate_layout();
                            shell.request_redraw();
                            return;
                        }
                    }

                    // Handle Ctrl+drag from anywhere in the overlay
                    if self.state.ctrl_pressed && cursor_over_overlay {
                        self.state.is_dragging = true;
                        self.state.drag_offset = Vector::new(
                            position.x - bounds.x,
                            position.y - bounds.y,
                        );
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
                    return;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // handle hover first
                if self.hover.enabled || self.hover_positions_on_click {
                    self.state.cursor_over_overlay = cursor.is_over(layout.bounds().expand(self.hover.config.buffer));
                    self.state.cursor_over_button = cursor.is_over(self.button_bounds.expand(self.hover.config.buffer));
                    
                    // Close if cursor over neither button nor overlay
                    if !self.state.cursor_over_button && !self.state.cursor_over_overlay && !has_open_descendant_overlays::<Renderer::Paragraph>(self.tree) {
                        self.state.reset();
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
                                new_width = (self.state.resize_start_size.width - delta_x).max(MIN_OVERLAY_SIZE);
                                new_x = self.state.resize_start_position.x + delta_x;
                            }
                            ResizeEdge::Right | ResizeEdge::TopRight | ResizeEdge::BottomRight => {
                                new_width = (self.state.resize_start_size.width + delta_x).max(MIN_OVERLAY_SIZE);
                                // x unchanged
                            }
                            _ => {}
                        }

                        // Height and y position
                        match self.state.resize_edge {
                            ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight => {
                                new_height = (self.state.resize_start_size.height - delta_y).max(MIN_OVERLAY_SIZE);
                                new_y = self.state.resize_start_position.y + delta_y;
                            }
                            ResizeEdge::Bottom | ResizeEdge::BottomLeft | ResizeEdge::BottomRight => {
                                new_height = (self.state.resize_start_size.height + delta_y).max(MIN_OVERLAY_SIZE);
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
                        new_x = new_x.max(0.0).min(self.state.window_bounds.width - new_width);
                        new_y = new_y.max(0.0).min(self.state.window_bounds.height - new_height);
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
                    return;
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                self.state.reset();
                if let Some(on_close) = self.on_close {
                    shell.publish(on_close());
                }
                return;
            }
            _ => {}
        }

        // If opaque, consume ALL mouse/touch events that are outside the overlay
        if self.opaque {
            match event {
                Event::Mouse(_) | Event::Touch(_) => {
                    if !cursor.is_over(bounds) {
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

        let content_layout_node = self.content_layout.clone()
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
                clipboard,
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
            if can_resize {
                if let Some(position) = cursor.position() {
                    let resize_edge = ResizeEdge::from_position(position, bounds);
                    if resize_edge != ResizeEdge::None {
                        return resize_edge.cursor_icon();
                    }
                }
            }

            // Show pointer when over close button (if header is visible)
            if !self.hide_header {
                let close_bounds = Rectangle {
                    x: bounds.x + bounds.width - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_OFFSET * 2.0,
                    y: bounds.y + (HEADER_HEIGHT - CLOSE_BUTTON_SIZE) / 2.0,
                    width: CLOSE_BUTTON_SIZE,
                    height: CLOSE_BUTTON_SIZE,
                };

                if cursor.is_over(close_bounds) {
                    return mouse::Interaction::Pointer;
                }

                // Show grab cursor when over header
                let header_bounds = Rectangle {
                    x: bounds.x,
                    y: bounds.y,
                    width: bounds.width,
                    height: HEADER_HEIGHT,
                };

                if cursor.is_over(header_bounds) {
                    return if self.state.is_dragging {
                        mouse::Interaction::Grabbing
                    } else {
                        mouse::Interaction::Grab
                    };
                }
            }

            // Show grab/grabbing when Ctrl is pressed
            if self.state.ctrl_pressed {
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
            
            // If content doesn't want a specific interaction, return default to still block passthrough
            if content_interaction == mouse::Interaction::default() {
                return mouse::Interaction::Idle;  // ADD THIS - blocks passthrough
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
        // Get the actual bounds of the overlay window
        let bounds = layout.bounds();
        
        let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
        
        let content_bounds = Rectangle {
            x: bounds.x + self.padding,
            y: bounds.y + header_height + self.padding,
            width: bounds.width - self.padding * 2.0,
            height: bounds.height - header_height - self.padding * 2.0,
        };
        
        // Use the stored content layout
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
    Theme: iced::widget::button::Catalog + iced::widget::text::Catalog + iced::widget::container::Catalog + Catalog + 'a,
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
                type DefaultParagraph = <iced::Renderer as iced::advanced::text::Renderer>::Paragraph;
                
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
            let palette = theme.extended_palette();
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