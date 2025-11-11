// Example usage of the enhanced generic_overlay widget
// This shows all the new features added to generic_overlay.rs

use iced::{
    widget::{button, checkbox, column, container, text, text_input, row, pick_list},
    Element, Length, Task, Theme, Alignment
};

use widgets::generic_overlay::{overlay_button, ResizeMode, interactive_tooltip, Position};

#[derive(Debug, Clone)]
enum Message {
    OverlayCheckboxToggled(bool),
    TextInputChanged(String),
    ButtonPressed,
    OverlayOpened,
    OverlayClosed,
    UpdatePosition(Position),
    UpdateAlignment(AlignmentOption),
    UpdateGap(String),
}

struct App {
    overlay_checkbox: bool,
    text_input_value: String,
    hover_position: Option<Position>,
    hover_alignment: Option<AlignmentOption>,
    hover_gap: f32,
    gap_text: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            overlay_checkbox: false,
            text_input_value: String::new(),
            hover_position: Some(Position::Right),
            hover_alignment: Some(AlignmentOption::Start),
            hover_gap: 5.0,
            gap_text: "5.0".to_string(),
        }
    }
}

impl App {
    pub fn new() -> Self {
        App::default()
    }

    fn theme(&self) -> Theme {
        iced::Theme::Dark
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::OverlayCheckboxToggled(bool) => {self.overlay_checkbox = bool;},
            Message::TextInputChanged(str) => {self.text_input_value = str},
            Message::ButtonPressed => {println!("Button was pressed")},
            Message::OverlayOpened => {println!("Overlay was opened")},
            Message::OverlayClosed => {println!("Overlay was closed")},
            Message::UpdateAlignment(alignment) => self.hover_alignment = Some(alignment),
            Message::UpdatePosition(position) => self.hover_position = Some(position),
            Message::UpdateGap(gap_text) => {
                self.gap_text = gap_text;
                match self.gap_text.as_str().trim().parse::<f32>() {
                    Ok(gap) => self.hover_gap = gap,
                    Err(_) => {}
                }
            }
        }
        Task::none()
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        // Create overlay content
        let basic_overlay_content: Element<Message> = column![
            text("Basic Overlay - No Options").size(20),
            text("This displays your Element, you can move the overlay by dragging the header."),
            text("you can also hold ctrl to drag from anywhere."),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let opaque_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .opaque()").size(20),
            text("Using .opaque() creates a darkened background, closes on clicking outside the overlay."),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let close_on_outside_click_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .close_on_click_outside()").size(20),
            text("You can also just not darken the background if you want."),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let headless_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .hide_header()").size(20),
            text("You can also remove the header, this automatically closes on clicking outside the overlay as well."),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let always_resizeable_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .resizable(ResizeMode::Always)").size(20),
            text("You have two options to enable resizing, Always"),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let ctrl_click_resizeable_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .resizable(ResizeMode::WithCtrl)").size(20),
            text("Or resizing when holding ctrl"),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let hover_to_open_overlay_content: Element<Message> = column![
            text("Overlay Dialog - Hover to Open").size(20),
            text("Positions mirror Tooltip, minus following the mouse"),
            row![
                text("Set Gap:"),
                text_input("Enter gap", &self.gap_text)
                    .on_input(Message::UpdateGap),
            ].spacing(10),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let interactive_tooltip_overlay_content: Element<Message> = column![
            text("Overlay Dialog - Hover to Open").size(20),
            text("Positions mirror Tooltip"),
            checkbox("Enable Feature", self.overlay_checkbox)
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        // EXAMPLE 1: Basic overlay with header (matching color_picker header size)
        let basic_overlay = overlay_button(
            text("Open Default Overlay"),
            "Default Generic Overlay Example",
            basic_overlay_content,
        )
        .on_open(|| Message::OverlayOpened)
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 2: Opaque overlay that blocks interaction with content behind it
        let opaque_overlay = overlay_button(
            "Open Opaque",
            "Opaque",
            opaque_overlay_content,
        )
        .opaque(true)
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 3: Click-outside-to-close overlay
        let click_outside_overlay = overlay_button(
            "Open (Click Outside)",
            "Close On Outside Click",
            close_on_outside_click_overlay_content,
        )
        .close_on_click_outside()
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 4: Headerless overlay (no title bar or close button)
        let headerless_overlay = overlay_button(
            "Open Headerless",
            "",  // Title is ignored when hide_header is true
            headless_overlay_content,
        )
        .hide_header()
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 5: Always Resizeable overlay
        let always_resizeable_overlay = overlay_button(
            "Open Always Resizeable",
            "Always Resizeable",
            always_resizeable_overlay_content,
        )
        .resizable(ResizeMode::Always)
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 6: Resizeable when holding control overlay
        let ctrl_click_resizeable_overlay = overlay_button(
            "Open Ctrl Click Resizeable",
            "Resizeable while holding control",
            ctrl_click_resizeable_overlay_content,
        )
        .resizable(ResizeMode::WithCtrl)
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 7: Open on Hover
        let on_hover_overlay = overlay_button(
            "Hover to Open - Bottom",
            "Tooltip like behavior",
            hover_to_open_overlay_content,
        )
        .hide_header()
        .on_hover(self.hover_position.unwrap_or(Position::Right).into())
        .gap(self.hover_gap)
        .alignment(self.hover_alignment.unwrap_or(AlignmentOption::Center).into())
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 8: Same as 7, but with the interactive_tooltip helper
        let interactive_tooltip1 = interactive_tooltip("Hover to Open - Right", interactive_tooltip_overlay_content);

        let menu2 = column![
            button("Menu2 option 1").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 2").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 3").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 4").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),

        ].width(Length::Fill);

        let menu1 = column![
            button("Menu option 1").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 2").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 3").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 4").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            interactive_tooltip("Menu2", menu2).overlay_width(150.0).style(button::subtle).width(Length::Fill).overlay_padding(1.0).alignment(Alignment::Start),
        ].width(Length::Fill);

        let nav_menu = container(row![
            interactive_tooltip("File", menu1).overlay_width(150.0).gap(0.0).overlay_padding(1.0),
        ].width(Length::Fill)).style(container::secondary);

        column![
            nav_menu,
            column![
                
                text("Generic Overlay Examples").size(24),
                basic_overlay,
                opaque_overlay,
                click_outside_overlay,
                headerless_overlay,
                always_resizeable_overlay,
                ctrl_click_resizeable_overlay,
                container(
                    column![
                        row![
                            column![
                                text("Set Position"),
                                pick_list(
                                    Position::ALL,
                                    self.hover_position,
                                    |position| Message::UpdatePosition(position)
                                ).width(100),
                            ].spacing(5),
                            column![
                                text("Set Alignment"),
                                pick_list(
                                    AlignmentOption::ALL,
                                    self.hover_alignment,
                                    |alignment| Message::UpdateAlignment(alignment)
                                ).width(100),
                            ].spacing(5),
                        ].spacing(10),
                        on_hover_overlay
                    ].spacing(10)
                ).center_x(Length::Fill),
                interactive_tooltip1,
            ]
            .spacing(20)
            .padding(40)
        ]
        .into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}



#[derive(Debug, Clone, Copy, PartialEq, Eq,)]
pub enum AlignmentOption {
    Start,
    Center,
    End,
}

impl AlignmentOption {
    pub const ALL: &'static [Self] = &[
        Self::Start,
        Self::Center,
        Self::End,     
    ];
}

impl std::fmt::Display for AlignmentOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlignmentOption::Start => write!(f, "Start"),
            AlignmentOption::Center => write!(f, "Center"),
            AlignmentOption::End => write!(f, "End"),
        }
    }
}

impl From<AlignmentOption> for Alignment {
    fn from(c: AlignmentOption) -> Self {
        match c {
            AlignmentOption::Start => Self::Start,
            AlignmentOption::Center => Self::Center,
            AlignmentOption::End => Self::End,
        }
    }
}

impl From<Alignment> for AlignmentOption {
    fn from(c: Alignment) -> Self {
        match c {
            Alignment::Start => Self::Start,
            Alignment::Center => Self::Center,
            Alignment::End => Self::End,
        }
    }
}