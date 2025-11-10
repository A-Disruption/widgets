use iced::{
    advanced::{
        layout::{self, padded, Limits, Node},
        overlay,
        renderer,
        text::Renderer as _,
        text,
        widget::{self, tree::Tree},
        Clipboard, Layout, Overlay as _, Renderer as _, Shell, Widget,
    }, alignment::Vertical, border::Radius, event, keyboard, mouse, touch, widget::button, Border, Color, Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow, Size, Theme, Vector
};

// Constants matching color_picker.rs for consistency
const HEADER_HEIGHT: f32 = 32.0;
const CLOSE_BUTTON_SIZE: f32 = 30.0;
const CLOSE_BUTTON_OFFSET: f32 = 1.0;
const CONTENT_PADDING: f32 = 20.0;
const RESIZE_HANDLE_SIZE: f32 = 8.0;  // Size of resize hit areas
const MIN_OVERLAY_SIZE: f32 = 100.0;   // Minimum overlay dimensions


/// Helper function to create an overlay button
pub fn overlay_button<'a, Message, Theme, Renderer>(
    label: impl Into<String>,
    title: impl Into<String>,
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer> 
where 
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    OverlayButton::new(label, title, content)
}

/// A button that opens a draggable overlay with custom content
#[allow(missing_debug_implementations)]
pub struct OverlayButton<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> 
where
    Theme: Catalog + button::Catalog,
    Renderer: text::Renderer,
{
    /// The button label
    label: String,
    /// text size for button text
    label_text_size: Option<Pixels>,
    /// font for button text
    label_font: Option<Renderer::Font>,
    /// The overlay title
    title: String,
    /// text size for title text
    title_text_size: Option<Pixels>,
    /// font for title text
    title_font: Option<Renderer::Font>,
    /// Function to create the overlay content (called each time)
    content: Element<'a, Message, Theme, Renderer>,
    /// Optional width for the overlay (defaults to 400px)
    overlay_width: Option<f32>,
    /// Optional height for the overlay (defaults to content height)
    overlay_height: Option<f32>,
    /// Button width
    width: Length,
    /// Button height
    height: Length,
    /// Button padding
    padding: Padding,
    /// Callback when the overlay is opened
    on_open: Option<Box<dyn Fn() -> Message + 'a>>,
    /// Callback when the overlay is closed
    on_close: Option<Box<dyn Fn() -> Message + 'a>>,
    /// Class of the Overlay
    class: <Theme as Catalog>::Class<'a>,
    /// Get full window size for overlay bounds
    window_size: Option<Rectangle>,
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
}

