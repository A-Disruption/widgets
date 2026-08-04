use iced::advanced::text::Renderer as _;
use iced::advanced::{Renderer, renderer, text};
use iced::{
    Border, Color, Font, Pixels, Point, Rectangle, Shadow, Size, Vector, alignment, border,
};
use std::f32::consts::PI;

use crate::flow_editor::geometry::Viewport2D;
use crate::flow_editor::hit_test;
use crate::flow_editor::model::{PaletteEntry, PortSide, Style};

pub fn quad(
    renderer: &mut iced::Renderer,
    bounds: Rectangle,
    background: Color,
    border_color: Color,
    border_width: f32,
    radius: f32,
    shadow: Shadow,
) {
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                color: border_color,
                width: border_width,
                radius: border::radius(radius),
            },
            shadow,
            snap: true,
        },
        background,
    );
}

pub fn line(renderer: &mut iced::Renderer, a: Point, b: Point, thickness: f32, color: Color) {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let width = (a.x - b.x).abs().max(thickness);
    let height = (a.y - b.y).abs().max(thickness);

    renderer.fill_quad(
        renderer::Quad {
            bounds: Rectangle {
                x,
                y,
                width,
                height,
            },
            border: Border::default(),
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
}

fn circle(renderer: &mut iced::Renderer, center: Point, radius: f32, color: Color) {
    quad(
        renderer,
        Rectangle {
            x: center.x - radius,
            y: center.y - radius,
            width: radius * 2.0,
            height: radius * 2.0,
        },
        color,
        Color::TRANSPARENT,
        0.0,
        999.0,
        Shadow::default(),
    );
}

const DOT_R: f32 = 1.55;
const ARC_STEPS: usize = 16;
const EDGE_CLEARANCE: f32 = 18.0;
const EDGE_CORNER_RADIUS: f32 = 16.0;
const ORTHO_EPS: f32 = 0.5;

fn stroke_dots(renderer: &mut iced::Renderer, a: Point, b: Point, radius: f32, color: Color) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    let steps = ((len / 2.0).ceil() as usize).max(1);

    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        circle(
            renderer,
            Point::new(a.x + dx * t, a.y + dy * t),
            radius,
            color,
        );
    }
}

fn seg_dots(renderer: &mut iced::Renderer, a: Point, b: Point, color: Color) {
    stroke_dots(renderer, a, b, DOT_R, color);
}

fn arc_dots(
    renderer: &mut iced::Renderer,
    center: Point,
    r: f32,
    start: f32,
    sweep: f32,
    color: Color,
) {
    for i in 0..=ARC_STEPS {
        let a = start + sweep * (i as f32 / ARC_STEPS as f32);
        circle(
            renderer,
            Point::new(center.x + r * a.cos(), center.y + r * a.sin()),
            DOT_R,
            color,
        );
    }
}

fn expand_rect(bounds: Rectangle, amount: f32) -> Rectangle {
    Rectangle {
        x: bounds.x - amount,
        y: bounds.y - amount,
        width: bounds.width + amount * 2.0,
        height: bounds.height + amount * 2.0,
    }
}

fn rect_left(bounds: Rectangle) -> f32 {
    bounds.x
}

fn rect_right(bounds: Rectangle) -> f32 {
    bounds.x + bounds.width
}

fn rect_top(bounds: Rectangle) -> f32 {
    bounds.y
}

fn rect_bottom(bounds: Rectangle) -> f32 {
    bounds.y + bounds.height
}

fn push_orthogonal_point(points: &mut Vec<Point>, point: Point) {
    if let Some(last) = points.last() {
        if (last.x - point.x).abs() <= ORTHO_EPS && (last.y - point.y).abs() <= ORTHO_EPS {
            return;
        }
    }

    if points.len() >= 2 {
        let prev = points[points.len() - 2];
        let last = points[points.len() - 1];
        let collinear_x =
            (prev.x - last.x).abs() <= ORTHO_EPS && (last.x - point.x).abs() <= ORTHO_EPS;
        let collinear_y =
            (prev.y - last.y).abs() <= ORTHO_EPS && (last.y - point.y).abs() <= ORTHO_EPS;

        if collinear_x || collinear_y {
            *points.last_mut().expect("orthogonal path has a point") = point;
            return;
        }
    }

    points.push(point);
}

