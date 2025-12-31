use iced::{
    advanced::{
        layout::{Limits, Node}, overlay, renderer, text::Renderer as _, widget::{self, tree::Tree}, Clipboard, Layout, Overlay, Renderer as _, Shell, Widget
    },
    alignment:: Vertical,
    keyboard, mouse, touch,
    widget::text,
    Border, Color, Element, Event, Length, Padding, Point, Rectangle, 
    Renderer, Shadow, Size, Vector,
};
use std::time::{Duration, Instant};
use std::cell::{RefCell, Cell};

static mut ACTIVE_COLOR_PICKER: Option<*mut bool> = None;

const HEADER_HEIGHT: f32 = 32.0;
const CLOSE_BUTTON_SIZE: f32 = 30.0;
const CLOSE_BUTTON_OFFSET: f32 = 2.5;
const TAB_HEIGHT: f32 = 32.0;
const TAB_SPACING: f32 = 8.0;
const CONTENT_PADDING: f32 = 20.0;

/// Helper function to create a color button
pub fn color_button<'a, Message>(
    color: Color,
) -> ColorButton<'a, Message> {
    ColorButton::new(color)
}

/// A button that displays a color and opens a color picker when clicked
pub struct ColorButton<'a, Message> {
    color: Color,
    on_change: Option<Box<dyn Fn(Color) -> Message + 'a>>,
    on_change_with_source: Option<Box<dyn Fn(Color, Option<String>) -> Message + 'a>>,
    width: Length,
    height: Length,
    padding: Padding,
    border_radius: f32,
    border_width: f32,
    title: String,
    text: Option<String>,
    show_hex: bool,
}

impl<'a, Message> ColorButton<'a, Message> {
    /// Creates a new color button with the given color
    pub fn new(color: Color) -> Self {
        Self {
            color,
            on_change: None,
            on_change_with_source: None,
            width: Length::Fixed(30.0),
            height: Length::Fixed(20.0),
            padding: Padding::ZERO,
            border_radius: 4.0,
            border_width: 1.0,
            title: "Color".to_string(),
            text: None,
            show_hex: false,
        }
    }

    /// Sets the title for the color picker overlay
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the width of the button
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the button
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the padding of the button
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the border radius
    pub fn border_radius(mut self, radius: f32) -> Self {
        self.border_radius = radius;
        self
    }

    /// Sets the border width
    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    fn is_light_color(&self, color: Color) -> bool {
        // Calculate luminance using standard formula
        let luminance = 0.299 * color.r + 0.587 * color.g + 0.114 * color.b;
        luminance > 0.5
    }

    /// Shows custom text in the center of the button
    pub fn show_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self.show_hex = false;
        self
    }

    /// Shows the hex value of the current color
    pub fn show_hex(mut self) -> Self {
        self.show_hex = true;
        self.text = None;
        self
    }

    /// Sets a callback that receives the color
    pub fn on_change(mut self, callback: impl Fn(Color) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Sets a callback that receives both color and optional theme path
    pub fn on_change_with_source(mut self, callback: impl Fn(Color, Option<String>) -> Message + 'a) -> Self {
        self.on_change_with_source = Some(Box::new(callback));
        self
    }

}

