use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, checkbox, column, container, horizontal_rule, horizontal_space, pick_list, progress_bar, radio, row, scrollable, slider, text, text_input, toggler, vertical_space, Button, Column, Container, Radio, Row, Space, Text, TextInput
    },
    Alignment, Background, Border, Color, Element, Font, Length::{self, FillPortion}, Padding, Shadow, Task,
    Theme, Vector,
};
use std::collections::HashSet;
use widgets::tree::{branch, tree_handle, DropInfo, DropPosition};

#[derive(Debug, Clone)]
pub enum Message {
    TreeToggle(String),
    TreeSelect(HashSet<usize>),
    ButtonPressed,
    HandleBranchDropped(DropInfo),
}

pub struct App {
    selected_items: Option<HashSet<usize>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            selected_items: None,
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
            Message::TreeSelect(selected_ids) => {
                // This will always return your provided IDs, or internal IDs starting from 0 and incrementing up by 1 in the order the branches were provided.
                self.selected_items = Some(selected_ids);
                println!("Selected: {:?}", self.selected_items);
            }
            Message::ButtonPressed => {
                println!("🎉 BUTTON WAS PRESSED! 🎉");
            }
            Message::HandleBranchDropped(drop_info) => {
                // Tree Widget handles the drag and drop, this will always return your provided IDs, if none are provided, it will return 0 for all branch IDs.
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
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        let tree_widget = tree_handle(vec![
            branch(button("Fruit").on_press(Message::ButtonPressed)).with_id(10).block_dragging()
                .with_children(vec![
                    branch(text("Strawberries")).with_id(1),
                    branch(text("Blueberries")).with_id(2),
                    branch(container(text("Citrus")).padding(5)).with_id(3)
                        .with_children(vec![
                            branch(text("Oranges")).with_id(4),
                            branch(text("Lemons")).with_id(5),
                        ]).accepts_drops(),
                ]).accepts_drops(),
            branch(button("Vegetables").on_press(Message::ButtonPressed)).with_id(6)
                .with_children(vec![
                    branch(text("Carrots")).with_id(7),
                    branch(text("Broccoli")).with_id(8),
                ]).accepts_drops(),
            branch(
                row![
                    button("button1").on_press(Message::ButtonPressed),
                    horizontal_space(),
                    button("button2").on_press(Message::ButtonPressed)
                ].spacing(50) // If using a horizonal_space() inside a row, set the row to shrink or the branch will not render
            ).with_id(19).accepts_drops(),
        ])
        .on_drop(Message::HandleBranchDropped)
        .on_select(Message::TreeSelect);

        scrollable(
            column![
                iced::widget::text("Tree Widget Example").size(24),
                tree_widget,
            ]
            .width(400)
            .spacing(20)
            .padding(20)
        )
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