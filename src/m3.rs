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
