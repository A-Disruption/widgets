//! ColorSlurp-inspired overlay color picker for iced 0.14.
//!
//! The widget is an invisible anchor that renders a floating overlay when
//! `is_open` is `true`, following the same external-state pattern as
//! [`crate::date_picker`].
//!
//! # Example
//! ```no_run
//! use iced::{Color, Element};
//! use widgets::color_picker_two::{
//!     ColorInfo, ColorModel, ContrastInfo, MagnifierRequest, PickerPage, color_picker_two,
//! };
//!
//! #[derive(Clone)]
//! enum Message {
//!     ClosePicker,
//!     ColorChanged(ColorInfo),
//!     ContrastChanged(ContrastInfo),
//!     MagnifierRequested(MagnifierRequest),
//! }
//!
//! let open = true;
//! let color = Color::from_rgb8(0x9C, 0xAA, 0x33);
//!
//! let _picker: Element<'_, Message> = color_picker_two(open, color)
//!     .model(ColorModel::Hsl)
//!     .page(PickerPage::Contrast)
//!     .on_change_with_info(Message::ColorChanged)
//!     .on_contrast_change(Message::ContrastChanged)
//!     .on_magnifier_request(Message::MagnifierRequested)
//!     .on_close(|| Message::ClosePicker)
//!     .into();
//! ```

use iced::{
    Background, Border, Color, Degrees, Element, Event, Length, Point, Rectangle, Shadow, Size,
    Vector,
    advanced::{
        Clipboard, Layout, Overlay, Shell, Widget,
        layout::{Limits, Node},
        overlay, renderer,
        widget::{self, tree::Tree},
    },
    alignment::Vertical,
    keyboard, mouse, touch,
    widget::text,
};
use std::marker::PhantomData;
use std::time::{Duration, Instant};

const PANEL_WIDTH: f32 = 336.0;
const HEADER_HEIGHT: f32 = 42.0;
const PREVIEW_HEIGHT: f32 = 110.0;
const CONTRAST_SUMMARY_HEIGHT: f32 = 92.0;
const CONTRAST_WELLS_HEIGHT: f32 = 58.0;
const CONTENT_PADDING_X: f32 = 16.0;
const CONTENT_PADDING_TOP: f32 = 14.0;
const CONTROL_ROW_HEIGHT: f32 = 28.0;
const CONTROL_GAP: f32 = 10.0;
const SLIDER_ROW_HEIGHT: f32 = 18.0;
const SLIDER_TRACK_HEIGHT: f32 = 16.0;
const SLIDER_KNOB_SIZE: f32 = 12.0;
const SLIDER_KNOB_RADIUS: f32 = SLIDER_KNOB_SIZE / 2.0;
const SLIDER_GAP: f32 = 6.0;
const LABEL_WIDTH: f32 = 0.0;
const TRACK_VALUE_GAP: f32 = 14.0;
const VALUE_WIDTH: f32 = 54.0;
const SWATCH_HEADER_HEIGHT: f32 = 22.0;
const SWATCH_SIZE: f32 = 32.0;
const SWATCH_GAP: f32 = 8.0;
const SWATCH_RADIUS: f32 = 10.0;
const SWATCH_TOP_MARGIN: f32 = 14.0;
const FOOTER_PADDING: f32 = 20.0;
const ANCHOR_SIZE: f32 = 1.0;
const COPY_FEEDBACK_DURATION: Duration = Duration::from_millis(1_200);
const HEADER_PAGE_BUTTON_SIZE: f32 = 28.0;
/// Creates a [`ColorPickerTwo`] overlay widget.
pub fn color_picker_two<'a, Message, Theme, Renderer>(
    is_open: bool,
    color: Color,
) -> ColorPickerTwo<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    ColorPickerTwo::new(is_open, color)
}

/// The active slider model shown in the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorModel {
    Hsl,
    Hsv,
    Rgb,
}

impl ColorModel {
    pub const ALL: &'static [Self] = &[Self::Hsl, Self::Rgb, Self::Hsv];

    fn label(self) -> &'static str {
        match self {
            Self::Hsl => "HSL",
            Self::Hsv => "HSV",
            Self::Rgb => "RGB",
        }
    }
}

impl std::fmt::Display for ColorModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// A named swatch group shown at the bottom of the picker.
#[derive(Debug, Clone, PartialEq)]
pub struct SwatchGroup {
    pub name: String,
    pub colors: Vec<Color>,
}

impl SwatchGroup {
    pub fn new(name: impl Into<String>, colors: impl Into<Vec<Color>>) -> Self {
        Self {
            name: name.into(),
            colors: colors.into(),
        }
    }
}

/// Rich information about the current picker value.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorInfo {
    pub color: Color,
    pub hex: String,
    pub model: ColorModel,
    pub formatted: String,
}

impl ColorInfo {
    pub fn new(color: Color, model: ColorModel) -> Self {
        Self {
            color,
            hex: color_to_hex(color),
            model,
            formatted: format_model_value(model, color),
        }
    }
}

/// The color well or picker section requesting a screen sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnifierTarget {
    CurrentColor,
    Foreground,
    Background,
}

/// Metadata published when the magnifier button is pressed.
#[derive(Debug, Clone, PartialEq)]
pub struct MagnifierRequest {
    pub target: MagnifierTarget,
    pub color: Color,
}

impl MagnifierRequest {
    fn new(target: MagnifierTarget, color: Color) -> Self {
        Self { target, color }
    }
}

/// The active panel shown in the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerPage {
    Picker,
    Contrast,
}

/// The contrast color currently being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastTarget {
    Foreground,
    Background,
}

/// The current WCAG contrast grade for a color pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContrastGrade {
    Aaa,
    Aa,
    Fail,
}

impl ContrastGrade {
    pub fn label(self) -> &'static str {
        match self {
            Self::Aaa => "AAA",
            Self::Aa => "AA",
            Self::Fail => "FAIL",
        }
    }

    pub fn rating(self) -> &'static str {
        match self {
            Self::Aaa => "Excellent",
            Self::Aa => "Good",
            Self::Fail => "Needs Work",
        }
    }
}

/// Rich information about the current contrast pair.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastInfo {
    pub foreground: Color,
    pub background: Color,
    pub ratio: f32,
    pub grade: ContrastGrade,
    pub rating: &'static str,
    pub active_target: ContrastTarget,
}

impl ContrastInfo {
    fn new(foreground: Color, background: Color, active_target: ContrastTarget) -> Self {
        let ratio = contrast_ratio(foreground, background);
        let grade = contrast_grade(ratio);

        Self {
            foreground,
            background,
            ratio,
            grade,
            rating: grade.rating(),
            active_target,
        }
    }
}

/// A ColorSlurp-inspired overlay color picker.
pub struct ColorPickerTwo<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    is_open: bool,
    color: Color,
    model: ColorModel,
    page: PickerPage,
    contrast_background: Color,
    position: Option<Point>,
    swatch_groups: Vec<SwatchGroup>,
    on_change: Option<Box<dyn Fn(Color) -> Message + 'a>>,
    on_change_with_info: Option<Box<dyn Fn(ColorInfo) -> Message + 'a>>,
    on_contrast_change: Option<Box<dyn Fn(ContrastInfo) -> Message + 'a>>,
    on_magnifier_request: Option<Box<dyn Fn(MagnifierRequest) -> Message + 'a>>,
    on_close: Option<Box<dyn Fn() -> Message + 'a>>,
    class: Theme::Class<'a>,
    _renderer: PhantomData<Renderer>,
}

