// Example usage of the enhanced generic_overlay widget
// This shows all the new features added to generic_overlay.rs

use iced::{
    Alignment, Element, Length, Task, Theme, widget::{button, checkbox, column, container, pick_list, row, text, text_editor, text_input, space}
};

use widgets::generic_overlay::{self, overlay_button, ResizeMode, interactive_tooltip, Position, dropdown_menu, dropdown_root, PositionMode, OverlayButton};

#[derive(Debug, Clone)]
enum Message {
    OverlayCheckboxToggled(bool),
    TextInputChanged(String),
    ButtonPressed,
    OverlayOpened(iced::Point, iced::Size),
    OverlayClosed,
    UpdatePosition(Position),
    UpdateAlignment(AlignmentOption),
    UpdateGap(String),
    CloseOverlay(iced::advanced::widget::Id),
    ToggleOverlay(bool),
}

struct App {
    overlay_checkbox: bool,
    text_input_value: String,
    hover_position: Option<Position>,
    hover_alignment: Option<AlignmentOption>,
    hover_gap: f32,
    gap_text: String,
    editor_content: text_editor::Content,
    overlay_status: bool,
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
            editor_content: text_editor::Content::new(),
            overlay_status: true,
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
            Message::OverlayOpened(overlay_position, overlay_size) => {println!("Overlay was opened. \n\tPosition: {}, \n\tSize: {:?}", overlay_position, overlay_size)},
            Message::OverlayClosed => {println!("Overlay was closed")},
            Message::UpdateAlignment(alignment) => self.hover_alignment = Some(alignment),
            Message::UpdatePosition(position) => self.hover_position = Some(position),
            Message::UpdateGap(gap_text) => {
                self.gap_text = gap_text;
                if let Ok(gap) = self.gap_text.as_str().trim().parse::<f32>() { self.hover_gap = gap }
            }
            Message::CloseOverlay(id) => {
                println!("Called on id: {:?}", id);
                return iced::advanced::widget::operate(
                    widgets::generic_overlay::close::<Message>(id)
                );
            }
            Message::ToggleOverlay(toggle) => {
                self.overlay_status = toggle
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
            button("Close Overlay from internal content")
                .on_press(Message::CloseOverlay("basic-overlay".into()))
                .style(button::danger),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let opaque_overlay_content: Element<Message> = column![
            text("Overlay Dialog - .opaque()").size(20),
            text("Using .opaque() creates a darkened background, closes on clicking outside the overlay."),
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
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
            checkbox(self.overlay_checkbox)
                .label("Enable Feature")
                .on_toggle(Message::OverlayCheckboxToggled),
            text_input("Type something...", &self.text_input_value)
                .on_input(Message::TextInputChanged),
            button("Do Something")
                .on_press(Message::ButtonPressed),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let dynamic_size_overlay_content: Element<Message> = column![
            button("Close Overlay from internal content")
                .on_press(Message::CloseOverlay("dynamic_overlay".into()))
                .style(button::danger),
        ]
        .spacing(15)
        .padding(10)
        .into();

        let custom_helper_overlay_content: Element<Message> = column![
            button("Close Overlay from internal content")
                .on_press(Message::CloseOverlay("custom_helper_overlay".into()))
                .style(button::danger),
        ]
        .spacing(15)
        .padding(10)
        .into();

        // EXAMPLE 1: Basic overlay with header (matching color_picker header size)
        let basic_overlay = overlay_button(
            text("Open Default Overlay"),
            "Default Generic Overlay Example",
            basic_overlay_content,
        ).id("basic-overlay");

        // EXAMPLE 2: Opaque overlay that blocks interaction with content behind it
        let opaque_overlay = overlay_button(
            "Open Opaque",
            "Opaque",
            opaque_overlay_content,
        )
        .opaque(true)
        .close_on_click_outside()
        .on_open(Message::OverlayOpened)
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
        .close_on_click_outside()
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
        .on_hover()
        .hover_position(self.hover_position.unwrap_or(Position::Right))
        .hover_gap(self.hover_gap)
        .hover_alignment(self.hover_alignment.unwrap_or(AlignmentOption::Center).into())
        .on_close(|| Message::OverlayClosed);

        // EXAMPLE 8: Same as 7, but with the interactive_tooltip helper
        let interactive_tooltip1 = interactive_tooltip("Hover to Open - Right", interactive_tooltip_overlay_content);

        let menu3 = column![
            button("Menu3 option 1").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu3 option 2").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu3 option 3").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu3 option 4").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),

        ].width(Length::Fill);

        let menu2 = column![
            button("Menu2 option 1").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 2").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 3").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu2 option 4").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            dropdown_menu("Menu3", menu3).style(button::subtle).width(Length::Fill),
        ].width(Length::Fill);

