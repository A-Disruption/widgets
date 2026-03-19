use iced::{
    Background, Border, Color, Element, Task, Theme,
    widget::{Space, button, column, container, text},
};
use widgets::color_picker_two::{ColorInfo, ColorModel, color_picker_two};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}

struct App {
    picker_open: bool,
    color: Color,
    hex: String,
    formatted: String,
    model: ColorModel,
}

#[derive(Debug, Clone)]
enum Message {
    OpenPicker,
    ClosePicker,
    PickerChanged(ColorInfo),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                picker_open: true,
                color: Color::from_rgb8(0x9C, 0xAA, 0x33),
                hex: "#9CAA33".to_string(),
                formatted: "hsl(67, 54%, 43%)".to_string(),
                model: ColorModel::Hsl,
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
        ]
        .spacing(6)
        .padding(32);

        let picker: Element<'_, Message> = color_picker_two(self.picker_open, self.color)
            .model(self.model)
            .on_change_with_info(Message::PickerChanged)
            .on_close(|| Message::ClosePicker)
            .into();

        container(column![content, picker]).into()
    }
}

fn is_light(color: Color) -> bool {
    (0.299 * color.r) + (0.587 * color.g) + (0.114 * color.b) > 0.55
}
