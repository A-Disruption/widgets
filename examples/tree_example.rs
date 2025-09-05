use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, progress_bar, radio, row, scrollable, slider, text, text_input, toggler, vertical_space, Button, Column, Container, Radio, Row, Space, Text, TextInput
    },
    Alignment, Background, Border, Color, Element, Font, Length::{self, FillPortion}, Padding, Shadow, Task,
    Theme, Vector,
};
use std::collections::HashMap;
use widgets::tree::{branch, tree_handle, DropInfo, DropPosition};

#[derive(Debug, Clone)]
pub enum Message {
    TreeToggle(String),
    TreeSelect(String),
    ButtonPressed,
    HandleBranchDropped(DropInfo),
}

pub struct App {
    selected_item: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            selected_item: None,
        }
    }
    
    fn theme(&self) -> Theme {
        iced::Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        println!("🚀 APP.update called with message: {:?}", message);
        match message {
            Message::TreeToggle(id) => {
                println!("Toggled: {}", id);
                // Tree state is now managed internally by the widget
            }
            Message::TreeSelect(id) => {
                self.selected_item = Some(id.clone());
                println!("Selected: {}", id);
            }
            Message::ButtonPressed => {
                println!("🎉 BUTTON WAS PRESSED! 🎉");
            }
            Message::HandleBranchDropped(drop_info) => {
                // This is where you handle the actual reordering of your data
                println!("🎯 DROP OCCURRED!");
                println!("  Dragged IDs: {:?}", drop_info.dragged_ids);
                println!("  Target ID: {:?}", drop_info.target_id);
                println!("  Position: {:?}", drop_info.position);
                
                // Example of how to handle the drop:
                match drop_info.position {
                    DropPosition::Before => {
                        // Move dragged items before the target
                        // You would update your data structure here
                        println!("  -> Moving items BEFORE target");
                    }
                    DropPosition::After => {
                        // Move dragged items after the target
                        // You would update your data structure here
                        println!("  -> Moving items AFTER target");
                    }
                    DropPosition::Into => {
                        // Make dragged items children of the target
                        // You would update your data structure here
                        println!("  -> Moving items INTO target (as children)");
                    }
                }
                
                // After updating your data structure, you would typically
                // rebuild the tree widget in the view() method
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let tree_widget = tree_handle(vec![
            branch(button("Fruit").on_press(Message::ButtonPressed))
                .with_children(vec![
                    branch(text("Strawberries")),
                    branch(text("Blueberries")),
                    branch(container(text("Citrus")).padding(5))
                        .with_children(vec![
                            branch(text("Oranges")),
                            branch(text("Lemons")),
                        ]).accepts_drops(),
                ]).accepts_drops(),
            branch(button("Vegetables").on_press(Message::ButtonPressed))
                .with_children(vec![
                    branch(text("Carrots")),
                    branch(text("Broccoli")),
                ]).accepts_drops(),
            branch(
                row![
                    button("button1").on_press(Message::ButtonPressed), 
                    button("button2").on_press(Message::ButtonPressed)
                ].spacing(50)
            ).accepts_drops(),
        ]).on_drop(Message::HandleBranchDropped);

        column![
            iced::widget::text("Tree Widget Example").size(24),
            tree_widget,
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}