        let menu1 = column![
            button("Menu option 1").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 2").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 3").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            button("Menu option 4").on_press(Message::ButtonPressed).style(button::subtle).width(Length::Fill),
            dropdown_menu("Menu2", menu2).style(button::subtle).width(Length::Fill),
        ].width(Length::Fill);

        let nav_menu = container(row![
            dropdown_root("File", menu1),
        ].width(Length::Fill)).style(container::secondary);

        let on_hover_internal = container(
                overlay_button(
                        text_editor(&self.editor_content).width(500.0).height(400.0),
                    "Hidden",
                    button("C").width(Length::Shrink).height(Length::Shrink).on_press(Message::ButtonPressed).style(button::subtle),
                )
                .style(button::text)
                .hide_header()
                .on_hover()
                .overlay_padding(0.0)
                .overlay_padding(0.0)
                .hover_gap(self.hover_gap)
                .overlay_height(Length::Shrink)
                .overlay_width(Length::Shrink)
                .hover_position(self.hover_position.unwrap_or(Position::Right))
                .hover_mode(PositionMode::Inside)
                .hover_alignment(self.hover_alignment.unwrap_or(AlignmentOption::Start).into()),
        );

        // EXAMPLE 10: Dynamically sized based on parent viewport
        let dynamic_size = overlay_button(
            text("Open Dynamic Overlay"),
            "Dynamic Size Overlay Example",
            dynamic_size_overlay_content,
        )
        .overlay_width_dynamic(|window_width| Length::Fixed(window_width * 0.8))
        .overlay_height_dynamic(|window_height| Length::Fixed(window_height * 0.8))
        .reset_on_close()
        .id("dynamic_overlay");

        // EXAMPLE 11: Using a custom helper for making generic overlay easier to use in your application.
        let custom_overlay = my_custom_headerless_overlay(
            "custom helper", 
            custom_helper_overlay_content
        ).id("custom_helper_overlay");

        // Example 12: Controlling open state via app state
        let app_state_controlled = overlay_button(
            Option::<Element<'_, _>>::None,
            "Warning!",
            column![text("Do the thing!"), button("close").on_press(Message::ToggleOverlay(!self.overlay_status))].spacing(10.0)
        )
        .style(button::text)
        .hide_close_button() // remove the X in the header, keep the header
        .is_open(self.overlay_status)  // bool held in state
        .on_toggle(Message::ToggleOverlay) // change app state with internal open/close calls
        .overlay_style(generic_overlay::danger);

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
                                    Message::UpdatePosition
                                ).width(100),
                            ].spacing(5),
                            column![
                                text("Set Alignment"),
                                pick_list(
                                    AlignmentOption::ALL,
                                    self.hover_alignment,
                                    Message::UpdateAlignment
                                ).width(100),
                            ].spacing(5),
                        ].spacing(10),
                        on_hover_overlay,
                    ].spacing(10)
                ).center_x(Length::Fill),
                interactive_tooltip1,
                dynamic_size,
                custom_overlay,
                app_state_controlled,
                button("Some External Toggle for an overlay").on_press(Message::ToggleOverlay(!self.overlay_status)),
                on_hover_internal,
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


/// Creating your own helper function for overlays.
pub fn my_custom_headerless_overlay<'a, Message, Theme, Renderer>(
    button_label: impl Into<Element<'a, Message, Theme, Renderer>>,
    overlay_content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> OverlayButton<'a, Message, Theme, Renderer> 
where 
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer,
    Theme: widgets::generic_overlay::Catalog + button::Catalog,
{
    OverlayButton::new(button_label, "", overlay_content)
        .hide_header()
        .close_on_click_outside()
        .hover_positions_on_click()
        .reset_on_close()
        .opaque(true)
        .hover_gap(10.0)
        .overlay_height_dynamic(|h| iced::Length::Fixed(h * 0.6))
        .overlay_width_dynamic(|w| iced::Length::Fixed(w * 0.75))
        .overlay_padding(7.5)
        .overlay_radius(5.0)
}