impl<'a, Message, Theme, Renderer> ColorPickerTwo<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    pub fn new(is_open: bool, color: Color) -> Self {
        Self {
            is_open,
            color,
            model: ColorModel::Hsl,
            page: PickerPage::Picker,
            contrast_background: Color::from_rgb8(0x2B, 0x2D, 0x3A),
            position: None,
            swatch_groups: default_swatch_groups(),
            on_change: None,
            on_change_with_info: None,
            on_contrast_change: None,
            on_magnifier_request: None,
            on_close: None,
            class: Theme::default(),
            _renderer: PhantomData,
        }
    }

    /// Sets the initial slider model.
    pub fn model(mut self, model: ColorModel) -> Self {
        self.model = model;
        self
    }

    /// Sets the initial page shown by the overlay.
    pub fn page(mut self, page: PickerPage) -> Self {
        self.page = page;
        self
    }

    /// Sets the contrast background color.
    pub fn contrast_background(mut self, color: Color) -> Self {
        self.contrast_background = color;
        self
    }

    /// Overrides the initial overlay position. Defaults to centered.
    pub fn position(mut self, position: Point) -> Self {
        self.position = Some(position);
        self
    }

    /// Replaces the default swatch groups.
    pub fn swatches(mut self, swatches: Vec<SwatchGroup>) -> Self {
        self.swatch_groups = normalize_swatch_groups(swatches);
        self
    }

    /// Publishes color changes as a plain [`Color`].
    pub fn on_change(mut self, f: impl Fn(Color) -> Message + 'a) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Publishes color changes together with formatted picker metadata.
    pub fn on_change_with_info(mut self, f: impl Fn(ColorInfo) -> Message + 'a) -> Self {
        self.on_change_with_info = Some(Box::new(f));
        self
    }

    /// Publishes contrast pair changes together with the current ratio metadata.
    pub fn on_contrast_change(mut self, f: impl Fn(ContrastInfo) -> Message + 'a) -> Self {
        self.on_contrast_change = Some(Box::new(f));
        self
    }

    /// Publishes a request to sample a color from outside the picker UI.
    pub fn on_magnifier_request(mut self, f: impl Fn(MagnifierRequest) -> Message + 'a) -> Self {
        self.on_magnifier_request = Some(Box::new(f));
        self
    }

    /// Publishes a close request.
    pub fn on_close(mut self, f: impl Fn() -> Message + 'a) -> Self {
        self.on_close = Some(Box::new(f));
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

#[derive(Debug, Clone)]
struct State {
    color: Color,
    foreground_color: Color,
    background_color: Color,
    model: ColorModel,
    page: PickerPage,
    contrast_target: ContrastTarget,
    dragging_slider: Option<SliderChannel>,
    is_dragging_overlay: bool,
    drag_offset: Vector,
    overlay_position: Point,
    viewport_size: Size,
    swatch_groups: Vec<SwatchGroup>,
    active_swatch_group: usize,
    open_menu: Option<MenuKind>,
    feedback: Option<(FeedbackKind, Instant)>,
}

impl State {
    fn new(
        color: Color,
        background_color: Color,
        model: ColorModel,
        page: PickerPage,
        swatches: Vec<SwatchGroup>,
    ) -> Self {
        Self {
            color,
            foreground_color: color,
            background_color,
            model,
            page,
            contrast_target: ContrastTarget::Foreground,
            dragging_slider: None,
            is_dragging_overlay: false,
            drag_offset: Vector::ZERO,
            overlay_position: Point::ORIGIN,
            viewport_size: Size::new(1920.0, 1080.0),
            swatch_groups: normalize_swatch_groups(swatches),
            active_swatch_group: 0,
            open_menu: None,
            feedback: None,
        }
    }

    fn sync_from_external(
        &mut self,
        color: Color,
        background_color: Color,
        model: ColorModel,
        page: PickerPage,
        swatches: &[SwatchGroup],
    ) {
        self.foreground_color = color;
        self.background_color = background_color;
        self.model = model;
        self.page = page;
        if self.page == PickerPage::Picker {
            self.contrast_target = ContrastTarget::Foreground;
        }
        self.swatch_groups = normalize_swatch_groups(swatches.to_vec());
        if self.active_swatch_group >= self.swatch_groups.len() {
            self.active_swatch_group = 0;
        }
        self.sync_active_color();
    }

    fn active_group(&self) -> &SwatchGroup {
        &self.swatch_groups[self.active_swatch_group]
    }

    fn active_group_mut(&mut self) -> &mut SwatchGroup {
        &mut self.swatch_groups[self.active_swatch_group]
    }

    fn feedback_message(&self) -> Option<&'static str> {
        let Some((kind, at)) = self.feedback else {
            return None;
        };

        if at.elapsed() > COPY_FEEDBACK_DURATION {
            return None;
        }

        Some(kind.label())
    }

    fn sync_active_color(&mut self) {
        self.color = match self.contrast_target {
            ContrastTarget::Foreground => self.foreground_color,
            ContrastTarget::Background => self.background_color,
        };
    }

    fn active_contrast_info(&self) -> ContrastInfo {
        ContrastInfo::new(
            self.foreground_color,
            self.background_color,
            self.contrast_target,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SliderChannel {
    Primary,
    Secondary,
    Tertiary,
    Alpha,
}

impl SliderChannel {
    const ALL: [Self; 4] = [Self::Primary, Self::Secondary, Self::Tertiary, Self::Alpha];

    fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
            Self::Tertiary => 2,
            Self::Alpha => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedbackKind {
    Hex,
    Formatted,
    Pasted,
    InvalidPaste,
}

impl FeedbackKind {
    fn label(self) -> &'static str {
        match self {
            Self::Hex => "HEX COPIED",
            Self::Formatted => "VALUE COPIED",
            Self::Pasted => "COLOR PASTED",
            Self::InvalidPaste => "INVALID COLOR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuKind {
    Model,
    SwatchGroup,
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ColorPickerTwo<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::new(
            self.color,
            self.contrast_background,
            self.model,
            self.page,
            self.swatch_groups.clone(),
        ))
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();

        if !self.is_open || (state.dragging_slider.is_none() && !state.is_dragging_overlay) {
            state.foreground_color = self.color;
            state.background_color = self.contrast_background;
            state.page = self.page;
            if state.page == PickerPage::Picker {
                state.contrast_target = ContrastTarget::Foreground;
            }
            state.sync_active_color();
        }

        if !self.is_open {
            state.model = self.model;
            state.swatch_groups = normalize_swatch_groups(self.swatch_groups.clone());
            if state.active_swatch_group >= state.swatch_groups.len() {
                state.active_swatch_group = 0;
            }
            state.contrast_target = ContrastTarget::Foreground;
            state.sync_active_color();
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
        // Invisible anchor.
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = state.state.downcast_mut::<State>();

        if let Event::Window(iced::window::Event::Opened { size, .. })
        | Event::Window(iced::window::Event::Resized(size)) = event
        {
            state.viewport_size = Size::new(size.width, size.height);
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
        let state = state.state.downcast_mut::<State>();

        if !self.is_open {
            state.overlay_position = Point::ORIGIN;
            state.dragging_slider = None;
            state.is_dragging_overlay = false;
            state.open_menu = None;
            return None;
        }

        if state.overlay_position == Point::ORIGIN {
            state.sync_from_external(
                self.color,
                self.contrast_background,
                self.model,
                self.page,
                &self.swatch_groups,
            );

            let size = overlay_size(state.page, state.active_group().colors.len());
            let centered = Point::new(
                ((viewport.width - size.width) / 2.0).max(0.0),
                ((viewport.height - size.height) / 2.0).max(0.0),
            );

            state.overlay_position =
                clamp_overlay_position(self.position.unwrap_or(centered), viewport.size(), size);
        }

        Some(
            ColorPickerTwoOverlay {
                state,
                on_change: &self.on_change,
                on_change_with_info: &self.on_change_with_info,
                on_contrast_change: &self.on_contrast_change,
                on_magnifier_request: &self.on_magnifier_request,
                on_close: &self.on_close,
                class: &self.class,
                _renderer: PhantomData,
            }
            .into_overlay(),
        )
    }
}

impl<'a, Message, Theme, Renderer> From<ColorPickerTwo<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    fn from(picker: ColorPickerTwo<'a, Message, Theme, Renderer>) -> Self {
        Self::new(picker)
    }
}

struct ColorPickerTwoOverlay<'r, 'w, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    state: &'r mut State,
    on_change: &'r Option<Box<dyn Fn(Color) -> Message + 'w>>,
    on_change_with_info: &'r Option<Box<dyn Fn(ColorInfo) -> Message + 'w>>,
    on_contrast_change: &'r Option<Box<dyn Fn(ContrastInfo) -> Message + 'w>>,
    on_magnifier_request: &'r Option<Box<dyn Fn(MagnifierRequest) -> Message + 'w>>,
    on_close: &'r Option<Box<dyn Fn() -> Message + 'w>>,
    class: &'r Theme::Class<'w>,
    _renderer: PhantomData<Renderer>,
}

impl<'r, 'w, Message, Theme, Renderer> ColorPickerTwoOverlay<'r, 'w, Message, Theme, Renderer>
where
    Message: Clone + 'r,
    Theme: Catalog + 'r,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'r,
{
    fn into_overlay(self) -> overlay::Element<'r, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    fn publish_change(&self, shell: &mut Shell<'_, Message>) {
        if let Some(callback) = self.on_change_with_info {
            shell.publish(callback(ColorInfo::new(
                self.state.foreground_color,
                self.state.model,
            )));
        } else if let Some(callback) = self.on_change {
            shell.publish(callback(self.state.foreground_color));
        }
    }

    fn publish_contrast_change(&self, shell: &mut Shell<'_, Message>) {
        if let Some(callback) = self.on_contrast_change {
            shell.publish(callback(self.state.active_contrast_info()));
        }
    }

    fn publish_magnifier_request(
        &self,
        target: MagnifierTarget,
        color: Color,
        shell: &mut Shell<'_, Message>,
    ) {
        if let Some(callback) = self.on_magnifier_request {
            shell.publish(callback(MagnifierRequest::new(target, color)));
        }
    }

    fn publish_close(&self, shell: &mut Shell<'_, Message>) {
        if let Some(callback) = self.on_close {
            shell.publish(callback());
        }
    }

    fn set_feedback(&mut self, kind: FeedbackKind) {
        self.state.feedback = Some((kind, Instant::now()));
    }

    fn set_color(&mut self, color: Color, shell: &mut Shell<'_, Message>) {
        if same_color(self.state.color, color) {
            shell.request_redraw();
            return;
        }

        self.state.color = color;

        let foreground_changed = if self.state.contrast_target == ContrastTarget::Foreground {
            if !same_color(self.state.foreground_color, color) {
                self.state.foreground_color = color;
                true
            } else {
                false
            }
        } else {
            false
        };

        let background_changed = if self.state.contrast_target == ContrastTarget::Background {
            if !same_color(self.state.background_color, color) {
                self.state.background_color = color;
                true
            } else {
                false
            }
        } else {
            false
        };

        if foreground_changed {
            self.publish_change(shell);
        }

        if foreground_changed || background_changed {
            self.publish_contrast_change(shell);
        }

        shell.request_redraw();
    }

    fn toggle_menu(&mut self, menu: MenuKind, shell: &mut Shell<'_, Message>) {
        self.state.open_menu = if self.state.open_menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        shell.request_redraw();
    }

    fn close_menu(&mut self, shell: &mut Shell<'_, Message>) {
        if self.state.open_menu.take().is_some() {
            shell.request_redraw();
        }
    }

    fn select_model(&mut self, model: ColorModel, shell: &mut Shell<'_, Message>) {
        self.state.model = model;
        self.state.open_menu = None;
        shell.request_redraw();
    }

    fn set_page(&mut self, page: PickerPage, shell: &mut Shell<'_, Message>) {
        if self.state.page == page {
            return;
        }

        self.state.page = page;
        self.state.open_menu = None;

        if page == PickerPage::Picker {
            self.state.contrast_target = ContrastTarget::Foreground;
            self.state.sync_active_color();
        }

        self.refresh_overlay_layout(shell);
        shell.request_redraw();
    }

    fn select_contrast_target(&mut self, target: ContrastTarget, shell: &mut Shell<'_, Message>) {
        if self.state.contrast_target == target {
            return;
        }

        self.state.contrast_target = target;
        self.state.sync_active_color();
        shell.request_redraw();
    }

    fn swap_contrast_colors(&mut self, shell: &mut Shell<'_, Message>) {
        std::mem::swap(
            &mut self.state.foreground_color,
            &mut self.state.background_color,
        );
        self.state.sync_active_color();
        self.publish_change(shell);
        self.publish_contrast_change(shell);
        shell.request_redraw();
    }

    fn fix_contrast_pair(&mut self, shell: &mut Shell<'_, Message>) {
        let locked = match self.state.contrast_target {
            ContrastTarget::Foreground => self.state.background_color,
            ContrastTarget::Background => self.state.foreground_color,
        };

        let fixed = best_contrast_fix(self.state.color, locked);
        self.set_color(fixed, shell);
    }

    fn select_swatch_group(&mut self, index: usize, shell: &mut Shell<'_, Message>) {
        if index < self.state.swatch_groups.len() {
            self.state.active_swatch_group = index;
        }
        self.state.open_menu = None;
        self.refresh_overlay_layout(shell);
        shell.request_redraw();
    }

    fn copy_hex(&mut self, clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>) {
        clipboard.write(
            iced::advanced::clipboard::Kind::Standard,
            color_to_hex(self.state.color),
        );
        self.set_feedback(FeedbackKind::Hex);
        shell.request_redraw();
    }

    fn copy_formatted(&mut self, clipboard: &mut dyn Clipboard, shell: &mut Shell<'_, Message>) {
        clipboard.write(
            iced::advanced::clipboard::Kind::Standard,
            format_model_value(self.state.model, self.state.color),
        );
        self.set_feedback(FeedbackKind::Formatted);
        shell.request_redraw();
    }

    fn request_magnifier(&mut self, target: MagnifierTarget, shell: &mut Shell<'_, Message>) {
        let color = match target {
            MagnifierTarget::CurrentColor => self.state.color,
            MagnifierTarget::Foreground => self.state.foreground_color,
            MagnifierTarget::Background => self.state.background_color,
        };

        self.publish_magnifier_request(target, color, shell);
        shell.request_redraw();
    }

    fn paste_clipboard_color(&mut self, clipboard: &dyn Clipboard, shell: &mut Shell<'_, Message>) {
        let Some(contents) = clipboard.read(iced::advanced::clipboard::Kind::Standard) else {
            self.set_feedback(FeedbackKind::InvalidPaste);
            shell.request_redraw();
            return;
        };

        let Some(color) = parse_color_string(&contents) else {
            self.set_feedback(FeedbackKind::InvalidPaste);
            shell.request_redraw();
            return;
        };

        self.set_color(color, shell);
        self.set_feedback(FeedbackKind::Pasted);
        shell.request_redraw();
    }

    fn add_current_swatch(&mut self, shell: &mut Shell<'_, Message>) {
        let color = self.state.color;
        let group = self.state.active_group_mut();

        if group
            .colors
            .iter()
            .any(|candidate| same_color(*candidate, color))
        {
            return;
        }

        group.colors.push(color);
        self.refresh_overlay_layout(shell);
        shell.request_redraw();
    }

    fn refresh_overlay_layout(&mut self, shell: &mut Shell<'_, Message>) {
        self.state.overlay_position = clamp_overlay_position(
            self.state.overlay_position,
            self.state.viewport_size,
            overlay_size(self.state.page, self.state.active_group().colors.len()),
        );
        shell.invalidate_layout();
    }

    fn start_overlay_drag(&mut self, cursor_pos: Point) {
        self.state.is_dragging_overlay = true;
        self.state.drag_offset = Vector::new(
            cursor_pos.x - self.state.overlay_position.x,
            cursor_pos.y - self.state.overlay_position.y,
        );
    }

    fn update_overlay_drag(&mut self, cursor_pos: Point, shell: &mut Shell<'_, Message>) {
        let next = Point::new(
            cursor_pos.x - self.state.drag_offset.x,
            cursor_pos.y - self.state.drag_offset.y,
        );
        self.state.overlay_position = clamp_overlay_position(
            next,
            self.state.viewport_size,
            overlay_size(self.state.page, self.state.active_group().colors.len()),
        );
        shell.invalidate_layout();
        shell.request_redraw();
    }

    fn handle_slider_press(
        &mut self,
        bounds: Rectangle,
        cursor_pos: Point,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        for channel in SliderChannel::ALL {
            let track = slider_track_rect(bounds, self.state.page, channel);
            if track.contains(cursor_pos) {
                self.state.dragging_slider = Some(channel);
                self.update_slider_drag(bounds, cursor_pos, shell);
                return true;
            }
        }

        false
    }

    fn update_slider_drag(
        &mut self,
        bounds: Rectangle,
        cursor_pos: Point,
        shell: &mut Shell<'_, Message>,
    ) {
        let Some(channel) = self.state.dragging_slider else {
            return;
        };

        let track = slider_track_rect(bounds, self.state.page, channel);
        let value = slider_value_from_cursor(track, cursor_pos.x);
        let mut color = self.state.color;

        match (self.state.model, channel) {
            (ColorModel::Hsl, SliderChannel::Primary) => {
                let (_, s, l) = rgb_to_hsl(self.state.color);
                color = hsl_to_color(value * 360.0, s, l, self.state.color.a);
            }
            (ColorModel::Hsl, SliderChannel::Secondary) => {
                let (h, _, l) = rgb_to_hsl(self.state.color);
                color = hsl_to_color(h, value, l, self.state.color.a);
            }
            (ColorModel::Hsl, SliderChannel::Tertiary) => {
                let (h, s, _) = rgb_to_hsl(self.state.color);
                color = hsl_to_color(h, s, value, self.state.color.a);
            }
            (ColorModel::Hsl, SliderChannel::Alpha) => {
                color.a = value;
            }
            (ColorModel::Hsv, SliderChannel::Primary) => {
                let (_, s, v) = rgb_to_hsv(self.state.color);
                color = hsv_to_color(value * 360.0, s, v, self.state.color.a);
            }
            (ColorModel::Hsv, SliderChannel::Secondary) => {
                let (h, _, v) = rgb_to_hsv(self.state.color);
                color = hsv_to_color(h, value, v, self.state.color.a);
            }
            (ColorModel::Hsv, SliderChannel::Tertiary) => {
                let (h, s, _) = rgb_to_hsv(self.state.color);
                color = hsv_to_color(h, s, value, self.state.color.a);
            }
            (ColorModel::Hsv, SliderChannel::Alpha) => {
                color.a = value;
            }
            (ColorModel::Rgb, SliderChannel::Primary) => {
                color.r = value;
            }
            (ColorModel::Rgb, SliderChannel::Secondary) => {
                color.g = value;
            }
            (ColorModel::Rgb, SliderChannel::Tertiary) => {
                color.b = value;
            }
            (ColorModel::Rgb, SliderChannel::Alpha) => {
                color.a = value;
            }
        }

        self.set_color(color, shell);
    }

    fn try_pick_swatch(
        &mut self,
        bounds: Rectangle,
        cursor_pos: Point,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        let visible = visible_swatches(self.state.active_group()).to_vec();

        for (index, color) in visible.into_iter().enumerate() {
            if swatch_rect(bounds, self.state.page, index).contains(cursor_pos) {
                self.set_color(color, shell);
                return true;
            }
        }

        false
    }

    fn try_select_open_menu(
        &mut self,
        bounds: Rectangle,
        cursor_pos: Point,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        let Some(menu) = self.state.open_menu else {
            return false;
        };

        let menu_bounds = menu_rect(
            bounds,
            self.state.page,
            menu,
            self.state.swatch_groups.len(),
        );
        if !menu_bounds.contains(cursor_pos) {
            return false;
        }

        let item_count = match menu {
            MenuKind::Model => ColorModel::ALL.len(),
            MenuKind::SwatchGroup => self.state.swatch_groups.len(),
        };

        for index in 0..item_count {
            let item = menu_item_rect(menu_bounds, index);
            if item.contains(cursor_pos) {
                match menu {
                    MenuKind::Model => self.select_model(ColorModel::ALL[index], shell),
                    MenuKind::SwatchGroup => self.select_swatch_group(index, shell),
                }
                return true;
            }
        }

        true
    }

    fn cursor_is_over_open_menu(&self, bounds: Rectangle, cursor_pos: Point) -> bool {
        self.state.open_menu.is_some_and(|menu| {
            menu_rect(
                bounds,
                self.state.page,
                menu,
                self.state.swatch_groups.len(),
            )
            .contains(cursor_pos)
        })
    }
}

impl<'r, Message, Theme, Renderer> Overlay<Message, Theme, Renderer>
    for ColorPickerTwoOverlay<'r, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'r,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font> + 'r,
{
    fn layout(&mut self, _renderer: &Renderer, bounds: Size) -> Node {
        self.state.viewport_size = bounds;
        Node::new(overlay_size(
            self.state.page,
            self.state.active_group().colors.len(),
        ))
        .move_to(self.state.overlay_position)
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
        let status = if cursor.is_over(bounds) {
            Status::Hovered
        } else {
            Status::Active
        };
        let style = theme.style(self.class, status);

        draw_panel(renderer, bounds, style);
        draw_header(
            renderer,
            bounds,
            style,
            self.state.page,
            self.state.feedback_message(),
            cursor,
        );
        match self.state.page {
            PickerPage::Picker => {
                draw_preview(
                    renderer,
                    preview_rect(bounds, self.state.page),
                    self.state.color,
                    style,
                );
            }
            PickerPage::Contrast => {
                draw_contrast_summary(
                    renderer,
                    contrast_summary_rect(bounds),
                    self.state.foreground_color,
                    self.state.background_color,
                    style,
                );
                draw_contrast_wells(
                    renderer,
                    bounds,
                    style,
                    self.state.foreground_color,
                    self.state.background_color,
                    self.state.contrast_target,
                    cursor,
                );
            }
        }
        draw_controls(
            renderer,
            bounds,
            style,
            self.state.color,
            self.state.model,
            cursor,
            self.state.page,
        );
        draw_sliders(
            renderer,
            bounds,
            style,
            self.state.color,
            self.state.model,
            cursor,
            self.state.page,
        );
        draw_swatches(
            renderer,
            bounds,
            style,
            &self.state.swatch_groups,
            self.state.active_swatch_group,
            self.state.color,
            cursor,
            self.state.page,
        );
        if let Some(menu) = self.state.open_menu {
            draw_menu(
                renderer,
                bounds,
                style,
                self.state.page,
                menu,
                self.state.model,
                &self.state.swatch_groups,
                self.state.active_swatch_group,
                cursor,
            );
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

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                let Some(cursor_pos) = cursor.position() else {
                    return;
                };

                let cursor_in_open_menu = self.cursor_is_over_open_menu(bounds, cursor_pos);

                if !bounds.contains(cursor_pos) && !cursor_in_open_menu {
                    return;
                }

                let header = header_rect(bounds);
                let close = close_button_rect(header);
                let picker_tab = page_button_rect(header, PickerPage::Picker);
                let contrast_tab = page_button_rect(header, PickerPage::Contrast);

                if close.contains(cursor_pos) {
                    self.publish_close(shell);
                    shell.capture_event();
                    return;
                }

                if picker_tab.contains(cursor_pos) {
                    self.set_page(PickerPage::Picker, shell);
                    shell.capture_event();
                    return;
                }

                if contrast_tab.contains(cursor_pos) {
                    self.set_page(PickerPage::Contrast, shell);
                    shell.capture_event();
                    return;
                }

                if self.try_select_open_menu(bounds, cursor_pos, shell) {
                    shell.capture_event();
                    return;
                }

                if header.contains(cursor_pos) {
                    self.close_menu(shell);
                    self.start_overlay_drag(cursor_pos);
                    shell.capture_event();
                    return;
                }

                if hex_value_rect(bounds, self.state.page).contains(cursor_pos) {
                    self.close_menu(shell);
                    self.copy_hex(clipboard, shell);
                    shell.capture_event();
                    return;
                }

                if copy_button_rect(bounds, self.state.page).contains(cursor_pos) {
                    self.close_menu(shell);
                    self.copy_formatted(clipboard, shell);
                    shell.capture_event();
                    return;
                }

                if model_button_rect(bounds, self.state.page).contains(cursor_pos) {
                    self.toggle_menu(MenuKind::Model, shell);
                    shell.capture_event();
                    return;
                }

                if self.state.page == PickerPage::Contrast {
                    if contrast_well_magnifier_rect(bounds, ContrastTarget::Foreground)
                        .contains(cursor_pos)
                    {
                        self.close_menu(shell);
                        self.select_contrast_target(ContrastTarget::Foreground, shell);
                        self.request_magnifier(MagnifierTarget::Foreground, shell);
                        shell.capture_event();
                        return;
                    }

                    if contrast_well_magnifier_rect(bounds, ContrastTarget::Background)
                        .contains(cursor_pos)
                    {
                        self.close_menu(shell);
                        self.select_contrast_target(ContrastTarget::Background, shell);
                        self.request_magnifier(MagnifierTarget::Background, shell);
                        shell.capture_event();
                        return;
                    }

                    if contrast_well_rect(bounds, ContrastTarget::Foreground).contains(cursor_pos) {
                        self.close_menu(shell);
                        self.select_contrast_target(ContrastTarget::Foreground, shell);
                        shell.capture_event();
                        return;
                    }

                    if contrast_well_rect(bounds, ContrastTarget::Background).contains(cursor_pos) {
                        self.close_menu(shell);
                        self.select_contrast_target(ContrastTarget::Background, shell);
                        shell.capture_event();
                        return;
                    }

                    if contrast_swap_rect(bounds).contains(cursor_pos) {
                        self.close_menu(shell);
                        self.swap_contrast_colors(shell);
                        shell.capture_event();
                        return;
                    }

                    if contrast_fix_rect(bounds).contains(cursor_pos) {
                        self.close_menu(shell);
                        self.fix_contrast_pair(shell);
                        shell.capture_event();
                        return;
                    }
                }

                if self.handle_slider_press(bounds, cursor_pos, shell) {
                    self.close_menu(shell);
                    shell.capture_event();
                    return;
                }

                if swatch_group_label_rect(bounds, self.state.page).contains(cursor_pos) {
                    self.toggle_menu(MenuKind::SwatchGroup, shell);
                    shell.capture_event();
                    return;
                }

                if self.try_pick_swatch(bounds, cursor_pos, shell) {
                    self.close_menu(shell);
                    shell.capture_event();
                    return;
                }

                if add_swatch_rect(
                    bounds,
                    self.state.page,
                    visible_swatch_count(self.state.active_group()),
                )
                .contains(cursor_pos)
                {
                    self.close_menu(shell);
                    self.add_current_swatch(shell);
                    shell.capture_event();
                    return;
                }

                if self.state.open_menu.is_some() {
                    self.close_menu(shell);
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. }) => {
                self.state.dragging_slider = None;
                self.state.is_dragging_overlay = false;
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if self.state.is_dragging_overlay {
                    self.update_overlay_drag(*position, shell);
                    shell.capture_event();
                    return;
                }

                if self.state.dragging_slider.is_some() {
                    self.update_slider_drag(bounds, *position, shell);
                    shell.capture_event();
                    return;
                }

                if self.state.open_menu.is_some() {
                    shell.request_redraw();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) => {
                if matches!(
                    key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Escape)
                ) {
                    if self.state.open_menu.is_some() {
                        self.close_menu(shell);
                    } else {
                        self.publish_close(shell);
                    }
                    shell.capture_event();
                    return;
                }

                if key.to_latin(*physical_key) == Some('v')
                    && modifiers.command()
                    && !modifiers.alt()
                {
                    self.close_menu(shell);
                    self.paste_clipboard_color(clipboard, shell);
                    shell.capture_event();
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
        let Some(cursor_pos) = cursor.position() else {
            return mouse::Interaction::default();
        };

        let cursor_in_open_menu = self.cursor_is_over_open_menu(bounds, cursor_pos);

        if !bounds.contains(cursor_pos) && !cursor_in_open_menu {
            return mouse::Interaction::default();
        }

        if self.state.is_dragging_overlay {
            return mouse::Interaction::Grabbing;
        }

        let header = header_rect(bounds);
        let picker_tab = page_button_rect(header, PickerPage::Picker);
        let contrast_tab = page_button_rect(header, PickerPage::Contrast);
        if close_button_rect(header).contains(cursor_pos) {
            return mouse::Interaction::Pointer;
        }
        if picker_tab.contains(cursor_pos) || contrast_tab.contains(cursor_pos) {
            return mouse::Interaction::Pointer;
        }
        if header.contains(cursor_pos) {
            return mouse::Interaction::Grab;
        }

        if hex_value_rect(bounds, self.state.page).contains(cursor_pos)
            || model_button_rect(bounds, self.state.page).contains(cursor_pos)
            || copy_button_rect(bounds, self.state.page).contains(cursor_pos)
            || (self.state.page == PickerPage::Contrast
                && (contrast_well_rect(bounds, ContrastTarget::Foreground).contains(cursor_pos)
                    || contrast_well_rect(bounds, ContrastTarget::Background).contains(cursor_pos)
                    || contrast_swap_rect(bounds).contains(cursor_pos)
                    || contrast_fix_rect(bounds).contains(cursor_pos)))
            || swatch_group_label_rect(bounds, self.state.page).contains(cursor_pos)
            || add_swatch_rect(
                bounds,
                self.state.page,
                visible_swatch_count(self.state.active_group()),
            )
            .contains(cursor_pos)
        {
            return mouse::Interaction::Pointer;
        }

        if cursor_in_open_menu {
            return mouse::Interaction::Pointer;
        }

        if SliderChannel::ALL
            .into_iter()
            .any(|channel| slider_track_rect(bounds, self.state.page, channel).contains(cursor_pos))
        {
            return mouse::Interaction::Pointer;
        }

        if (0..visible_swatch_count(self.state.active_group()))
            .any(|index| swatch_rect(bounds, self.state.page, index).contains(cursor_pos))
        {
            return mouse::Interaction::Pointer;
        }

        mouse::Interaction::default()
    }
}

fn draw_panel<Renderer>(renderer: &mut Renderer, bounds: Rectangle, style: Style)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.border,
            shadow: style.shadow,
            snap: true,
        },
        style.background,
    );
}

fn draw_header<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    page: PickerPage,
    feedback: Option<&'static str>,
    cursor: mouse::Cursor,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let header = header_rect(bounds);
    renderer.fill_quad(
        renderer::Quad {
            bounds: header,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: iced::border::Radius {
                    top_left: style.border.radius.top_left,
                    top_right: style.border.radius.top_right,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
            },
            shadow: Shadow::default(),
            snap: true,
        },
        style.header_background,
    );

    let divider = Rectangle {
        x: header.x,
        y: header.y + header.height - 1.0,
        width: header.width,
        height: 1.0,
    };
    renderer.fill_quad(
        renderer::Quad {
            bounds: divider,
            border: Border::default(),
            shadow: Shadow::default(),
            snap: true,
        },
        style.header_divider,
    );

    let close_bounds = close_button_rect(header);
    let picker_tab = page_button_rect(header, PickerPage::Picker);
    let contrast_tab = page_button_rect(header, PickerPage::Contrast);

    for (tab_page, rect) in [
        (PickerPage::Picker, picker_tab),
        (PickerPage::Contrast, contrast_tab),
    ] {
        let active = page == tab_page;
        let hovered = cursor.is_over(rect);

        if active || hovered {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: rect,
                    border: Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: 9.0.into(),
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                if active {
                    style.control_hover_background
                } else {
                    Color::from_rgba(
                        style.control_hover_background.r,
                        style.control_hover_background.g,
                        style.control_hover_background.b,
                        0.55,
                    )
                },
            );
        }

        match tab_page {
            PickerPage::Picker => draw_picker_page_icon(renderer, rect, style.text_color),
            PickerPage::Contrast => draw_contrast_page_icon(renderer, rect, style.text_color),
        }
    }

    if cursor.is_over(close_bounds) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: close_bounds,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            style.control_hover_background,
        );
    }

    draw_text(
        renderer,
        close_bounds,
        "X",
        14.0,
        style.text_color,
        text::Alignment::Center,
    );

    if let Some(feedback) = feedback {
        let text_bounds = Rectangle {
            x: header.x + 18.0,
            y: header.y,
            width: picker_tab.x - header.x - 26.0,
            height: header.height,
        };
        draw_text(
            renderer,
            text_bounds,
            feedback,
            11.0,
            style.muted_text_color,
            text::Alignment::Left,
        );
    }
}

