use iced::{
    Background, Border, Color, Element, Task, Theme,
    widget::{Space, button, column, container, text},
    window,
};
use widgets::color_picker_two::{
    ColorInfo, ColorModel, ContrastInfo, MagnifierError, MagnifierRequest, MagnifierTarget,
    PickerPage, color_picker_two, native_magnifier_supported, pick_color_task,
};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}

struct App {
    picker_open: bool,
    window_id: Option<window::Id>,
    color: Color,
    background: Color,
    hex: String,
    formatted: String,
    model: ColorModel,
    contrast_ratio: f32,
    contrast_grade: String,
    magnifier_status: Option<String>,
}

#[derive(Debug, Clone)]
enum Message {
    WindowReady(Option<window::Id>),
    OpenPicker,
    ClosePicker,
    PickerChanged(ColorInfo),
    ContrastChanged(ContrastInfo),
    MagnifierRequested(MagnifierRequest),
    MagnifierFinished(MagnifierRequest, Result<Color, MagnifierError>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let color = Color::from_rgb8(0x9C, 0xAA, 0x33);
        let background = Color::from_rgb8(0x2B, 0x2D, 0x3A);
        let ratio = contrast_ratio(color, background);

        (
            Self {
                picker_open: true,
                window_id: None,
                color,
                background,
                hex: "#9CAA33".to_string(),
                formatted: "hsl(67, 54%, 43%)".to_string(),
                model: ColorModel::Hsl,
                contrast_ratio: ratio,
                contrast_grade: contrast_grade_label(ratio).to_string(),
                magnifier_status: None,
            },
            window::latest().map(Message::WindowReady),
        )
    }

    fn title(&self) -> String {
        "Color Picker Two Example".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::TokyoNight
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowReady(id) => self.window_id = id,
            Message::OpenPicker => self.picker_open = true,
            Message::ClosePicker => self.picker_open = false,
            Message::PickerChanged(info) => {
                self.color = info.color;
                self.hex = info.hex;
                self.formatted = info.formatted;
                self.model = info.model;
                self.magnifier_status = None;
            }
            Message::ContrastChanged(info) => {
                self.color = info.foreground;
                self.background = info.background;
                self.contrast_ratio = info.ratio;
                self.contrast_grade = info.grade.label().to_string();
                self.magnifier_status = None;
            }
            Message::MagnifierRequested(request) => {
                self.picker_open = false;
                self.magnifier_status = Some(if native_magnifier_supported() {
                    "Click anywhere to sample a color. Press Escape to cancel.".to_string()
                } else {
                    "Native magnifier is only implemented on Windows and macOS.".to_string()
                });

                let pick = pick_color_task()
                    .map(move |result| Message::MagnifierFinished(request.clone(), result));

                if let Some(id) = self.window_id {
                    return Task::batch([window::minimize::<Message>(id, true), pick]);
                }

                return pick;
            }
            Message::MagnifierFinished(request, result) => {
                self.picker_open = true;

                match result {
                    Ok(color) => {
                        match request.target {
                            MagnifierTarget::CurrentColor | MagnifierTarget::Foreground => {
                                let info = ColorInfo::new(color, self.model);
                                self.color = color;
                                self.hex = info.hex;
                                self.formatted = info.formatted;
                            }
                            MagnifierTarget::Background => {
                                self.background = color;
                            }
                        }

                        let ratio = contrast_ratio(self.color, self.background);
                        self.contrast_ratio = ratio;
                        self.contrast_grade = contrast_grade_label(ratio).to_string();
                        self.magnifier_status = Some(format!(
                            "Sampled {} from the screen.",
                            ColorInfo::new(color, self.model).hex
                        ));
                    }
                    Err(MagnifierError::Cancelled) => {
                        self.magnifier_status = Some("Magnifier cancelled.".to_string());
                    }
                    Err(error) => {
                        self.magnifier_status = Some(error.to_string());
                    }
                }

                if let Some(id) = self.window_id {
                    return Task::batch([
                        window::minimize::<Message>(id, false),
                        window::gain_focus::<Message>(id),
                    ]);
                }
            }
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let trigger = button(text(format!("Open {}", self.hex)))
            .on_press(Message::OpenPicker)
            .style(|_, _| button::Style {
                background: Some(Background::Color(self.color)),
                text_color: if is_light(self.color) {
                    Color::BLACK
                } else {
                    Color::WHITE
                },
                border: Border {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            });

        let content = column![
            text("Color Picker Two").size(26),
            text("ColorSlurp-inspired overlay for iced 0.14."),
            Space::new().height(12.0),
            trigger,
            Space::new().height(16.0),
            text(format!("Hex: {}", self.hex)),
            text(format!("Formatted: {}", self.formatted)),
            text(format!("Model: {}", self.model)),
            text(format!(
                "Contrast: {:.1}:1 ({})",
                self.contrast_ratio, self.contrast_grade
            )),
            text(self.magnifier_status.as_deref().unwrap_or(
                "Magnifier icons now use the native screen picker on supported platforms."
            ),)
            .size(14),
        ]
        .spacing(6)
        .padding(32);

        let picker: Element<'_, Message> = color_picker_two(self.picker_open, self.color)
            .model(self.model)
            .page(PickerPage::Contrast)
            .contrast_background(self.background)
            .on_change_with_info(Message::PickerChanged)
            .on_contrast_change(Message::ContrastChanged)
            .on_magnifier_request(Message::MagnifierRequested)
            .on_close(|| Message::ClosePicker)
            .into();

        container(column![content, picker]).into()
    }
}

fn is_light(color: Color) -> bool {
    (0.299 * color.r) + (0.587 * color.g) + (0.114 * color.b) > 0.55
}

fn contrast_ratio(foreground: Color, background: Color) -> f32 {
    let lighter = relative_luminance(foreground).max(relative_luminance(background));
    let darker = relative_luminance(foreground).min(relative_luminance(background));

    (lighter + 0.05) / (darker + 0.05)
}

fn contrast_grade_label(ratio: f32) -> &'static str {
    if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else {
        "FAIL"
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
