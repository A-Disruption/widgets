use iced::theme::{self, Palette};
use iced::{clipboard, Color, Element, Length, Task, Theme};
use iced::widget::{button, checkbox, column, container, pick_list, progress_bar, radio, row, slider, text, text_input, toggler, Space};
use widgets::color_picker::color_button;

#[derive(Debug, Clone)]
pub struct CustomPalette {
    background: Color,
    text: Color,
    primary: Color,
    success: Color,
    warning: Color,
    danger: Color,
}

impl Default for CustomPalette {
    fn default() -> Self {
        Self {
            background: Color::WHITE,
            text: Color::BLACK,
            primary: Color::from_rgb8(0x58, 0x65, 0xF2), // Discord blue
            success: Color::from_rgb8(0x12, 0x66, 0x4F),
            warning: Color::from_rgb8(0xFF, 0xC1, 0x4E),
            danger: Color::from_rgb8(0xC3, 0x42, 0x3F),
        }
    }
}

impl CustomPalette {
    pub fn default() -> Self {
        Self {
            background: Color::WHITE,
            text: Color::BLACK,
            primary: Color::from_rgb8(0x58, 0x65, 0xF2), // Discord blue
            success: Color::from_rgb8(0x12, 0x66, 0x4F),
            warning: Color::from_rgb8(0xFF, 0xC1, 0x4E),
            danger: Color::from_rgb8(0xC3, 0x42, 0x3F),
        }
    }

    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb8(0x2B, 0x2D, 0x31),
            text: Color::from_rgb(0.90, 0.90, 0.90),
            primary: Color::from_rgb8(0x58, 0x65, 0xF2),
            success: Color::from_rgb8(0x12, 0x66, 0x4F),
            warning: Color::from_rgb8(0xFF, 0xC1, 0x4E),
            danger: Color::from_rgb8(0xC3, 0x42, 0x3F),
        }
    }

    pub fn preset_blue() -> Self {
        let mut palette = Self::default();
        palette.primary = Color::from_rgb8(0x3B, 0x82, 0xF6);
        palette.success = Color::from_rgb8(0x10, 0xB9, 0x81);
        palette.warning = Color::from_rgb8(0xF5, 0x9E, 0x0B);
        palette.danger = Color::from_rgb8(0xEF, 0x44, 0x44);
        palette
    }

    pub fn preset_purple() -> Self {
        let mut palette = Self::default();
        palette.primary = Color::from_rgb8(0x8B, 0x5C, 0xF6);
        palette.success = Color::from_rgb8(0x10, 0xB9, 0x81);
        palette.warning = Color::from_rgb8(0xF5, 0x9E, 0x0B);
        palette.danger = Color::from_rgb8(0xEF, 0x44, 0x44);
        palette
    }

    pub fn preset_green() -> Self {
        let mut palette = Self::default();
        palette.primary = Color::from_rgb8(0x10, 0xB9, 0x81);
        palette.success = Color::from_rgb8(0x05, 0x96, 0x69);
        palette.warning = Color::from_rgb8(0xF5, 0x9E, 0x0B);
        palette.danger = Color::from_rgb8(0xEF, 0x44, 0x44);
        palette
    }

    pub fn pallet_to_rust_code(&self) -> String {
        format!(
            "let custom_palette = Palette {{\n    background: {},\n    text: {},\n    primary: {},\n    success: {},\n    warning: {},\n    danger: {},\n}};",
            color_to_rust_code(self.background),
            color_to_rust_code(self.text),
            color_to_rust_code(self.primary),
            color_to_rust_code(self.success),
            color_to_rust_code(self.warning),
            color_to_rust_code(self.danger),
        )
    }

    pub fn theme_to_rust_code(&self) -> String {
        format!(
            "\nlet custom_theme = iced::Theme::custom( \"Custom\".to_string() , custom_palette );"
        )
    }

    pub fn copy_complete_code_to_clipboard(&self) -> Task<Message> {
        // Combine both function outputs
        let complete_code = format!("{}{}", 
            self.pallet_to_rust_code(), 
            self.theme_to_rust_code()
        );
        
        // Create clipboard instance and set contents
        clipboard::write::<Message>(complete_code)
    }

    pub fn to_iced_palette(&self) -> Palette {
        Palette {
            background: self.background,
            text: self.text,
            primary: self.primary,
            success: self.success,
            warning: self.warning,
            danger: self.danger,
        }
    }

    pub fn to_iced_theme(&self, name: &str) -> theme::Custom {
        theme::Custom::new(name.to_string(), self.to_iced_palette())
    }

    pub fn to_iced_theme_frfr(&self, name: &str) -> iced::Theme {
        Theme::custom( name.to_string() , self.to_iced_palette() )
    }
}

