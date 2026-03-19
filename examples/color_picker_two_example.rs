use iced::{
    Background, Border, Color, Element, Task, Theme, clipboard,
    widget::{Space, button, column, container, text},
};
use widgets::color_picker_two::{
    ColorInfo, ColorModel, ContrastInfo, MagnifierRequest, MagnifierTarget, PickerPage,
    color_picker_two, parse_color_string,
};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}

struct App {
    picker_open: bool,
    color: Color,
    background: Color,
    hex: String,
    formatted: String,
    model: ColorModel,
    contrast_ratio: f32,
    contrast_grade: String,
}

#[derive(Debug, Clone)]
enum Message {
    OpenPicker,
    ClosePicker,
    PickerChanged(ColorInfo),
    ContrastChanged(ContrastInfo),
    MagnifierRequested(MagnifierRequest),
    MagnifierClipboardLoaded(MagnifierRequest, Option<String>),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let color = Color::from_rgb8(0x9C, 0xAA, 0x33);
        let background = Color::from_rgb8(0x2B, 0x2D, 0x3A);
        let ratio = contrast_ratio(color, background);

        (
            Self {
                picker_open: true,
                color,
                background,
                hex: "#9CAA33".to_string(),
                formatted: "hsl(67, 54%, 43%)".to_string(),
                model: ColorModel::Hsl,
                contrast_ratio: ratio,
                contrast_grade: contrast_grade_label(ratio).to_string(),
            },
            Task::none(),
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
            Message::OpenPicker => self.picker_open = true,
            Message::ClosePicker => self.picker_open = false,
            Message::PickerChanged(info) => {
                self.color = info.color;
                self.hex = info.hex;
                self.formatted = info.formatted;
                self.model = info.model;
            }
            Message::ContrastChanged(info) => {
                self.color = info.foreground;
                self.background = info.background;
                self.contrast_ratio = info.ratio;
                self.contrast_grade = info.grade.label().to_string();
            }
            Message::MagnifierRequested(request) => {
                return clipboard::read().map(move |contents| {
                    Message::MagnifierClipboardLoaded(request.clone(), contents)
                });
            }
            Message::MagnifierClipboardLoaded(request, contents) => {
                let Some(color) = contents.as_deref().and_then(parse_color_string) else {
                    return Task::none();
                };

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