impl<'a, Message, Theme, Renderer> OverlayButton<'a, Message, Theme, Renderer> 
where 
    Renderer: iced::advanced::Renderer + text::Renderer,
    Theme: Catalog + button::Catalog,
{
    /// Creates a new overlay button with the given label and content function
    pub fn new(
        label: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {

        Self {
            label: label.into(),
            label_text_size: None,
            label_font: None,
            title: title.into(),
            title_text_size: None,
            title_font: None,
            content: content.into(),
            overlay_width: None,
            overlay_height: None,
            width: Length::Fixed(50.0),
            height: Length::Fixed(30.0),
            padding: DEFAULT_PADDING,
            on_open: None,
            on_close: None,
            class: <Theme as Catalog>::default(),
            window_size: None,
            status: None,
            button_class: <Theme as button::Catalog>::default(),
            is_pressed: false,
            opaque: false,
            close_on_click_outside: false,
            hide_header: false,
            resizable: ResizeMode::None,
        }
    }

    /// Sets the overlay width
    pub fn overlay_width(mut self, width: f32) -> Self {
        self.overlay_width = Some(width);
        self
    }

    /// Sets the overlay height
    pub fn overlay_height(mut self, height: f32) -> Self {
        self.overlay_height = Some(height);
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
    pub fn on_open(mut self, callback: impl Fn() -> Message + 'a) -> Self {
        self.on_open = Some(Box::new(callback));
        self
    }

    /// Sets a callback for when the overlay is closed
    pub fn on_close(mut self, callback: impl Fn() -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(callback));
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
    /// Automatically enables close_on_click_outside.
    #[must_use]
    pub fn opaque(mut self, opaque: bool) -> Self {
        self.close_on_click_outside = true;
        self.opaque = opaque;
        self
    }

    /// If true, hides the header (no title bar or close button)
    /// Automatically enables close_on_click_outside.
    #[must_use]
    pub fn hide_header(mut self, hide: bool) -> Self {
        self.close_on_click_outside = true;
        self.hide_header = hide;
        self
    }

    /// Sets the resize mode for the overlay
    #[must_use]
    pub fn resizable(mut self, mode: ResizeMode) -> Self {
        self.resizable = mode;
        self
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

#[derive(Debug, Clone)]
struct State<P>
where 
    P: iced::advanced::text::Paragraph
{
    is_open: bool,
    position: Point,
    is_dragging: bool,
    drag_offset: Vector,
    window_size: Size,
    ctrl_pressed: bool,
    is_resizing: bool,
    resize_edge: ResizeEdge,
    resize_start_size: Size,
    resize_start_position: Point,
    resize_start_cursor: Point,
    current_width: f32,
    current_height: f32,
    height_auto: bool,
    label_text: widget::text::State<P>,
    title_text: widget::text::State<P>,
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
                window_size: Size::new(0.0, 0.0),
                ctrl_pressed: false,
                is_resizing: false,
                resize_edge: ResizeEdge::None,
                resize_start_size: Size::ZERO,
                resize_start_position: Point::ORIGIN,
                resize_start_cursor: Point::ORIGIN,
                current_width: 0.0,
                current_height: 0.0,
                height_auto: false,
                label_text: widget::text::State::<Renderer::Paragraph>::default(),
                title_text: widget::text::State::<Renderer::Paragraph>::default(),
            }
        )
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&(self.content))]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
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
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        let size = self.size();

        let available_label_width = limits.max().width + self.padding.horizontal();
        let available_label_height = limits.max().height + self.padding.vertical();
        let label_limits = layout::Limits::new(
            Size::ZERO,
            Size::new(available_label_width.max(0.0), available_label_height.max(0.0)),
        );

        // Calculate intrinsic size based on text content
        let label_node = widget::text::layout(
            &mut state.label_text,
            renderer,
            &label_limits,
            &self.label,
            widget::text::Format {
                width: Length::Shrink,
                height: Length::Shrink,
                line_height: text::LineHeight::default(),
                size: self.label_text_size,
                font: self.label_font,
                align_x: text::Alignment::Default,
                align_y: iced::alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::default(),
            },
        );
        
        label_node
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) 
    where 
        Theme: Catalog + button::Catalog,
    {
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        let bounds = layout.bounds().expand(self.padding);
//        let is_hovered = cursor.is_over(bounds);
        let style = <Theme as button::Catalog>::style(theme, &self.button_class, self.status.unwrap_or(button::Status::Active));

        // Draw button background
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    color: style.border.color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            style.background.unwrap()
        );

        // Draw button text
        renderer.fill_text(
            iced::advanced::Text {
                content: self.label.clone(),
                bounds: Size::new(bounds.width, bounds.height),
                size: iced::Pixels(16.0),
                font: iced::Font::default(),
                align_x: iced::advanced::text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: iced::advanced::text::LineHeight::default(),
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::advanced::text::Wrapping::default(),
            },
            Point::new(bounds.center_x(), bounds.center_y()),
            style.text_color,
            bounds,
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
        let bounds = layout.bounds().expand(self.padding);

        match event {
            Event::Window(iced::window::Event::Opened { size, .. })
            | Event::Window(iced::window::Event::Resized(size)) => {
                state.window_size = Size::new(size.width, size.height);
            }

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
                    shell.invalidate_layout();
                }
            }
            _ => {}
        }

        if state.is_open {
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) {
                    self.status = Some(button::Status::Pressed);
                    self.is_pressed = true;
                    state.is_open = true;
                    shell.invalidate_layout();
                }
            }

            Event::Window(iced::window::Event::Opened { position: _, size }) => {
                let window_size = Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: size.width,
                    height: size.height,
                };

                self.window_size = Some(window_size);
            }
            Event::Window(iced::window::Event::Resized(size)) => {
                let window_size = Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width: size.width,
                    height: size.height,
                };

                self.window_size = Some(window_size);
            }
            _ => {}
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
        let padding = CONTENT_PADDING * 2.0;

        // Get fallback window if uninitialized
        let window_size = if state.window_size.width > 0.0 && state.window_size.height > 0.0 {
            state.window_size
        } else {
            Size::new(800.0, 600.0)  // Fallback
        };
        let fullscreen = Rectangle::new(Point::ORIGIN, window_size);

        let content_tree = &mut tree.children[0];

        let mut content_node: Node;
        let mut computed_content_h: f32;

        // Initialize sizes if needed
        if state.current_width == 0.0 {
            state.current_width = self.overlay_width.unwrap_or(400.0);
            let init_auto = self.overlay_height.is_none();
            state.height_auto = init_auto;

            // First layout with infinite height to measure natural content height
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
                state.current_height = self.overlay_height.unwrap_or(300.0);

                // Relayout with constrained height for non-auto init
                let constrained_h = state.current_height - header_height - padding;
                let constrained_limits = Limits::new(
                    Size::ZERO,
                    Size::new(state.current_width - padding, constrained_h),
                );
                content_node = self.content
                    .as_widget_mut()
                    .layout(content_tree, renderer, &constrained_limits);
                computed_content_h = content_node.size().height;  // Updated, but not used for height
            }
        } else if state.height_auto {
            // For auto-height runtime: layout infinite, update height
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
            // For fixed-height runtime: layout constrained
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

        // Initial position if ORIGIN (use computed sizes)
        if state.position == Point::ORIGIN {
            let ow = state.current_width;
            let oh = state.current_height;
            state.position = Point::new(
                (window_size.width - ow) / 2.0,
                (window_size.height - oh) / 2.0,
            );
        }

        let total_w = state.current_width;
        let total_h = state.current_height;

        Some(overlay::Element::new(Box::new(Overlay {
            state,
            title: &self.title,
            class: <Theme as Catalog>::default(),
            content: &mut self.content,
            tree: content_tree,
            width: total_w,
            height: total_h,
            viewport: fullscreen,
            on_close: self.on_close.as_deref(),
            content_layout: content_node,
            opaque: self.opaque,
            close_on_click_outside: self.close_on_click_outside,
            hide_header: self.hide_header,
            resizable: self.resizable,
        })))
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
    viewport: Rectangle,
    on_close: Option<&'a dyn Fn() -> Message>,
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
        let size = Size::new(self.width, self.height);
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
        renderer.with_layer(self.viewport, |renderer| {
            // Draw opaque backdrop if requested
            if self.opaque {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: self.viewport,
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
                        radius: 12.0.into(),
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
                                top_left: 12.0,
                                top_right: 12.0,
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
                x: bounds.x + CONTENT_PADDING,
                y: bounds.y + header_height + CONTENT_PADDING,
                width: bounds.width - CONTENT_PADDING * 2.0,
                height: bounds.height - header_height - CONTENT_PADDING * 2.0,
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

        // Handle header interactions (if header is visible)
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let cursor_over_overlay = cursor.is_over(bounds);
                
                if self.close_on_click_outside && !cursor_over_overlay {
                    self.state.is_open = false;
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
                            self.state.is_open = false;
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
                self.state.is_dragging = false;
                self.state.is_resizing = false;
                self.state.resize_edge = ResizeEdge::None;
                shell.invalidate_layout();
                shell.request_redraw();
                
                // If opaque, consume the event
                if self.opaque {
                    return;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(position) = cursor.position() {
                    // Handle resizing
                    if self.state.is_resizing {
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
                        new_x = new_x.max(0.0).min(self.viewport.width - new_width);
                        new_y = new_y.max(0.0).min(self.viewport.height - new_height);
                        self.state.position = Point::new(new_x, new_y);
                        
                        shell.invalidate_layout();
                        shell.request_redraw();
                        return;
                    }

                    // Handle dragging
                    if self.state.is_dragging {
                        let new_x = position.x - self.state.drag_offset.x;
                        let new_y = position.y - self.state.drag_offset.y;

                        self.state.position.x = new_x
                            .max(0.0)
                            .min(self.viewport.width - self.state.current_width);
                        self.state.position.y = new_y
                            .max(0.0)
                            .min(self.viewport.height - self.state.current_height);

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
                self.state.is_open = false;
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
            x: bounds.x + CONTENT_PADDING,
            y: bounds.y + header_height + CONTENT_PADDING,
            width: bounds.width - CONTENT_PADDING * 2.0,
            height: bounds.height - header_height - CONTENT_PADDING * 2.0,
        };

        // Only forward events to content if not dragging and if cursor is in content area
        if !self.state.is_dragging && !self.state.is_resizing {
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

            // Use pre-computed content layout for proper event handling
            self.content.as_widget_mut().update(
                self.tree,
                event,
                Layout::new(&self.content_layout),
                adjusted_cursor,
                renderer,
                clipboard,
                shell,
                &Rectangle::new(Point::ORIGIN, content_bounds.size()),
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

        // Determine if we should be resizable
        let can_resize = match self.resizable {
            ResizeMode::None => false,
            ResizeMode::Always => true,
            ResizeMode::WithCtrl => self.state.ctrl_pressed,
        };

        // Show resize cursors if resizable
        if can_resize {
            if let Some(position) = cursor.position() {
                if cursor.is_over(bounds) {
                    let resize_edge = ResizeEdge::from_position(position, bounds);
                    if resize_edge != ResizeEdge::None {
                        return resize_edge.cursor_icon();
                    }
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

        // Show grab/grabbing when Ctrl is pressed and over the overlay
        if self.state.ctrl_pressed && cursor.is_over(bounds) {
            return if self.state.is_dragging {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Grab
            };
        }

        // Forward to content
        let header_height = if self.hide_header { 0.0 } else { HEADER_HEIGHT };
        let content_bounds = Rectangle {
            x: bounds.x + CONTENT_PADDING,
            y: bounds.y + header_height + CONTENT_PADDING,
            width: bounds.width - CONTENT_PADDING * 2.0,
            height: bounds.height - header_height - CONTENT_PADDING * 2.0,
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

        self.content.as_widget().mouse_interaction(
            self.tree,
            Layout::new(&self.content_layout),
            adjusted_cursor,
            &Rectangle::new(Point::ORIGIN, content_bounds.size()),
            renderer,
        )
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
            x: bounds.x + CONTENT_PADDING,
            y: bounds.y + header_height + CONTENT_PADDING,
            width: bounds.width - CONTENT_PADDING * 2.0,
            height: bounds.height - header_height - CONTENT_PADDING * 2.0,
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