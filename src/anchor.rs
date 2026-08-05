//! Placement of an overlay surface relative to a base rectangle.
//!
//! This module is the shared positioning core for every base-relative widget:
//! menus, popovers and interactive tooltips. It holds no state and knows
//! nothing about events, styling or the widget tree — it maps a base
//! rectangle, a surface size and a viewport onto a position.
//!
//! The placement strategy is *flip, then shift*:
//!
//! 1. The surface is placed on the requested [`Side`] of the base.
//! 2. If that side cannot fit the surface but the opposite side has more room,
//!    the surface flips to the opposite side.
//! 3. The surface is shifted along the cross axis until it lies inside the
//!    viewport.
//! 4. Only if the surface still overflows on the main axis is it clamped there.
//!
//! Flipping before clamping is what keeps a submenu next to its parent panel
//! near a screen edge instead of sliding on top of it.

use iced::{Point, Rectangle, Size};

/// The side of the base rectangle a surface is placed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    /// Above the base.
    Top,
    /// Below the base.
    #[default]
    Bottom,
    /// To the left of the base.
    Left,
    /// To the right of the base.
    Right,
}

impl Side {
    /// Returns the opposing [`Side`].
    pub fn flip(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Returns `true` when the main axis of this [`Side`] is horizontal.
    ///
    /// [`Side::Left`] and [`Side::Right`] place the surface along the x axis,
    /// which makes the y axis their cross axis.
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// The alignment of a surface along the cross axis of its [`Side`].
///
/// For [`Side::Top`] and [`Side::Bottom`] this aligns the surface horizontally;
/// for [`Side::Left`] and [`Side::Right`] it aligns it vertically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Align the leading edges of the surface and the base.
    #[default]
    Start,
    /// Center the surface on the base.
    Center,
    /// Align the trailing edges of the surface and the base.
    End,
}

/// A request to place a surface relative to a base rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// The preferred [`Side`] of the base.
    pub side: Side,
    /// The cross-axis [`Align`]ment.
    pub align: Align,
    /// The space between the base and the surface, in logical pixels.
    ///
    /// A negative gap overlaps the base, which submenus use to sit flush
    /// against the border of their parent panel.
    pub gap: f32,
    /// Whether the surface may flip to the opposite [`Side`] to fit.
    pub flip: bool,
}

impl Placement {
    /// Creates a [`Placement`] on the given [`Side`] with no gap, aligned to
    /// the start of the cross axis, and flipping enabled.
    pub fn new(side: Side) -> Self {
        Self {
            side,
            align: Align::Start,
            gap: 0.0,
            flip: true,
        }
    }

    /// Sets the cross-axis [`Align`]ment.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the gap between the base and the surface.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Sets whether the surface may flip to the opposite [`Side`] to fit.
    pub fn flip(mut self, flip: bool) -> Self {
        self.flip = flip;
        self
    }
}

impl Default for Placement {
    fn default() -> Self {
        Self::new(Side::default())
    }
}

/// The outcome of [`place`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    /// The top-left corner of the surface.
    pub position: Point,
    /// The [`Side`] the surface ended up on, after any flip.
    ///
    /// Callers use this to point a caret at the base, or to pick the direction
    /// an open animation slides in from.
    pub side: Side,
}

/// Returns the room available between the base and the edge of the viewport on
/// the given [`Side`].
pub fn room(base: Rectangle, viewport: Rectangle, side: Side) -> f32 {
    let room = match side {
        Side::Top => base.y - viewport.y,
        Side::Bottom => (viewport.y + viewport.height) - (base.y + base.height),
        Side::Left => base.x - viewport.x,
        Side::Right => (viewport.x + viewport.width) - (base.x + base.width),
    };

    room.max(0.0)
}

/// Places a surface of the given `size` relative to `base`, inside `viewport`.
///
/// See the [module documentation](self) for the strategy this uses.
pub fn place(
    base: Rectangle,
    size: Size,
    viewport: Rectangle,
    placement: Placement,
) -> Placed {
    let side = resolve_side(base, size, viewport, placement);

    let main = main_coordinate(base, size, side, placement.gap);
    let cross = cross_coordinate(base, size, side, placement.align);

    let (x, y) = if side.is_horizontal() {
        (main, cross)
    } else {
        (cross, main)
    };

    // The cross axis shifts to fit. The main axis reaches this point only
    // after flipping has already had its chance, so clamping it is the last
    // resort rather than the first move.
    Placed {
        position: Point::new(
            shift_into(x, size.width, viewport.x, viewport.width),
            shift_into(y, size.height, viewport.y, viewport.height),
        ),
        side,
    }
}

