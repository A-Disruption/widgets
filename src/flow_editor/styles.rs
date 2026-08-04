use iced::border;
use iced::widget::{button, container, pick_list};
use iced::{Background, Border, Color, Shadow, Theme};

use crate::flow_editor::model::Style;

/// Side-panel container (e.g. a properties panel beside the canvas).
/// Colors derived from the active editor `Style`.
pub fn panel_style(_theme: &Theme, style: &Style) -> container::Style {
    container::Style::default()
        .background(style.node_bg)
        .color(style.title_text)
        .border(Border {
            color: style.node_border,
            width: 1.0,
            radius: border::right(18.0),
        })
}

/// Small pill / badge container.
/// Tinted with the selection-highlight colour from the editor `Style`.
pub fn pill_style(_theme: &Theme, style: &Style) -> container::Style {
    let accent = style.node_border_selected;
    container::Style::default()
        .background(Color { a: 0.14, ..accent })
        .color(Color {
            r: accent.r * 1.3,
            g: accent.g * 1.3,
            b: accent.b * 1.3,
            a: 1.0,
        })
        .border(Border {
            color: Color { a: 0.35, ..accent },
            width: 1.0,
            radius: border::radius(999.0),
        })
}

/// Transparent container used as a node body wrapper.
pub fn node_body_style(_theme: &Theme, style: &Style) -> container::Style {
    container::Style::default()
        .background(Background::Color(Color::TRANSPARENT))
        .color(style.title_text)
}

/// Red "danger" icon button (e.g. delete).
pub fn danger_icon_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let danger = theme.palette().danger;
    let (bg, border, text) = match status {
        button::Status::Hovered => (danger.base.color, danger.strong.color, danger.base.text),
        button::Status::Pressed => (danger.strong.color, danger.strong.color, danger.strong.text),
        button::Status::Disabled => (
            Color {
                a: 0.45,
                ..danger.weak.color
            },
            Color {
                a: 0.30,
                ..danger.base.color
            },
            Color {
                a: 0.45,
                ..danger.weak.text
            },
        ),
        button::Status::Active => (danger.weak.color, danger.base.color, danger.weak.text),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text,
        border: Border {
            color: border,
            width: 1.0,
            radius: border::radius(6.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Blue "add / action" icon button.
pub fn add_icon_button_style(
    _theme: &Theme,
    status: button::Status,
    style: &Style,
) -> button::Style {
    let accent = style.node_border_selected;
    let (bg, border, text) = match status {
        button::Status::Hovered => (
            Color { a: 0.40, ..accent },
            Color { a: 0.80, ..accent },
            style.title_text,
        ),
        button::Status::Pressed => (Color { a: 0.55, ..accent }, accent, style.title_text),
        button::Status::Disabled => (
            Color { a: 0.15, ..accent },
            Color { a: 0.20, ..accent },
            Color {
                a: 0.40,
                ..style.title_text
            },
        ),
        button::Status::Active => (
            Color { a: 0.28, ..accent },
            Color { a: 0.55, ..accent },
            style.kind_label_text,
        ),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: text,
        border: Border {
            color: border,
            width: 1.0,
            radius: border::radius(6.0),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

/// Style a `pick_list` so it blends into the node header area.
/// Apply via `.style(|theme, status| header_pick_list_style(theme, status, &editor_style))`.
pub fn header_pick_list_style(
    _theme: &Theme,
    status: pick_list::Status,
    style: &Style,
) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Opened { .. } | pick_list::Status::Hovered => Color {
            a: 0.45,
            ..style.node_border_selected
        },
        pick_list::Status::Active | pick_list::Status::Disabled => Color::TRANSPARENT,
    };
    pick_list::Style {
        text_color: style.title_text,
        placeholder_color: style.kind_label_text,
        handle_color: style.kind_label_text,
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: border::radius(4.0),
        },
    }
}
