// Example usage of the enhanced generic_overlay widget
// This shows all the new features added to generic_overlay.rs

use iced::{
    widget::{button, checkbox, column, text, text_input},
    Element, Length, Task, Theme,
};

use widgets::generic_overlay::{overlay_button, ResizeMode};

// Import your generic_overlay module
// use crate::generic_overlay::overlay_button;

#[derive(Debug, Clone)]
enum Message {
    OverlayCheckboxToggled(bool),
    TextInputChanged(String),
    ButtonPressed,
    OverlayOpened,
    OverlayClosed,
}

struct App {
    overlay_checkbox: bool,
    text_input_value: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            overlay_checkbox: false,
            text_input_value: String::new(),
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
        }
        Task::none()
    }

    fn view(&self) -> Element<Message> {
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
        .hide_header(true)
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

        column![
            text("Generic Overlay Examples").size(24),
            basic_overlay,
            opaque_overlay,
            click_outside_overlay,
            headerless_overlay,
            always_resizeable_overlay,
            ctrl_click_resizeable_overlay,
        ]
        .spacing(20)
        .padding(40)
        .into()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}