/// Picks the [`Side`] the surface is actually placed on.
///
/// The requested side wins whenever the surface fits. Otherwise the surface
/// flips, but only if the opposite side is genuinely roomier — flipping into an
/// equally cramped side would just trade one overflow for another.
fn resolve_side(
    base: Rectangle,
    size: Size,
    viewport: Rectangle,
    placement: Placement,
) -> Side {
    if !placement.flip {
        return placement.side;
    }

    let needed = if placement.side.is_horizontal() {
        size.width
    } else {
        size.height
    } + placement.gap;

    let preferred = room(base, viewport, placement.side);

    if preferred >= needed {
        return placement.side;
    }

    let opposite = placement.side.flip();

    if room(base, viewport, opposite) > preferred {
        opposite
    } else {
        placement.side
    }
}

/// Returns the main-axis coordinate of the surface: the one the [`Side`]
/// determines outright.
fn main_coordinate(base: Rectangle, size: Size, side: Side, gap: f32) -> f32 {
    match side {
        Side::Top => base.y - gap - size.height,
        Side::Bottom => base.y + base.height + gap,
        Side::Left => base.x - gap - size.width,
        Side::Right => base.x + base.width + gap,
    }
}

/// Returns the cross-axis coordinate of the surface, before any shifting.
fn cross_coordinate(base: Rectangle, size: Size, side: Side, align: Align) -> f32 {
    let (base_start, base_extent, extent) = if side.is_horizontal() {
        (base.y, base.height, size.height)
    } else {
        (base.x, base.width, size.width)
    };

    match align {
        Align::Start => base_start,
        Align::Center => base_start + (base_extent - extent) / 2.0,
        Align::End => base_start + base_extent - extent,
    }
}

/// Slides a span of length `extent` starting at `start` so that it lies inside
/// `[origin, origin + available]`.
///
/// A span too large to fit is aligned to the origin rather than pushed off the
/// leading edge, so its beginning always stays visible.
fn shift_into(start: f32, extent: f32, origin: f32, available: f32) -> f32 {
    if extent >= available {
        return origin;
    }

    start.clamp(origin, origin + available - extent)
}

/// Returns the two corners of a placed surface that face its base.
///
/// `extend` widens the pair outwards along the cross axis, which gives the
/// corridor below a more forgiving mouth when the surface and its base are
/// closely aligned.
pub fn facing_corners(surface: Rectangle, side: Side, extend: f32) -> (Point, Point) {
    match side {
        Side::Right => (
            Point::new(surface.x, surface.y - extend),
            Point::new(surface.x, surface.y + surface.height + extend),
        ),
        Side::Left => (
            Point::new(surface.x + surface.width, surface.y - extend),
            Point::new(surface.x + surface.width, surface.y + surface.height + extend),
        ),
        Side::Bottom => (
            Point::new(surface.x - extend, surface.y),
            Point::new(surface.x + surface.width + extend, surface.y),
        ),
        Side::Top => (
            Point::new(surface.x - extend, surface.y + surface.height),
            Point::new(surface.x + surface.width + extend, surface.y + surface.height),
        ),
    }
}

/// Returns `true` when `point` lies within the triangle `a`–`b`–`c`.
///
/// Uses the sign-of-cross-product test, so points exactly on an edge count as
/// inside.
pub fn point_in_triangle(point: Point, a: Point, b: Point, c: Point) -> bool {
    let sign = |p: Point, q: Point, r: Point| (p.x - r.x) * (q.y - r.y) - (q.x - r.x) * (p.y - r.y);

    let ab = sign(point, a, b);
    let bc = sign(point, b, c);
    let ca = sign(point, c, a);

    let has_negative = ab < 0.0 || bc < 0.0 || ca < 0.0;
    let has_positive = ab > 0.0 || bc > 0.0 || ca > 0.0;

    !(has_negative && has_positive)
}