fn draw_preview<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color, style: Style)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.preview_border,
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
}

fn draw_contrast_summary<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    foreground: Color,
    background: Color,
    style: Style,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let info = ContrastInfo::new(foreground, background, ContrastTarget::Foreground);

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.preview_border,
            shadow: Shadow::default(),
            snap: true,
        },
        background,
    );

    draw_text_with_font(
        renderer,
        Rectangle {
            x: bounds.x + 24.0,
            y: bounds.y + 10.0,
            width: bounds.width * 0.45,
            height: 38.0,
        },
        &format!("{:.1}", info.ratio),
        21.0,
        foreground,
        text::Alignment::Left,
        bold_font(),
    );
    draw_text_with_font(
        renderer,
        Rectangle {
            x: bounds.x + 24.0,
            y: bounds.y + 42.0,
            width: bounds.width * 0.45,
            height: 24.0,
        },
        info.rating,
        11.0,
        foreground,
        text::Alignment::Left,
        bold_font(),
    );
    draw_text_with_font(
        renderer,
        Rectangle {
            x: bounds.x + bounds.width * 0.55,
            y: bounds.y + 16.0,
            width: bounds.width * 0.25,
            height: 34.0,
        },
        info.grade.label(),
        18.0,
        foreground,
        text::Alignment::Right,
        bold_font(),
    );
}