pub fn edge_path_points(
    from: Point,
    from_side: PortSide,
    from_bounds: Rectangle,
    to: Point,
    to_side: PortSide,
    to_bounds: Rectangle,
) -> Vec<Point> {
    let source = expand_rect(from_bounds, EDGE_CLEARANCE);
    let target = expand_rect(to_bounds, EDGE_CLEARANCE);
    let mut points = Vec::with_capacity(6);
    push_orthogonal_point(&mut points, from);

    let can_use_middle_channel = match (from_side, to_side) {
        (PortSide::Output, PortSide::Input) => rect_right(source) <= rect_left(target),
        (PortSide::Input, PortSide::Output) => rect_right(target) <= rect_left(source),
        _ => false,
    };

    if can_use_middle_channel {
        let channel_x = match (from_side, to_side) {
            (PortSide::Output, PortSide::Input) => (rect_right(source) + rect_left(target)) * 0.5,
            (PortSide::Input, PortSide::Output) => (rect_left(source) + rect_right(target)) * 0.5,
            _ => unreachable!("middle channel only applies to opposing sides"),
        };

        push_orthogonal_point(&mut points, Point::new(channel_x, from.y));
        push_orthogonal_point(&mut points, Point::new(channel_x, to.y));
        push_orthogonal_point(&mut points, to);
        return points;
    }

    let source_edge_x = match from_side {
        PortSide::Output => rect_right(source),
        PortSide::Input => rect_left(source),
    };
    let target_edge_x = match to_side {
        PortSide::Input => rect_left(target),
        PortSide::Output => rect_right(target),
    };

    let vertical_gap_lane = if rect_bottom(source) <= rect_top(target) {
        Some((rect_bottom(source) + rect_top(target)) * 0.5)
    } else if rect_bottom(target) <= rect_top(source) {
        Some((rect_bottom(target) + rect_top(source)) * 0.5)
    } else {
        None
    };

    if let Some(lane_y) = vertical_gap_lane {
        push_orthogonal_point(&mut points, Point::new(source_edge_x, from.y));
        push_orthogonal_point(&mut points, Point::new(source_edge_x, lane_y));
        push_orthogonal_point(&mut points, Point::new(target_edge_x, lane_y));
        push_orthogonal_point(&mut points, Point::new(target_edge_x, to.y));
        push_orthogonal_point(&mut points, to);
        return points;
    }

    let lane_y_above = rect_top(source).min(rect_top(target)) - EDGE_CLEARANCE;
    let lane_y_below = rect_bottom(source).max(rect_bottom(target)) + EDGE_CLEARANCE;
    let above_cost = (from.y - lane_y_above).abs() + (to.y - lane_y_above).abs();
    let below_cost = (from.y - lane_y_below).abs() + (to.y - lane_y_below).abs();
    let lane_y = if above_cost <= below_cost {
        lane_y_above
    } else {
        lane_y_below
    };

    let outer_x = match from_side {
        PortSide::Output => rect_right(source).max(rect_right(target)) + EDGE_CLEARANCE,
        PortSide::Input => rect_left(source).min(rect_left(target)) - EDGE_CLEARANCE,
    };
    push_orthogonal_point(&mut points, Point::new(outer_x, from.y));
    push_orthogonal_point(&mut points, Point::new(outer_x, lane_y));
    push_orthogonal_point(&mut points, Point::new(target_edge_x, lane_y));
    push_orthogonal_point(&mut points, Point::new(target_edge_x, to.y));
    push_orthogonal_point(&mut points, to);
    points
}

