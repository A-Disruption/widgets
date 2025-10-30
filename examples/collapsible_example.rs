use iced::widget::{column, text, scrollable};
use iced::{Element, Theme};
use widgets::collapsible::{self, collapsible};
use widgets::collapsible_group;

#[derive(Debug, Clone)]
enum Message {
}


struct CollapsibleExample {}

impl CollapsibleExample {
    fn new() -> (Self, iced::Task<Message>) {
        (Self {}, iced::Task::none())
    }

    fn title(&self) -> String {
        String::from("Collapsible Group Example")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) {
        match message {

        }
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        scrollable(
            column![
                text("Collapsible Examples").size(25),

                text("Standalone").size(20),
                collapsible(
                    "Help Section",
                    column![
                        text("No State Stored in the App"),
                        text("Header is clickable by default, or limit to icon bounds"),
                    ]
                    .spacing(10)
                    .padding(15),
                ),
                
                collapsible(
                    "About Section (Starts Open)",
                    text("Can set .expanded(true) to start open"),
                )
                .expanded(true),
                
                text("Grouped, one open at a time").size(20),
                
                collapsible_group![
                    collapsible(
                        "General Settings",
                        column![
                            text("• Theme: Light/Dark"),
                            text("• Language: English"),
                            text("• Auto-save: Enabled"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                    
                    collapsible(
                        "Advanced Settings",
                        column![
                            text("• Debug mode: Off"),
                            text("• Cache size: 100MB"),
                            text("• Network timeout: 30s"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                    
                    collapsible(
                        "Privacy Settings",
                        column![
                            text("• Analytics: Disabled"),
                            text("• Crash reports: Enabled"),
                            text("• Usage data: Anonymous"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                ]
                .spacing(10.0),
                
                text("Group 2").size(20),
                
                collapsible_group![
                    collapsible(
                        "Profile",
                        column![
                            text("Name: A Disruption"),
                            text("Email: ADisruption@iced_discord.com"),
                            text("Member since: 2024"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                    
                    collapsible(
                        "Notifications",
                        column![
                            text("Email notifications: On"),
                            text("Push notifications: Off"),
                            text("Weekly digest: On"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                    
                    collapsible(
                        "Security",
                        column![
                            text("Two-factor auth: Enabled"),
                            text("Last login: Today"),
                            text("Active sessions: 2"),
                        ]
                        .spacing(5)
                        .padding(15),
                    ),
                ]
                .spacing(10.0),
                
                text("Styled").size(20),
                
                collapsible_group![
                    collapsible(
                        "Styled Section 1",
                        column![
                            text("Content with custom styling"),
                        ]
                        .spacing(5)
                        .padding(15),
                    )
                    .style(collapsible::primary),
                    
                    collapsible(
                        "Styled Section 2",
                        column![
                            text("Another styled section"),
                        ]
                        .spacing(5)
                        .padding(15),
                    )
                    .style(collapsible::success),
                    
                    collapsible(
                        "Styled Section 3",
                        column![
                            text("Danger color theme"),
                        ]
                        .spacing(5)
                        .padding(15),
                    )
                    .style(collapsible::danger),

                    collapsible(
                        "Styled Section 4",
                        column![
                            text("Warning color theme"),
                        ]
                        .spacing(5)
                        .padding(15),
                    )
                    .style(collapsible::warning),
                ]
                .spacing(10.0),
            ]
            .width(400)
            .spacing(20)
            .padding(20)
        ).into()
    }
}

fn main() -> iced::Result {
    iced::application(
        CollapsibleExample::new,
        CollapsibleExample::update,
        CollapsibleExample::view
    )
    .theme(CollapsibleExample::theme)
    .title(CollapsibleExample::title)
    .run()
}