fn draw_contrast_wells<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    foreground: Color,
    background: Color,
    active_target: ContrastTarget,
    cursor: mouse::Cursor,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    for (target, color) in [
        (ContrastTarget::Foreground, foreground),
        (ContrastTarget::Background, background),
    ] {
        let rect = contrast_well_rect(bounds, target);
        let magnifier = contrast_well_magnifier_rect(bounds, target);
        let active = active_target == target;
        let hovered = cursor.is_over(rect);
        let magnifier_hovered = cursor.is_over(magnifier);

        renderer.fill_quad(
            renderer::Quad {
                bounds: rect,
                border: Border {
                    color: if active {
                        style.selection_ring
                    } else {
                        style.control_border.color
                    },
                    width: if active { 2.0 } else { 1.0 },
                    radius: 10.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            if hovered {
                style.control_hover_background
            } else {
                style.control_background
            },
        );

        let swatch = Rectangle {
            x: rect.x + 4.0,
            y: rect.y + 4.0,
            width: (magnifier.x - rect.x - 6.0).max(0.0),
            height: rect.height - 8.0,
        };
        renderer.fill_quad(
            renderer::Quad {
                bounds: swatch,
                border: Border {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.14),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            color,
        );
        draw_text_with_font(
            renderer,
            Rectangle {
                x: swatch.x + 10.0,
                y: swatch.y,
                width: swatch.width - 16.0,
                height: swatch.height,
            },
            match target {
                ContrastTarget::Foreground => "FG",
                ContrastTarget::Background => "BG",
            },
            11.0,
            contrasting_text_color(color),
            text::Alignment::Left,
            bold_font(),
        );

        renderer.fill_quad(
            renderer::Quad {
                bounds: magnifier,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 9.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            if magnifier_hovered {
                style.control_hover_background
            } else {
                Color::TRANSPARENT
            },
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: magnifier.x,
                    y: magnifier.y + 7.0,
                    width: 1.0,
                    height: (magnifier.height - 14.0).max(0.0),
                },
                border: Border::default(),
                shadow: Shadow::default(),
                snap: true,
            },
            Color::from_rgba(0.0, 0.0, 0.0, 0.10),
        );
        draw_magnifier_icon(
            renderer,
            Rectangle {
                x: magnifier.x + (magnifier.width - 18.0) / 2.0,
                y: magnifier.y + (magnifier.height - 18.0) / 2.0,
                width: 18.0,
                height: 18.0,
            },
            if magnifier_hovered {
                style.text_color
            } else {
                style.muted_text_color
            },
        );
    }

    let swap_rect = contrast_swap_rect(bounds);
    if cursor.is_over(swap_rect) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: swap_rect,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            style.control_hover_background,
        );
    }
    draw_text(
        renderer,
        swap_rect,
        "<>",
        13.0,
        style.muted_text_color,
        text::Alignment::Center,
    );

    let fix_rect = contrast_fix_rect(bounds);
    let fix_enabled = contrast_grade(contrast_ratio(foreground, background)) == ContrastGrade::Fail;
    renderer.fill_quad(
        renderer::Quad {
            bounds: fix_rect,
            border: style.control_border,
            shadow: Shadow::default(),
            snap: true,
        },
        if fix_enabled && cursor.is_over(fix_rect) {
            style.control_hover_background
        } else {
            style.control_background
        },
    );
    draw_text_with_font(
        renderer,
        fix_rect,
        "Fix",
        12.0,
        if fix_enabled {
            style.text_color
        } else {
            style.muted_text_color
        },
        text::Alignment::Center,
        if fix_enabled {
            bold_font()
        } else {
            iced::Font::default()
        },
    );
}

fn draw_controls<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    color: Color,
    model: ColorModel,
    cursor: mouse::Cursor,
    page: PickerPage,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let hex_rect = hex_value_rect(bounds, page);
    let model_rect = model_button_rect(bounds, page);
    let copy_rect = copy_button_rect(bounds, page);

    draw_text_with_font(
        renderer,
        Rectangle {
            x: hex_rect.x,
            y: hex_rect.y,
            width: hex_rect.width,
            height: hex_rect.height,
        },
        &color_to_hex(color),
        14.0,
        style.text_color,
        text::Alignment::Left,
        bold_font(),
    );
    draw_text(
        renderer,
        Rectangle {
            x: model_rect.x,
            y: model_rect.y,
            width: model_rect.width - 10.0,
            height: model_rect.height,
        },
        model.label(),
        13.0,
        style.text_color,
        text::Alignment::Center,
    );
    draw_text(
        renderer,
        Rectangle {
            x: model_rect.x + model_rect.width - 16.0,
            y: model_rect.y,
            width: 12.0,
            height: model_rect.height,
        },
        "v",
        10.0,
        style.muted_text_color,
        text::Alignment::Center,
    );

    draw_copy_icon(
        renderer,
        Rectangle {
            x: copy_rect.x + (copy_rect.width - 16.0) / 2.0,
            y: copy_rect.y + (copy_rect.height - 16.0) / 2.0,
            width: 16.0,
            height: 16.0,
        },
        if cursor.is_over(copy_rect) {
            style.text_color
        } else {
            style.muted_text_color
        },
    );
}