#[derive(Debug, Clone)]
struct State {
    is_open: bool,
    color: Color,
    overlay_state: OverlayState,
    title: String,
    overlay_position: Point,
    window_size: Option<Size>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            is_open: false,
            color: Color::WHITE,
            overlay_state: OverlayState::from_color(Color::WHITE),
            title: "Color".to_string(),
            overlay_position: Point::new(0.0, 0.0),
            window_size: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PickTarget { Color, Text }

#[derive(Clone, Copy, Debug, PartialEq)]
enum ColorString { Hex, Rgb}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Tone { color: Color, text: Color }

#[derive(Clone, Debug)]
struct PaletteRow {
    name: &'static str,
    tones: Vec<(&'static str, Tone)>, // (label, tone)
}

impl<'a, Message: Clone + 'a> Widget<Message, iced::Theme, Renderer> for ColorButton<'a, Message> {
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            color: self.color,
            overlay_state: OverlayState::from_color(self.color),
            title: self.title.clone(),
            ..State::default()
        })
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &Limits,
    ) -> Node {
        let limits = limits.width(self.width).height(self.height);
        let size = limits.resolve(self.width, self.height, Size::ZERO);
        Node::new(size)
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = state.state.downcast_ref::<State>();

        // Draw the color button
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    color: if state.is_open { 
                        theme.palette().primary 
                    } else { 
                        Color::from_rgb(0.5, 0.5, 0.5) 
                    },
                    width: self.border_width,
                    radius: self.border_radius.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            state.color,
        );

        // Render text if enabled
        if self.show_hex || self.text.is_some() {
            let text_content = if self.show_hex {
                color_to_hex(state.color)
            } else {
                self.text.as_ref().unwrap().clone()
            };

            // Choose contrasting text color
            let text_color = if self.is_light_color(state.color) {
                Color::BLACK
            } else {
                Color::WHITE
            };

            // Calculate appropriate font size based on button size
            let font_size = (bounds.height * 0.3).min(14.0).max(8.0);

            renderer.fill_text(
                iced::advanced::Text {
                    content: text_content,
                    bounds: Size::new(bounds.width, bounds.height),
                    size: iced::Pixels(font_size),
                    font: iced::Font::default(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(bounds.center_x(), bounds.center_y()),
                text_color,
                bounds,
            );
        }

    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = state.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(bounds) {
                    state.is_open = !state.is_open;
                    state.overlay_state.palette_cache_dirty.set(true);
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
            }
            Event::Window(iced::window::Event::Opened { size, .. })
            | Event::Window(iced::window::Event::Resized(size)) => {
                state.window_size = Some(Size::new(size.width, size.height));
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _state: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn overlay<'b>(
        &'b mut self,
        state: &'b mut Tree,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, Renderer>> {
        let widget_state = state.state.downcast_mut::<State>();
        
        if widget_state.is_open {

            unsafe {   // Doesn't seem like a good idea?
                if let Some(active) = ACTIVE_COLOR_PICKER
                    && !std::ptr::eq(active, &mut widget_state.is_open) {
                        // Close the other picker
                        *active = false;
                    }
                
                widget_state.is_open = true;
                ACTIVE_COLOR_PICKER = Some(&mut widget_state.is_open as *mut bool);
            }

            // Calculate centered position
            let overlay_width = 320.0;
            let overlay_height = 440.0;

            // We need to handle the state updates through a wrapper
            let overlay_state = &mut widget_state.overlay_state;
            let is_open = &mut widget_state.is_open;
            let color = &mut widget_state.color;
            let position = &mut widget_state.overlay_position;
            let on_change = &self.on_change;
            let on_change_with_source = &self.on_change_with_source;

            if position.x == 0.0 && position.y == 0.0 {
                *position = Point::new(
                    (viewport.width - overlay_width) / 2.0,
                    (viewport.height - overlay_height) / 2.0,
                );
            }
            
            Some(
                ModernColorPickerOverlay {
                    overlay_state,
                    is_open,
                    color,
                    on_change,
                    on_change_with_source,
                    position,
                    title: widget_state.title.clone(),
                    viewport_size: widget_state.window_size.unwrap_or(viewport.size()),
                }
                .overlay()
            )
        } else {
            None
        }
    }
}

impl<'a, Message: Clone + 'a> From<ColorButton<'a, Message>> for Element<'a, Message, iced::Theme, Renderer> {
    fn from(button: ColorButton<'a, Message>) -> Self {
        Self::new(button)
    }
}


// Modern overlay implementation with tabs
#[derive(Debug, Clone)]
struct OverlayState {
    active_tab: ColorPickerTab,
    // Grid tab state
    // Spectrum tab state
    hue: f32,
    saturation: f32,
    value: f32,
    spectrum_dragging: bool,
    // Sliders tab state
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
    hex_input: String,
    // Common
    preset_colors: Vec<Color>,
    // Dragging sliders
    hue_dragging: bool,
    dragging_slider: Option<SliderType>,
    // Dragging state for the overlay window
    is_dragging: bool,
    drag_offset: Vector,
    // feedback timer for "Copied!"
    copied_at: Option<Instant>, 

    // filled in draw, read in update
    palette_cache: RefCell<Vec<PaletteRow>>,
    // mark true when overlay opens or tab switches to Palette
    palette_cache_dirty: Cell<bool>,

    // Track if current color came from palette
    palette_source: Option<PaletteSource>,    
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ColorPickerTab {
    Grid,
    Spectrum,
    Sliders,
    Palette
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SliderType {
    Red,
    Green,
    Blue,
    Alpha,
}

impl OverlayState {
    fn from_color(color: Color) -> Self {
        let (h, s, v) = rgb_to_hsv(color);
        Self {
            active_tab: ColorPickerTab::Grid,
            hue: h,
            saturation: s,
            value: v,
            spectrum_dragging: false,
            red: color.r,
            green: color.g,
            blue: color.b,
            alpha: color.a,
            hex_input: color_to_hex(color),
            preset_colors: vec![
                Color::BLACK,
                Color::WHITE,
                Color::from_rgb8(0x00, 0x7A, 0xFF), // Blue
                Color::from_rgb8(0x00, 0xC8, 0x00), // Green
                Color::from_rgb8(0xFF, 0xD7, 0x00), // Yellow
                Color::from_rgb8(0xFF, 0x00, 0x00), // Red
            ],
            hue_dragging: false,
            dragging_slider: None,
            is_dragging: false,
            drag_offset: Vector::new(0.0, 0.0),
            copied_at: None,
            palette_cache: RefCell::new(Vec::new()),
            palette_cache_dirty: Cell::new(true),
            palette_source: None
        }
    }

    fn update_from_hsv(&mut self) {
        let color = hsv_to_rgb(self.hue, self.saturation, self.value);
        self.red = color.r;
        self.green = color.g;
        self.blue = color.b;
        self.hex_input = color_to_hex(color);
    }

    fn update_from_rgb(&mut self) {
        let color = Color::from_rgba(self.red, self.green, self.blue, self.alpha);
        let (h, s, v) = rgb_to_hsv(color);

        // If saturation is zero, hue is undefined: keep last hue value
        self.hue = if s == 0.0 { self.hue } else { h };
        self.saturation = s;
        self.value = v;
        self.hex_input = color_to_hex(color);
    }

    /// Generate theme path code from palette source
    fn palette_to_code(&self) -> Option<String> {
        let source = self.palette_source.as_ref()?;
        
        // Build the base theme path
        let base = match (source.row, source.tone, source.pick_target) {
            // Background paths
            ("Background", "Base", PickTarget::Color) => "theme.extended_palette()    \n.background.base.color\n",
            ("Background", "Base", PickTarget::Text) => "theme.extended_palette()    \n.background.base.text\n",
            ("Background", "Neutral", PickTarget::Color) => "theme.extended_palette()    \n.background.neutral.color\n",
            ("Background", "Neutral", PickTarget::Text) => "theme.extended_palette()    \n.background.neutral.text\n",
            ("Background", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.background.weak.color\n",
            ("Background", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.background.weak.text\n",
            ("Background", "Weaker", PickTarget::Color) => "theme.extended_palette()    \n.background.weaker.color\n",
            ("Background", "Weaker", PickTarget::Text) => "theme.extended_palette()    \n.background.weaker.text\n",
            ("Background", "Weakest", PickTarget::Color) => "theme.extended_palette()    \n.background.weakest.color\n",
            ("Background", "Weakest", PickTarget::Text) => "theme.extended_palette()    \n.background.weakest.text\n",
            ("Background", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.background.strong.color\n",
            ("Background", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.background.strong.text\n",
            ("Background", "Stronger", PickTarget::Color) => "theme.extended_palette()    \n.background.stronger.color\n",
            ("Background", "Stronger", PickTarget::Text) => "theme.extended_palette()    \n.background.stronger.text\n",
            ("Background", "Strongest", PickTarget::Color) => "theme.extended_palette()    \n.background.strongest.color\n",
            ("Background", "Strongest", PickTarget::Text) => "theme.extended_palette()    \n.background.strongest.text\n",
            
            // Primary paths
            ("Primary", "Base", PickTarget::Color) => "theme.extended_palette()    \n.primary.base.color\n",
            ("Primary", "Base", PickTarget::Text) => "theme.extended_palette()    \n.primary.base.text\n",
            ("Primary", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.primary.weak.color\n",
            ("Primary", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.primary.weak.text\n",
            ("Primary", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.primary.strong.color\n",
            ("Primary", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.primary.strong.text\n",
            
            // Secondary paths
            ("Secondary", "Base", PickTarget::Color) => "theme.extended_palette()    \n.secondary.base.color\n",
            ("Secondary", "Base", PickTarget::Text) => "theme.extended_palette()    \n.secondary.base.text\n",
            ("Secondary", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.secondary.weak.color\n",
            ("Secondary", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.secondary.weak.text\n",
            ("Secondary", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.secondary.strong.color\n",
            ("Secondary", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.secondary.strong.text\n",
            
            // Success paths
            ("Success", "Base", PickTarget::Color) => "theme.extended_palette()    \n.success.base.color\n",
            ("Success", "Base", PickTarget::Text) => "theme.extended_palette()    \n.success.base.text\n",
            ("Success", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.success.weak.color\n",
            ("Success", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.success.weak.text\n",
            ("Success", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.success.strong.color\n",
            ("Success", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.success.strong.text\n",
            
            // Warning paths
            ("Warning", "Base", PickTarget::Color) => "theme.extended_palette()    \n.warning.base.color\n",
            ("Warning", "Base", PickTarget::Text) => "theme.extended_palette()    \n.warning.base.text\n",
            ("Warning", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.warning.weak.color\n",
            ("Warning", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.warning.weak.text\n",
            ("Warning", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.warning.strong.color\n",
            ("Warning", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.warning.strong.text\n",
            
            // Danger paths
            ("Danger", "Base", PickTarget::Color) => "theme.extended_palette()    \n.danger.base.color\n",
            ("Danger", "Base", PickTarget::Text) => "theme.extended_palette()    \n.danger.base.text\n",
            ("Danger", "Weak", PickTarget::Color) => "theme.extended_palette()    \n.danger.weak.color\n",
            ("Danger", "Weak", PickTarget::Text) => "theme.extended_palette()    \n.danger.weak.text\n",
            ("Danger", "Strong", PickTarget::Color) => "theme.extended_palette()    \n.danger.strong.color\n",
            ("Danger", "Strong", PickTarget::Text) => "theme.extended_palette()    \n.danger.strong.text\n",
            
            _ => return None,
        };
        
        // If alpha is modified from 1.0, append scale_alpha
        if (self.alpha - 1.0).abs() > 0.001 {
            Some(format!("{}    .scale_alpha({:.3})", base, self.alpha))
        } else {
            Some(base.to_string())
        }
    }

    /// Generate compact theme path code (single line, no extra whitespace)
    fn palette_to_code_compact(&self) -> Option<String> {
        self.palette_to_code().map(|code| {
            // Remove extra spaces and newlines
            code.replace("    ", "").replace("\n", "")
        })
    }

    fn current_color(&self) -> Color {
        Color::from_rgba(self.red, self.green, self.blue, self.alpha)
    }
}

struct ModernColorPickerOverlay<'a, Message> {
    overlay_state: &'a mut OverlayState,
    is_open: &'a mut bool,
    color: &'a mut Color,
    on_change: &'a Option<Box<dyn Fn(Color) -> Message + 'a>>,
    on_change_with_source: &'a Option<Box<dyn Fn(Color, Option<String>) -> Message + 'a>>,
    position: &'a mut Point,
    title: String,
    viewport_size: Size,
}

impl<'a, Message> ModernColorPickerOverlay<'a, Message> 
where
    Message: Clone
{
    fn overlay(self) -> overlay::Element<'a, Message, iced::Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    fn publish_color_change(&self, color: Color, shell: &mut Shell<'_, Message>) {
        if let Some(callback) = self.on_change_with_source {
            let source = self.overlay_state.palette_to_code_compact();
            shell.publish(callback(color, source));
        } else if let Some(callback) = self.on_change {
            shell.publish(callback(color));
        }
    }
}

impl<'a, Message: Clone> Overlay<Message, iced::Theme, Renderer> for ModernColorPickerOverlay<'a, Message> {
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> Node {
        let size = Size::new(320.0, 440.0);
        let node = Node::new(size);
        
        node.move_to(*self.position)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        let header_bounds = header_rect(bounds);
        let close_bounds = close_button_rect(bounds);
        let content_bounds = content_rect(bounds);
        
        // Draw background with shadow
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    color: theme.extended_palette().background.weak.color,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                },
                snap: true,
            },
            theme.extended_palette().background.base.color,
        );

        // Draw header background
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: header_bounds.x,
                    y: header_bounds.y,
                    width: header_bounds.width,
                    height: header_bounds.height,
                },
                border: Border {
                    radius: iced::border::Radius {
                        top_left: 12.0, 
                        top_right: 12.0, 
                        bottom_right: 0.0, 
                        bottom_left: 0.0
                    },
                    ..Default::default()
                },
                shadow: Shadow::default(),
                snap: true,
            },
            theme.extended_palette().background.neutral.color,
        );        

        // Shadow under header with no bleed to left / right
        for i in 0..4 {
            let alpha = (1.0 - (i as f32 / 4.0)) * 0.15; // Fade out
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + 1.0, // Inset by border width
                        y: header_bounds.y + header_bounds.height + i as f32,
                        width: bounds.width - 2.0, // Account for borders
                        height: 1.0,
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                Color::from_rgba(0.0, 0.0, 0.0, alpha),
            );
        }

        // Draw Title
        renderer.fill_text(
            iced::advanced::Text {
                content: self.title.clone(),
                bounds: Size::new(header_bounds.width, header_bounds.height),
                size: iced::Pixels(18.0),
                font: iced::Font::default(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: iced::advanced::text::LineHeight::default(),
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::widget::text::Wrapping::default(),
            },
            Point::new(header_bounds.center_x(), header_bounds.center_y()),
            theme.extended_palette().background.weak.text,
            header_bounds,
        );

        if cursor.is_over(close_bounds) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: close_bounds,
                    border: Border {
                        radius: 15.0.into(),
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
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: iced::advanced::text::LineHeight::default(),
                shaping: iced::advanced::text::Shaping::Basic,
                wrapping: iced::widget::text::Wrapping::default(),
            },
            Point::new(close_bounds.center_x(), close_bounds.center_y()),
            style.text_color,
            close_bounds,
        );

        let tabs = [
            (ColorPickerTab::Grid, "Grid"),
            (ColorPickerTab::Spectrum, "Spectrum"),
            (ColorPickerTab::Sliders, "Sliders"),
            (ColorPickerTab::Palette, "Palette"),
        ];

        let rects = tab_rects(bounds, tabs.len());

        for ((tab, label), tab_bounds) in tabs.iter().zip(rects.iter()) {
            let is_active = self.overlay_state.active_tab == *tab;
            let is_hovered = cursor.is_over(*tab_bounds);
            renderer.fill_quad(
                renderer::Quad {
                    bounds: *tab_bounds,
                    border: Border { width: 1.0, radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                },
                if is_active { theme.extended_palette().primary.base.color }
                else if is_hovered { theme.extended_palette().background.weak.color }
                else { Color::TRANSPARENT },
            );
            renderer.fill_text(
                iced::advanced::Text {
                    content: label.to_string(),
                    bounds: Size::new(tab_bounds.width, tab_bounds.height),
                    size: iced::Pixels(13.0),
                    font: iced::Font::default(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(tab_bounds.center_x(), tab_bounds.center_y()),
                if is_active { Color::WHITE } else { style.text_color },
                *tab_bounds,
            );
        }

        match self.overlay_state.active_tab {
            ColorPickerTab::Grid => self.draw_grid_tab(renderer, theme, content_bounds, cursor),
            ColorPickerTab::Spectrum => self.draw_spectrum_tab(renderer, theme, content_bounds, cursor),
            ColorPickerTab::Sliders => self.draw_sliders_tab(renderer, theme, style, content_bounds),
            ColorPickerTab::Palette => self.draw_palette_tab(renderer, theme, content_bounds, cursor),
        }

        if self.overlay_state.active_tab != ColorPickerTab::Palette {

            // Preset colors
            let preset_y = bounds.y + 355.0;
            let preset_size = 30.0;
            let preset_spacing = 8.0;
            let preset_per_row = ((bounds.width - 40.0) / (preset_size + preset_spacing)) as usize;

            for (i, color) in self.overlay_state.preset_colors.iter().enumerate() {
                let row = i / preset_per_row;
                let col = i % preset_per_row;

                let preset_x = bounds.x + 20.0 + (preset_size + preset_spacing) * col as f32;
                let preset_y = preset_y + (preset_size + preset_spacing) * row as f32;

                let preset_bounds = Rectangle {
                    x: preset_x,
                    y: preset_y,
                    width: preset_size,
                    height: preset_size,
                };

                let is_hovered = cursor.is_over(preset_bounds);

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: preset_bounds,
                        border: Border {
                            color: if is_hovered {
                                theme.palette().primary
                            } else {
                                Color::from_rgba(0.5, 0.5, 0.5, 0.9)
                            },
                            width: if is_hovered { 2.0 } else { 1.0 },
                            radius: 15.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    *color,
                );
            }

            // Add button (+)
            let last_preset_idx = self.overlay_state.preset_colors.len();
            let add_row = last_preset_idx / preset_per_row;
            let add_col = last_preset_idx % preset_per_row;

            
            if add_row < 2 && add_col < preset_per_row {
                let add_preset_bounds = Rectangle {
                    x: bounds.x + 20.0 + (preset_size + preset_spacing) * add_col as f32,
                    y: preset_y + (preset_size + preset_spacing) * add_row as f32,
                    width: preset_size,
                    height: preset_size,
                };

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: add_preset_bounds,
                        border: Border {
                            color: Color::from_rgba(0.0, 0.0, 0.0, 0.2),
                            width: 1.0,
                            radius: 20.0.into(),
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    theme.extended_palette().background.weak.color,
                );

                renderer.fill_text(
                    iced::advanced::Text {
                        content: "+".to_string(),
                        bounds: Size::new(add_preset_bounds.width, add_preset_bounds.height),
                        size: iced::Pixels(24.0),
                        font: iced::Font::default(),
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Basic,
                        wrapping: iced::widget::text::Wrapping::default(),
                    },
                    Point::new(add_preset_bounds.center_x(), add_preset_bounds.center_y()),
                    style.text_color,
                    add_preset_bounds,
                );

            }
        }
        
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();
        let header_bounds = header_rect(bounds);
        let close_bounds = close_button_rect(bounds);
        let content_bounds = content_rect(bounds);

        // Clear "Copied" flag
        if let Some(t) = self.overlay_state.copied_at
            && t.elapsed() > Duration::from_millis(1200) {
                self.overlay_state.copied_at = None;
            }

        // Palette tab specific clicks
        let palette_bounds = Rectangle {
            x: content_bounds.x,
            y: content_bounds.y,  
            width: content_bounds.width,
            height: content_bounds.height + 78.0,
        };
        
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {

                if cursor.is_over(header_bounds) && !cursor.is_over(close_bounds) && !self.overlay_state.is_dragging
                    && !self.overlay_state.spectrum_dragging && 
                        !self.overlay_state.hue_dragging && 
                        self.overlay_state.dragging_slider.is_none()
                            && let Some(position) = cursor.position() {
                                self.overlay_state.is_dragging = true;
                                self.overlay_state.drag_offset = Vector::new(
                                    position.x - bounds.x,
                                    position.y - bounds.y,
                                );
                                return;
                            }

                if cursor.is_over(close_bounds) {
                    *self.is_open = false;
                    shell.request_redraw();
                    shell.invalidate_layout();
                    shell.invalidate_widgets();
                    shell.capture_event();
                    return;
                }

                let tabs_only = [ColorPickerTab::Grid, ColorPickerTab::Spectrum, ColorPickerTab::Sliders, ColorPickerTab::Palette];
                let rects = tab_rects(bounds, tabs_only.len());
                for (tab, r) in tabs_only.iter().zip(rects.iter()) {
                    if cursor.is_over(*r) {
                        self.overlay_state.active_tab = *tab;
                        if *tab == ColorPickerTab::Palette {
                            self.overlay_state.palette_cache_dirty.set(true);
                        }
                        shell.invalidate_layout();
                        shell.invalidate_widgets();
                        shell.capture_event();
                        return;
                    }
                }

                if self.overlay_state.active_tab != ColorPickerTab::Palette {
                    // Check preset colors
                    let preset_y = bounds.y + 355.0;
                    let preset_size = 30.0;
                    let preset_spacing = 8.0;
                    let presets_per_row = ((bounds.width - 40.0) / (preset_size + preset_spacing)) as usize;

                    for (i, color) in self.overlay_state.preset_colors.clone().iter().enumerate() {
                        let row = i / presets_per_row;
                        let col = i % presets_per_row;
                        
                        if row >= 2 {
                            continue;
                        }
                        
                        let preset_x = bounds.x + 20.0 + (preset_size + preset_spacing) * col as f32;
                        let preset_y = preset_y + (preset_size + preset_spacing) * row as f32;
                        
                        let preset_bounds = Rectangle {
                            x: preset_x,
                            y: preset_y,
                            width: preset_size,
                            height: preset_size,
                        };

                        if cursor.is_over(preset_bounds) {

                            self.overlay_state.red = color.r;
                            self.overlay_state.green = color.g;
                            self.overlay_state.blue = color.b;
                            self.overlay_state.alpha = color.a;
                            self.overlay_state.update_from_rgb();

                            *self.color = *color;
                            self.publish_color_change(*color, shell);
                            shell.invalidate_layout();
                            shell.invalidate_widgets();
                            shell.capture_event();
                            return;
                        }
                    }

                    // Check add preset button
                    let last_preset_idx = self.overlay_state.preset_colors.len();
                    let add_row = last_preset_idx / presets_per_row;
                    let add_col = last_preset_idx % presets_per_row;

                    if add_row < 2 {  // Only check if we haven't exceeded 2 rows
                        let add_preset_bounds = Rectangle {
                            x: bounds.x + 20.0 + (preset_size + preset_spacing) * add_col as f32,
                            y: preset_y + (preset_size + preset_spacing) * add_row as f32,
                            width: preset_size,
                            height: preset_size,
                        };

                        if cursor.is_over(add_preset_bounds) {
                            let current_color = self.overlay_state.current_color();
                            if !self.overlay_state.preset_colors.contains(&current_color) {
                                self.overlay_state.preset_colors.push(current_color);
                                shell.invalidate_layout();
                                shell.invalidate_widgets();
                                shell.capture_event();
                            }
                            return;
                        }
                    }
                }

                match self.overlay_state.active_tab {
                    ColorPickerTab::Grid => {
                        self.handle_grid_click(content_bounds, cursor, shell);
                    }
                    ColorPickerTab::Spectrum => {
                        self.handle_spectrum_click(content_bounds, cursor, shell);
                    }
                    ColorPickerTab::Sliders => {
                        self.handle_slider_click(content_bounds, cursor, clipboard, shell, ColorString::Hex);
                    }
                    ColorPickerTab::Palette => {
                        self.overlay_state.palette_cache_dirty.set(true);
                        self.handle_palette_click(palette_bounds, cursor, shell, PickTarget::Color);
                    }
                }
                shell.invalidate_layout();
                shell.invalidate_widgets();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                self.overlay_state.is_dragging = false;
                self.overlay_state.spectrum_dragging = false;
                self.overlay_state.hue_dragging = false;
                self.overlay_state.dragging_slider = None;
                shell.invalidate_layout();
                shell.invalidate_widgets();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                match self.overlay_state.active_tab {
                    ColorPickerTab::Sliders => {
                        self.handle_slider_click(content_bounds, cursor, clipboard, shell, ColorString::Rgb);
                    }
                    ColorPickerTab::Palette => {
                        self.overlay_state.palette_cache_dirty.set(true);
                        self.handle_palette_click(palette_bounds, cursor, shell, PickTarget::Text);
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if self.overlay_state.is_dragging {
                    if let Some(position) = cursor.position() {
                        let new_x = position.x - self.overlay_state.drag_offset.x;
                        let new_y = position.y - self.overlay_state.drag_offset.y;
                        
                        // Keep within viewport bounds
                        self.position.x = new_x.max(0.0).min(self.viewport_size.width - bounds.width);
                        self.position.y = new_y.max(0.0).min(self.viewport_size.height - bounds.height);
                        
                        shell.invalidate_layout();
                        shell.invalidate_widgets();
                        shell.capture_event();
                    }
                } else if self.overlay_state.spectrum_dragging || self.overlay_state.hue_dragging {
                    self.handle_spectrum_drag(content_bounds, cursor, shell);
                    shell.invalidate_layout();
                    shell.invalidate_widgets();
                    shell.capture_event();
                } else if self.overlay_state.dragging_slider.is_some() {
                    self.handle_slider_drag(content_bounds, cursor, shell);
                    shell.invalidate_layout();
                    shell.invalidate_widgets();
                    shell.capture_event();
                }

                shell.invalidate_layout();
                shell.invalidate_widgets();
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed { 
                key: keyboard::Key::Named(keyboard::key::Named::Escape), 
                .. 
            }) => {
                *self.is_open = false;
                shell.request_redraw();
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
        let header_bounds = header_rect(bounds);
        let close_bounds = close_button_rect(bounds);
        let content_bounds = content_rect(bounds);

        if cursor.is_over(close_bounds) {
            return mouse::Interaction::Pointer;
        }

        // Grab interaction while dragging spectrum tab elements
        if self.overlay_state.spectrum_dragging || self.overlay_state.hue_dragging {
            return mouse::Interaction::Grabbing;
        }

        if cursor.is_over(header_bounds) {
            return mouse::Interaction::Grab;
        }
        
        mouse::Interaction::None
            
    }
}

impl<'a, Message: Clone> ModernColorPickerOverlay<'a, Message> {
        fn draw_grid_tab(
        &self,
        renderer: &mut Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        let cell_size = bounds.width / 12.0;
        let rows = 8;
        let cols = 12;

        for row in 0..rows {
            for col in 0..cols {
                let x = bounds.x + col as f32 * cell_size;
                let y = bounds.y + row as f32 * cell_size;
                
                let hue = (col as f32 / cols as f32) * 360.0;
                let saturation = 1.0 - (row as f32 / rows as f32) * 0.7;
                let value = 1.0 - (row as f32 / rows as f32) * 0.5;
                
                let color = hsv_to_rgb(hue, saturation, value);
                
                let cell_bounds = Rectangle {
                    x,
                    y,
                    width: cell_size - 1.0,
                    height: cell_size - 1.0,
                };

                let is_hovered = cursor.is_over(cell_bounds);

                renderer.fill_quad(
                    renderer::Quad {
                        bounds: cell_bounds,
                        border: if is_hovered {
                            Border {
                                color: Color::WHITE,
                                width: 2.0,
                                radius: 0.0.into(),
                            }
                        } else {
                            Border::default()
                        },
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    color,
                );
            }
        }

        // Add grayscale row at the bottom
        let gray_y = bounds.y + rows as f32 * cell_size + 10.0;
        for col in 0..cols {
            let x = bounds.x + col as f32 * cell_size;
            let gray_value = col as f32 / (cols - 1) as f32;
            let color = Color::from_rgb(gray_value, gray_value, gray_value);
            
            let cell_bounds = Rectangle {
                x,
                y: gray_y,
                width: cell_size - 1.0,
                height: cell_size - 1.0,
            };

            let is_hovered = cursor.is_over(cell_bounds);

            renderer.fill_quad(
                renderer::Quad {
                    bounds: cell_bounds,
                    border: if is_hovered {
                        Border {
                            color: if gray_value > 0.5 { Color::BLACK } else { Color::WHITE },
                            width: 2.0,
                            radius: 0.0.into(),
                        }
                    } else {
                        Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                color,
            );
        }
    }

    fn draw_spectrum_tab(
        &self,
        renderer: &mut Renderer,
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) {
        // Draw HSV spectrum
        let spectrum_height = bounds.height - 30.0;
        let spectrum_size = bounds.width.min(spectrum_height);

        let spectrum_bounds = Rectangle {
            x: bounds.x + (bounds.width - spectrum_size) / 2.0,
            y: bounds.y,
            width: spectrum_size,
            height: spectrum_size,
        };

        // Draw saturation/value gradient
        for y in 0..spectrum_size as u32 {
            for x in 0..spectrum_size as u32 {
                let saturation = x as f32 / spectrum_size;
                let value = 1.0 - (y as f32 / spectrum_size);
                let color = hsv_to_rgb(self.overlay_state.hue, saturation, value);
                
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: spectrum_bounds.x + x as f32,
                            y: spectrum_bounds.y + y as f32,
                            width: 1.0,
                            height: 1.0,
                        },
                        border: Border::default(),
                        shadow: Shadow::default(),
                        snap: true,
                    },
                    color,
                );
            }
        }

        // Draw selection indicator
        let indicator_x = spectrum_bounds.x + self.overlay_state.saturation * spectrum_size;
        let indicator_y = spectrum_bounds.y + (1.0 - self.overlay_state.value) * spectrum_size;
        
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: indicator_x - 8.0,
                    y: indicator_y - 8.0,
                    width: 16.0,
                    height: 16.0,
                },
                border: Border {
                    color: Color::WHITE,
                    width: 2.0,
                    radius: 8.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            Color::TRANSPARENT,
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: indicator_x - 6.0,
                    y: indicator_y - 6.0,
                    width: 12.0,
                    height: 12.0,
                },
                border: Border {
                    color: Color::BLACK,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            Color::TRANSPARENT,
        );

        // Draw hue slider
        let hue_y = spectrum_bounds.y + spectrum_bounds.height + 10.0;
        let hue_bounds = Rectangle {
            x: spectrum_bounds.x,
            y: hue_y,
            width: spectrum_bounds.width,
            height: 20.0,
        };

        // Draw hue gradient
        for x in 0..spectrum_bounds.width as u32 {
            let hue = (x as f32 / spectrum_bounds.width) * 360.0;
            let color = hsv_to_rgb(hue, 1.0, 1.0);
            
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: hue_bounds.x + x as f32,
                        y: hue_bounds.y,
                        width: 1.0,
                        height: hue_bounds.height,
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                color,
            );
        }

        // Draw hue indicator
        let hue_indicator_x = hue_bounds.x + (self.overlay_state.hue / 360.0) * hue_bounds.width;
        
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: hue_indicator_x - 2.0,
                    y: hue_bounds.y - 2.0,
                    width: 4.0,
                    height: hue_bounds.height + 4.0,
                },
                border: Border {
                    color: Color::WHITE,
                    width: 2.0,
                    radius: 2.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            Color::BLACK,
        );
    }

    fn draw_sliders_tab(
        &self,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        bounds: Rectangle,
    ) {
        let slider_height = 30.0;
        let spacing = 35.0;
        let label_width = 60.0;
        let value_width = 40.0;
        let slider_width = bounds.width - label_width - value_width - 20.0;

        // RGB sliders
        let sliders = [
            ("RED", self.overlay_state.red, Color::from_rgb(1.0, 0.0, 0.0)),
            ("GREEN", self.overlay_state.green, Color::from_rgb(0.0, 1.0, 0.0)),
            ("BLUE", self.overlay_state.blue, Color::from_rgb(0.0, 0.0, 1.0)),
            ("ALPHA", self.overlay_state.alpha, Color::from_rgba(1.0, 1.0, 1.0, 0.5))
        ];

        for (i, (label, value, color)) in sliders.iter().enumerate() {
            let y = bounds.y + i as f32 * spacing;

            // Label
            renderer.fill_text(
                iced::advanced::Text {
                    content: label.to_string(),
                    bounds: Size::new(label_width, slider_height),
                    size: iced::Pixels(12.0),
                    font: iced::Font::default(),
                    align_x: text::Alignment::Left,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(bounds.x, y + slider_height / 2.0),
                style.text_color,
                Rectangle {
                    x: bounds.x,
                    y,
                    width: label_width,
                    height: slider_height,
                },
            );

            // Slider track
            let track_bounds = Rectangle {
                x: bounds.x + label_width,
                y: y + slider_height / 2.0 - 2.0,
                width: slider_width,
                height: 4.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: track_bounds,
                    border: Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                theme.extended_palette().background.weak.color,
            );

            // Slider fill
            let fill_bounds = Rectangle {
                x: track_bounds.x,
                y: track_bounds.y,
                width: track_bounds.width * value,
                height: track_bounds.height,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: fill_bounds,
                    border: Border {
                        radius: 2.0.into(),
                        ..Default::default()
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                *color,
            );

            // Slider handle
            let handle_x = track_bounds.x + track_bounds.width * value;
            let handle_bounds = Rectangle {
                x: handle_x - 8.0,
                y: y + slider_height / 2.0 - 8.0,
                width: 16.0,
                height: 16.0,
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: handle_bounds,
                    border: Border {
                        color: theme.extended_palette().background.weak.color,
                        width: 2.0,
                        radius: 8.0.into(),
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                Color::WHITE,
            );

            // Value text
            let value_text = format!("{}", (*value * 255.0).round() as u8);
            renderer.fill_text(
                iced::advanced::Text {
                    content: value_text,
                    bounds: Size::new(value_width, slider_height),
                    size: iced::Pixels(12.0),
                    font: iced::Font::default(),
                    align_x: text::Alignment::Right,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(bounds.x + bounds.width - value_width / 2.0, y + slider_height / 2.0),
                style.text_color,
                Rectangle {
                    x: bounds.x + bounds.width - value_width,
                    y,
                    width: value_width,
                    height: slider_height,
                },
            );
        }

        let chip_w = bounds.width * 0.80;
        let chip_h = 56.0;
        let chip_x = bounds.x + (bounds.width - chip_w) / 2.0;
        let chip_y = bounds.y + 4.0 * spacing + 8.0;

        let chip_bounds = Rectangle { x: chip_x, y: chip_y, width: chip_w, height: chip_h };

        let chip_color = self.overlay_state.current_color();

        // Draw chip
        renderer.fill_quad(
            renderer::Quad {
                bounds: chip_bounds,
                border: Border {
                    color: theme.extended_palette().primary.base.color,
                    width: 0.0,
                    radius: 10.0.into(),
                },
                shadow: Shadow {
                    color: theme.extended_palette().background.strong.color,
                    offset: Vector::new(0.0, 0.0),
                    blur_radius: 20.0,
                },
                snap: true,
            },
            chip_color,
        ); 
        
        // pick contrasting text
        let lum = 0.299 * chip_color.r + 0.587 * chip_color.g + 0.114 * chip_color.b;
        let text_color = if lum > 0.5 { Color::BLACK } else { Color::WHITE };


        // Chip label: either hex or "Copied!"
        let show_copied = self.overlay_state.copied_at
            .map(|t| t.elapsed() < Duration::from_millis(1200))
            .unwrap_or(false);

        let mut chip_label_size = 18.0;
        let mut chip_label_y_position = chip_bounds.center_y() - 8.0;

        let (chip_label, small_label) = if show_copied {
            ("Copied!".to_string(), String::new())
        } else if let Some(palette_code) = self.overlay_state.palette_to_code() {
            // Show palette theme code when available
            chip_label_size = 12.0;
            chip_label_y_position = chip_bounds.center_y();
            (palette_code, String::new())

        } else {
            // Fall back to hex + rgb
            (
                self.overlay_state.hex_input.to_uppercase(),
                rgb_or_rgba_string(chip_color)
            )
        };

        // Hex / Copied label
        renderer.fill_text(
            iced::advanced::Text {
                content: chip_label,
                bounds: Size::new(chip_w, chip_h),
                size: iced::Pixels(chip_label_size),
                font: iced::Font::default(),
                align_x: iced::widget::text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: iced::advanced::text::LineHeight::default(),
                shaping: iced::advanced::text::Shaping::Advanced,
                wrapping: iced::widget::text::Wrapping::default(),
            },
            Point::new(chip_bounds.center_x(), chip_label_y_position),
            text_color,
            chip_bounds,
        );

        // Second, smaller line under it
        if !show_copied{
            renderer.fill_text(
                iced::advanced::Text {
                    content: small_label,
                    bounds: Size::new(chip_w, chip_h),
                    size: iced::Pixels(12.0),
                    font: iced::Font::default(),
                    align_x: iced::widget::text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(chip_bounds.center_x(), chip_bounds.center_y() + 10.0),
                text_color,
                chip_bounds,
            );
        }
    }

    fn draw_palette_tab(
        &self,
        renderer: &mut Renderer,
        theme: &iced::Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        // Refresh cache if needed
        if self.overlay_state.palette_cache.borrow().is_empty()
            || self.overlay_state.palette_cache_dirty.get()
        {
            let ep = theme.extended_palette();
            let bg = &ep.background;
            *self.overlay_state.palette_cache.borrow_mut() = build_palette_rows_compact(ep, bg);
            self.overlay_state.palette_cache_dirty.set(false);
        }

        // theme change guard
        let needs_update = {
            let cache = self.overlay_state.palette_cache.borrow();
            if let Some(first) = cache.first() {
                first.tones[0].1.color != theme.extended_palette().background.base.color
            } else {
                false
            }
        };
        
        if needs_update {
            let ep = theme.extended_palette();
            let bg = &ep.background;
            *self.overlay_state.palette_cache.borrow_mut() = build_palette_rows_compact(ep, bg);
            self.overlay_state.palette_cache_dirty.set(false);
        }

        let rows = self.overlay_state.palette_cache.borrow();
        let g = palette_geom_compact(bounds);
        let title_color = theme.extended_palette().background.weak.text;

        let mut y = bounds.y;
        let max_y = bounds.y + bounds.height;

        // Helper function to draw section title
        let draw_title = |renderer: &mut Renderer, y: f32, text: &str| {
            renderer.fill_text(
                iced::advanced::Text {
                    content: text.into(),
                    bounds: Size::new(bounds.width, g.label_h),
                    size: iced::Pixels(11.0),
                    font: iced::Font::default(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    line_height: iced::advanced::text::LineHeight::default(),
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::widget::text::Wrapping::default(),
                },
                Point::new(bounds.center_x(), y + g.label_h * 0.5),
                title_color,
                Rectangle { x: bounds.x, y, width: bounds.width, height: g.label_h },
            );
        };

        // Background section
        if y + g.label_h <= max_y {
            draw_title(renderer, y, "Background");
        }
        y += g.label_h + g.row_gap;

        let bg = rows.iter().find(|r| r.name == "Background").unwrap();

        // Background Row 1: Base and Neutral
        if y + g.pill_h <= max_y {
            let long_w = (bounds.width - g.col_gap) / 2.0;
            let mut x = bounds.x;
            for i in 0..2 {
                let r = Rectangle { x, y, width: long_w, height: g.pill_h };
                draw_pill(renderer, r, bg.tones[i].1, cursor.is_over(r), theme);
                draw_pill_label(renderer, r, bg.tones[i].0, bg.tones[i].1.text);
                x += long_w + g.col_gap;
            }
        }
        y += g.pill_h + g.row_gap;

        // Background Row 2: Weak, Weaker, Weakest
        if y + g.pill_h <= max_y {
            let mut x = bounds.x;
            for i in 2..5 {
                let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                draw_pill(renderer, r, bg.tones[i].1, cursor.is_over(r), theme);
                draw_pill_label(renderer, r, bg.tones[i].0, bg.tones[i].1.text);
                x += g.eq_w3 + g.col_gap;
            }
        }
        y += g.pill_h + g.row_gap;

        // Background Row 3: Strong, Stronger, Strongest
        if y + g.pill_h <= max_y {
            let mut x = bounds.x;
            for i in 5..8 {
                let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                draw_pill(renderer, r, bg.tones[i].1, cursor.is_over(r), theme);
                draw_pill_label(renderer, r, bg.tones[i].0, bg.tones[i].1.text);
                x += g.eq_w3 + g.col_gap;
            }
        }
        y += g.pill_h + g.section_gap;

        // Color sections (Primary, Secondary, Success, Warning, Danger)
        let names = ["Primary", "Secondary", "Success", "Warning", "Danger"];
        for name in names.iter() {
    
            // Title
            draw_title(renderer, y, name);
            y += g.label_h + g.row_gap;

            // Pills
            if let Some(row) = rows.iter().find(|r| r.name == *name) {
                let mut x = bounds.x;
                for i in 0..3 {
                    let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                    draw_pill(renderer, r, row.tones[i].1, cursor.is_over(r), theme);
                    draw_pill_label(renderer, r, row.tones[i].0, row.tones[i].1.text);
                    x += g.eq_w3 + g.col_gap;
                }
            }
            y += g.pill_h;
            y += g.section_gap;
        }
    }

    fn handle_grid_click(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Some(position) = cursor.position_in(bounds) {
            let cell_size = bounds.width / 12.0;
            let col = (position.x / cell_size) as usize;
            let row = (position.y / cell_size) as usize;
            
            if row < 8 && col < 12 {
                self.overlay_state.palette_source = None;
                
                let hue = (col as f32 / 12.0) * 360.0;
                let saturation = 1.0 - (row as f32 / 8.0) * 0.7;
                let value = 1.0 - (row as f32 / 8.0) * 0.5;
                
                self.overlay_state.hue = hue;
                self.overlay_state.saturation = saturation;
                self.overlay_state.value = value;
                self.overlay_state.update_from_hsv();
                
                let color = self.overlay_state.current_color();
                *self.color = color;
                self.publish_color_change(color, shell);
            } else {
                let gray_y_start = 8.0 * cell_size + 10.0;
                let gray_col = ((position.x / cell_size) as usize).min(11);
                
                if position.y >= gray_y_start && position.y < gray_y_start + cell_size {
                    self.overlay_state.palette_source = None;
                    
                    let gray_value = gray_col as f32 / 11.0;
                    let color = Color::from_rgb(gray_value, gray_value, gray_value);
                    
                    self.overlay_state.red = gray_value;
                    self.overlay_state.green = gray_value;
                    self.overlay_state.blue = gray_value;
                    self.overlay_state.update_from_rgb();
                    
                    *self.color = color;
                    self.publish_color_change(color, shell);
                }
            }
        }
    }

    fn handle_spectrum_drag(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        let spectrum_height = bounds.height - 30.0;
        let spectrum_size = bounds.width.min(spectrum_height);

        let spectrum_bounds = Rectangle {
            x: bounds.x + (bounds.width - spectrum_size) / 2.0,
            y: bounds.y,
            width: spectrum_size,
            height: spectrum_size,
        };

        let hue_bounds = Rectangle {
            x: spectrum_bounds.x,
            y: spectrum_bounds.y + spectrum_bounds.height + 20.0,
            width: spectrum_bounds.width,
            height: 20.0,
        };

        if let Some(pos) = cursor.position() {
            if self.overlay_state.spectrum_dragging {
                self.overlay_state.palette_source = None;
                
                // Use global position, clamp into the rect
                let local_x = (pos.x - spectrum_bounds.x).clamp(0.0, spectrum_bounds.width);
                let local_y = (pos.y - spectrum_bounds.y).clamp(0.0, spectrum_bounds.height);

                self.overlay_state.saturation = local_x / spectrum_bounds.width;
                self.overlay_state.value = 1.0 - (local_y / spectrum_bounds.height);
                self.overlay_state.update_from_hsv();

                let color = self.overlay_state.current_color();
                *self.color = color;
                self.publish_color_change(color, shell);
                shell.request_redraw();
            } else if self.overlay_state.hue_dragging {
                self.overlay_state.palette_source = None;
                
                let local_x = (pos.x - hue_bounds.x).clamp(0.0, hue_bounds.width);
                self.overlay_state.hue = (local_x / hue_bounds.width) * 360.0;
                self.overlay_state.update_from_hsv();

                let color = self.overlay_state.current_color();
                *self.color = color;
                self.publish_color_change(color, shell);
                shell.request_redraw();
            }
        }
    }

    fn handle_spectrum_click(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        let spectrum_height = bounds.height - 30.0;
        let spectrum_size = bounds.width.min(spectrum_height);
        let spectrum_bounds = Rectangle {
            x: bounds.x + (bounds.width - spectrum_size) / 2.0,
            y: bounds.y,
            width: spectrum_size,
            height: spectrum_size,
        };

        let hue_bounds = Rectangle {
            x: spectrum_bounds.x,
            y: spectrum_bounds.y + spectrum_bounds.height + 10.0,
            width: spectrum_bounds.width,
            height: 20.0,
        };

        if cursor.is_over(spectrum_bounds) {
            self.overlay_state.spectrum_dragging = true;
        } else if cursor.is_over(hue_bounds) {
            self.overlay_state.hue_dragging = true;
        } else {
            return;
        }

        self.handle_spectrum_drag(bounds, cursor, shell);
    }

    fn handle_slider_click(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        copy_string: ColorString
    ) {
        // shared
        let spacing = 35.0;

        // slider
        let slider_height = 30.0;
        let label_width = 60.0;
        let value_width = 40.0;
        let slider_width = bounds.width - label_width - value_width - 20.0;

        let chip_w = bounds.width * 0.80;
        let chip_h = 56.0;
        let chip_x = bounds.x + (bounds.width - chip_w) / 2.0;
        let chip_y = bounds.y + 4.0 * spacing + 8.0;

        let chip_bounds = Rectangle { x: chip_x, y: chip_y, width: chip_w, height: chip_h };

        if cursor.is_over(chip_bounds) {
            // Priority: palette code > hex > rgb
            if let Some(palette_code) = self.overlay_state.palette_to_code_compact() {
                clipboard.write(iced::advanced::clipboard::Kind::Standard, palette_code);
            } else if copy_string != ColorString::Rgb {
                clipboard.write(iced::advanced::clipboard::Kind::Standard, self.overlay_state.hex_input.clone());
            } else {
                let rgb = rgb_or_rgba_string(self.overlay_state.current_color());
                clipboard.write(iced::advanced::clipboard::Kind::Standard, rgb);
            }
            

            // flash "Copied!"
            self.overlay_state.copied_at = Some(Instant::now());
            shell.request_redraw();
            shell.capture_event();
            return;
        }

        for i in 0..4 {
            let y = bounds.y + i as f32 * spacing;
            let track_bounds = Rectangle {
                x: bounds.x + label_width,
                y,
                width: slider_width,
                height: slider_height,
            };

            if cursor.is_over(track_bounds) {
                self.overlay_state.dragging_slider = Some(match i {
                    0 => SliderType::Red,
                    1 => SliderType::Green,
                    2 => SliderType::Blue,
                    3 => SliderType::Alpha,
                    _ => unreachable!(),
                });
                self.handle_slider_drag(bounds, cursor, shell);
                break;
            }
        }
    }

    fn handle_slider_drag(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Some(slider_type) = self.overlay_state.dragging_slider {
            let slider_height = 30.0;
            let spacing = 35.0;
            let label_width = 60.0;
            let value_width = 40.0;
            let slider_width = bounds.width - label_width - value_width - 20.0;

            let slider_index = match slider_type {
                SliderType::Red => 0,
                SliderType::Green => 1,
                SliderType::Blue => 2,
                SliderType::Alpha => 3,
            };

            let y = bounds.y + slider_index as f32 * spacing;
            let track_bounds = Rectangle {
                x: bounds.x + label_width,
                y,
                width: slider_width,
                height: slider_height,
            };

            if let Some(pos) = cursor.position() {
                let local_x = (pos.x - track_bounds.x).clamp(0.0, track_bounds.width);
                let value = (local_x / track_bounds.width).clamp(0.0, 1.0);
                
                match slider_type {
                    SliderType::Red => {
                        self.overlay_state.palette_source = None;
                        self.overlay_state.red = value;
                    },
                    SliderType::Green => {
                        self.overlay_state.palette_source = None;
                        self.overlay_state.green = value;
                    },
                    SliderType::Blue => {
                        self.overlay_state.palette_source = None;
                        self.overlay_state.blue = value;
                    },
                    SliderType::Alpha => {
                        self.overlay_state.alpha = value;
                    },
                }
                
                self.overlay_state.update_from_rgb();
                let color = self.overlay_state.current_color();
                *self.color = color;
                self.publish_color_change(color, shell);
                shell.request_redraw();
            }
        }
    }

    fn handle_palette_click(
        &mut self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
        shell: &mut Shell<'_, Message>,
        target: PickTarget,
    ) {
        if !cursor.is_over(bounds) { return; }
        let Some(_) = cursor.position() else { return; };

        let picked: Option<(Color, &'static str, &'static str)> = {
            let rows = self.overlay_state.palette_cache.borrow();
            let g = palette_geom_compact(bounds);

            let choose = |tone: Tone| -> Color {
                match target { 
                    PickTarget::Color => tone.color, 
                    PickTarget::Text => tone.text 
                }
            };

            let mut y = bounds.y;
            let max_y = bounds.y + bounds.height;

            'scan: {
                // Background title
                y += g.label_h + g.row_gap;
                
                if let Some(bg) = rows.iter().find(|r| r.name == "Background") {
                    // Row 1
                    if y + g.pill_h <= max_y {
                        let long_w = (bounds.width - g.col_gap) / 2.0;
                        let mut x = bounds.x;
                        for i in 0..2 {
                            let r = Rectangle { x, y, width: long_w, height: g.pill_h };
                            if cursor.is_over(r) { 
                                break 'scan Some((choose(bg.tones[i].1), "Background", bg.tones[i].0)); 
                            }
                            x += long_w + g.col_gap;
                        }
                    }
                    y += g.pill_h + g.row_gap;

                    // Row 2
                    if y + g.pill_h <= max_y {
                        let mut x = bounds.x;
                        for i in 2..5 {
                            let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                            if cursor.is_over(r) { 
                                break 'scan Some((choose(bg.tones[i].1), "Background", bg.tones[i].0)); 
                            }
                            x += g.eq_w3 + g.col_gap;
                        }
                    }
                    y += g.pill_h + g.row_gap;

                    // Row 3
                    if y + g.pill_h <= max_y {
                        let mut x = bounds.x;
                        for i in 5..8 {
                            let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                            if cursor.is_over(r) { 
                                break 'scan Some((choose(bg.tones[i].1), "Background", bg.tones[i].0)); 
                            }
                            x += g.eq_w3 + g.col_gap;
                        }
                    }
                    y += g.pill_h + g.section_gap;
                }

                // Color sections
                let names = ["Primary", "Secondary", "Success", "Warning", "Danger"];
                for name in names.iter() {

                    
                    // Title row
                    y += g.label_h + g.row_gap;

                    if let Some(row) = rows.iter().find(|r| r.name == *name) {
                        let mut x = bounds.x;
                        for i in 0..3 {
                            let r = Rectangle { x, y, width: g.eq_w3, height: g.pill_h };
                            if cursor.is_over(r) {
                                break 'scan Some((choose(row.tones[i].1), *name, row.tones[i].0));
                            }
                            x += g.eq_w3 + g.col_gap;
                        }
                    }
                    y += g.pill_h + g.section_gap;
                }

                None
            }
        };

        if let Some((color, row, tone)) = picked {
            self.overlay_state.palette_source = Some(PaletteSource {
                row,
                tone,
                pick_target: target,
            });

            self.overlay_state.red = color.r;
            self.overlay_state.green = color.g;
            self.overlay_state.blue = color.b;
            self.overlay_state.alpha = color.a;
            self.overlay_state.update_from_rgb();

            *self.color = color;
            self.publish_color_change(color, shell);
            shell.capture_event();
        }
    }

}

// Helper functions
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    
    Color::from_rgb(r + m, g + m, b + m)
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color.r;
    let g = color.g;
    let b = color.b;
    
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    
    let h = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };
    
    let h = if h < 0.0 { h + 360.0 } else { h };
    
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let v = max;
    
    (h, s, v)
}

fn color_to_hex(color: Color) -> String {
    if color.a < 1.0 {
        format!("#{:02X}{:02X}{:02X}{:02X}", 
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8,
            (color.a * 255.0) as u8
        )
    } else {
        format!("#{:02X}{:02X}{:02X}", 
            (color.r * 255.0) as u8,
            (color.g * 255.0) as u8,
            (color.b * 255.0) as u8
        )
    }
}

fn rgb_or_rgba_string(c: Color) -> String {
    let r = (c.r * 255.0).round() as u8;
    let g = (c.g * 255.0).round() as u8;
    let b = (c.b * 255.0).round() as u8;
    let a8 = (c.a * 255.0).round() as u8;

    if a8 == 255 {
        format!("{r}, {g}, {b}")
    } else {
        // CSS-like rgba with alpha 0..1 (trim trailing zeros)
        let mut a = a8 as f32 / 255.0;
        // clamp minor fp noise
        if (a - 1.0).abs() < 1e-4 { a = 1.0; }
        if (a - 0.0).abs() < 1e-4 { a = 0.0; }
        let s = format!("{a:.3}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        format!("{r:1}, {g:1}, {b:1}, {s}")
    }
}

impl<'a, Message> std::fmt::Debug for ModernColorPickerOverlay<'a, Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModernColorPickerOverlay")
            .field("position", &self.position)
            .field("overlay_state", &self.overlay_state)
            .finish()
    }
}

fn build_palette_rows_compact(
    ep: &iced::theme::palette::Extended,
    bg: &iced::theme::palette::Background,
) -> Vec<PaletteRow> {
    // helpers
    let t = |c: iced::Color, tx: iced::Color| Tone { color: c, text: tx };
    vec![
        // Background: 3 visual rows (2 long + 3 + 3)
        PaletteRow {
            name: "Background",
            tones: vec![
                ("Base",     t(bg.base.color,     bg.base.text)),
                ("Neutral",  t(bg.neutral.color,  bg.neutral.text)),
                ("Weak",     t(bg.weak.color,     bg.weak.text)),
                ("Weaker",   t(bg.weaker.color,   bg.weaker.text)),
                ("Weakest",  t(bg.weakest.color,  bg.weakest.text)),
                ("Strong",   t(bg.strong.color,   bg.strong.text)),
                ("Stronger", t(bg.stronger.color, bg.stronger.text)),
                ("Strongest",t(bg.strongest.color,bg.strongest.text)),
            ],
        },
        // 3-tone rows
        PaletteRow { name: "Primary",   tones: vec![
            ("Base", t(ep.primary.base.color,   ep.primary.base.text)),
            ("Weak", t(ep.primary.weak.color,   ep.primary.weak.text)),
            ("Strong",t(ep.primary.strong.color,ep.primary.strong.text)),
        ]},
        PaletteRow { name: "Secondary", tones: vec![
            ("Base", t(ep.secondary.base.color, ep.secondary.base.text)),
            ("Weak", t(ep.secondary.weak.color, ep.secondary.weak.text)),
            ("Strong",t(ep.secondary.strong.color,ep.secondary.strong.text)),
        ]},
        PaletteRow { name: "Success",   tones: vec![
            ("Base", t(ep.success.base.color,   ep.success.base.text)),
            ("Weak", t(ep.success.weak.color,   ep.success.weak.text)),
            ("Strong",t(ep.success.strong.color,ep.success.strong.text)),
        ]},
        PaletteRow { name: "Warning",   tones: vec![
            ("Base", t(ep.warning.base.color,   ep.warning.base.text)),
            ("Weak", t(ep.warning.weak.color,   ep.warning.weak.text)),
            ("Strong",t(ep.warning.strong.color,ep.warning.strong.text)),
        ]},
        PaletteRow { name: "Danger",    tones: vec![
            ("Base", t(ep.danger.base.color,    ep.danger.base.text)),
            ("Weak", t(ep.danger.weak.color,    ep.danger.weak.text)),
            ("Strong",t(ep.danger.strong.color, ep.danger.strong.text)),
        ]},
    ]
}

fn draw_pill(renderer: &mut Renderer, r: Rectangle, tone: Tone, hovered: bool, theme: &iced::Theme) {
    renderer.fill_quad(
        renderer::Quad {
            bounds: r,
            border: Border {
                color: if hovered { theme.palette().primary }
                       else { Color::from_rgba(0.0,0.0,0.0,0.25) },
                width: if hovered { 2.0 } else { 1.0 },
                radius: 8.0.into(),
            },
            ..Default::default()
        },
        tone.color,
    );
}

fn draw_pill_label(renderer: &mut Renderer, r: Rectangle, text: &str, color: Color) {
    let font_px = (r.height * 0.5).clamp(9.0, 12.0); // scale with pill height
    renderer.fill_text(
        iced::advanced::Text {
            content: text.into(),
            bounds: Size::new(r.width, r.height),
            size: iced::Pixels(font_px),
            font: iced::Font::default(),
            align_x: iced::widget::text::Alignment::Center,
            align_y: Vertical::Center,
            line_height: iced::advanced::text::LineHeight::default(),
            shaping: iced::advanced::text::Shaping::Basic,
            wrapping: iced::widget::text::Wrapping::default(),
        },
        Point::new(r.center_x(), r.center_y()),
        color,
        r,
    );
}

#[inline]
fn tab_rects(bounds: Rectangle, n: usize) -> Vec<Rectangle> {
    let tab_y = bounds.y + HEADER_HEIGHT + TAB_SPACING;
    let left = bounds.x + CONTENT_PADDING;
    let right = bounds.x + bounds.width - CONTENT_PADDING;

    let total_w = right - left;
    let w = (total_w - TAB_SPACING * (n as f32 - 1.0)) / n as f32;
    (0..n).map(|i| Rectangle {
        x: left + i as f32 * (w + TAB_SPACING),
        y: tab_y,
        width: w,
        height: TAB_HEIGHT,
    }).collect()
}

#[inline]
fn header_rect(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: HEADER_HEIGHT,
    }
}

#[inline]
fn close_button_rect(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + bounds.width - CLOSE_BUTTON_SIZE - CLOSE_BUTTON_OFFSET,
        y: bounds.y + CLOSE_BUTTON_OFFSET,
        width: CLOSE_BUTTON_SIZE,
        height: CLOSE_BUTTON_SIZE,
    }
}

#[inline]
fn content_rect(bounds: Rectangle) -> Rectangle {
    let tab_y = HEADER_HEIGHT + TAB_SPACING;
    let content_y = tab_y + TAB_HEIGHT + TAB_SPACING;
    
    Rectangle {
        x: bounds.x + CONTENT_PADDING,
        y: bounds.y + content_y,
        width: bounds.width - (CONTENT_PADDING * 2.0),
        height: 250.0,  // Or calculate dynamically
    }
}


#[derive(Clone, Copy)]
struct PalGeom {
    label_h: f32,
    pill_h: f32,
    row_gap: f32,
    col_gap: f32,
    section_gap: f32,
    eq_w3: f32,
}

// Fixed palette geometry calculation
fn palette_geom_compact(content: Rectangle) -> PalGeom {
    let label_h = 16.0;
    let pill_h = 22.0;
    let row_gap = 3.0;
    let col_gap = 10.0;
    let section_gap = 6.0;
    
    // Total: 6*16 + 8*22 + 6*3 + 5*6 + 2*3 = 96 + 176 + 18 + 30 + 6 = 326
    
    let eq_w3 = (content.width - 2.0 * col_gap) / 3.0;
    
    PalGeom { label_h, pill_h, row_gap, col_gap, section_gap, eq_w3 }
}

#[derive(Debug, Clone, PartialEq)]
struct PaletteSource {
    row: &'static str,      // "Background", "Primary", etc.
    tone: &'static str,     // "Base", "Weak", "Strong", etc.
    pick_target: PickTarget, // Color or Text
}