/// Returns `true` when the cursor is inside the corridor between where it left
/// the base and the near edge of the surface.
///
/// A hover-triggered surface separated from its base by a gap would otherwise
/// close the instant the cursor entered that gap, making the surface
/// unreachable by any path except a perfectly straight one. Treating the
/// triangle swept from the cursor's last position on the base to the surface's
/// two facing corners as "still inside" lets the cursor cut a diagonal.
///
/// `from` is the last cursor position known to be over the base.
pub fn in_safe_corridor(
    cursor: Point,
    from: Point,
    surface: Rectangle,
    side: Side,
    extend: f32,
) -> bool {
    let (near, far) = facing_corners(surface, side, extend);

    point_in_triangle(cursor, from, near, far)
}

/// Where a surface sits in the viewport.
///
/// Used by the widgets that anchor to the window rather than to another
/// element — [`crate::layer`] and [`crate::toast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    /// The top-left corner.
    TopLeft,
    /// Centred against the top edge.
    Top,
    /// The top-right corner.
    TopRight,
    /// Centred against the left edge.
    Left,
    /// The middle of the viewport.
    #[default]
    Center,
    /// Centred against the right edge.
    Right,
    /// The bottom-left corner.
    BottomLeft,
    /// Centred against the bottom edge.
    Bottom,
    /// The bottom-right corner.
    BottomRight,
}

impl Anchor {
    /// Returns the top-left corner of a surface of `size` anchored here.
    pub fn position(self, size: Size, viewport: Rectangle, margin: f32) -> Point {
        let free_x = (viewport.width - size.width).max(0.0);
        let free_y = (viewport.height - size.height).max(0.0);

        let left = viewport.x + margin.min(free_x);
        let right = viewport.x + free_x - margin.min(free_x);
        let middle_x = viewport.x + free_x / 2.0;

        let top = viewport.y + margin.min(free_y);
        let bottom = viewport.y + free_y - margin.min(free_y);
        let middle_y = viewport.y + free_y / 2.0;

        match self {
            Self::TopLeft => Point::new(left, top),
            Self::Top => Point::new(middle_x, top),
            Self::TopRight => Point::new(right, top),
            Self::Left => Point::new(left, middle_y),
            Self::Center => Point::new(middle_x, middle_y),
            Self::Right => Point::new(right, middle_y),
            Self::BottomLeft => Point::new(left, bottom),
            Self::Bottom => Point::new(middle_x, bottom),
            Self::BottomRight => Point::new(right, bottom),
        }
    }