fn draw_sliders<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    color: Color,
    model: ColorModel,
    cursor: mouse::Cursor,
    page: PickerPage,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    for channel in SliderChannel::ALL {
        let row = slider_row_rect(bounds, page, channel);
        let track_rect = slider_track_rect(bounds, page, channel);
        let value_rect = slider_value_rect(bounds, page, channel);

        draw_slider_track(renderer, track_rect, style, color, model, channel);

        let knob_x = slider_knob_center_x(track_rect, slider_value(model, color, channel));
        let knob_color = Color { a: 1.0, ..color };
        let knob_bounds = Rectangle {
            x: knob_x - SLIDER_KNOB_RADIUS,
            y: row.center_y() - SLIDER_KNOB_RADIUS,
            width: SLIDER_KNOB_SIZE,
            height: SLIDER_KNOB_SIZE,
        };

        renderer.fill_quad(
            renderer::Quad {
                bounds: knob_bounds,
                border: Border {
                    color: Color::WHITE,
                    width: 2.0,
                    radius: SLIDER_KNOB_RADIUS.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
                    offset: Vector::new(0.0, 1.0),
                    blur_radius: 3.0,
                },
                snap: true,
            },
            knob_color,
        );

        draw_text(
            renderer,
            value_rect,
            &slider_value_label(model, color, channel),
            12.0,
            if cursor.is_over(track_rect) {
                style.text_color
            } else {
                style.slider_value_color
            },
            text::Alignment::Right,
        );
    }
}

fn draw_slider_track<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    color: Color,
    model: ColorModel,
    channel: SliderChannel,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    if channel == SliderChannel::Alpha {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 8.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            Color::WHITE,
        );
        draw_checkerboard(renderer, bounds);
    }

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: style.slider_border,
            shadow: Shadow::default(),
            snap: true,
        },
        slider_background(model, color, channel),
    );
}

fn draw_checkerboard<Renderer>(renderer: &mut Renderer, bounds: Rectangle)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let inset = 1.0;
    let bounds = Rectangle {
        x: bounds.x + inset,
        y: bounds.y + inset,
        width: (bounds.width - inset * 2.0).max(0.0),
        height: (bounds.height - inset * 2.0).max(0.0),
    };
    let cell = 6.0;
    let cols = (bounds.width / cell).ceil() as usize;
    let rows = (bounds.height / cell).ceil() as usize;

    for row in 0..rows {
        for col in 0..cols {
            let color = if (row + col) % 2 == 0 {
                Color::from_rgb8(0xEC, 0xEC, 0xE6)
            } else {
                Color::from_rgb8(0xD8, 0xD8, 0xD1)
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: bounds.x + col as f32 * cell,
                        y: bounds.y + row as f32 * cell,
                        width: (bounds.width - col as f32 * cell).min(cell).max(0.0),
                        height: (bounds.height - row as f32 * cell).min(cell).max(0.0),
                    },
                    border: Border::default(),
                    shadow: Shadow::default(),
                    snap: true,
                },
                color,
            );
        }
    }
}

fn draw_swatches<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    groups: &[SwatchGroup],
    active_group: usize,
    color: Color,
    cursor: mouse::Cursor,
    page: PickerPage,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let label_rect = swatch_group_label_rect(bounds, page);
    let active = &groups[active_group];
    let visible = visible_swatches(active);
    let add_rect = add_swatch_rect(bounds, page, visible.len());

    draw_text_with_font(
        renderer,
        Rectangle {
            x: label_rect.x,
            y: label_rect.y,
            width: label_rect.width - 14.0,
            height: label_rect.height,
        },
        &active.name,
        14.0,
        style.text_color,
        text::Alignment::Left,
        bold_font(),
    );
    draw_text(
        renderer,
        Rectangle {
            x: label_rect.x + label_rect.width - 14.0,
            y: label_rect.y,
            width: 12.0,
            height: label_rect.height,
        },
        "v",
        10.0,
        style.muted_text_color,
        text::Alignment::Center,
    );

    for (index, swatch) in visible.iter().enumerate() {
        let rect = swatch_rect(bounds, page, index);
        let selected = same_color(*swatch, color);
        let hovered = cursor.is_over(rect);

        renderer.fill_quad(
            renderer::Quad {
                bounds: rect,
                border: Border {
                    color: if selected {
                        style.selection_ring
                    } else if hovered {
                        style.text_color
                    } else {
                        style.swatch_border.color
                    },
                    width: if selected { 2.0 } else { 1.0 },
                    radius: SWATCH_RADIUS.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            *swatch,
        );
    }

    renderer.fill_quad(
        renderer::Quad {
            bounds: add_rect,
            border: style.swatch_border,
            shadow: Shadow::default(),
            snap: true,
        },
        if cursor.is_over(add_rect) {
            style.control_hover_background
        } else {
            style.swatch_add_background
        },
    );

    draw_text(
        renderer,
        add_rect,
        "+",
        18.0,
        style.swatch_add_text_color,
        text::Alignment::Center,
    );
}

fn draw_menu<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    style: Style,
    page: PickerPage,
    menu: MenuKind,
    active_model: ColorModel,
    swatch_groups: &[SwatchGroup],
    active_group: usize,
    cursor: mouse::Cursor,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let menu_bounds = menu_rect(bounds, page, menu, swatch_groups.len());
    renderer.fill_quad(
        renderer::Quad {
            bounds: menu_bounds,
            border: style.control_border,
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.14),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            snap: true,
        },
        style.control_background,
    );

    let item_count = match menu {
        MenuKind::Model => ColorModel::ALL.len(),
        MenuKind::SwatchGroup => swatch_groups.len(),
    };

    for index in 0..item_count {
        let item_bounds = menu_item_rect(menu_bounds, index);
        let hovered = cursor.is_over(item_bounds);
        let selected = match menu {
            MenuKind::Model => ColorModel::ALL[index] == active_model,
            MenuKind::SwatchGroup => index == active_group,
        };

        if hovered || selected {
            let highlight = if hovered {
                Color::from_rgba8(0x2E, 0x2D, 0x29, 0.10)
            } else {
                style.control_hover_background
            };
            renderer.fill_quad(
                renderer::Quad {
                    bounds: item_bounds,
                    border: Border {
                        color: if hovered {
                            Color::from_rgba8(0x2E, 0x2D, 0x29, 0.10)
                        } else {
                            Color::TRANSPARENT
                        },
                        width: if hovered { 1.0 } else { 0.0 },
                        radius: 8.0.into(),
                    },
                    shadow: Shadow::default(),
                    snap: true,
                },
                highlight,
            );
        }

        let label = match menu {
            MenuKind::Model => ColorModel::ALL[index].label(),
            MenuKind::SwatchGroup => swatch_groups[index].name.as_str(),
        };

        draw_text_with_font(
            renderer,
            Rectangle {
                x: item_bounds.x + 10.0,
                y: item_bounds.y,
                width: item_bounds.width - 20.0,
                height: item_bounds.height,
            },
            label,
            13.0,
            style.text_color,
            text::Alignment::Left,
            if selected {
                bold_font()
            } else {
                iced::Font::default()
            },
        );
    }
}

fn draw_text<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    content: &str,
    size: f32,
    color: Color,
    align_x: text::Alignment,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    draw_text_with_font(
        renderer,
        bounds,
        content,
        size,
        color,
        align_x,
        iced::Font::default(),
    );
}

fn draw_text_with_font<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    content: &str,
    size: f32,
    color: Color,
    align_x: text::Alignment,
    font: iced::Font,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let anchor = match align_x {
        text::Alignment::Left | text::Alignment::Default | text::Alignment::Justified => {
            Point::new(bounds.x, bounds.y + bounds.height / 2.0)
        }
        text::Alignment::Center => Point::new(
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        ),
        text::Alignment::Right => {
            Point::new(bounds.x + bounds.width, bounds.y + bounds.height / 2.0)
        }
    };

    renderer.fill_text(
        iced::advanced::Text {
            content: content.to_string(),
            bounds: Size::new(bounds.width, bounds.height),
            size: iced::Pixels(size),
            font,
            align_x,
            align_y: Vertical::Center,
            line_height: iced::advanced::text::LineHeight::default(),
            shaping: iced::advanced::text::Shaping::Basic,
            wrapping: iced::widget::text::Wrapping::None,
        },
        anchor,
        color,
        bounds,
    );
}

fn draw_copy_icon<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let stroke = Border {
        color,
        width: 1.0,
        radius: 2.5.into(),
    };

    for rect in [
        Rectangle {
            x: bounds.x + 5.0,
            y: bounds.y + 2.0,
            width: 8.5,
            height: 9.5,
        },
        Rectangle {
            x: bounds.x + 2.5,
            y: bounds.y + 4.5,
            width: 8.5,
            height: 9.5,
        },
    ] {
        renderer.fill_quad(
            renderer::Quad {
                bounds: rect,
                border: stroke,
                shadow: Shadow::default(),
                snap: true,
            },
            Color::TRANSPARENT,
        );
    }
}

fn draw_magnifier_icon<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    // A compact, pixel-stepped eyedropper silhouette that stays readable at small sizes.
    let pixels = [
        Rectangle {
            x: bounds.x + 10.0,
            y: bounds.y + 2.0,
            width: 4.0,
            height: 4.0,
        },
        Rectangle {
            x: bounds.x + 8.0,
            y: bounds.y + 5.0,
            width: 3.0,
            height: 3.0,
        },
        Rectangle {
            x: bounds.x + 6.0,
            y: bounds.y + 8.0,
            width: 3.0,
            height: 3.0,
        },
        Rectangle {
            x: bounds.x + 4.0,
            y: bounds.y + 10.0,
            width: 3.0,
            height: 3.0,
        },
        Rectangle {
            x: bounds.x + 3.0,
            y: bounds.y + 12.0,
            width: 4.0,
            height: 3.0,
        },
        Rectangle {
            x: bounds.x + 2.0,
            y: bounds.y + 15.0,
            width: 2.0,
            height: 2.0,
        },
    ];

    for pixel in pixels {
        renderer.fill_quad(
            renderer::Quad {
                bounds: pixel,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 1.5.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            color,
        );
    }
}

fn draw_picker_page_icon<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let tracks = [
        (
            bounds.y + 7.0,
            bounds.x + 5.0,
            bounds.width - 10.0,
            bounds.x + 9.0,
        ),
        (
            bounds.y + 14.0,
            bounds.x + 5.0,
            bounds.width - 10.0,
            bounds.x + 15.0,
        ),
        (
            bounds.y + 21.0,
            bounds.x + 5.0,
            bounds.width - 10.0,
            bounds.x + 11.0,
        ),
    ];

    for (y, x, width, knob_x) in tracks {
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x,
                    y,
                    width,
                    height: 1.5,
                },
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 1.0.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            color,
        );
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: knob_x,
                    y: y - 2.0,
                    width: 5.0,
                    height: 5.0,
                },
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 2.5.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            color,
        );
    }
}

fn draw_contrast_page_icon<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let pill = Rectangle {
        x: bounds.x + 6.0,
        y: bounds.y + 5.0,
        width: bounds.width - 12.0,
        height: bounds.height - 10.0,
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: pill,
            border: Border {
                color,
                width: 1.0,
                radius: (pill.height / 2.0).into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        Color::WHITE,
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: pill.x,
                y: pill.y,
                width: pill.width / 2.0,
                height: pill.height,
            },
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: (pill.height / 2.0).into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
}

fn contrasting_text_color(color: Color) -> Color {
    if relative_luminance(color) > 0.45 {
        Color::from_rgba(0.0, 0.0, 0.0, 0.78)
    } else {
        Color::WHITE
    }
}

fn bold_font() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Bold,
        ..iced::Font::default()
    }
}

fn slider_knob_center_x(track: Rectangle, value: f32) -> f32 {
    let min = track.x + SLIDER_KNOB_RADIUS;
    let max = track.x + track.width - SLIDER_KNOB_RADIUS;

    if max <= min {
        track.center_x()
    } else {
        min + value.clamp(0.0, 1.0) * (max - min)
    }
}

fn slider_value_from_cursor(track: Rectangle, cursor_x: f32) -> f32 {
    let min = track.x + SLIDER_KNOB_RADIUS;
    let max = track.x + track.width - SLIDER_KNOB_RADIUS;

    if max <= min {
        0.0
    } else {
        ((cursor_x - min) / (max - min)).clamp(0.0, 1.0)
    }
}