pub fn edge_path_midpoint(points: &[Point]) -> Point {
    if points.is_empty() {
        return Point::ORIGIN;
    }
    if points.len() == 1 {
        return points[0];
    }

    let total_len: f32 = points
        .windows(2)
        .map(|segment| {
            let a = segment[0];
            let b = segment[1];
            (b.x - a.x).abs() + (b.y - a.y).abs()
        })
        .sum();

    if total_len <= ORTHO_EPS {
        return points[0];
    }

    let target_len = total_len * 0.5;
    let mut walked = 0.0;

    for segment in points.windows(2) {
        let a = segment[0];
        let b = segment[1];
        let seg_len = (b.x - a.x).abs() + (b.y - a.y).abs();

        if seg_len <= ORTHO_EPS {
            continue;
        }

        if walked + seg_len >= target_len {
            let t = (target_len - walked) / seg_len;
            return Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }

        walked += seg_len;
    }

    *points.last().unwrap_or(&Point::ORIGIN)
}

fn point_segment_distance(point: Point, a: Point, b: Point) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let len_sq = ab_x * ab_x + ab_y * ab_y;

    if len_sq <= ORTHO_EPS {
        let dx = point.x - a.x;
        let dy = point.y - a.y;
        return (dx * dx + dy * dy).sqrt();
    }

    let t = (((point.x - a.x) * ab_x) + ((point.y - a.y) * ab_y)) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let closest = Point::new(a.x + ab_x * t, a.y + ab_y * t);
    let dx = point.x - closest.x;
    let dy = point.y - closest.y;
    (dx * dx + dy * dy).sqrt()
}

pub fn edge_path_hits_point(points: &[Point], point: Point, tolerance: f32) -> bool {
    points
        .windows(2)
        .any(|segment| point_segment_distance(point, segment[0], segment[1]) <= tolerance)
}

pub fn edge_path_distance(points: &[Point], point: Point) -> f32 {
    points
        .windows(2)
        .map(|segment| point_segment_distance(point, segment[0], segment[1]))
        .fold(f32::INFINITY, f32::min)
}

pub fn edge_path(renderer: &mut iced::Renderer, points: &[Point], color: Color) {
    if points.len() < 2 {
        return;
    }

    let mut current = points[0];

    for index in 1..points.len() - 1 {
        let corner = points[index];
        let next = points[index + 1];
        let dx1 = corner.x - current.x;
        let dy1 = corner.y - current.y;
        let dx2 = next.x - corner.x;
        let dy2 = next.y - corner.y;

        if (dx1.abs() <= ORTHO_EPS && dy1.abs() <= ORTHO_EPS)
            || (dx2.abs() <= ORTHO_EPS && dy2.abs() <= ORTHO_EPS)
        {
            continue;
        }

        let same_axis = (dx1.abs() <= ORTHO_EPS && dx2.abs() <= ORTHO_EPS)
            || (dy1.abs() <= ORTHO_EPS && dy2.abs() <= ORTHO_EPS);
        if same_axis {
            seg_dots(renderer, current, corner, color);
            current = corner;
            continue;
        }

        let len1 = dx1.abs().max(dy1.abs());
        let len2 = dx2.abs().max(dy2.abs());
        let radius = len1.min(len2).min(EDGE_CORNER_RADIUS);

        if radius <= ORTHO_EPS {
            seg_dots(renderer, current, corner, color);
            current = corner;
            continue;
        }

        let before = if dx1.abs() > ORTHO_EPS {
            Point::new(corner.x - dx1.signum() * radius, corner.y)
        } else {
            Point::new(corner.x, corner.y - dy1.signum() * radius)
        };
        let after = if dx2.abs() > ORTHO_EPS {
            Point::new(corner.x + dx2.signum() * radius, corner.y)
        } else {
            Point::new(corner.x, corner.y + dy2.signum() * radius)
        };

        seg_dots(renderer, current, before, color);

        let center = if dx1.abs() > ORTHO_EPS {
            Point::new(before.x, after.y)
        } else {
            Point::new(after.x, before.y)
        };
        let start = (before.y - center.y).atan2(before.x - center.x);
        let end = (after.y - center.y).atan2(after.x - center.x);
        let mut sweep = end - start;
        while sweep <= -PI {
            sweep += PI * 2.0;
        }
        while sweep > PI {
            sweep -= PI * 2.0;
        }

        arc_dots(renderer, center, radius, start, sweep, color);
        current = after;
    }

    seg_dots(
        renderer,
        current,
        *points.last().expect("path has a last point"),
        color,
    );
}

