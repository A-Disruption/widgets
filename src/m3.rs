//! Material Design 3 styles for this crate's widgets.
//!
//! Enabled by the `m3_theme` feature, which pulls in the `material_theme`
//! crate. Without it, nothing here is compiled and `material_theme` is not a
//! dependency at all — so this crate stays usable with a plain
//! [`iced::Theme`].
//!
//! ```ignore
//! # widgets = { version = "*", features = ["color_picker_two", "m3_theme"] }
//! use widgets::color_picker_two::color_picker_two;
//! use widgets::m3;
//!
//! color_picker_two(is_open, color).style(m3::color_picker_two::material)
//! ```
//!
//! ## Adding a widget to this module
//!
//! Every style function follows the same three steps:
//!
//! 1. `let s = material_theme::scheme_for(theme);` — resolves the active M3
//!    scheme, falling back to an approximation for non-Material themes, so the
//!    style never panics or looks broken on `Theme::Dark`.
//! 2. Map each field of the widget's `Style` to a *role* on `s` (`s.surface`,
//!    `s.on_surface_variant`, …) rather than a literal color. This is what
//!    makes the widget track the user's seed color and light/dark mode.
//! 3. Use `material_theme::tokens` for state-layer opacities, the shape scale,
//!    and elevation, so interaction states match the built-in widgets exactly.
//!
//! ## Where M3 has no component
//!
//! Several widgets here have no counterpart in the specification — there is no
//! M3 tree, flow editor, or toast *variant*. Those are mapped onto the nearest
//! sanctioned pattern and the choice is recorded on the function, so the
//! deviation is visible rather than implied. Everything still comes from a
//! role, so it tracks the seed and the light/dark mode either way.

#[cfg(feature = "color_picker_two")]
pub mod color_picker_two {
    use iced::{Border, Theme};
    use material_theme::tokens::{elevation, shape, state};
    use material_theme::{mix_alpha, scheme_for};

    use crate::color_picker_two::{SWATCH_RADIUS, Status, Style};

    /// M3 styling for [`crate::color_picker_two`].
    ///
    /// Unlike `default_style`, this tracks light/dark mode and the seed color.
    pub fn material(theme: &Theme, _status: Status) -> Style {
        let s = scheme_for(theme);

        let hairline = |radius: f32| Border {
            color: s.outline_variant,
            width: 1.0,
            radius: radius.into(),
        };

        Style {
            // The picker floats above the app, so it sits on a raised
            // container at dialog elevation.
            background: s.surface_container_high,
            border: hairline(shape::LARGE),
            shadow: elevation(&s, 3),

            header_background: s.surface_container,
            header_divider: s.outline_variant,

            text_color: s.on_surface,
            muted_text_color: s.on_surface_variant,

            // Controls read as filled fields, matching `text_input::filled`.
            control_background: s.surface_container_highest,
            control_hover_background: mix_alpha(
                s.surface_container_highest,
                s.on_surface,
                state::HOVER,
            ),
            control_border: hairline(shape::SMALL),

            // The preview and swatches show arbitrary user colors, so they need
            // an outline that stays visible when the color matches the surface.
            preview_border: Border {
                color: s.outline,
                width: 1.0,
                radius: shape::SMALL.into(),
            },
            swatch_border: Border {
                color: s.outline,
                width: 1.0,
                // Must match what the widget draws, not the M3 shape scale.
                radius: SWATCH_RADIUS.into(),
            },

            slider_border: hairline(shape::SMALL),
            slider_value_color: s.on_surface_variant,

            swatch_add_background: s.surface_container_highest,
            swatch_add_text_color: s.on_surface_variant,

            selection_ring: s.primary,
        }
    }
}

#[cfg(feature = "collapsible")]
pub mod collapsible {
    use iced::{Border, Shadow, Theme};
    use material_theme::tokens::{shape, state};
    use material_theme::{mix_alpha, scheme_for};

    use crate::collapsible::{Status, Style};

    /// M3 styling for [`crate::collapsible`].
    ///
    /// The header is an M3 list item on `surface_container_high` and the body
    /// the `surface` beneath it, wrapped in an outlined-card border. The header
    /// takes the 8% hover / 10% pressed state layer.
    pub fn material(theme: &Theme, status: Status) -> Style {
        let s = scheme_for(theme);

        let header = match status {
            Status::Active => s.surface_container_high,
            Status::Hovered => mix_alpha(s.surface_container_high, s.on_surface, state::HOVER),
            Status::Pressed => mix_alpha(s.surface_container_high, s.on_surface, state::PRESSED),
        };

        Style {
            title_text_color: Some(s.on_surface),
            header_background: Some(header.into()),

            content_text_color: Some(s.on_surface_variant),
            content_background: Some(s.surface.into()),

            border: Border {
                color: s.outline_variant,
                width: 1.0,
                radius: shape::MEDIUM.into(),
            },

            // An outlined card carries no elevation, and the header seam is
            // the border rather than a shadow.
            shadow: Shadow::default(),
            header_shadow: Shadow::default(),
        }
    }
}