fn visible_swatches(group: &SwatchGroup) -> &[Color] {
    &group.colors
}

fn visible_swatch_count(group: &SwatchGroup) -> usize {
    group.colors.len()
}

fn swatch_columns() -> usize {
    (((PANEL_WIDTH - CONTENT_PADDING_X * 2.0) + SWATCH_GAP) / (SWATCH_SIZE + SWATCH_GAP))
        .floor()
        .max(1.0) as usize
}

fn swatch_rows(color_count: usize) -> usize {
    let slots = color_count + 1;
    slots.div_ceil(swatch_columns())
}

fn swatch_grid_height(color_count: usize) -> f32 {
    let rows = swatch_rows(color_count) as f32;
    rows * SWATCH_SIZE + (rows - 1.0).max(0.0) * SWATCH_GAP
}

fn top_section_height(page: PickerPage) -> f32 {
    match page {
        PickerPage::Picker => PREVIEW_HEIGHT,
        PickerPage::Contrast => CONTRAST_SUMMARY_HEIGHT + CONTRAST_WELLS_HEIGHT + 12.0,
    }
}

fn overlay_size(page: PickerPage, color_count: usize) -> Size {
    Size::new(PANEL_WIDTH, overlay_height(page, color_count))
}

fn overlay_height(page: PickerPage, color_count: usize) -> f32 {
    HEADER_HEIGHT
        + top_section_height(page)
        + CONTENT_PADDING_TOP
        + CONTROL_ROW_HEIGHT
        + 18.0
        + (SLIDER_ROW_HEIGHT * 4.0)
        + (SLIDER_GAP * 3.0)
        + 18.0
        + SWATCH_HEADER_HEIGHT
        + SWATCH_TOP_MARGIN
        + swatch_grid_height(color_count)
        + FOOTER_PADDING
}

fn clamp_overlay_position(position: Point, viewport: Size, overlay: Size) -> Point {
    let max_x = (viewport.width - overlay.width).max(0.0);
    let max_y = (viewport.height - overlay.height).max(0.0);
    Point::new(position.x.clamp(0.0, max_x), position.y.clamp(0.0, max_y))
}

fn header_rect(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + 1.0,
        y: bounds.y + 1.0,
        width: (bounds.width - 2.0).max(0.0),
        height: (HEADER_HEIGHT - 1.0).max(0.0),
    }
}

fn preview_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    Rectangle {
        x: bounds.x + 1.0,
        y: bounds.y + HEADER_HEIGHT,
        width: (bounds.width - 2.0).max(0.0),
        height: top_section_height(page),
    }
}

fn contrast_summary_rect(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + 1.0,
        y: bounds.y + HEADER_HEIGHT,
        width: (bounds.width - 2.0).max(0.0),
        height: CONTRAST_SUMMARY_HEIGHT,
    }
}

fn contrast_wells_row_rect(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + CONTENT_PADDING_X,
        y: bounds.y + HEADER_HEIGHT + CONTRAST_SUMMARY_HEIGHT + 12.0,
        width: bounds.width - CONTENT_PADDING_X * 2.0,
        height: CONTRAST_WELLS_HEIGHT,
    }
}

fn content_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let top = bounds.y + HEADER_HEIGHT + top_section_height(page) + CONTENT_PADDING_TOP;
    Rectangle {
        x: bounds.x + CONTENT_PADDING_X,
        y: top,
        width: bounds.width - CONTENT_PADDING_X * 2.0,
        height: bounds.height - (top - bounds.y) - FOOTER_PADDING,
    }
}

fn close_button_rect(header: Rectangle) -> Rectangle {
    Rectangle {
        x: header.x + header.width - 30.0,
        y: header.y + (header.height - 24.0) / 2.0,
        width: 20.0,
        height: 24.0,
    }
}

fn page_button_rect(header: Rectangle, page: PickerPage) -> Rectangle {
    let close = close_button_rect(header);
    let offset = match page {
        PickerPage::Contrast => 1.0,
        PickerPage::Picker => 2.0,
    };

    Rectangle {
        x: close.x - offset * (HEADER_PAGE_BUTTON_SIZE + 8.0),
        y: header.y + (header.height - HEADER_PAGE_BUTTON_SIZE) / 2.0,
        width: HEADER_PAGE_BUTTON_SIZE,
        height: HEADER_PAGE_BUTTON_SIZE,
    }
}

fn contrast_well_rect(bounds: Rectangle, target: ContrastTarget) -> Rectangle {
    let row = contrast_wells_row_rect(bounds);
    let gap = 8.0;
    let swap_w = 28.0;
    let fix_w = 46.0;
    let well_w = (row.width - swap_w - fix_w - gap * 3.0) / 2.0;

    match target {
        ContrastTarget::Foreground => Rectangle {
            x: row.x,
            y: row.y,
            width: well_w,
            height: row.height,
        },
        ContrastTarget::Background => Rectangle {
            x: row.x + well_w + gap + swap_w + gap,
            y: row.y,
            width: well_w,
            height: row.height,
        },
    }
}

fn contrast_well_magnifier_rect(bounds: Rectangle, target: ContrastTarget) -> Rectangle {
    let rect = contrast_well_rect(bounds, target);

    Rectangle {
        x: rect.x + rect.width - 34.0,
        y: rect.y + 2.0,
        width: 30.0,
        height: rect.height - 4.0,
    }
}

fn contrast_swap_rect(bounds: Rectangle) -> Rectangle {
    let fg = contrast_well_rect(bounds, ContrastTarget::Foreground);
    let bg = contrast_well_rect(bounds, ContrastTarget::Background);

    Rectangle {
        x: fg.x + fg.width + 8.0,
        y: fg.y + (fg.height - 24.0) / 2.0,
        width: bg.x - (fg.x + fg.width) - 16.0,
        height: 24.0,
    }
}

fn contrast_fix_rect(bounds: Rectangle) -> Rectangle {
    let bg = contrast_well_rect(bounds, ContrastTarget::Background);
    let row = contrast_wells_row_rect(bounds);

    Rectangle {
        x: bg.x + bg.width + 8.0,
        y: row.y + (row.height - 28.0) / 2.0,
        width: row.x + row.width - (bg.x + bg.width + 8.0),
        height: 28.0,
    }
}

fn control_row_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let content = content_rect(bounds, page);
    Rectangle {
        x: content.x,
        y: content.y,
        width: content.width,
        height: CONTROL_ROW_HEIGHT,
    }
}

fn hex_value_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let row = control_row_rect(bounds, page);
    let model_w = 58.0;
    let copy_w = 28.0;
    Rectangle {
        x: row.x,
        y: row.y,
        width: row.width - model_w - copy_w - CONTROL_GAP * 2.0,
        height: row.height,
    }
}

fn model_button_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let row = control_row_rect(bounds, page);
    let hex = hex_value_rect(bounds, page);
    Rectangle {
        x: hex.x + hex.width + CONTROL_GAP,
        y: row.y,
        width: 58.0,
        height: row.height,
    }
}

fn copy_button_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let model = model_button_rect(bounds, page);
    Rectangle {
        x: model.x + model.width + CONTROL_GAP,
        y: model.y,
        width: 28.0,
        height: model.height,
    }
}

fn sliders_top(bounds: Rectangle, page: PickerPage) -> f32 {
    control_row_rect(bounds, page).y + CONTROL_ROW_HEIGHT + 18.0
}

fn slider_row_rect(bounds: Rectangle, page: PickerPage, channel: SliderChannel) -> Rectangle {
    let content = content_rect(bounds, page);
    Rectangle {
        x: content.x,
        y: sliders_top(bounds, page) + channel.index() as f32 * (SLIDER_ROW_HEIGHT + SLIDER_GAP),
        width: content.width,
        height: SLIDER_ROW_HEIGHT,
    }
}

fn slider_track_rect(bounds: Rectangle, page: PickerPage, channel: SliderChannel) -> Rectangle {
    let row = slider_row_rect(bounds, page, channel);
    Rectangle {
        x: row.x + LABEL_WIDTH,
        y: row.y + (row.height - SLIDER_TRACK_HEIGHT) / 2.0,
        width: row.width - LABEL_WIDTH - TRACK_VALUE_GAP - VALUE_WIDTH,
        height: SLIDER_TRACK_HEIGHT,
    }
}

fn slider_value_rect(bounds: Rectangle, page: PickerPage, channel: SliderChannel) -> Rectangle {
    let row = slider_row_rect(bounds, page, channel);
    Rectangle {
        x: row.x + row.width - VALUE_WIDTH,
        y: row.y,
        width: VALUE_WIDTH,
        height: row.height,
    }
}

fn swatch_group_label_rect(bounds: Rectangle, page: PickerPage) -> Rectangle {
    let content = content_rect(bounds, page);
    let last_slider = slider_row_rect(bounds, page, SliderChannel::Alpha);
    Rectangle {
        x: content.x,
        y: last_slider.y + last_slider.height + 18.0,
        width: 92.0,
        height: SWATCH_HEADER_HEIGHT,
    }
}

fn swatch_row_y(bounds: Rectangle, page: PickerPage) -> f32 {
    swatch_group_label_rect(bounds, page).y + SWATCH_HEADER_HEIGHT + SWATCH_TOP_MARGIN
}

fn swatch_rect(bounds: Rectangle, page: PickerPage, index: usize) -> Rectangle {
    let content = content_rect(bounds, page);
    let columns = swatch_columns();
    let column = index % columns;
    let row = index / columns;

    Rectangle {
        x: content.x + column as f32 * (SWATCH_SIZE + SWATCH_GAP),
        y: swatch_row_y(bounds, page) + row as f32 * (SWATCH_SIZE + SWATCH_GAP),
        width: SWATCH_SIZE,
        height: SWATCH_SIZE,
    }
}

fn add_swatch_rect(bounds: Rectangle, page: PickerPage, visible_count: usize) -> Rectangle {
    swatch_rect(bounds, page, visible_count)
}

fn menu_rect(
    bounds: Rectangle,
    page: PickerPage,
    menu: MenuKind,
    swatch_group_count: usize,
) -> Rectangle {
    let item_height = 28.0;
    match menu {
        MenuKind::Model => {
            let trigger = model_button_rect(bounds, page);
            let width = 92.0;
            Rectangle {
                x: trigger.x + trigger.width - width - 6.0,
                y: trigger.y + trigger.height + 6.0,
                width,
                height: item_height * ColorModel::ALL.len() as f32 + 8.0,
            }
        }
        MenuKind::SwatchGroup => {
            let trigger = swatch_group_label_rect(bounds, page);
            let height = item_height * swatch_group_count as f32 + 8.0;
            Rectangle {
                x: trigger.x - 10.0,
                y: trigger.y + trigger.height + 6.0,
                width: 126.0,
                height,
            }
        }
    }
}

fn menu_item_rect(menu_bounds: Rectangle, index: usize) -> Rectangle {
    Rectangle {
        x: menu_bounds.x + 4.0,
        y: menu_bounds.y + 4.0 + index as f32 * 28.0,
        width: menu_bounds.width - 8.0,
        height: 24.0,
    }
}

fn contrast_ratio(foreground: Color, background: Color) -> f32 {
    let lighter = relative_luminance(foreground).max(relative_luminance(background));
    let darker = relative_luminance(foreground).min(relative_luminance(background));

    (lighter + 0.05) / (darker + 0.05)
}

fn contrast_grade(ratio: f32) -> ContrastGrade {
    if ratio >= 7.0 {
        ContrastGrade::Aaa
    } else if ratio >= 4.5 {
        ContrastGrade::Aa
    } else {
        ContrastGrade::Fail
    }
}

