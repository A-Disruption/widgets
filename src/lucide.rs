//! The embedded Lucide icon font.
//!
//! Widgets in this crate that need a glyph of their own — the checkmark and
//! chevron drawn by [`crate::menu`], for instance — take it from here rather
//! than from the renderer's built-in icon font, whose shapes are heavier and
//! vary between renderers.
//!
//! # Registering the font
//!
//! The bytes are embedded in the binary, but iced still has to be told about
//! them once, at startup:
//!
//! ```rust,ignore
//! iced::application(App::new, App::update, App::view)
//!     .font(widgets::lucide::FONT_BYTES)
//!     .run()
//! ```
//!
//! Applications that build their fonts at runtime can use [`load`] instead.
//! Without either, glyphs from this font render as blanks.

use iced::{Font, widget::text};

/// The bytes of the embedded Lucide 1.27.0 icon font.
///
/// Register these with `iced::application(..).font(..)` before any widget in
/// this crate draws an icon. See the [module documentation](self).
pub const FONT_BYTES: &[u8] = include_bytes!("../assets/lucide.ttf");

/// The [`Font`] descriptor matching [`FONT_BYTES`].
pub const FONT: Font = Font::new("lucide");

/// Loads the embedded Lucide font at runtime.
///
/// Prefer registering [`FONT_BYTES`] on the application builder; this exists
/// for applications that assemble their fonts dynamically.
pub fn load() -> iced::Task<Result<(), iced::font::Error>> {
    iced::font::load(FONT_BYTES)
}

/// A glyph from the embedded Lucide font.
///
/// The variants are named after their Lucide glyph names, and the code points
/// are the private-use characters assigned by the 1.27.0 release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// Lucide's `check` glyph: a bare checkmark with no enclosing shape.
    Check,
    /// Lucide's `chevron-right` glyph.
    ChevronRight,
    /// Lucide's `chevron-left` glyph.
    ChevronLeft,
    /// Lucide's `chevron-down` glyph.
    ChevronDown,
    /// Lucide's `chevron-up` glyph.
    ChevronUp,
    /// Lucide's `file-plus` glyph.
    FilePlus,
    /// Lucide's `folder-open` glyph.
    FolderOpen,
    /// Lucide's `save` glyph.
    Save,
    /// Lucide's `scissors` glyph.
    Scissors,
    /// Lucide's `copy` glyph.
    Copy,
    /// Lucide's `clipboard` glyph.
    Clipboard,
    /// Lucide's `settings` glyph.
    Settings,
    /// Lucide's `info` glyph.
    Info,
    /// Lucide's `alert-triangle` glyph.
    TriangleAlert,
    /// Lucide's `check-circle-2` glyph.
    CircleCheck,
    /// Lucide's `circle-x` glyph.
    CircleX,
    /// Lucide's `x` glyph.
    X,
}

impl Icon {
    /// Returns the private-use character this [`Icon`] occupies.
    pub const fn character(self) -> char {
        match self {
            Self::Check => '\u{e06c}',
            Self::ChevronRight => '\u{e06f}',
            Self::ChevronLeft => '\u{e06e}',
            Self::ChevronDown => '\u{e06d}',
            Self::ChevronUp => '\u{e070}',
            Self::FilePlus => '\u{e0c9}',
            Self::FolderOpen => '\u{e247}',
            Self::Save => '\u{e14d}',
            Self::Scissors => '\u{e14e}',
            Self::Copy => '\u{e09e}',
            Self::Clipboard => '\u{e085}',
            Self::Settings => '\u{e154}',
            Self::Info => '\u{e0f9}',
            Self::TriangleAlert => '\u{e193}',
            Self::CircleCheck => '\u{e226}',
            Self::CircleX => '\u{e084}',
            Self::X => '\u{e1b2}',
        }
    }
}

/// Creates a text widget containing a Lucide [`Icon`].
pub fn icon<'a, Theme, Renderer>(icon: Icon) -> text::Text<'a, Theme, Renderer>
where
    Theme: text::Catalog + 'a,
    Renderer: iced::advanced::text::Renderer<Font = Font>,
{
    text(icon.character()).font(FONT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These code points are read off the 1.27.0 font in `assets/`. If the font
    /// is ever refreshed, re-extract them rather than assuming they held.
    #[test]
    fn code_points_match_the_embedded_font_release() {
        assert_eq!(Icon::Check.character(), '\u{e06c}');
        assert_eq!(Icon::ChevronLeft.character(), '\u{e06e}');
        assert_eq!(Icon::ChevronRight.character(), '\u{e06f}');
        assert_eq!(Icon::FilePlus.character(), '\u{e0c9}');
        assert_eq!(Icon::FolderOpen.character(), '\u{e247}');
        assert_eq!(Icon::Save.character(), '\u{e14d}');
    }

    /// Every glyph sits in the private-use area. A code point outside it means
    /// the wrong character was transcribed from the font.
    #[test]
    fn every_code_point_is_in_the_private_use_area() {
        for icon in [
            Icon::Check,
            Icon::ChevronRight,
            Icon::ChevronLeft,
            Icon::ChevronDown,
            Icon::ChevronUp,
            Icon::FilePlus,
            Icon::FolderOpen,
            Icon::Save,
            Icon::Scissors,
            Icon::Copy,
            Icon::Clipboard,
            Icon::Settings,
            Icon::Info,
            Icon::TriangleAlert,
            Icon::CircleCheck,
            Icon::CircleX,
            Icon::X,
        ] {
            let code = icon.character() as u32;

            assert!(
                (0xE000..=0xF8FF).contains(&code),
                "{icon:?} is at U+{code:04X}, outside the private-use area"
            );
        }
    }

    #[test]
    fn the_font_is_actually_embedded() {
        assert!(FONT_BYTES.len() > 100_000);
        // A TrueType file starts with the 0x00010000 version tag.
        assert_eq!(&FONT_BYTES[..4], &[0x00, 0x01, 0x00, 0x00]);
    }
}