fn color_to_rust_code(color: Color) -> String {
    if color == Color::WHITE {
        "Color::WHITE".to_string()
    } else if color == Color::BLACK {
        "Color::BLACK".to_string()
    } else {
        let r = (color.r * 255.0) as u32;
        let g = (color.g * 255.0) as u32;
        let b = (color.b * 255.0) as u32;
        let hex = (r << 16) | (g << 8) | b;
        format!("color!(0x{:06X})", hex)
    }
}

fn color_to_hex(color: Color) -> String {
    let r = (color.r * 255.0) as u32;
    let g = (color.g * 255.0) as u32;
    let b = (color.b * 255.0) as u32;
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

fn hex_to_color(hex: &str) -> Result<Color, ()> {
    if !hex.starts_with('#') || hex.len() != 7 {
        return Err(());
    }
    
    let hex = &hex[1..];
    if let Ok(num) = u32::from_str_radix(hex, 16) {
        let r = ((num >> 16) & 0xFF) as f32 / 255.0;
        let g = ((num >> 8) & 0xFF) as f32 / 255.0;
        let b = (num & 0xFF) as f32 / 255.0;
        Ok(Color::from_rgb(r, g, b))
    } else {
        Err(())
    }
}

#[derive(Debug, Clone)]
pub enum ColorField {
    Background,
    Text,
    Primary,
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetMode(bool), // true for dark, false for light
    ApplyPreset(PresetType),
    ColorChanged(ColorField, String),
    CopyCode,

    //Color_picker widget
    ColorPickerChanged(ColorField, Color),
    ColorPickerChangedWithSource(ColorField, Color, Option<String>),

    UpdateTheme(Theme),
}

#[derive(Debug, Clone)]
pub enum PresetType {
    Blue,
    Purple,
    Green,
}

pub struct PaletteBuilder {
    pub palette: CustomPalette,
    pub is_dark_mode: bool,
    pub theme: Theme,

    // Text input states for color hex values
    pub background_input: String,
    pub text_input: String,
    pub primary_input: String,
    pub success_input: String,
    pub warning_input: String,
    pub danger_input: String,

    // Custom Widget Styles
    pub button_style: Option<button::Style>,
    pub check_box_style: Option<checkbox::Style>,
//    pub combo_box_stle: Option<combo_box::Style>,
    pub container_style: Option<container::Style>,
    pub pick_list_style: Option<pick_list::Style>,
    pub progress_bar_style: Option<progress_bar::Style>,
    pub radio_style: Option<radio::Style>,
    pub slider_style: Option<slider::Style>,
    pub text_style: Option<text::Style>,
    pub text_input_style: Option<text_input::Style>,
    pub toggler_style: Option<toggler::Style>,



}

impl Default for PaletteBuilder {
    fn default() -> Self {
        let palette = CustomPalette::default();
        Self {
            theme: Theme::Light,
            background_input: color_to_hex(palette.background),
            text_input: color_to_hex(palette.text),
            primary_input: color_to_hex(palette.primary),
            success_input: color_to_hex(palette.success),
            warning_input: color_to_hex(palette.warning),
            danger_input: color_to_hex(palette.danger),
            palette: palette.clone(),
            is_dark_mode: false,

            button_style: None,
            check_box_style: None,
//            combo_box_stle: None,
            container_style: None,
            pick_list_style: None,
            progress_bar_style: None,
            radio_style: None,
            slider_style: None,
            text_style: None,
            text_input_style: None,
            toggler_style: None,
        }
    }
}

impl PaletteBuilder {
    fn update_input_fields(&mut self) {
        self.background_input = color_to_hex(self.palette.background);
        self.text_input = color_to_hex(self.palette.text);
        self.primary_input = color_to_hex(self.palette.primary);
        self.success_input = color_to_hex(self.palette.success);
        self.warning_input = color_to_hex(self.palette.warning);
        self.danger_input = color_to_hex(self.palette.danger);
    }
}

impl PaletteBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> { 
        match message {
            Message::SetMode(is_dark) => {
                self.is_dark_mode = is_dark;
                if is_dark {
                    self.palette = CustomPalette::dark();
                } else {
                    self.palette = CustomPalette::default();
                }
                self.update_input_fields();
            }
            Message::ApplyPreset(preset_type) => {
                let base_palette = match preset_type {
                    PresetType::Blue => CustomPalette::preset_blue(),
                    PresetType::Purple => CustomPalette::preset_purple(),
                    PresetType::Green => CustomPalette::preset_green(),
                };

                let theme = self.palette.to_iced_theme_frfr("Custom");
                
                // Keep the current background/text based on mode
                self.palette.primary = base_palette.primary;
                self.palette.success = base_palette.success;
                self.palette.warning = base_palette.warning;
                self.palette.danger = base_palette.danger;
                self.update_input_fields();

                self.theme = theme;
            }
            Message::ColorChanged(field, hex_string) => {
                // Update the input field
                match field {
                    ColorField::Background => self.background_input = hex_string.clone(),
                    ColorField::Text => self.text_input = hex_string.clone(),
                    ColorField::Primary => self.primary_input = hex_string.clone(),
                    ColorField::Success => self.success_input = hex_string.clone(),
                    ColorField::Warning => self.warning_input = hex_string.clone(),
                    ColorField::Danger => self.danger_input = hex_string.clone(),
                }

                // Try to parse and update the color
                if let Ok(color) = hex_to_color(&hex_string) {
                    match field {
                        ColorField::Background => self.palette.background = color,
                        ColorField::Text => self.palette.text = color,
                        ColorField::Primary => self.palette.primary = color,
                        ColorField::Success => self.palette.success = color,
                        ColorField::Warning => self.palette.warning = color,
                        ColorField::Danger => self.palette.danger = color,
                    }
                }
            }
            Message::CopyCode => {
                    return self.palette.copy_complete_code_to_clipboard()
            }
            Message::UpdateTheme(theme) => {
                self.theme = theme;
            }
            Message::ColorPickerChanged(field, color) => {
                match field {
                    ColorField::Background => {
                        self.palette.background = color;
                        self.background_input = color_to_hex(color);
                    }
                    ColorField::Text => {
                        self.palette.text = color;
                        self.text_input = color_to_hex(color);
                    }
                    ColorField::Primary => {
                        self.palette.primary = color;
                        self.primary_input = color_to_hex(color);
                    }
                    ColorField::Success => {
                        self.palette.success = color;
                        self.success_input = color_to_hex(color);
                    }
                    ColorField::Warning => {
                        self.palette.warning = color;
                        self.warning_input = color_to_hex(color);
                    }
                    ColorField::Danger => {
                        self.palette.danger = color;
                        self.danger_input = color_to_hex(color);
                    }
                }

                //let theme = self.palette.to_iced_theme_frfr("Custom");
                //self.theme = theme;
            }
            Message::ColorPickerChangedWithSource(field, color, source) => {
                // Update the color
                match field {
                    ColorField::Background => {
                        self.palette.background = color;
                        self.background_input = color_to_hex(color);
                    }
                    ColorField::Text => {
                        self.palette.text = color;
                        self.text_input = color_to_hex(color);
                    }
                    ColorField::Primary => {
                        self.palette.primary = color;
                        self.primary_input = color_to_hex(color);
                    }
                    ColorField::Success => {
                        self.palette.success = color;
                        self.success_input = color_to_hex(color);
                    }
                    ColorField::Warning => {
                        self.palette.warning = color;
                        self.warning_input = color_to_hex(color);
                    }
                    ColorField::Danger => {
                        self.palette.danger = color;
                        self.danger_input = color_to_hex(color);
                    }
                }
                
                // Update the text in text_input
                let display_text = source.unwrap_or_else(|| color_to_hex(color));
                match field {
                    ColorField::Background => {
                        self.background_input = display_text;
                    }
                    ColorField::Text => {
                        self.text_input = display_text;
                    }
                    ColorField::Primary => {
                        self.primary_input = display_text;
                    }
                    ColorField::Success => {
                        self.success_input = display_text;
                    }
                    ColorField::Warning => {
                        self.warning_input = display_text;
                    }
                    ColorField::Danger => {
                        self.danger_input = display_text;
                    }
                }
                

            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {

        let content = row![
            container(
                column![
                    text("Iced Palette Builder").size(24),
                    Space::new().height(16),
                    
                    // Presets
                    text("Quick Start").size(16),
                    Space::new().height(8),
                    row![
                        button("Light")
                            .on_press(Message::SetMode(false))
                            .style(|_, _| button_background_color(Color::WHITE)),
                        button("Dark")
                            .on_press(Message::SetMode(true))
                            .style(|_, _| button_background_color(Color::from_rgb8(0x2B, 0x2D, 0x31))),
                        button("Blue")
                            .on_press(Message::ApplyPreset(PresetType::Blue))
                            .style(|_, _| button_background_color(Color::from_rgb8(0x3B, 0x82, 0xF6))),
                        button("Purple")
                            .on_press(Message::ApplyPreset(PresetType::Purple))
                            .style(|_, _| button_background_color(Color::from_rgb8(0x8B, 0x5C, 0xF6))),
                        button("Green")
                            .on_press(Message::ApplyPreset(PresetType::Green))
                            .style(|_, _| button_background_color(Color::from_rgb8(0x10, 0xB9, 0x81))),
                    ].spacing(20),

                    Space::new().height(16),

                    // Color Selection
                    row![
                        column![
                            column![
                                text("Background"),
                                row![
                                    color_button(
                                        self.palette.background,
                                    )
                                    .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Background, color, source))
                                    .title("Background Color")
                                    .width(30)
                                    .height(20),
                                    text_input("Background", &self.background_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Background, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                            
                            column![
                                text("Primary"),
                                row![
                                    color_button(self.palette.primary)
                                        .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Primary, color, source))
                                        .title("Primary Color")
                                        .width(30)
                                        .height(20),
                                    text_input("Primary", &self.primary_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Primary, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                            
                            column![
                                text("Warning"),
                                row![
                                    color_button(self.palette.warning)
                                        .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Warning, color, source))
                                        .title("Warning Color")
                                        .width(30)
                                        .height(20),
                                    text_input("Warning", &self.warning_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Warning, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                        ].spacing(10),
                        
                        column![
                            column![
                                text("Text"),
                                row![
                                    color_button(self.palette.text)
                                        .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Text, color, source))
                                        .title("Text Color")
                                        .width(30)
                                        .height(20),
                                    text_input("Text", &self.text_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Text, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                            
                            column![
                                text("Success"),
                                row![
                                    color_button(self.palette.success)
                                        .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Success, color, source))
                                        .title("Success Color")
                                        .width(30)
                                        .height(20),
                                    text_input("Success", &self.success_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Success, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                            
                            column![
                                text("Danger"),
                                row![
                                    color_button(self.palette.danger)
                                        .on_change_with_source(|color, source| Message::ColorPickerChangedWithSource(ColorField::Danger, color, source))
                                        .title("Danger Color")
                                        .width(30)
                                        .height(20),
                                    text_input("Danger", &self.danger_input)
                                        .on_input(|s| Message::ColorChanged(ColorField::Danger, s))
                                ].align_y(iced::Alignment::Center).spacing(5),
                            ].spacing(5),
                        ].spacing(10),
                    ].spacing(20),

                    Space::new().height(16),
                    
                    // Generated code section
                    row![
//                        text("Generated Rust Code").size(16),
                        button("Copy to clipboard")
                            .on_press(Message::CopyCode)
                            .style(button::secondary),
                    ].align_y(iced::Alignment::Center).spacing(10),
                    container(
                        column!(
                            text(self.palette.pallet_to_rust_code()).size(12),
                            text(self.palette.theme_to_rust_code()).size(12),
                        )
                        
                    )
                    .width(Length::Fill)
                    .style(container::bordered_box)
                    .padding(12),
                ]
                .spacing(4)
                .padding(20)
            )
            .style(container::bordered_box)
            .width(Length::FillPortion(1)),
            
            Space::new().height(24),
            

        ]
        .spacing(0)
        .padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }
}

fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    u32::from_str_radix(s, 16).ok().map(|rgb| {
        let r = ((rgb >> 16) & 0xFF) as f32 / 255.0;
        let g = ((rgb >> 8) & 0xFF) as f32 / 255.0;
        let b = (rgb & 0xFF) as f32 / 255.0;
        Color::from_rgb(r, g, b)
    })
}

fn create_background_color(color_str: &str) -> button::Style {
    let color = parse_hex_color(color_str);

    let color = color.unwrap_or_default();
    let background = iced::Background::Color(color);
    let border = iced::Border {
        color: Color::WHITE,
        width: 1_f32,
        radius: iced::border::Radius::new(0.2),
    };

    button::Style {
        background: Some(background),
        border: border,
        text_color: Color::TRANSPARENT,
        ..Default::default()
    }
}

fn button_background_color(color: Color) -> button::Style {
    let background = iced::Background::Color(color);
    let border = iced::Border {
        color: Color::WHITE,
        width: 1_f32,
        radius: iced::border::Radius::new(5),
    };

    let text = theme::palette::Pair{
        color: color,
        text: Color::BLACK
    };

    button::Style {
        background: Some(background),
        border: border,
        text_color: text.text,
        ..Default::default()
    }
}

fn main() -> iced::Result {
    iced::application(PaletteBuilder::new, PaletteBuilder::update, PaletteBuilder::view)
        .theme(PaletteBuilder::theme)
        .run()
}