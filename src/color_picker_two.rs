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
    Task, Vector,
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

/// Errors that can occur while running the native magnifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MagnifierError {
    Cancelled,
    PermissionDenied,
    UnsupportedPlatform,
    CaptureUnavailable(&'static str),
    PlatformFailure(String),
    WorkerClosed,
}

impl std::fmt::Display for MagnifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("magnifier cancelled"),
            Self::PermissionDenied => f.write_str("screen capture permission denied"),
            Self::UnsupportedPlatform => {
                f.write_str("native magnifier is only implemented on Windows and macOS")
            }
            Self::CaptureUnavailable(reason) => write!(f, "screen capture unavailable: {reason}"),
            Self::PlatformFailure(reason) => write!(f, "native magnifier failed: {reason}"),
            Self::WorkerClosed => f.write_str("native magnifier worker closed unexpectedly"),
        }
    }
}

impl std::error::Error for MagnifierError {}

/// Returns whether a native magnifier implementation is available for this platform.
pub fn native_magnifier_supported() -> bool {
    cfg!(any(target_os = "windows", target_os = "macos"))
}

/// Launches the native magnifier on a worker thread and resolves with the sampled color.
pub fn pick_color_task() -> Task<Result<Color, MagnifierError>> {
    if !native_magnifier_supported() {
        return Task::done(Err(MagnifierError::UnsupportedPlatform));
    }

    Task::future(async {
        let (sender, receiver) = iced::futures::channel::oneshot::channel();

        std::thread::spawn(move || {
            let _ = sender.send(native_magnifier::pick_color_blocking());
        });

        receiver.await.unwrap_or(Err(MagnifierError::WorkerClosed))
    })
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

    fn sync_live_colors(&mut self, color: Color, background_color: Color) {
        self.foreground_color = color;
        self.background_color = background_color;
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
            state.sync_live_colors(self.color, self.contrast_background);
            if !self.is_open {
                state.page = self.page;
            }
            if state.page == PickerPage::Picker {
                state.contrast_target = ContrastTarget::Foreground;
            }
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
        let tab_background = if active {
            style.control_hover_background
        } else if hovered {
            Color::from_rgba(
                style.control_hover_background.r,
                style.control_hover_background.g,
                style.control_hover_background.b,
                0.55,
            )
        } else {
            style.header_background
        };

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
                tab_background,
            );
        }

        match tab_page {
            PickerPage::Picker => draw_picker_page_icon(renderer, rect, style.text_color),
            PickerPage::Contrast => {
                draw_contrast_page_icon(renderer, rect, style.text_color, tab_background)
            }
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
    draw_swap_icon(
        renderer,
        Rectangle {
            x: swap_rect.x + (swap_rect.width - 18.0) / 2.0,
            y: swap_rect.y + (swap_rect.height - 16.0) / 2.0,
            width: 18.0,
            height: 16.0,
        },
        style.muted_text_color,
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
        width: 1.5,
        radius: 2.0.into(),
    };

    for rect in [
        Rectangle {
            x: bounds.x + 5.5,
            y: bounds.y + 2.5,
            width: 8.0,
            height: 8.0,
        },
        Rectangle {
            x: bounds.x + 2.5,
            y: bounds.y + 5.5,
            width: 8.0,
            height: 8.0,
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
    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: bounds.x + 2.0,
                y: bounds.y + 2.0,
                width: 9.0,
                height: 9.0,
            },
            border: Border {
                color,
                width: 1.5,
                radius: 4.5.into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        Color::TRANSPARENT,
    );

    let handle = [
        Rectangle {
            x: bounds.x + 9.0,
            y: bounds.y + 9.0,
            width: 2.5,
            height: 2.5,
        },
        Rectangle {
            x: bounds.x + 11.2,
            y: bounds.y + 11.2,
            width: 2.5,
            height: 2.5,
        },
        Rectangle {
            x: bounds.x + 13.4,
            y: bounds.y + 13.4,
            width: 2.5,
            height: 2.5,
        },
        Rectangle {
            x: bounds.x + 12.1,
            y: bounds.y + 12.1,
            width: 2.0,
            height: 2.0,
        },
    ];

    for segment in handle {
        renderer.fill_quad(
            renderer::Quad {
                bounds: segment,
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

fn draw_swap_icon<Renderer>(renderer: &mut Renderer, bounds: Rectangle, color: Color)
where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let segments = [
        Rectangle {
            x: bounds.x + 2.0,
            y: bounds.y + 3.0,
            width: 9.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 10.0,
            y: bounds.y + 1.0,
            width: 2.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 12.0,
            y: bounds.y + 3.0,
            width: 2.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 10.0,
            y: bounds.y + 5.0,
            width: 2.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 7.0,
            y: bounds.y + 11.0,
            width: 9.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 5.0,
            y: bounds.y + 9.0,
            width: 2.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 3.0,
            y: bounds.y + 11.0,
            width: 2.0,
            height: 2.0,
        },
        Rectangle {
            x: bounds.x + 5.0,
            y: bounds.y + 13.0,
            width: 2.0,
            height: 2.0,
        },
    ];

    for segment in segments {
        renderer.fill_quad(
            renderer::Quad {
                bounds: segment,
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

fn draw_contrast_page_icon<Renderer>(
    renderer: &mut Renderer,
    bounds: Rectangle,
    color: Color,
    background: Color,
) where
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    let diameter = (bounds.width.min(bounds.height) - 10.0).max(12.0);
    let circle = Rectangle {
        x: bounds.x + (bounds.width - diameter) / 2.0,
        y: bounds.y + (bounds.height - diameter) / 2.0,
        width: diameter,
        height: diameter,
    };
    let inner = Rectangle {
        x: circle.x + 2.2,
        y: circle.y + 2.2,
        width: (circle.width - 4.4).max(0.0),
        height: (circle.height - 4.4).max(0.0),
    };

    renderer.fill_quad(
        renderer::Quad {
            bounds: inner,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: (inner.height / 2.0).into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: inner.x + inner.width / 2.0,
                y: inner.y,
                width: inner.width / 2.0,
                height: inner.height,
            },
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: (inner.height / 2.0).into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        background,
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x: circle.x + (circle.width / 2.0) - 0.6,
                y: inner.y + 0.8,
                width: 1.2,
                height: (inner.height - 1.6).max(0.0),
            },
            border: Border::default(),
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
    renderer.fill_quad(
        renderer::Quad {
            bounds: circle,
            border: Border {
                color,
                width: 1.3,
                radius: (circle.height / 2.0).into(),
            },
            shadow: Shadow::default(),
            snap: true,
        },
        Color::TRANSPARENT,
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

mod native_magnifier {
    use super::MagnifierError;
    use iced::Color;
    #[cfg(target_os = "macos")]
    use std::time::Duration;

    #[cfg(target_os = "macos")]
    const POLL_INTERVAL: Duration = Duration::from_millis(8);

    pub(super) fn pick_color_blocking() -> Result<Color, MagnifierError> {
        platform::pick_color_blocking()
    }

    #[cfg(target_os = "macos")]
    fn wait_for_pick(
        is_left_down: impl Fn() -> bool,
        is_cancelled: impl Fn() -> bool,
        sample: impl Fn() -> Result<Color, MagnifierError>,
    ) -> Result<Color, MagnifierError> {
        let mut armed = !is_left_down();
        let mut was_down = is_left_down();

        loop {
            if is_cancelled() {
                return Err(MagnifierError::Cancelled);
            }

            let is_down = is_left_down();

            if !is_down {
                armed = true;
            } else if armed && !was_down {
                return sample();
            }

            was_down = is_down;
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    #[cfg(target_os = "windows")]
    mod platform {
        use super::MagnifierError;
        use crate::color_picker_two::color_to_hex;
        use iced::Color;
        use std::ffi::c_void;
        use std::mem::MaybeUninit;
        use std::ptr::{null, null_mut};
        use std::sync::atomic::{AtomicI32, Ordering};
        use std::time::Duration;

        type Bool = i32;
        type Hbitmap = *mut c_void;
        type Hbrush = *mut c_void;
        type Hdc = *mut c_void;
        type Hfont = *mut c_void;
        type HgdObj = *mut c_void;
        type Hhook = *mut c_void;
        type Hinstance = *mut c_void;
        type Hmodule = *mut c_void;
        type Hwnd = *mut c_void;
        type Hpen = *mut c_void;
        type Hrgn = *mut c_void;
        type Lparam = isize;
        type Lresult = isize;
        type Uint = u32;
        type Wparam = usize;

        #[repr(C)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Point {
            x: i32,
            y: i32,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct Rect {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct Msg {
            hwnd: Hwnd,
            message: Uint,
            w_param: Wparam,
            l_param: Lparam,
            time: u32,
            pt: Point,
            l_private: u32,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct MouseLowLevelHookStruct {
            pt: Point,
            mouse_data: u32,
            flags: u32,
            time: u32,
            dw_extra_info: usize,
        }

        #[repr(C)]
        struct WndClassW {
            style: u32,
            wnd_proc: Option<unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult>,
            cls_extra: i32,
            wnd_extra: i32,
            instance: Hinstance,
            icon: *mut c_void,
            cursor: *mut c_void,
            background: Hbrush,
            menu_name: *const u16,
            class_name: *const u16,
        }

        const VK_ESCAPE: i32 = 0x1B;
        const VK_LBUTTON: i32 = 0x01;
        const SW_HIDE: i32 = 0;
        const SW_SHOWNOACTIVATE: i32 = 4;
        const WS_POPUP: u32 = 0x8000_0000;
        const WS_EX_LAYERED: u32 = 0x0008_0000;
        const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
        const WS_EX_TOPMOST: u32 = 0x0000_0008;
        const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
        const LWA_COLORKEY: u32 = 0x0000_0001;
        const PM_REMOVE: u32 = 0x0001;
        const WH_MOUSE_LL: i32 = 14;
        const WM_ERASEBKGND: u32 = 0x0014;
        const WM_MOUSEWHEEL: u32 = 0x020A;
        const WM_NCHITTEST: u32 = 0x0084;
        const WM_QUIT: u32 = 0x0012;
        const TRANSPARENT_BKMODE: i32 = 1;
        const PS_SOLID: i32 = 0;
        const DEFAULT_CHARSET: u32 = 1;
        const OUT_DEFAULT_PRECIS: u32 = 0;
        const CLIP_DEFAULT_PRECIS: u32 = 0;
        const CLEARTYPE_QUALITY: u32 = 5;
        const DEFAULT_PITCH: u32 = 0;
        const FF_DONTCARE: u32 = 0;
        const HWND_TOPMOST: Hwnd = -1isize as Hwnd;
        const SWP_NOACTIVATE: u32 = 0x0010;
        const SM_XVIRTUALSCREEN: i32 = 76;
        const SM_YVIRTUALSCREEN: i32 = 77;
        const SM_CXVIRTUALSCREEN: i32 = 78;
        const SM_CYVIRTUALSCREEN: i32 = 79;
        const FRAME_INTERVAL: Duration = Duration::from_millis(16);

        const WINDOW_WIDTH: i32 = 214;
        const WINDOW_HEIGHT: i32 = 230;
        const LOUPE_DIAMETER: i32 = 166;
        const LOUPE_TOP: i32 = 8;
        const LOUPE_LEFT: i32 = (WINDOW_WIDTH - LOUPE_DIAMETER) / 2;
        const TEXT_TOP: i32 = LOUPE_TOP + LOUPE_DIAMETER + 14;
        const TEXT_HEIGHT: i32 = 28;
        const MIN_ZOOM: i32 = 1;
        const MAX_ZOOM: i32 = 64;
        const DEFAULT_ZOOM: i32 = 10;
        const TRANSPARENT_KEY: u32 = 0x00FF_00FF;
        const LABEL_WIDTH: i32 = 110;
        const TARGET_SIZE: i32 = 10;
        const GRID_THRESHOLD: f32 = 12.0;
        const SRCCOPY: u32 = 0x00CC_0020;
        const CAPTUREBLT: u32 = 0x4000_0000;
        const BI_RGB: u32 = 0;
        const DIB_RGB_COLORS: u32 = 0;
        const COLORONCOLOR: i32 = 3;
        const HTTRANSPARENT: isize = -1;
        const WDA_EXCLUDEFROMCAPTURE: u32 = 17;
        const CURSOR_VISIBILITY_ATTEMPTS: i32 = 32;
        const CAPTURE_BITMAP_SIZE: i32 = LOUPE_DIAMETER + 1;

        static WHEEL_DELTA_ACCUM: AtomicI32 = AtomicI32::new(0);

        unsafe extern "system" fn magnifier_window_proc(
            window: Hwnd,
            message: Uint,
            w_param: Wparam,
            l_param: Lparam,
        ) -> Lresult {
            match message {
                WM_ERASEBKGND => 1,
                WM_NCHITTEST => HTTRANSPARENT,
                _ => unsafe { DefWindowProcW(window, message, w_param, l_param) },
            }
        }

        unsafe extern "system" {
            fn GetAsyncKeyState(vkey: i32) -> i16;
            fn GetCursorPos(point: *mut Point) -> Bool;
            fn GetPhysicalCursorPos(point: *mut Point) -> Bool;
            fn GetDC(window: Hwnd) -> Hdc;
            fn ReleaseDC(window: Hwnd, dc: Hdc) -> i32;
            fn PeekMessageW(
                msg: *mut Msg,
                window: Hwnd,
                min_filter: u32,
                max_filter: u32,
                remove_msg: u32,
            ) -> Bool;
            fn TranslateMessage(msg: *const Msg) -> Bool;
            fn DispatchMessageW(msg: *const Msg) -> Lresult;
            fn CreateWindowExW(
                ex_style: u32,
                class_name: *const u16,
                window_name: *const u16,
                style: u32,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                parent: Hwnd,
                menu: *mut c_void,
                instance: Hinstance,
                param: *mut c_void,
            ) -> Hwnd;
            fn DestroyWindow(window: Hwnd) -> Bool;
            fn ShowWindow(window: Hwnd, cmd_show: i32) -> Bool;
            fn SetWindowPos(
                window: Hwnd,
                insert_after: Hwnd,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                flags: u32,
            ) -> Bool;
            fn SetLayeredWindowAttributes(
                window: Hwnd,
                color_key: u32,
                alpha: u8,
                flags: u32,
            ) -> Bool;
            fn SetWindowRgn(window: Hwnd, region: Hrgn, redraw: Bool) -> i32;
            fn RegisterClassW(class: *const WndClassW) -> u16;
            fn DefWindowProcW(
                window: Hwnd,
                message: Uint,
                w_param: Wparam,
                l_param: Lparam,
            ) -> Lresult;
            fn GetModuleHandleW(module_name: *const u16) -> Hmodule;
            fn SetWindowsHookExW(
                id_hook: i32,
                hook_proc: Option<unsafe extern "system" fn(i32, Wparam, Lparam) -> Lresult>,
                module: Hinstance,
                thread_id: u32,
            ) -> Hhook;
            fn UnhookWindowsHookEx(hook: Hhook) -> Bool;
            fn CallNextHookEx(hook: Hhook, code: i32, w_param: Wparam, l_param: Lparam) -> Lresult;
            fn GetSystemMetrics(index: i32) -> i32;
            fn CreateSolidBrush(color: u32) -> Hbrush;
            fn DeleteObject(object: HgdObj) -> Bool;
            fn FillRect(dc: Hdc, rect: *const Rect, brush: Hbrush) -> i32;
            fn CreatePen(style: i32, width: i32, color: u32) -> Hpen;
            fn SelectObject(dc: Hdc, object: HgdObj) -> HgdObj;
            fn Ellipse(dc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> Bool;
            fn Rectangle(dc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> Bool;
            fn CreateEllipticRgn(left: i32, top: i32, right: i32, bottom: i32) -> Hrgn;
            fn SetBkMode(dc: Hdc, mode: i32) -> i32;
            fn SetTextColor(dc: Hdc, color: u32) -> u32;
            fn TextOutW(dc: Hdc, x: i32, y: i32, string: *const u16, count: i32) -> Bool;
            fn CreateFontW(
                height: i32,
                width: i32,
                escapement: i32,
                orientation: i32,
                weight: i32,
                italic: u32,
                underline: u32,
                strike_out: u32,
                char_set: u32,
                output_precision: u32,
                clip_precision: u32,
                quality: u32,
                pitch_and_family: u32,
                face_name: *const u16,
            ) -> Hfont;
            fn RoundRect(
                dc: Hdc,
                left: i32,
                top: i32,
                right: i32,
                bottom: i32,
                width: i32,
                height: i32,
            ) -> Bool;
        }

        unsafe extern "system" {
            fn SetWindowDisplayAffinity(window: Hwnd, affinity: u32) -> Bool;
            fn ShowCursor(show: Bool) -> i32;
            fn CreateCompatibleDC(dc: Hdc) -> Hdc;
            fn DeleteDC(dc: Hdc) -> Bool;
            fn CreateDIBSection(
                dc: Hdc,
                info: *const BitmapInfo,
                usage: Uint,
                bits: *mut *mut c_void,
                section: *mut c_void,
                offset: u32,
            ) -> Hbitmap;
            fn BitBlt(
                dc: Hdc,
                x: i32,
                y: i32,
                width: i32,
                height: i32,
                source_dc: Hdc,
                source_x: i32,
                source_y: i32,
                rop: u32,
            ) -> Bool;
            fn StretchDIBits(
                dc: Hdc,
                dest_x: i32,
                dest_y: i32,
                dest_width: i32,
                dest_height: i32,
                source_x: i32,
                source_y: i32,
                source_width: i32,
                source_height: i32,
                bits: *const c_void,
                info: *const BitmapInfo,
                usage: Uint,
                rop: u32,
            ) -> i32;
            fn SetStretchBltMode(dc: Hdc, mode: i32) -> i32;
            fn MoveToEx(dc: Hdc, x: i32, y: i32, previous: *mut Point) -> Bool;
            fn LineTo(dc: Hdc, x: i32, y: i32) -> Bool;
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy, Default)]
        struct BitmapInfoHeader {
            size: u32,
            width: i32,
            height: i32,
            planes: u16,
            bit_count: u16,
            compression: u32,
            size_image: u32,
            x_pels_per_meter: i32,
            y_pels_per_meter: i32,
            clr_used: u32,
            clr_important: u32,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy, Default)]
        struct RgbQuad {
            blue: u8,
            green: u8,
            red: u8,
            reserved: u8,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct BitmapInfo {
            header: BitmapInfoHeader,
            colors: [RgbQuad; 1],
        }

        #[derive(Debug, Clone, Copy)]
        struct CaptureFrame {
            span: i32,
            cursor_offset: Point,
            color: Color,
        }

        pub(super) fn pick_color_blocking() -> Result<Color, MagnifierError> {
            WHEEL_DELTA_ACCUM.store(0, Ordering::Relaxed);

            let _cursor_guard = SystemCursorGuard::hide()?;
            let hook = install_mouse_hook()?;
            let _hook_guard = HookGuard(hook);
            let mut window = NativeMagnifierWindow::create()?;

            let mut armed = !left_button_down();
            let mut was_down = left_button_down();
            let mut zoom = DEFAULT_ZOOM;
            let mut last_cursor = None;
            let mut last_zoom = zoom;
            let mut last_color = None;

            loop {
                pump_messages()?;

                if escape_down() {
                    return Err(MagnifierError::Cancelled);
                }

                let cursor = cursor_position()?;
                let wheel = WHEEL_DELTA_ACCUM.swap(0, Ordering::Relaxed);
                if wheel != 0 {
                    zoom = adjust_zoom(zoom, wheel);
                }

                if last_cursor != Some(cursor) || last_zoom != zoom || last_color.is_none() {
                    let color = window.refresh(cursor, zoom)?;
                    last_cursor = Some(cursor);
                    last_zoom = zoom;
                    last_color = Some(color);
                }

                let is_down = left_button_down();
                if !is_down {
                    armed = true;
                } else if armed && !was_down {
                    return last_color.ok_or_else(|| {
                        MagnifierError::CaptureUnavailable(
                            "no sampled color was available when selection completed",
                        )
                    });
                }

                was_down = is_down;
                std::thread::sleep(FRAME_INTERVAL);
            }
        }

        fn left_button_down() -> bool {
            unsafe { GetAsyncKeyState(VK_LBUTTON) < 0 }
        }

        fn escape_down() -> bool {
            unsafe { GetAsyncKeyState(VK_ESCAPE) < 0 }
        }

        fn cursor_position() -> Result<Point, MagnifierError> {
            let mut point = Point { x: 0, y: 0 };

            let ok = unsafe {
                if GetPhysicalCursorPos(&mut point) != 0 {
                    true
                } else {
                    GetCursorPos(&mut point) != 0
                }
            };

            if ok {
                Ok(point)
            } else {
                Err(MagnifierError::PlatformFailure(
                    "failed to read the current cursor position".to_string(),
                ))
            }
        }

        fn pump_messages() -> Result<(), MagnifierError> {
            loop {
                let mut msg = MaybeUninit::<Msg>::zeroed();
                let has_message =
                    unsafe { PeekMessageW(msg.as_mut_ptr(), null_mut(), 0, 0, PM_REMOVE) };

                if has_message == 0 {
                    return Ok(());
                }

                let msg = unsafe { msg.assume_init() };

                if msg.message == WM_QUIT {
                    return Err(MagnifierError::Cancelled);
                }

                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }

        fn adjust_zoom(current: i32, wheel_delta: i32) -> i32 {
            let steps = wheel_delta / 120;

            if steps == 0 {
                current
            } else {
                (current + steps).clamp(MIN_ZOOM, MAX_ZOOM)
            }
        }

        fn pixel_size(span: i32) -> f32 {
            LOUPE_DIAMETER as f32 / span.max(1) as f32
        }

        fn popup_position_for_frame(cursor: Point, frame: CaptureFrame) -> Point {
            let scale = pixel_size(frame.span);
            let selected_center_x =
                LOUPE_LEFT + (((frame.cursor_offset.x as f32) + 0.5) * scale).round() as i32;
            let selected_center_y =
                LOUPE_TOP + (((frame.cursor_offset.y as f32) + 0.5) * scale).round() as i32;

            Point {
                x: cursor.x - selected_center_x,
                y: cursor.y - selected_center_y,
            }
        }

        fn loupe_position(overlay: Point) -> Point {
            Point {
                x: overlay.x + LOUPE_LEFT,
                y: overlay.y + LOUPE_TOP,
            }
        }

        fn selection_rect(frame: CaptureFrame) -> Rect {
            let scale = pixel_size(frame.span);
            let center_x =
                LOUPE_LEFT + (((frame.cursor_offset.x as f32) + 0.5) * scale).round() as i32;
            let center_y =
                LOUPE_TOP + (((frame.cursor_offset.y as f32) + 0.5) * scale).round() as i32;
            let size = if scale >= GRID_THRESHOLD {
                scale.ceil() as i32
            } else {
                TARGET_SIZE
            };

            Rect {
                left: center_x - (size / 2),
                top: center_y - (size / 2),
                right: center_x + ((size + 1) / 2),
                bottom: center_y + ((size + 1) / 2),
            }
        }

        fn source_span(zoom: i32) -> i32 {
            let mut span = ((LOUPE_DIAMETER as f32) / (zoom as f32)).ceil() as i32;
            span = span.max(1);

            if span % 2 == 0 {
                span += 1;
            }

            span
        }

        fn source_rect(cursor: Point, zoom: i32) -> Rect {
            let span = source_span(zoom);
            let virtual_left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let virtual_top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let virtual_right = virtual_left + unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
            let virtual_bottom = virtual_top + unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            let half = span / 2;

            let left = (cursor.x - half).clamp(virtual_left, virtual_right - span);
            let top = (cursor.y - half).clamp(virtual_top, virtual_bottom - span);

            Rect {
                left,
                top,
                right: left + span,
                bottom: top + span,
            }
        }

        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }

        fn encode_wide(text: &str) -> Vec<u16> {
            text.encode_utf16().chain(std::iter::once(0)).collect()
        }

        unsafe extern "system" fn mouse_hook_proc(
            code: i32,
            w_param: Wparam,
            l_param: Lparam,
        ) -> Lresult {
            if code >= 0 && w_param as u32 == WM_MOUSEWHEEL {
                let info = unsafe { &*(l_param as *const MouseLowLevelHookStruct) };
                let delta = ((info.mouse_data >> 16) as i16) as i32;
                WHEEL_DELTA_ACCUM.fetch_add(delta, Ordering::Relaxed);
            }

            unsafe { CallNextHookEx(null_mut(), code, w_param, l_param) }
        }

        fn install_mouse_hook() -> Result<Hhook, MagnifierError> {
            let module = unsafe { GetModuleHandleW(null()) };
            let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), module, 0) };

            if hook.is_null() {
                Err(MagnifierError::PlatformFailure(
                    "SetWindowsHookExW failed".to_string(),
                ))
            } else {
                Ok(hook)
            }
        }

        struct HookGuard(Hhook);

        impl Drop for HookGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    let _ = unsafe { UnhookWindowsHookEx(self.0) };
                }
            }
        }

        struct SystemCursorGuard {
            adjustments: i32,
        }

        impl SystemCursorGuard {
            fn hide() -> Result<Self, MagnifierError> {
                let mut adjustments = 0;

                while adjustments < CURSOR_VISIBILITY_ATTEMPTS {
                    adjustments += 1;

                    if unsafe { ShowCursor(0) } < 0 {
                        break;
                    }
                }

                Ok(Self { adjustments })
            }
        }

        impl Drop for SystemCursorGuard {
            fn drop(&mut self) {
                for _ in 0..self.adjustments {
                    let _ = unsafe { ShowCursor(1) };
                }
            }
        }

        struct CaptureSurface {
            screen_dc: Hdc,
            capture_dc: Hdc,
            bitmap: Hbitmap,
            stock_bitmap: HgdObj,
            pixels: *mut u8,
            bitmap_info: BitmapInfo,
            side: i32,
        }

        impl CaptureSurface {
            fn new() -> Result<Self, MagnifierError> {
                let screen_dc = unsafe { GetDC(null_mut()) };

                if screen_dc.is_null() {
                    return Err(MagnifierError::CaptureUnavailable(
                        "GetDC returned a null screen device context",
                    ));
                }

                let capture_dc = unsafe { CreateCompatibleDC(screen_dc) };
                if capture_dc.is_null() {
                    let _ = unsafe { ReleaseDC(null_mut(), screen_dc) };
                    return Err(MagnifierError::PlatformFailure(
                        "CreateCompatibleDC failed".to_string(),
                    ));
                }

                let bitmap_info = Self::bitmap_info_for_side(CAPTURE_BITMAP_SIZE);

                let mut bits = null_mut();
                let bitmap = unsafe {
                    CreateDIBSection(
                        screen_dc,
                        &bitmap_info,
                        DIB_RGB_COLORS,
                        &mut bits,
                        null_mut(),
                        0,
                    )
                };

                if bitmap.is_null() || bits.is_null() {
                    let _ = unsafe { DeleteDC(capture_dc) };
                    let _ = unsafe { ReleaseDC(null_mut(), screen_dc) };
                    return Err(MagnifierError::PlatformFailure(
                        "CreateDIBSection failed".to_string(),
                    ));
                }

                let stock_bitmap = unsafe { SelectObject(capture_dc, bitmap as HgdObj) };
                if stock_bitmap.is_null() {
                    let _ = unsafe { DeleteObject(bitmap as HgdObj) };
                    let _ = unsafe { DeleteDC(capture_dc) };
                    let _ = unsafe { ReleaseDC(null_mut(), screen_dc) };
                    return Err(MagnifierError::PlatformFailure(
                        "SelectObject failed for the capture bitmap".to_string(),
                    ));
                }

                Ok(Self {
                    screen_dc,
                    capture_dc,
                    bitmap,
                    stock_bitmap,
                    pixels: bits as *mut u8,
                    bitmap_info,
                    side: CAPTURE_BITMAP_SIZE,
                })
            }

            fn bitmap_info_for_side(side: i32) -> BitmapInfo {
                BitmapInfo {
                    header: BitmapInfoHeader {
                        size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                        width: side,
                        height: -side,
                        planes: 1,
                        bit_count: 32,
                        compression: BI_RGB,
                        size_image: (side * side * 4) as u32,
                        x_pels_per_meter: 0,
                        y_pels_per_meter: 0,
                        clr_used: 0,
                        clr_important: 0,
                    },
                    colors: [RgbQuad::default()],
                }
            }

            fn resize_if_needed(&mut self, side: i32) -> Result<(), MagnifierError> {
                if self.side == side {
                    return Ok(());
                }

                unsafe {
                    SelectObject(self.capture_dc, self.stock_bitmap);
                }

                if !self.bitmap.is_null() {
                    let _ = unsafe { DeleteObject(self.bitmap as HgdObj) };
                }

                let bitmap_info = Self::bitmap_info_for_side(side);
                let mut bits = null_mut();
                let bitmap = unsafe {
                    CreateDIBSection(
                        self.screen_dc,
                        &bitmap_info,
                        DIB_RGB_COLORS,
                        &mut bits,
                        null_mut(),
                        0,
                    )
                };

                if bitmap.is_null() || bits.is_null() {
                    self.bitmap = null_mut();
                    self.pixels = null_mut();
                    return Err(MagnifierError::PlatformFailure(
                        "CreateDIBSection failed while resizing the capture bitmap".to_string(),
                    ));
                }

                let replaced = unsafe { SelectObject(self.capture_dc, bitmap as HgdObj) };
                if replaced.is_null() {
                    let _ = unsafe { DeleteObject(bitmap as HgdObj) };
                    self.bitmap = null_mut();
                    self.pixels = null_mut();
                    return Err(MagnifierError::PlatformFailure(
                        "SelectObject failed while resizing the capture bitmap".to_string(),
                    ));
                }

                self.bitmap = bitmap;
                self.pixels = bits as *mut u8;
                self.bitmap_info = bitmap_info;
                self.side = side;

                Ok(())
            }

            fn capture(
                &mut self,
                source: Rect,
                cursor: Point,
            ) -> Result<CaptureFrame, MagnifierError> {
                let span = (source.right - source.left).max(1);
                self.resize_if_needed(span)?;
                let copied = unsafe {
                    BitBlt(
                        self.capture_dc,
                        0,
                        0,
                        span,
                        span,
                        self.screen_dc,
                        source.left,
                        source.top,
                        SRCCOPY | CAPTUREBLT,
                    )
                };

                if copied == 0 {
                    return Err(MagnifierError::CaptureUnavailable(
                        "BitBlt failed while copying the screen region",
                    ));
                }

                let cursor_offset = Point {
                    x: (cursor.x - source.left).clamp(0, span - 1),
                    y: (cursor.y - source.top).clamp(0, span - 1),
                };
                let color = self.color_at(cursor_offset)?;

                Ok(CaptureFrame {
                    span,
                    cursor_offset,
                    color,
                })
            }

            fn color_at(&self, point: Point) -> Result<Color, MagnifierError> {
                let stride = self.side as usize * 4;
                let offset = point.y as usize * stride + point.x as usize * 4;
                let pixel = unsafe { self.pixels.add(offset) };

                let b = unsafe { *pixel };
                let g = unsafe { *pixel.add(1) };
                let r = unsafe { *pixel.add(2) };

                Ok(Color::from_rgb8(r, g, b))
            }
        }

        impl Drop for CaptureSurface {
            fn drop(&mut self) {
                if !self.capture_dc.is_null() && !self.stock_bitmap.is_null() {
                    let _ = unsafe { SelectObject(self.capture_dc, self.stock_bitmap) };
                }

                if !self.bitmap.is_null() {
                    let _ = unsafe { DeleteObject(self.bitmap as HgdObj) };
                }

                if !self.capture_dc.is_null() {
                    let _ = unsafe { DeleteDC(self.capture_dc) };
                }

                if !self.screen_dc.is_null() {
                    let _ = unsafe { ReleaseDC(null_mut(), self.screen_dc) };
                }
            }
        }

        struct NativeMagnifierWindow {
            loupe_hwnd: Hwnd,
            overlay_hwnd: Hwnd,
            text_font: Hfont,
            capture: CaptureSurface,
            capture_exclusion_active: bool,
        }

        impl NativeMagnifierWindow {
            fn create() -> Result<Self, MagnifierError> {
                let window_class_name = encode_wide("color_picker_two_magnifier_window");
                let loupe_title = encode_wide("color_picker_two_loupe_window");
                let overlay_title = encode_wide("color_picker_two_magnifier_overlay");
                let face_name = encode_wide("Segoe UI");
                let module = unsafe { GetModuleHandleW(null()) };
                let mut loupe_hwnd = null_mut();
                let mut overlay_hwnd = null_mut();
                let mut text_font = null_mut();

                let build = (|| -> Result<(CaptureSurface, bool), MagnifierError> {
                    let window_class = WndClassW {
                        style: 0,
                        wnd_proc: Some(magnifier_window_proc),
                        cls_extra: 0,
                        wnd_extra: 0,
                        instance: module,
                        icon: null_mut(),
                        cursor: null_mut(),
                        background: null_mut(),
                        menu_name: null(),
                        class_name: window_class_name.as_ptr(),
                    };

                    let _ = unsafe { RegisterClassW(&window_class) };

                    loupe_hwnd = unsafe {
                        CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                            window_class_name.as_ptr(),
                            loupe_title.as_ptr(),
                            WS_POPUP,
                            0,
                            0,
                            LOUPE_DIAMETER,
                            LOUPE_DIAMETER,
                            null_mut(),
                            null_mut(),
                            module,
                            null_mut(),
                        )
                    };

                    if loupe_hwnd.is_null() {
                        return Err(MagnifierError::PlatformFailure(
                            "CreateWindowExW failed for the loupe window".to_string(),
                        ));
                    }

                    let loupe_region = Region::ellipse(Rect {
                        left: 0,
                        top: 0,
                        right: LOUPE_DIAMETER,
                        bottom: LOUPE_DIAMETER,
                    })?;
                    let region = loupe_region.into_raw();

                    if unsafe { SetWindowRgn(loupe_hwnd, region, 1) } == 0 {
                        return Err(MagnifierError::PlatformFailure(
                            "SetWindowRgn failed for the loupe window".to_string(),
                        ));
                    }

                    overlay_hwnd = unsafe {
                        CreateWindowExW(
                            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
                            window_class_name.as_ptr(),
                            overlay_title.as_ptr(),
                            WS_POPUP,
                            0,
                            0,
                            WINDOW_WIDTH,
                            WINDOW_HEIGHT,
                            null_mut(),
                            null_mut(),
                            module,
                            null_mut(),
                        )
                    };

                    if overlay_hwnd.is_null() {
                        return Err(MagnifierError::PlatformFailure(
                            "CreateWindowExW failed for magnifier overlay".to_string(),
                        ));
                    }

                    if unsafe {
                        SetLayeredWindowAttributes(overlay_hwnd, TRANSPARENT_KEY, 0, LWA_COLORKEY)
                    } == 0
                    {
                        return Err(MagnifierError::PlatformFailure(
                            "SetLayeredWindowAttributes failed for magnifier overlay".to_string(),
                        ));
                    }

                    text_font = unsafe {
                        CreateFontW(
                            -18,
                            0,
                            0,
                            0,
                            600,
                            0,
                            0,
                            0,
                            DEFAULT_CHARSET,
                            OUT_DEFAULT_PRECIS,
                            CLIP_DEFAULT_PRECIS,
                            CLEARTYPE_QUALITY,
                            DEFAULT_PITCH | FF_DONTCARE,
                            face_name.as_ptr(),
                        )
                    };

                    let loupe_excluded =
                        unsafe { SetWindowDisplayAffinity(loupe_hwnd, WDA_EXCLUDEFROMCAPTURE) }
                            != 0;
                    let overlay_excluded =
                        unsafe { SetWindowDisplayAffinity(overlay_hwnd, WDA_EXCLUDEFROMCAPTURE) }
                            != 0;

                    Ok((CaptureSurface::new()?, loupe_excluded && overlay_excluded))
                })();

                let (capture, capture_exclusion_active) = match build {
                    Ok(result) => result,
                    Err(error) => {
                        if !overlay_hwnd.is_null() {
                            let _ = unsafe { DestroyWindow(overlay_hwnd) };
                        }
                        if !loupe_hwnd.is_null() {
                            let _ = unsafe { DestroyWindow(loupe_hwnd) };
                        }
                        if !text_font.is_null() {
                            let _ = unsafe { DeleteObject(text_font as HgdObj) };
                        }
                        return Err(error);
                    }
                };

                Ok(Self {
                    loupe_hwnd,
                    overlay_hwnd,
                    text_font,
                    capture,
                    capture_exclusion_active,
                })
            }

            fn refresh(&mut self, cursor: Point, zoom: i32) -> Result<Color, MagnifierError> {
                let source = source_rect(cursor, zoom);
                if !self.capture_exclusion_active {
                    self.hide_windows();
                }
                let frame = self.capture.capture(source, cursor)?;
                let overlay = popup_position_for_frame(cursor, frame);
                let loupe = loupe_position(overlay);

                if self.capture_exclusion_active {
                    self.draw_loupe(frame)?;
                    self.draw_overlay(frame)?;
                    self.move_windows(overlay, loupe)?;
                } else {
                    self.move_windows(overlay, loupe)?;
                    self.draw_loupe(frame)?;
                    self.draw_overlay(frame)?;
                }
                self.show_windows();

                Ok(frame.color)
            }

            fn hide_windows(&self) {
                unsafe {
                    ShowWindow(self.loupe_hwnd, SW_HIDE);
                    ShowWindow(self.overlay_hwnd, SW_HIDE);
                }
            }

            fn show_windows(&self) {
                unsafe {
                    ShowWindow(self.loupe_hwnd, SW_SHOWNOACTIVATE);
                    ShowWindow(self.overlay_hwnd, SW_SHOWNOACTIVATE);
                }
            }

            fn move_windows(&self, overlay: Point, loupe: Point) -> Result<(), MagnifierError> {
                let loupe_ok = unsafe {
                    SetWindowPos(
                        self.loupe_hwnd,
                        HWND_TOPMOST,
                        loupe.x,
                        loupe.y,
                        LOUPE_DIAMETER,
                        LOUPE_DIAMETER,
                        SWP_NOACTIVATE,
                    )
                };

                if loupe_ok == 0 {
                    return Err(MagnifierError::PlatformFailure(
                        "SetWindowPos failed for the loupe window".to_string(),
                    ));
                }

                let overlay_ok = unsafe {
                    SetWindowPos(
                        self.overlay_hwnd,
                        HWND_TOPMOST,
                        overlay.x,
                        overlay.y,
                        WINDOW_WIDTH,
                        WINDOW_HEIGHT,
                        SWP_NOACTIVATE,
                    )
                };

                if overlay_ok == 0 {
                    Err(MagnifierError::PlatformFailure(
                        "SetWindowPos failed for magnifier overlay".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }

            fn draw_loupe(&self, frame: CaptureFrame) -> Result<(), MagnifierError> {
                let dc = unsafe { GetDC(self.loupe_hwnd) };

                if dc.is_null() {
                    return Err(MagnifierError::PlatformFailure(
                        "GetDC failed for the loupe window".to_string(),
                    ));
                }

                let result = self.draw_loupe_into_dc(dc, frame);
                let _ = unsafe { ReleaseDC(self.loupe_hwnd, dc) };
                result
            }

            fn draw_loupe_into_dc(
                &self,
                dc: Hdc,
                frame: CaptureFrame,
            ) -> Result<(), MagnifierError> {
                unsafe {
                    SetStretchBltMode(dc, COLORONCOLOR);
                }

                let drawn = unsafe {
                    StretchDIBits(
                        dc,
                        0,
                        0,
                        LOUPE_DIAMETER,
                        LOUPE_DIAMETER,
                        0,
                        0,
                        frame.span,
                        frame.span,
                        self.capture.pixels as *const c_void,
                        &self.capture.bitmap_info,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    )
                };

                if drawn == 0 {
                    return Err(MagnifierError::CaptureUnavailable(
                        "StretchDIBits failed while rendering the loupe",
                    ));
                }

                self.draw_grid(dc, frame)
            }

            fn draw_grid(&self, dc: Hdc, frame: CaptureFrame) -> Result<(), MagnifierError> {
                let scale = pixel_size(frame.span);
                if scale < GRID_THRESHOLD {
                    return Ok(());
                }

                let grid_pen = Pen::solid(rgb(232, 232, 232), 1)?;
                let previous_pen = unsafe { SelectObject(dc, grid_pen.0 as HgdObj) };

                for index in 1..frame.span {
                    let offset = (index as f32 * scale).round() as i32;

                    unsafe {
                        MoveToEx(dc, offset, 0, null_mut());
                        LineTo(dc, offset, LOUPE_DIAMETER);
                        MoveToEx(dc, 0, offset, null_mut());
                        LineTo(dc, LOUPE_DIAMETER, offset);
                    }
                }

                unsafe {
                    SelectObject(dc, previous_pen);
                }

                Ok(())
            }

            fn draw_overlay(&self, frame: CaptureFrame) -> Result<(), MagnifierError> {
                let dc = unsafe { GetDC(self.overlay_hwnd) };

                if dc.is_null() {
                    return Err(MagnifierError::PlatformFailure(
                        "GetDC failed for magnifier overlay".to_string(),
                    ));
                }

                let result = self.draw_overlay_into_dc(dc, frame);
                let _ = unsafe { ReleaseDC(self.overlay_hwnd, dc) };
                result
            }

            fn draw_overlay_into_dc(
                &self,
                dc: Hdc,
                frame: CaptureFrame,
            ) -> Result<(), MagnifierError> {
                let transparent_brush = Brush::solid(TRANSPARENT_KEY)?;
                let line = rgb(250, 250, 250);
                let pill = rgb(24, 26, 31);

                unsafe {
                    FillRect(
                        dc,
                        &Rect {
                            left: 0,
                            top: 0,
                            right: WINDOW_WIDTH,
                            bottom: WINDOW_HEIGHT,
                        },
                        transparent_brush.0,
                    );
                }

                let circle = Rect {
                    left: LOUPE_LEFT,
                    top: LOUPE_TOP,
                    right: LOUPE_LEFT + LOUPE_DIAMETER,
                    bottom: LOUPE_TOP + LOUPE_DIAMETER,
                };
                let outline_pen = Pen::solid(line, 1)?;
                let previous_pen = unsafe { SelectObject(dc, outline_pen.0 as HgdObj) };
                let previous_brush = unsafe { SelectObject(dc, transparent_brush.0 as HgdObj) };

                unsafe {
                    Ellipse(dc, circle.left, circle.top, circle.right, circle.bottom);
                }

                let selected = selection_rect(frame);
                let target_pen = Pen::solid(line, 1)?;
                let previous_target_pen = unsafe { SelectObject(dc, target_pen.0 as HgdObj) };

                unsafe {
                    Rectangle(
                        dc,
                        selected.left,
                        selected.top,
                        selected.right,
                        selected.bottom,
                    );
                    SelectObject(dc, previous_target_pen);
                    SelectObject(dc, previous_pen);
                    SelectObject(dc, previous_brush);
                }

                let label_rect = Rect {
                    left: (WINDOW_WIDTH - LABEL_WIDTH) / 2,
                    top: TEXT_TOP,
                    right: (WINDOW_WIDTH + LABEL_WIDTH) / 2,
                    bottom: TEXT_TOP + TEXT_HEIGHT,
                };
                let pill_brush = Brush::solid(pill)?;
                let pill_pen = Pen::solid(line, 1)?;
                let previous_pen = unsafe { SelectObject(dc, pill_pen.0 as HgdObj) };
                let previous_brush = unsafe { SelectObject(dc, pill_brush.0 as HgdObj) };

                unsafe {
                    RoundRect(
                        dc,
                        label_rect.left,
                        label_rect.top,
                        label_rect.right,
                        label_rect.bottom,
                        14,
                        14,
                    );
                    SelectObject(dc, previous_pen);
                    SelectObject(dc, previous_brush);
                }

                if !self.text_font.is_null() {
                    unsafe {
                        SelectObject(dc, self.text_font as HgdObj);
                    }
                }

                unsafe {
                    SetBkMode(dc, TRANSPARENT_BKMODE);
                    SetTextColor(dc, line);
                }

                let label = color_to_hex(frame.color)
                    .trim_start_matches('#')
                    .to_string();
                let wide = encode_wide(&label);
                let text_x = (WINDOW_WIDTH - (label.len() as i32 * 10)) / 2;

                unsafe {
                    TextOutW(dc, text_x, TEXT_TOP + 5, wide.as_ptr(), label.len() as i32);
                }

                Ok(())
            }
        }

        impl Drop for NativeMagnifierWindow {
            fn drop(&mut self) {
                if !self.overlay_hwnd.is_null() {
                    let _ = unsafe { DestroyWindow(self.overlay_hwnd) };
                }

                if !self.loupe_hwnd.is_null() {
                    let _ = unsafe { DestroyWindow(self.loupe_hwnd) };
                }

                if !self.text_font.is_null() {
                    let _ = unsafe { DeleteObject(self.text_font as HgdObj) };
                }
            }
        }

        struct Brush(Hbrush);

        impl Brush {
            fn solid(color: u32) -> Result<Self, MagnifierError> {
                let brush = unsafe { CreateSolidBrush(color) };

                if brush.is_null() {
                    Err(MagnifierError::PlatformFailure(
                        "CreateSolidBrush failed".to_string(),
                    ))
                } else {
                    Ok(Self(brush))
                }
            }
        }

        impl Drop for Brush {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    let _ = unsafe { DeleteObject(self.0 as HgdObj) };
                }
            }
        }

        struct Pen(Hpen);

        impl Pen {
            fn solid(color: u32, width: i32) -> Result<Self, MagnifierError> {
                let pen = unsafe { CreatePen(PS_SOLID, width, color) };

                if pen.is_null() {
                    Err(MagnifierError::PlatformFailure(
                        "CreatePen failed".to_string(),
                    ))
                } else {
                    Ok(Self(pen))
                }
            }
        }

        impl Drop for Pen {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    let _ = unsafe { DeleteObject(self.0 as HgdObj) };
                }
            }
        }

        struct Region(Hrgn);

        impl Region {
            fn ellipse(rect: Rect) -> Result<Self, MagnifierError> {
                let region =
                    unsafe { CreateEllipticRgn(rect.left, rect.top, rect.right, rect.bottom) };

                if region.is_null() {
                    Err(MagnifierError::PlatformFailure(
                        "CreateEllipticRgn failed".to_string(),
                    ))
                } else {
                    Ok(Self(region))
                }
            }

            fn into_raw(self) -> Hrgn {
                let raw = self.0;
                std::mem::forget(self);
                raw
            }
        }

        impl Drop for Region {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    let _ = unsafe { DeleteObject(self.0 as HgdObj) };
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    mod platform {
        use super::{MagnifierError, wait_for_pick};
        use iced::Color;
        use std::ffi::c_void;

        type CfTypeRef = *const c_void;
        type CgImageRef = *mut c_void;
        type CgContextRef = *mut c_void;
        type CgColorSpaceRef = *mut c_void;
        type CgEventRef = *mut c_void;

        const KCG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;
        const KCG_NULL_WINDOW_ID: u32 = 0;
        const KCG_WINDOW_IMAGE_DEFAULT: u32 = 0;
        const KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: i32 = 0;
        const KCG_MOUSE_BUTTON_LEFT: u32 = 0;
        const ESCAPE_KEY_CODE: u16 = 53;
        const KCG_IMAGE_ALPHA_PREMULTIPLIED_LAST: u32 = 1;
        const KCG_BITMAP_BYTE_ORDER_32_BIG: u32 = 4 << 12;

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct CGPoint {
            x: f64,
            y: f64,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct CGSize {
            width: f64,
            height: f64,
        }

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        struct CGRect {
            origin: CGPoint,
            size: CGSize,
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        unsafe extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
            fn CGRequestScreenCaptureAccess() -> bool;
            fn CGEventCreate(source: *const c_void) -> CgEventRef;
            fn CGEventGetLocation(event: CgEventRef) -> CGPoint;
            fn CGEventSourceButtonState(state_id: i32, button: u32) -> bool;
            fn CGEventSourceKeyState(state_id: i32, key: u16) -> bool;
            fn CGWindowListCreateImage(
                screen_bounds: CGRect,
                list_option: u32,
                window_id: u32,
                image_option: u32,
            ) -> CgImageRef;
            fn CGColorSpaceCreateDeviceRGB() -> CgColorSpaceRef;
            fn CGBitmapContextCreate(
                data: *mut c_void,
                width: usize,
                height: usize,
                bits_per_component: usize,
                bytes_per_row: usize,
                space: CgColorSpaceRef,
                bitmap_info: u32,
            ) -> CgContextRef;
            fn CGContextDrawImage(context: CgContextRef, rect: CGRect, image: CgImageRef);
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        unsafe extern "C" {
            fn CFRelease(value: CfTypeRef);
        }

        pub(super) fn pick_color_blocking() -> Result<Color, MagnifierError> {
            ensure_screen_capture_access()?;
            wait_for_pick(left_button_down, escape_down, sample_color_at_cursor)
        }

        fn ensure_screen_capture_access() -> Result<(), MagnifierError> {
            let granted = unsafe {
                if CGPreflightScreenCaptureAccess() {
                    true
                } else {
                    CGRequestScreenCaptureAccess()
                }
            };

            if granted {
                Ok(())
            } else {
                Err(MagnifierError::PermissionDenied)
            }
        }

        fn left_button_down() -> bool {
            unsafe {
                CGEventSourceButtonState(
                    KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                    KCG_MOUSE_BUTTON_LEFT,
                )
            }
        }

        fn escape_down() -> bool {
            unsafe {
                CGEventSourceKeyState(
                    KCG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                    ESCAPE_KEY_CODE,
                )
            }
        }

        fn sample_color_at_cursor() -> Result<Color, MagnifierError> {
            let point = cursor_position()?;
            let image = unsafe {
                CGWindowListCreateImage(
                    CGRect {
                        origin: point,
                        size: CGSize {
                            width: 1.0,
                            height: 1.0,
                        },
                    },
                    KCG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY,
                    KCG_NULL_WINDOW_ID,
                    KCG_WINDOW_IMAGE_DEFAULT,
                )
            };

            if image.is_null() {
                return Err(MagnifierError::CaptureUnavailable(
                    "CGWindowListCreateImage returned null",
                ));
            }

            let color = render_image_pixel(image);
            unsafe { CFRelease(image as CfTypeRef) };
            color
        }

        fn render_image_pixel(image: CgImageRef) -> Result<Color, MagnifierError> {
            let color_space = unsafe { CGColorSpaceCreateDeviceRGB() };

            if color_space.is_null() {
                return Err(MagnifierError::CaptureUnavailable(
                    "CGColorSpaceCreateDeviceRGB returned null",
                ));
            }

            let mut pixel = [0u8; 4];
            let context = unsafe {
                CGBitmapContextCreate(
                    pixel.as_mut_ptr().cast(),
                    1,
                    1,
                    8,
                    4,
                    color_space,
                    KCG_IMAGE_ALPHA_PREMULTIPLIED_LAST | KCG_BITMAP_BYTE_ORDER_32_BIG,
                )
            };

            if context.is_null() {
                unsafe { CFRelease(color_space as CfTypeRef) };
                return Err(MagnifierError::CaptureUnavailable(
                    "CGBitmapContextCreate returned null",
                ));
            }

            unsafe {
                CGContextDrawImage(
                    context,
                    CGRect {
                        origin: CGPoint { x: 0.0, y: 0.0 },
                        size: CGSize {
                            width: 1.0,
                            height: 1.0,
                        },
                    },
                    image,
                );
                CFRelease(context as CfTypeRef);
                CFRelease(color_space as CfTypeRef);
            }

            Ok(Color::from_rgba(
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
                pixel[3] as f32 / 255.0,
            ))
        }

        fn cursor_position() -> Result<CGPoint, MagnifierError> {
            let event = unsafe { CGEventCreate(std::ptr::null()) };

            if event.is_null() {
                return Err(MagnifierError::PlatformFailure(
                    "CGEventCreate returned null".to_string(),
                ));
            }

            let position = unsafe { CGEventGetLocation(event) };
            unsafe { CFRelease(event as CfTypeRef) };

            Ok(position)
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    mod platform {
        use super::MagnifierError;
        use iced::Color;

        pub(super) fn pick_color_blocking() -> Result<Color, MagnifierError> {
            Err(MagnifierError::UnsupportedPlatform)
        }
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
    fn live_color_sync_preserves_current_page() {
        let mut state = State::new(
            Color::from_rgb8(0x22, 0x33, 0x44),
            Color::from_rgb8(0x11, 0x11, 0x11),
            ColorModel::Hsl,
            PickerPage::Contrast,
            Vec::new(),
        );

        state.page = PickerPage::Picker;
        state.contrast_target = ContrastTarget::Foreground;
        state.sync_live_colors(
            Color::from_rgb8(0x9C, 0xAA, 0x33),
            Color::from_rgb8(0x0A, 0x0A, 0x0A),
        );

        assert_eq!(state.page, PickerPage::Picker);
        assert_color_close(state.color, Color::from_rgb8(0x9C, 0xAA, 0x33));
    }

    #[test]
    fn external_sync_updates_page_when_reopened() {
        let mut state = State::new(
            Color::from_rgb8(0x22, 0x33, 0x44),
            Color::from_rgb8(0x11, 0x11, 0x11),
            ColorModel::Hsl,
            PickerPage::Picker,
            Vec::new(),
        );

        state.sync_from_external(
            Color::from_rgb8(0x9C, 0xAA, 0x33),
            Color::from_rgb8(0xF7, 0xF7, 0xF7),
            ColorModel::Rgb,
            PickerPage::Contrast,
            &[],
        );

        assert_eq!(state.page, PickerPage::Contrast);
        assert_eq!(state.model, ColorModel::Rgb);
        assert_color_close(state.color, Color::from_rgb8(0x9C, 0xAA, 0x33));
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