fn relative_luminance(color: Color) -> f32 {
    fn channel(value: f32) -> f32 {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    (0.2126 * channel(color.r)) + (0.7152 * channel(color.g)) + (0.0722 * channel(color.b))
}

fn best_contrast_fix(current: Color, locked: Color) -> Color {
    if contrast_grade(contrast_ratio(current, locked)) != ContrastGrade::Fail {
        return current;
    }

    let black = Color::BLACK;
    let white = Color::WHITE;
    let black_ratio = contrast_ratio(black, locked);
    let white_ratio = contrast_ratio(white, locked);

    if black_ratio > white_ratio {
        black
    } else {
        white
    }
}

fn slider_background(model: ColorModel, color: Color, channel: SliderChannel) -> Background {
    use iced::gradient::Linear;

    match (model, channel) {
        (ColorModel::Hsl, SliderChannel::Primary) | (ColorModel::Hsv, SliderChannel::Primary) => {
            Linear::new(Degrees(90.0))
                .add_stop(0.0, Color::from_rgb8(0xFF, 0x00, 0x00))
                .add_stop(0.17, Color::from_rgb8(0xFF, 0xFF, 0x00))
                .add_stop(0.33, Color::from_rgb8(0x00, 0xFF, 0x00))
                .add_stop(0.5, Color::from_rgb8(0x00, 0xFF, 0xFF))
                .add_stop(0.67, Color::from_rgb8(0x00, 0x00, 0xFF))
                .add_stop(0.83, Color::from_rgb8(0xFF, 0x00, 0xFF))
                .add_stop(1.0, Color::from_rgb8(0xFF, 0x00, 0x00))
                .into()
        }
        (ColorModel::Hsl, SliderChannel::Secondary) => {
            let (h, _, l) = rgb_to_hsl(color);
            Linear::new(Degrees(90.0))
                .add_stop(0.0, hsl_to_color(h, 0.0, l, 1.0))
                .add_stop(1.0, hsl_to_color(h, 1.0, l, 1.0))
                .into()
        }
        (ColorModel::Hsl, SliderChannel::Tertiary) => {
            let (h, s, _) = rgb_to_hsl(color);
            Linear::new(Degrees(90.0))
                .add_stop(0.0, hsl_to_color(h, s, 0.0, 1.0))
                .add_stop(0.5, hsl_to_color(h, s, 0.5, 1.0))
                .add_stop(1.0, hsl_to_color(h, s, 1.0, 1.0))
                .into()
        }
        (ColorModel::Hsv, SliderChannel::Secondary) => {
            let (h, _, v) = rgb_to_hsv(color);
            Linear::new(Degrees(90.0))
                .add_stop(0.0, hsv_to_color(h, 0.0, v, 1.0))
                .add_stop(1.0, hsv_to_color(h, 1.0, v, 1.0))
                .into()
        }
        (ColorModel::Hsv, SliderChannel::Tertiary) => {
            let (h, s, _) = rgb_to_hsv(color);
            Linear::new(Degrees(90.0))
                .add_stop(0.0, hsv_to_color(h, s, 0.0, 1.0))
                .add_stop(1.0, hsv_to_color(h, s, 1.0, 1.0))
                .into()
        }
        (ColorModel::Rgb, SliderChannel::Primary) => Linear::new(Degrees(90.0))
            .add_stop(0.0, Color::from_rgba(0.0, color.g, color.b, 1.0))
            .add_stop(1.0, Color::from_rgba(1.0, color.g, color.b, 1.0))
            .into(),
        (ColorModel::Rgb, SliderChannel::Secondary) => Linear::new(Degrees(90.0))
            .add_stop(0.0, Color::from_rgba(color.r, 0.0, color.b, 1.0))
            .add_stop(1.0, Color::from_rgba(color.r, 1.0, color.b, 1.0))
            .into(),
        (ColorModel::Rgb, SliderChannel::Tertiary) => Linear::new(Degrees(90.0))
            .add_stop(0.0, Color::from_rgba(color.r, color.g, 0.0, 1.0))
            .add_stop(1.0, Color::from_rgba(color.r, color.g, 1.0, 1.0))
            .into(),
        (_, SliderChannel::Alpha) => Linear::new(Degrees(90.0))
            .add_stop(0.0, Color::from_rgba(color.r, color.g, color.b, 0.0))
            .add_stop(1.0, Color::from_rgba(color.r, color.g, color.b, 1.0))
            .into(),
    }
}

fn slider_value(model: ColorModel, color: Color, channel: SliderChannel) -> f32 {
    match (model, channel) {
        (ColorModel::Hsl, SliderChannel::Primary) => rgb_to_hsl(color).0 / 360.0,
        (ColorModel::Hsl, SliderChannel::Secondary) => rgb_to_hsl(color).1,
        (ColorModel::Hsl, SliderChannel::Tertiary) => rgb_to_hsl(color).2,
        (ColorModel::Hsl, SliderChannel::Alpha) => color.a,
        (ColorModel::Hsv, SliderChannel::Primary) => rgb_to_hsv(color).0 / 360.0,
        (ColorModel::Hsv, SliderChannel::Secondary) => rgb_to_hsv(color).1,
        (ColorModel::Hsv, SliderChannel::Tertiary) => rgb_to_hsv(color).2,
        (ColorModel::Hsv, SliderChannel::Alpha) => color.a,
        (ColorModel::Rgb, SliderChannel::Primary) => color.r,
        (ColorModel::Rgb, SliderChannel::Secondary) => color.g,
        (ColorModel::Rgb, SliderChannel::Tertiary) => color.b,
        (ColorModel::Rgb, SliderChannel::Alpha) => color.a,
    }
}

fn slider_value_label(model: ColorModel, color: Color, channel: SliderChannel) -> String {
    match (model, channel) {
        (ColorModel::Hsl, SliderChannel::Primary) => {
            format!("{}", rgb_to_hsl(color).0.round() as i32)
        }
        (ColorModel::Hsl, SliderChannel::Secondary) => percent_label(rgb_to_hsl(color).1),
        (ColorModel::Hsl, SliderChannel::Tertiary) => percent_label(rgb_to_hsl(color).2),
        (ColorModel::Hsl, SliderChannel::Alpha) => percent_label(color.a),
        (ColorModel::Hsv, SliderChannel::Primary) => {
            format!("{}", rgb_to_hsv(color).0.round() as i32)
        }
        (ColorModel::Hsv, SliderChannel::Secondary) => percent_label(rgb_to_hsv(color).1),
        (ColorModel::Hsv, SliderChannel::Tertiary) => percent_label(rgb_to_hsv(color).2),
        (ColorModel::Hsv, SliderChannel::Alpha) => percent_label(color.a),
        (ColorModel::Rgb, SliderChannel::Primary) => {
            format!("{}", (color.r * 255.0).round() as i32)
        }
        (ColorModel::Rgb, SliderChannel::Secondary) => {
            format!("{}", (color.g * 255.0).round() as i32)
        }
        (ColorModel::Rgb, SliderChannel::Tertiary) => {
            format!("{}", (color.b * 255.0).round() as i32)
        }
        (ColorModel::Rgb, SliderChannel::Alpha) => percent_label(color.a),
    }
}

fn percent_label(value: f32) -> String {
    format!("{}", (value * 100.0).round() as i32)
}

fn color_to_hex(color: Color) -> String {
    let r = (color.r * 255.0).round() as u8;
    let g = (color.g * 255.0).round() as u8;
    let b = (color.b * 255.0).round() as u8;
    let a = (color.a * 255.0).round() as u8;

    if a == 255 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{a:02X}")
    }
}

/// Parses a pasted color string into an [`iced::Color`].
///
/// Supported formats include hex (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA`),
/// `rgb/rgba`, `hsl/hsla`, and `hsv/hsva`/`hsb/hsba`.
pub fn parse_color_string(input: &str) -> Option<Color> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return None;
    }

    parse_hex_color(trimmed).or_else(|| parse_function_color(trimmed))
}

fn parse_hex_color(input: &str) -> Option<Color> {
    let hex = input
        .strip_prefix('#')
        .or_else(|| input.strip_prefix("0x"))
        .or_else(|| input.strip_prefix("0X"))
        .unwrap_or(input)
        .trim();

    let expanded = match hex.len() {
        3 => {
            let mut out = String::with_capacity(6);
            for ch in hex.chars() {
                out.push(ch);
                out.push(ch);
            }
            out
        }
        4 => {
            let mut out = String::with_capacity(8);
            for ch in hex.chars() {
                out.push(ch);
                out.push(ch);
            }
            out
        }
        6 | 8 => hex.to_string(),
        _ => return None,
    };

    let value = u32::from_str_radix(&expanded, 16).ok()?;

    let (r, g, b, a) = match expanded.len() {
        6 => (
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
            0xFF,
        ),
        8 => (
            ((value >> 24) & 0xFF) as u8,
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        ),
        _ => return None,
    };

    Some(Color::from_rgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ))
}

fn parse_function_color(input: &str) -> Option<Color> {
    let open = input.find('(')?;
    let close = input.rfind(')')?;

    if close <= open || !input[close + 1..].trim().is_empty() {
        return None;
    }

    let name = input[..open].trim().to_ascii_lowercase();
    let args: Vec<_> = input[open + 1..close]
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    match name.as_str() {
        "rgb" if args.len() == 3 => Some(Color::from_rgba(
            parse_rgb_component(args[0])?,
            parse_rgb_component(args[1])?,
            parse_rgb_component(args[2])?,
            1.0,
        )),
        "rgba" if args.len() == 4 => Some(Color::from_rgba(
            parse_rgb_component(args[0])?,
            parse_rgb_component(args[1])?,
            parse_rgb_component(args[2])?,
            parse_alpha_component(args[3])?,
        )),
        "hsl" if args.len() == 3 => Some(hsl_to_color(
            parse_hue_component(args[0])?,
            parse_percentage_component(args[1])?,
            parse_percentage_component(args[2])?,
            1.0,
        )),
        "hsla" if args.len() == 4 => Some(hsl_to_color(
            parse_hue_component(args[0])?,
            parse_percentage_component(args[1])?,
            parse_percentage_component(args[2])?,
            parse_alpha_component(args[3])?,
        )),
        "hsv" | "hsb" if args.len() == 3 => Some(hsv_to_color(
            parse_hue_component(args[0])?,
            parse_percentage_component(args[1])?,
            parse_percentage_component(args[2])?,
            1.0,
        )),
        "hsva" | "hsba" if args.len() == 4 => Some(hsv_to_color(
            parse_hue_component(args[0])?,
            parse_percentage_component(args[1])?,
            parse_percentage_component(args[2])?,
            parse_alpha_component(args[3])?,
        )),
        _ => None,
    }
}

fn parse_rgb_component(input: &str) -> Option<f32> {
    if let Some(percent) = input.strip_suffix('%') {
        return parse_clamped(percent, 0.0, 100.0).map(|value| value / 100.0);
    }

    parse_clamped(input, 0.0, 255.0).map(|value| value / 255.0)
}

fn parse_percentage_component(input: &str) -> Option<f32> {
    if let Some(percent) = input.strip_suffix('%') {
        return parse_clamped(percent, 0.0, 100.0).map(|value| value / 100.0);
    }

    parse_clamped(input, 0.0, 1.0)
}

fn parse_alpha_component(input: &str) -> Option<f32> {
    if let Some(percent) = input.strip_suffix('%') {
        return parse_clamped(percent, 0.0, 100.0).map(|value| value / 100.0);
    }

    parse_clamped(input, 0.0, 1.0)
}

fn parse_hue_component(input: &str) -> Option<f32> {
    let normalized = input
        .strip_suffix("deg")
        .or_else(|| input.strip_suffix("DEG"))
        .unwrap_or(input);

    Some(normalized.trim().parse::<f32>().ok()?.rem_euclid(360.0))
}

fn parse_clamped(input: &str, min: f32, max: f32) -> Option<f32> {
    let value = input.trim().parse::<f32>().ok()?;

    if !(min..=max).contains(&value) {
        return None;
    }

    Some(value)
}