#[cfg(feature = "popover")]
pub mod popover {
    use iced::{Background, Border, Theme};
    use material_theme::scheme_for;
    use material_theme::tokens::{elevation, shape};

    use crate::popover::Style;

    /// M3 styling for [`crate::popover`].
    ///
    /// M3 has no popover; the closest sanctioned surface is the menu, so this
    /// uses the menu's container, shape and level-2 elevation. The radius here
    /// is overridden by `Popover::radius` when that is set.
    pub fn material(theme: &Theme) -> Style {
        let s = scheme_for(theme);

        Style {
            background: Background::Color(s.surface_container),
            border: Border {
                color: s.outline_variant,
                width: 1.0,
                radius: shape::EXTRA_SMALL.into(),
            },
            shadow: elevation(&s, 2),
            text_color: s.on_surface,
        }
    }
}

#[cfg(feature = "menu")]
pub mod menu {
    use iced::{Background, Border, Theme};
    use material_theme::tokens::{disabled, elevation, shape, state};
    use material_theme::{mix_alpha, scheme_for, with_alpha};

    use crate::menu::Style;

    /// M3 styling for [`crate::menu`].
    ///
    /// Panels are M3 menus: `surface_container`, 4dp corners, level-2
    /// elevation, rows highlighted with the 8% state layer and separated by
    /// `outline_variant`.
    ///
    /// The trigger has no M3 counterpart — the specification has no menu bar —
    /// so it is treated as a text button: no resting container, a state layer
    /// under the cursor, and a stronger one while its menu is open.
    pub fn material(theme: &Theme) -> Style {
        let s = scheme_for(theme);

        Style {
            background: Background::Color(s.surface_container),
            border: Border {
                color: s.outline_variant,
                width: 1.0,
                radius: shape::EXTRA_SMALL.into(),
            },
            shadow: elevation(&s, 2),

            text_color: s.on_surface,
            hovered_text_color: s.on_surface,
            disabled_text_color: with_alpha(s.on_surface, disabled::CONTENT),

            hovered_item_background: Background::Color(mix_alpha(
                s.surface_container,
                s.on_surface,
                state::HOVER,
            )),
            // M3 menu rows are full-bleed rectangles.
            item_radius: shape::NONE,

            separator_color: s.outline_variant,

            trigger_background: None,
            trigger_hovered_background: Some(Background::Color(with_alpha(
                s.on_surface,
                state::HOVER,
            ))),
            trigger_open_background: Some(Background::Color(with_alpha(
                s.on_surface,
                state::PRESSED,
            ))),
            trigger_text_color: s.on_surface_variant,
            trigger_active_text_color: s.on_surface,
            trigger_radius: shape::EXTRA_SMALL,
        }
    }
}

#[cfg(feature = "toast")]
pub mod toast {
    use iced::{Background, Border, Color, Theme};
    use material_theme::scheme_for;
    use material_theme::tokens::{elevation, shape};

    use crate::toast::{Style, Variant};

    /// M3 styling for [`crate::toast`].
    ///
    /// M3's snackbar is a single unvaried surface, and the specification has no
    /// success or warning role at all. Rather than invent colors, each variant
    /// is given an M3 *container* pair — those are contrast-verified against
    /// each other by the scheme generator, so every variant stays legible at
    /// any seed:
    ///
    /// | variant   | container            |
    /// |-----------|----------------------|
    /// | `Neutral` | `surface_container_high` |
    /// | `Info`    | `primary_container`   |
    /// | `Success` | `tertiary_container`  |
    /// | `Warning` | `secondary_container` |
    /// | `Danger`  | `error_container`     |
    pub fn material(theme: &Theme, variant: Variant) -> Style {
        let s = scheme_for(theme);

        let (background, text_color, accent): (Color, Color, Color) = match variant {
            Variant::Neutral => (s.surface_container_high, s.on_surface, s.on_surface_variant),
            Variant::Info => (s.primary_container, s.on_primary_container, s.primary),
            Variant::Success => (s.tertiary_container, s.on_tertiary_container, s.tertiary),
            Variant::Warning => (s.secondary_container, s.on_secondary_container, s.secondary),
            Variant::Danger => (s.error_container, s.on_error_container, s.error),
        };

        Style {
            background: Background::Color(background),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: shape::EXTRA_SMALL.into(),
            },
            shadow: elevation(&s, 3),
            text_color,
            countdown: Background::Color(accent),
        }
    }
}

#[cfg(feature = "layer")]
pub mod layer {
    use iced::{Background, Border, Theme};
    use material_theme::tokens::{elevation, shape, state};
    use material_theme::{scheme_for, with_alpha};

    use crate::layer::Style;

    /// M3 styling for [`crate::layer`].
    ///
    /// An M3 dialog: `surface_container_high`, 28dp corners, level-3 elevation,
    /// dimmed by the scheme's own `scrim`. The header shares the container so
    /// the title bar is seamless — the border color draws the seam under it.
    pub fn material(theme: &Theme) -> Style {
        let s = scheme_for(theme);

        Style {
            background: Background::Color(s.surface_container_high),
            border: Border {
                color: s.outline_variant,
                width: 1.0,
                radius: shape::EXTRA_LARGE.into(),
            },
            shadow: elevation(&s, 3),
            text_color: s.on_surface,

            // The alpha is replaced by whatever `Layer::backdrop` was given.
            backdrop_color: s.scrim,

            header_background: Background::Color(s.surface_container_high),
            title_color: s.on_surface,
            close_background: with_alpha(s.on_surface, state::HOVER),
        }
    }
}