pub fn bezier_edge(
    renderer: &mut iced::Renderer,
    from: Point,
    from_side: PortSide,
    from_bounds: Rectangle,
    to: Point,
    to_side: PortSide,
    to_bounds: Rectangle,
    color: Color,
) {
    let points = edge_path_points(from, from_side, from_bounds, to, to_side, to_bounds);
    edge_path(renderer, &points, color);
}

pub fn edge_delete_button(
    renderer: &mut iced::Renderer,
    bounds: Rectangle,
    background: Color,
    border: Color,
    text: Color,
) {
    quad(
        renderer,
        bounds,
        background,
        border,
        1.0,
        bounds.height * 0.5,
        Shadow {
            color: Color::from_rgba8(0, 0, 0, 0.18),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 6.0,
        },
    );

    let inset = bounds.width * 0.31;
    let stroke = (bounds.width * 0.075).clamp(0.8, 1.25);
    let a = Point::new(bounds.x + inset, bounds.y + inset);
    let b = Point::new(
        bounds.x + bounds.width - inset,
        bounds.y + bounds.height - inset,
    );
    let c = Point::new(bounds.x + bounds.width - inset, bounds.y + inset);
    let d = Point::new(bounds.x + inset, bounds.y + bounds.height - inset);
    stroke_dots(renderer, a, b, stroke, text);
    stroke_dots(renderer, c, d, stroke, text);
}

pub fn selection_rect(
    renderer: &mut iced::Renderer,
    bounds: Rectangle,
    fill: Color,
    border: Color,
) {
    quad(renderer, bounds, fill, border, 1.0, 10.0, Shadow::default());
}

pub fn palette(
    renderer: &mut iced::Renderer,
    entries: &[PaletteEntry],
    viewport: Viewport2D,
    root_bounds: Rectangle,
    origin_scene: Point,
    colors: &Style,
) {
    for item in hit_test::palette_items(entries, origin_scene) {
        let entry = entries
            .iter()
            .find(|e| e.id == item.template_id)
            .expect("palette entry always exists");
        let rect = viewport.scene_rect_to_screen(item.bounds, root_bounds);
        quad(
            renderer,
            rect,
            colors.palette_bg,
            colors.palette_item_border,
            1.0,
            12.0,
            Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.25),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 18.0,
            },
        );

        quad(
            renderer,
            Rectangle {
                x: rect.x + 8.0,
                y: rect.y + 8.0,
                width: 8.0,
                height: rect.height - 16.0,
            },
            entry.accent_color,
            Color::TRANSPARENT,
            0.0,
            999.0,
            Shadow::default(),
        );

        let lbl_size = (13.0 * viewport.zoom).clamp(4.0, 26.0);
        label(
            renderer,
            root_bounds,
            Point::new(rect.x + 24.0, rect.y + (rect.height - lbl_size) * 0.5),
            rect.width - 32.0,
            entry.label,
            lbl_size,
            colors.palette_text,
        );
    }
}

pub fn port_tab(renderer: &mut iced::Renderer, bounds: Rectangle, color: Color, radius: [f32; 4]) {
    use iced::border::Radius;

    renderer.fill_quad(
        renderer::Quad {
            bounds,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius {
                    top_left: radius[0],
                    top_right: radius[1],
                    bottom_right: radius[2],
                    bottom_left: radius[3],
                },
            },
            shadow: Shadow::default(),
            snap: true,
        },
        color,
    );
}

pub fn label(
    renderer: &mut iced::Renderer,
    clip_bounds: Rectangle,
    position: Point,
    max_width: f32,
    content: &str,
    size: f32,
    color: Color,
) {
    if content.is_empty() {
        return;
    }

    renderer.fill_text(
        text::Text {
            content: content.to_owned(),
            bounds: Size::new(max_width.max(1.0), size * 1.4),
            size: Pixels(size),
            line_height: text::LineHeight::default(),
            font: Font::DEFAULT,
            align_x: text::Alignment::Left,
            align_y: alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
            ellipsis: iced::advanced::text::Ellipsis::None,
            hint_factor: None,
        },
        position,
        color,
        clip_bounds,
    );
}