    /// Returns the viewport edge this anchor slides in from.
    ///
    /// A surface against an edge emerges from it. A centred one has no edge of
    /// its own, so it rises from the bottom, which is the convention every
    /// dialog uses.
    pub fn slide_from(self) -> Side {
        match self {
            Self::TopLeft | Self::Top | Self::TopRight => Side::Top,
            Self::Left => Side::Left,
            Self::Right => Side::Right,
            Self::BottomLeft | Self::Bottom | Self::BottomRight | Self::Center => Side::Bottom,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    fn base(x: f32, y: f32) -> Rectangle {
        Rectangle {
            x,
            y,
            width: 100.0,
            height: 20.0,
        }
    }

    fn size() -> Size {
        Size::new(160.0, 200.0)
    }

    #[test]
    fn places_on_the_requested_side_when_it_fits() {
        let placed = place(
            base(100.0, 100.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom).gap(4.0),
        );

        assert_eq!(placed.side, Side::Bottom);
        assert_eq!(placed.position, Point::new(100.0, 124.0));
    }

    #[test]
    fn flips_to_the_opposite_side_when_the_preferred_one_is_too_tight() {
        // Only 80px of room below, but 500px above.
        let placed = place(
            base(100.0, 500.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom).gap(4.0),
        );

        assert_eq!(placed.side, Side::Top);
        assert_eq!(placed.position, Point::new(100.0, 296.0));
    }

    /// The regression that motivated this module: a submenu near the right
    /// edge must flip to the left of its parent row, not slide across it.
    #[test]
    fn submenu_near_the_right_edge_flips_instead_of_covering_its_parent() {
        let row = Rectangle {
            x: 620.0,
            y: 100.0,
            width: 170.0,
            height: 24.0,
        };

        let placed = place(
            row,
            Size::new(180.0, 120.0),
            VIEWPORT,
            Placement::new(Side::Right),
        );

        assert_eq!(placed.side, Side::Left);
        assert_eq!(placed.position.x, 440.0);
        // The panel ends exactly where its parent begins; it never overlaps it.
        assert_eq!(placed.position.x + 180.0, row.x);
    }

    #[test]
    fn keeps_the_requested_side_when_flipping_would_not_help() {
        // A surface wider than the viewport has nowhere better to go.
        let placed = place(
            base(300.0, 100.0),
            Size::new(900.0, 100.0),
            VIEWPORT,
            Placement::new(Side::Right),
        );

        assert_eq!(placed.side, Side::Right);
        assert_eq!(placed.position.x, 0.0);
    }

    #[test]
    fn shifts_along_the_cross_axis_to_stay_inside_the_viewport() {
        // Aligned to the start, this would run 60px past the right edge.
        let placed = place(
            base(700.0, 100.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom),
        );

        assert_eq!(placed.side, Side::Bottom);
        assert_eq!(placed.position.x, 640.0);
    }

    #[test]
    fn does_not_flip_when_flipping_is_disabled() {
        let placed = place(
            base(100.0, 500.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom).flip(false),
        );

        assert_eq!(placed.side, Side::Bottom);
        // Still clamped into the viewport on the main axis.
        assert_eq!(placed.position.y, 400.0);
    }

    #[test]
    fn center_alignment_centers_the_surface_on_the_base() {
        let placed = place(
            base(300.0, 100.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom).align(Align::Center),
        );

        assert_eq!(placed.position.x, 270.0);
    }

    #[test]
    fn end_alignment_lines_up_the_trailing_edges() {
        let placed = place(
            base(300.0, 100.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Bottom).align(Align::End),
        );

        assert_eq!(placed.position.x, 240.0);
    }

    #[test]
    fn facing_corners_are_the_edge_nearest_the_base() {
        let surface = Rectangle {
            x: 200.0,
            y: 100.0,
            width: 160.0,
            height: 80.0,
        };

        // A surface to the right of its base presents its left edge.
        let (near, far) = facing_corners(surface, Side::Right, 0.0);
        assert_eq!(near, Point::new(200.0, 100.0));
        assert_eq!(far, Point::new(200.0, 180.0));

        // Below its base, it presents its top edge.
        let (near, far) = facing_corners(surface, Side::Bottom, 0.0);
        assert_eq!(near, Point::new(200.0, 100.0));
        assert_eq!(far, Point::new(360.0, 100.0));
    }

    #[test]
    fn extending_widens_the_mouth_of_the_corridor() {
        let surface = Rectangle {
            x: 200.0,
            y: 100.0,
            width: 160.0,
            height: 80.0,
        };

        let (near, far) = facing_corners(surface, Side::Right, 10.0);

        assert_eq!(near, Point::new(200.0, 90.0));
        assert_eq!(far, Point::new(200.0, 190.0));
    }

    #[test]
    fn a_point_inside_a_triangle_is_detected() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 0.0);
        let c = Point::new(0.0, 10.0);

        assert!(point_in_triangle(Point::new(2.0, 2.0), a, b, c));
        assert!(point_in_triangle(a, a, b, c), "a vertex counts as inside");
        assert!(!point_in_triangle(Point::new(9.0, 9.0), a, b, c));
    }

    /// The whole point of the corridor: a diagonal path from the base to a
    /// surface offset below it must not be treated as leaving.
    #[test]
    fn a_diagonal_path_to_the_surface_stays_in_the_corridor() {
        let surface = Rectangle {
            x: 200.0,
            y: 100.0,
            width: 160.0,
            height: 80.0,
        };

        // The cursor left the base here, heading down and to the right.
        let from = Point::new(150.0, 90.0);

        assert!(in_safe_corridor(
            Point::new(180.0, 110.0),
            from,
            surface,
            Side::Right,
            0.0
        ));

        // Wandering well above the corridor is genuinely leaving.
        assert!(!in_safe_corridor(
            Point::new(180.0, 20.0),
            from,
            surface,
            Side::Right,
            0.0
        ));
    }

    #[test]
    fn a_negative_gap_overlaps_the_base() {
        let placed = place(
            base(100.0, 100.0),
            size(),
            VIEWPORT,
            Placement::new(Side::Right).gap(-4.0),
        );

        assert_eq!(placed.position.x, 196.0);
    }
}