#[cfg(feature = "tree")]
pub mod tree {
    use iced::Theme;
    use material_theme::scheme_for;

    use crate::tree::Style;

    /// M3 styling for [`crate::tree`].
    ///
    /// M3 has no tree, so rows are treated as list items: a selected row is the
    /// `secondary_container` chip M3 uses for list selection, and takes no
    /// border of its own — `selection_border` matches its fill for that reason.
    /// Keyboard focus uses `secondary`, the M3 focus-indicator role.
    pub fn material(theme: &Theme) -> Style {
        let s = scheme_for(theme);

        Style {
            text: s.on_surface,

            selection_background: s.secondary_container,
            selection_text: s.on_secondary_container,
            selection_border: s.secondary_container,
            focus_border: s.secondary,

            arrow_color: s.on_surface_variant,
            line_color: s.outline_variant,

            accept_drop_indicator_color: s.primary,
            deny_drop_indicator_color: s.error,
        }
    }
}

#[cfg(feature = "date_picker")]
pub mod date_picker {
    use iced::{Border, Theme};
    use material_theme::tokens::{elevation, shape, state};
    use material_theme::{scheme_for, with_alpha};

    use crate::date_picker::{Status, Style};

    /// M3 styling for [`crate::date_picker`].
    ///
    /// Follows the M3 date picker: a 28dp `surface_container_high` dialog at
    /// level-3 elevation, `primary` / `on_primary` for the selected day,
    /// `secondary_container` for the days between two endpoints, and the time
    /// field's active segment on `primary_container`.
    pub fn material(theme: &Theme, _status: Status) -> Style {
        let s = scheme_for(theme);

        Style {
            background: s.surface_container_high,
            border: Border {
                color: s.outline_variant,
                width: 1.0,
                radius: shape::EXTRA_LARGE.into(),
            },
            shadow: elevation(&s, 3),

            // M3 keeps the headline on the dialog container rather than
            // giving it a fill of its own.
            header_background: s.surface_container_high,
            header_text_color: s.on_surface_variant,

            text_color: s.on_surface,
            dim_text_color: s.on_surface_variant,

            // Today is an outlined cell in M3; with no outline field to set,
            // the tint carries it.
            today_background: with_alpha(s.primary, state::HOVER),
            today_text_color: s.primary,

            selected_background: s.primary,
            selected_text_color: s.on_primary,
            range_background: s.secondary_container,

            // Day cells are circular.
            hover_border: Border {
                color: s.outline,
                width: 1.0,
                radius: shape::FULL.into(),
            },

            arrow_color: s.on_surface_variant,
            arrow_hover_background: with_alpha(s.on_surface, state::HOVER),

            footer_button_text_color: s.primary,
            footer_button_hover_background: with_alpha(s.primary, state::HOVER),

            time_background: s.surface_container_highest,
            time_border: Border {
                color: s.outline,
                width: 1.0,
                radius: shape::EXTRA_SMALL.into(),
            },
            time_active_background: s.primary_container,
            time_active_text_color: s.on_primary_container,
        }
    }
}

/// Every style function has to be accepted by the widget it styles.
///
/// Building a `Style` correctly is only half of it: the *signature* has to
/// match the widget's `StyleFn` too, and a mismatch — a missing `Status`
/// argument, say — would otherwise only surface for whoever first wrote
/// `.style(m3::…::material)`. Coercing each one here turns that into a build
/// error in this crate.
///
/// Deliberately not `#[cfg(test)]`: this is a type-check, so it is worth
/// paying on every `cargo check` rather than only under `cargo test`.
#[allow(dead_code)]
fn assert_style_fn_signatures() {
    use iced::Theme;

    #[cfg(feature = "collapsible")]
    let _: crate::collapsible::StyleFn<'_, Theme> = Box::new(collapsible::material);

    #[cfg(feature = "color_picker_two")]
    let _: crate::color_picker_two::StyleFn<'_, Theme> = Box::new(color_picker_two::material);

    #[cfg(feature = "date_picker")]
    let _: crate::date_picker::StyleFn<'_, Theme> = Box::new(date_picker::material);

    #[cfg(feature = "layer")]
    let _: crate::layer::StyleFn<'_, Theme> = Box::new(layer::material);

    #[cfg(feature = "menu")]
    let _: crate::menu::StyleFn<'_, Theme> = Box::new(menu::material);

    #[cfg(feature = "popover")]
    let _: crate::popover::StyleFn<'_, Theme> = Box::new(popover::material);

    #[cfg(feature = "toast")]
    let _: crate::toast::StyleFn<'_, Theme> = Box::new(toast::material);

    #[cfg(feature = "tree")]
    let _: crate::tree::StyleFn<'_, Theme> = Box::new(tree::material);
}