fn format_model_value(model: ColorModel, color: Color) -> String {
    match model {
        ColorModel::Hsl => {
            let (h, s, l) = rgb_to_hsl(color);
            if (color.a - 1.0).abs() < f32::EPSILON {
                format!(
                    "hsl({}, {}%, {}%)",
                    h.round() as i32,
                    (s * 100.0).round() as i32,
                    (l * 100.0).round() as i32
                )
            } else {
                format!(
                    "hsla({}, {}%, {}%, {})",
                    h.round() as i32,
                    (s * 100.0).round() as i32,
                    (l * 100.0).round() as i32,
                    trim_float(color.a)
                )
            }
        }
        ColorModel::Hsv => {
            let (h, s, v) = rgb_to_hsv(color);
            if (color.a - 1.0).abs() < f32::EPSILON {
                format!(
                    "hsv({}, {}%, {}%)",
                    h.round() as i32,
                    (s * 100.0).round() as i32,
                    (v * 100.0).round() as i32
                )
            } else {
                format!(
                    "hsva({}, {}%, {}%, {})",
                    h.round() as i32,
                    (s * 100.0).round() as i32,
                    (v * 100.0).round() as i32,
                    trim_float(color.a)
                )
            }
        }
        ColorModel::Rgb => {
            let r = (color.r * 255.0).round() as i32;
            let g = (color.g * 255.0).round() as i32;
            let b = (color.b * 255.0).round() as i32;
            if (color.a - 1.0).abs() < f32::EPSILON {
                format!("rgb({r}, {g}, {b})")
            } else {
                format!("rgba({r}, {g}, {b}, {})", trim_float(color.a))
            }
        }
    }
}

fn trim_float(value: f32) -> String {
    let s = format!("{value:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn normalize_swatch_groups(mut swatches: Vec<SwatchGroup>) -> Vec<SwatchGroup> {
    if swatches.is_empty() {
        return default_swatch_groups();
    }

    for group in &mut swatches {
        if group.colors.is_empty() {
            group.colors = default_swatch_groups()[0].colors.clone();
        }
    }

    swatches
}

fn default_swatch_groups() -> Vec<SwatchGroup> {
    vec![
        SwatchGroup::new(
            "Grass",
            vec![
                Color::from_rgb8(0x8F, 0x88, 0x5F),
                Color::from_rgb8(0xEB, 0xE6, 0xD1),
                Color::from_rgb8(0x9C, 0xAA, 0x33),
                Color::from_rgb8(0x3F, 0x49, 0x27),
                Color::from_rgb8(0x0B, 0x0A, 0x07),
            ],
        ),
        SwatchGroup::new(
            "Sunset",
            vec![
                Color::from_rgb8(0xF7, 0xC2, 0x8F),
                Color::from_rgb8(0xF6, 0x8D, 0x74),
                Color::from_rgb8(0xD9, 0x53, 0x4F),
                Color::from_rgb8(0x74, 0x35, 0x43),
                Color::from_rgb8(0x2B, 0x1C, 0x2F),
            ],
        ),
        SwatchGroup::new(
            "Ocean",
            vec![
                Color::from_rgb8(0xCC, 0xE9, 0xF2),
                Color::from_rgb8(0x74, 0xC2, 0xE1),
                Color::from_rgb8(0x2C, 0x7D, 0xB8),
                Color::from_rgb8(0x24, 0x4B, 0x73),
                Color::from_rgb8(0x12, 0x1F, 0x34),
            ],
        ),
    ]
}

fn rgb_to_hsl(color: Color) -> (f32, f32, f32) {
    let r = color.r;
    let g = color.g;
    let b = color.b;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let l = (max + min) / 2.0;

    if delta.abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }

    let s = delta / (1.0 - (2.0 * l - 1.0).abs());
    let h = if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

fn hsl_to_color(h: f32, s: f32, l: f32, alpha: f32) -> Color {
    let h = h.rem_euclid(360.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0).rem_euclid(2.0)) - 1.0).abs());
    let m = l - c / 2.0;

    let (r1, g1, b1) = if h < 60.0 {
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

    Color::from_rgba(
        (r1 + m).clamp(0.0, 1.0),
        (g1 + m).clamp(0.0, 1.0),
        (b1 + m).clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    )
}

fn rgb_to_hsv(color: Color) -> (f32, f32, f32) {
    let r = color.r;
    let g = color.g;
    let b = color.b;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta.abs() < f32::EPSILON {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        60.0 * (((g - b) / delta).rem_euclid(6.0))
    } else if (max - g).abs() < f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let s = if max.abs() < f32::EPSILON {
        0.0
    } else {
        delta / max
    };

    (h.rem_euclid(360.0), s.clamp(0.0, 1.0), max.clamp(0.0, 1.0))
}

fn hsv_to_color(h: f32, s: f32, v: f32, alpha: f32) -> Color {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - (((h / 60.0).rem_euclid(2.0)) - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
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

    Color::from_rgba(
        (r1 + m).clamp(0.0, 1.0),
        (g1 + m).clamp(0.0, 1.0),
        (b1 + m).clamp(0.0, 1.0),
        alpha.clamp(0.0, 1.0),
    )
}

fn same_color(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 0.002
        && (a.g - b.g).abs() < 0.002
        && (a.b - b.b).abs() < 0.002
        && (a.a - b.a).abs() < 0.002
}

/// Interaction status for style queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Hovered,
}

/// Visual style for the picker.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub background: Color,
    pub border: Border,
    pub shadow: Shadow,
    pub header_background: Color,
    pub header_divider: Color,
    pub text_color: Color,
    pub muted_text_color: Color,
    pub control_background: Color,
    pub control_hover_background: Color,
    pub control_border: Border,
    pub preview_border: Border,
    pub slider_border: Border,
    pub slider_value_color: Color,
    pub swatch_border: Border,
    pub swatch_add_background: Color,
    pub swatch_add_text_color: Color,
    pub selection_ring: Color,
}

/// Theme catalog trait for the picker.
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

/// Default style inspired by the provided ColorSlurp UI.
pub fn default_style(theme: &iced::Theme, _status: Status) -> Style {
    let accent = theme.extended_palette().primary.base.color;

    Style {
        background: Color::from_rgb8(0xF7, 0xF6, 0xF3),
        border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            width: 1.0,
            radius: 14.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        header_background: Color::from_rgb8(0xF4, 0xF3, 0xEF),
        header_divider: Color::from_rgba(0.0, 0.0, 0.0, 0.08),
        text_color: Color::from_rgb8(0x2E, 0x2D, 0x29),
        muted_text_color: Color::from_rgba8(0x2E, 0x2D, 0x29, 0.58),
        control_background: Color::from_rgb8(0xFE, 0xFE, 0xFC),
        control_hover_background: Color::from_rgb8(0xEF, 0xEE, 0xE9),
        control_border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            width: 1.0,
            radius: 10.0.into(),
        },
        preview_border: Border::default(),
        slider_border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.10),
            width: 1.0,
            radius: 8.0.into(),
        },
        slider_value_color: Color::from_rgba8(0x2E, 0x2D, 0x29, 0.74),
        swatch_border: Border {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.12),
            width: 1.0,
            radius: SWATCH_RADIUS.into(),
        },
        swatch_add_background: Color::from_rgb8(0xFC, 0xFC, 0xFA),
        swatch_add_text_color: Color::from_rgba8(0x2E, 0x2D, 0x29, 0.65),
        selection_ring: accent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_color_close(actual: Color, expected: Color) {
        assert!((actual.r - expected.r).abs() < 0.01, "red differs");
        assert!((actual.g - expected.g).abs() < 0.01, "green differs");
        assert!((actual.b - expected.b).abs() < 0.01, "blue differs");
        assert!((actual.a - expected.a).abs() < 0.01, "alpha differs");
    }

    #[test]
    fn hsl_round_trip_preserves_color() {
        let original = Color::from_rgba(0.61, 0.66, 0.20, 0.75);
        let (h, s, l) = rgb_to_hsl(original);
        let rebuilt = hsl_to_color(h, s, l, original.a);

        assert_color_close(rebuilt, original);
    }

    #[test]
    fn hsv_round_trip_preserves_color() {
        let original = Color::from_rgba(0.21, 0.52, 0.78, 0.42);
        let (h, s, v) = rgb_to_hsv(original);
        let rebuilt = hsv_to_color(h, s, v, original.a);

        assert_color_close(rebuilt, original);
    }

    #[test]
    fn color_info_formats_hex_without_alpha_when_opaque() {
        let info = ColorInfo::new(Color::from_rgb8(0x9C, 0xAA, 0x33), ColorModel::Hsl);
        assert_eq!(info.hex, "#9CAA33");
        assert_eq!(info.formatted, "hsl(67, 54%, 43%)");
    }

    #[test]
    fn color_info_formats_alpha_when_needed() {
        let info = ColorInfo::new(Color::from_rgba8(0x9C, 0xAA, 0x33, 0.5), ColorModel::Rgb);

        assert_eq!(info.hex, "#9CAA3380");
        assert_eq!(info.formatted, "rgba(156, 170, 51, 0.5)");
    }

    #[test]
    fn parse_color_string_supports_hex_and_rgb_forms() {
        let hex = parse_color_string("#9CAA33").expect("hex should parse");
        let bare = parse_color_string("9CAA33").expect("bare hex should parse");
        let rgb = parse_color_string("rgb(156, 170, 51)").expect("rgb should parse");
        let rgba = parse_color_string("rgba(156, 170, 51, 0.5)").expect("rgba should parse");

        assert_color_close(hex, Color::from_rgb8(0x9C, 0xAA, 0x33));
        assert_color_close(bare, Color::from_rgb8(0x9C, 0xAA, 0x33));
        assert_color_close(rgb, Color::from_rgb8(0x9C, 0xAA, 0x33));
        assert_color_close(rgba, Color::from_rgba8(0x9C, 0xAA, 0x33, 0.5));
    }

    #[test]
    fn parse_color_string_supports_hsl_and_hsb_aliases() {
        let hsl = parse_color_string("hsl(67, 54%, 43%)").expect("hsl output should round-trip");
        let hsla =
            parse_color_string("hsla(67, 54%, 43%, 0.5)").expect("hsla output should round-trip");
        let hsb = parse_color_string("hsb(67, 70%, 67%)").expect("hsb alias should parse");

        assert_color_close(hsl, Color::from_rgb8(0x9C, 0xAA, 0x33));
        assert_color_close(hsla, Color::from_rgba8(0x9C, 0xAA, 0x33, 0.5));
        assert!(hsb.a > 0.99);
    }

    #[test]
    fn parse_color_string_rejects_invalid_input() {
        assert!(parse_color_string("not-a-color").is_none());
        assert!(parse_color_string("rgb(400, 0, 0)").is_none());
        assert!(parse_color_string("#12").is_none());
    }

    #[test]
    fn normalizing_empty_swatches_uses_defaults() {
        let groups = normalize_swatch_groups(Vec::new());
        assert!(!groups.is_empty());
        assert_eq!(groups[0].name, "Grass");
    }

    #[test]
    fn contrast_ratio_matches_black_white_reference() {
        let ratio = contrast_ratio(Color::WHITE, Color::BLACK);

        assert!((ratio - 21.0).abs() < 0.01);
        assert_eq!(contrast_grade(ratio), ContrastGrade::Aaa);
    }

    #[test]
    fn contrast_fix_chooses_high_contrast_endpoint() {
        let fixed = best_contrast_fix(
            Color::from_rgb8(0x77, 0x77, 0x77),
            Color::from_rgb8(0x88, 0x88, 0x88),
        );

        assert!(
            same_color(fixed, Color::BLACK) || same_color(fixed, Color::WHITE),
            "expected black or white fix"
        );
    }
}
