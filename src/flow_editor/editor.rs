use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
    widget::{Operation, Tree},
};
use iced::{
    Color, Element, Event, Length, Point, Rectangle, Renderer, Shadow, Size, Transformation,
    Vector, keyboard,
};

use iced::advanced::Renderer as _;

use crate::flow_editor::{
    draw,
    geometry::Viewport2D,
    hit_test,
    model::{
        Catalog, ConnectionPreview, ContextPalette, DragState, FlowNode, NodeContent, NodeId,
        PaletteEntry, PanState, PortDef, PortId, PortSide, SelectionRect, Style, StyleFn,
    },
};

// Re-export edge from model via the parent module
use crate::flow_editor::model::Edge;

// Action

#[derive(Debug, Clone, Copy)]
pub enum Action {
    UpdateModifiers {
        modifiers: keyboard::Modifiers,
    },

    SelectSingle(NodeId),
    ClearSelection,

    StartNodeDrag {
        id: NodeId,
        cursor_scene: Point,
    },
    DragNodeTo {
        cursor_scene: Point,
    },
    EndNodeDrag,

    StartPan {
        cursor_screen: Point,
    },
    PanTo {
        cursor_screen: Point,
    },
    EndPan,

    StartSelection {
        cursor_scene: Point,
        additive: bool,
    },
    UpdateSelection {
        cursor_scene: Point,
    },
    FinishSelection,

    StartConnection {
        node: NodeId,
        port: PortId,
        side: PortSide,
        cursor_scene: Point,
    },
    UpdateConnectionPreview {
        cursor_scene: Point,
    },
    FinishConnection {
        from_node: NodeId,
        from_port: PortId,
        to_node: NodeId,
        to_port: PortId,
    },
    CancelConnection,

    ZoomAt {
        cursor_screen: Point,
        root_bounds: Rectangle,
        delta: f32,
    },

    DeleteSelected,
    DeleteEdge(Edge),
    DuplicateSelected,
    ToggleSelectedEnabled,
    CopySelected,
    CutSelected,
    Paste,
    CenterView,

    OpenContextPalette {
        cursor_scene: Point,
    },
    OpenContextPaletteFromConnection {
        cursor_scene: Point,
        from_node: NodeId,
        from_port: PortId,
        side: PortSide,
    },
    CloseContextPalette,
    /// `template_id` is the opaque id from `PaletteEntry`. The host app
    /// interprets it to create the correct node type.
    CreateNodeFromTemplate {
        template_id: u64,
        position: Point,
    },
}

// Overlay hit regions

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChildSlotKind {
    Header,
    Body,
    SelectedTop,
    SelectedBottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChildSlot {
    content_idx: usize,
    kind: ChildSlotKind,
}

impl ChildSlot {
    fn uses_canvas_transform(self) -> bool {
        matches!(
            self.kind,
            ChildSlotKind::Header
                | ChildSlotKind::Body
                | ChildSlotKind::SelectedTop
                | ChildSlotKind::SelectedBottom
        )
    }

    fn is_selected_overlay(self) -> bool {
        matches!(
            self.kind,
            ChildSlotKind::SelectedTop | ChildSlotKind::SelectedBottom
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorOwner {
    CanvasNode(usize),
    SelectedOverlay(usize),
}

#[derive(Debug, Clone, Copy)]
struct HoveredEdge {
    edge: Edge,
    button_bounds: Rectangle,
    button_hovered: bool,
}

// FlowEditor widget

pub struct FlowEditor<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    nodes: &'a [FlowNode],
    z_order: &'a [NodeId],
    edges: &'a [Edge],
    viewport: Viewport2D,
    drag: Option<DragState>,
    pan: Option<PanState>,
    selection: Option<SelectionRect>,
    preview: Option<ConnectionPreview>,
    context_palette: Option<ContextPalette>,
    modifiers: keyboard::Modifiers,
    palette_entries: &'a [PaletteEntry],
    content: Vec<NodeContent<'a, Message, Theme>>,
    on_action: Option<Box<dyn Fn(Action) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message: 'a, Theme: Catalog> FlowEditor<'a, Message, Theme> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        nodes: &'a [FlowNode],
        z_order: &'a [NodeId],
        edges: &'a [Edge],
        viewport: Viewport2D,
        drag: Option<DragState>,
        pan: Option<PanState>,
        selection: Option<SelectionRect>,
        preview: Option<ConnectionPreview>,
        context_palette: Option<ContextPalette>,
        modifiers: keyboard::Modifiers,
        palette_entries: &'a [PaletteEntry],
        content: Vec<NodeContent<'a, Message, Theme>>,
    ) -> Self {
        Self {
            nodes,
            z_order,
            edges,
            viewport,
            drag,
            pan,
            selection,
            preview,
            context_palette,
            modifiers,
            palette_entries,
            content,
            on_action: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Set a style closure that derives colours from the active `Theme`.
    ///
    /// ```rust,ignore
    /// .style(|theme| flow_editor::Style {
    ///     canvas_bg: theme.extended_palette().background.base.color,
    ///     ..flow_editor::Style::from_theme(theme)
    /// })
    /// ```
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Set the style class directly (for custom `Catalog` implementations).
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    pub fn on_action(mut self, callback: impl Fn(Action) -> Message + 'a) -> Self {
        self.on_action = Some(Box::new(callback));
        self
    }

    fn publish(&self, shell: &mut Shell<'_, Message>, action: Action) {
        if let Some(callback) = &self.on_action {
            shell.publish(callback(action));
        }
    }

    fn ordered_nodes(&self) -> Vec<FlowNode> {
        self.z_order
            .iter()
            .filter_map(|id| self.nodes.iter().find(|node| node.id == *id))
            .cloned()
            .collect()
    }

    fn ordered_node_at(&self, index: usize) -> Option<&FlowNode> {
        self.z_order
            .get(index)
            .and_then(|id| self.nodes.iter().find(|node| node.id == *id))
    }

    /// Flat ordered list of child widgets.
    /// The position in this vec matches the iced widget-tree child index.
    fn child_slots(&self) -> Vec<ChildSlot> {
        let mut slots = Vec::new();
        for (i, c) in self.content.iter().enumerate() {
            if c.header.is_some() {
                slots.push(ChildSlot {
                    content_idx: i,
                    kind: ChildSlotKind::Header,
                });
            }
            if c.body.is_some() {
                slots.push(ChildSlot {
                    content_idx: i,
                    kind: ChildSlotKind::Body,
                });
            }
            if c.selected_top.is_some() {
                slots.push(ChildSlot {
                    content_idx: i,
                    kind: ChildSlotKind::SelectedTop,
                });
            }
            if c.selected_bottom.is_some() {
                slots.push(ChildSlot {
                    content_idx: i,
                    kind: ChildSlotKind::SelectedBottom,
                });
            }
        }
        slots
    }

    fn child_index(
        &self,
        slots: &[ChildSlot],
        content_idx: usize,
        kind: ChildSlotKind,
    ) -> Option<usize> {
        slots
            .iter()
            .position(|slot| slot.content_idx == content_idx && slot.kind == kind)
    }

    fn child_element(&self, slot: ChildSlot) -> &Element<'a, Message, Theme> {
        let content = &self.content[slot.content_idx];

        match slot.kind {
            ChildSlotKind::Header => content.header.as_ref().unwrap(),
            ChildSlotKind::Body => content.body.as_ref().unwrap(),
            ChildSlotKind::SelectedTop => content.selected_top.as_ref().unwrap(),
            ChildSlotKind::SelectedBottom => content.selected_bottom.as_ref().unwrap(),
        }
    }

    fn child_element_mut(&mut self, slot: ChildSlot) -> &mut Element<'a, Message, Theme> {
        let content = &mut self.content[slot.content_idx];

        match slot.kind {
            ChildSlotKind::Header => content.header.as_mut().unwrap(),
            ChildSlotKind::Body => content.body.as_mut().unwrap(),
            ChildSlotKind::SelectedTop => content.selected_top.as_mut().unwrap(),
            ChildSlotKind::SelectedBottom => content.selected_bottom.as_mut().unwrap(),
        }
    }

    fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
        if (edge1 - edge0).abs() <= f32::EPSILON {
            return 0.0;
        }
        let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn composite_color(base: Color, overlay: Color) -> Color {
        let out_a = overlay.a + base.a * (1.0 - overlay.a);

        if out_a <= f32::EPSILON {
            return Color::TRANSPARENT;
        }

        Color {
            r: (overlay.r * overlay.a + base.r * base.a * (1.0 - overlay.a)) / out_a,
            g: (overlay.g * overlay.a + base.g * base.a * (1.0 - overlay.a)) / out_a,
            b: (overlay.b * overlay.a + base.b * base.a * (1.0 - overlay.a)) / out_a,
            a: out_a,
        }
    }

    fn port_emphasis(cursor_screen: Option<Point>, center_screen: Point) -> f32 {
        let Some(cursor) = cursor_screen else {
            return 0.0;
        };
        let dx = cursor.x - center_screen.x;
        let dy = cursor.y - center_screen.y;
        let distance = (dx * dx + dy * dy).sqrt();
        1.0 - Self::smoothstep(14.0, 96.0, distance)
    }

    fn port_chrome_scale(&self) -> f32 {
        self.viewport.zoom.clamp(0.35, 1.0)
    }

    fn port_outside_extent(chrome_scale: f32, emphasis: f32) -> f32 {
        ((5.5 + emphasis * 5.5) * chrome_scale).clamp(2.5, 11.0)
    }

    fn visual_node_indices(ordered_nodes: &[FlowNode]) -> Vec<usize> {
        let mut indices = Vec::with_capacity(ordered_nodes.len());
        indices.extend(
            ordered_nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| (!node.selected).then_some(index)),
        );
        indices.extend(
            ordered_nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| node.selected.then_some(index)),
        );
        indices
    }

    fn selected_primary_index(
        ordered_nodes: &[FlowNode],
        visual_node_indices: &[usize],
    ) -> Option<usize> {
        visual_node_indices
            .iter()
            .rev()
            .copied()
            .find(|&index| ordered_nodes.get(index).is_some_and(|node| node.selected))
    }

    fn node_screen_rect(&self, node: &FlowNode, root_bounds: Rectangle) -> Rectangle {
        let top_left = self.viewport.scene_to_screen(node.position, root_bounds);
        let scale = self.viewport.zoom;
        Rectangle {
            x: top_left.x,
            y: top_left.y,
            width: node.width * scale,
            height: node.cached_height * scale,
        }
    }

    fn node_layout_rect(&self, node: &FlowNode, root_bounds: Rectangle) -> Rectangle {
        Rectangle {
            x: root_bounds.x + node.position.x,
            y: root_bounds.y + node.position.y,
            width: node.width,
            height: node.cached_height,
        }
    }

    fn scene_port_info(
        &self,
        ordered_nodes: &[FlowNode],
        node_id: NodeId,
        port_id: PortId,
    ) -> Option<(Point, PortSide, Rectangle)> {
        let node = ordered_nodes.iter().find(|node| node.id == node_id)?;
        let rect = node.rect();
        let port = node
            .inputs
            .iter()
            .chain(node.outputs.iter())
            .find(|port| port.id == port_id)?;
        let zoom = self.viewport.zoom.max(0.01);
        let outside_extent = Self::port_outside_extent(self.port_chrome_scale(), 0.0) / zoom;
        let anchor = match port.side {
            PortSide::Input => Point::new(rect.x - outside_extent, rect.y + port.y_offset),
            PortSide::Output => {
                Point::new(rect.x + rect.width + outside_extent, rect.y + port.y_offset)
            }
        };
        Some((anchor, port.side, rect))
    }

    fn edge_screen_path_points(
        &self,
        root_bounds: Rectangle,
        from: Point,
        from_side: PortSide,
        from_bounds: Rectangle,
        to: Point,
        to_side: PortSide,
        to_bounds: Rectangle,
    ) -> Vec<Point> {
        draw::edge_path_points(from, from_side, from_bounds, to, to_side, to_bounds)
            .into_iter()
            .map(|point| self.viewport.scene_to_screen(point, root_bounds))
            .collect()
    }

    fn edge_delete_button_bounds(&self, center: Point) -> Rectangle {
        let size = (18.0 * self.viewport.zoom.clamp(0.8, 1.4)).clamp(14.0, 25.0);
        Rectangle {
            x: center.x - size * 0.5,
            y: center.y - size * 0.5,
            width: size,
            height: size,
        }
    }

    fn hovered_edge_target(
        &self,
        ordered_nodes: &[FlowNode],
        root_bounds: Rectangle,
        cursor_screen: Point,
    ) -> Option<HoveredEdge> {
        const EDGE_HIT_TOLERANCE: f32 = 8.0;
        let mut best: Option<(bool, f32, HoveredEdge)> = None;

        for edge in self.edges.iter().rev() {
            let (from, from_side, from_rect) =
                self.scene_port_info(ordered_nodes, edge.from_node, edge.from_port)?;
            let (to, to_side, to_rect) =
                self.scene_port_info(ordered_nodes, edge.to_node, edge.to_port)?;
            let points = self.edge_screen_path_points(
                root_bounds,
                from,
                from_side,
                from_rect,
                to,
                to_side,
                to_rect,
            );
            let distance = draw::edge_path_distance(&points, cursor_screen);
            let midpoint = draw::edge_path_midpoint(&points);
            let button_bounds = self.edge_delete_button_bounds(midpoint);
            let button_hovered = button_bounds.contains(cursor_screen);

            if button_hovered || distance <= EDGE_HIT_TOLERANCE {
                let candidate = HoveredEdge {
                    edge: *edge,
                    button_bounds,
                    button_hovered,
                };

                let replace = match best {
                    None => true,
                    Some((best_button_hovered, best_distance, _)) => {
                        (button_hovered && !best_button_hovered)
                            || (button_hovered == best_button_hovered
                                && distance < best_distance - 0.01)
                    }
                };

                if replace {
                    best = Some((button_hovered, distance, candidate));
                }
            }
        }

        best.map(|(_, _, hovered)| hovered)
    }

    fn port_screen_y(_node: &FlowNode, rect: Rectangle, node_height: f32, port: &PortDef) -> f32 {
        let local_y = port.y_offset;
        let scale_y = if node_height.abs() > f32::EPSILON {
            rect.height / node_height
        } else {
            1.0
        };

        rect.y + local_y * scale_y
    }

    fn port_tab_geometry(
        node: &FlowNode,
        port: &PortDef,
        rect: Rectangle,
        node_height: f32,
        chrome_scale: f32,
        emphasis: f32,
    ) -> (Rectangle, [f32; 4], Point, Point) {
        let center_y = Self::port_screen_y(node, rect, node_height, port);
        let node_edge_x = match port.side {
            PortSide::Input => rect.x,
            PortSide::Output => rect.x + rect.width,
        };
        let outside_extent = Self::port_outside_extent(chrome_scale, emphasis);
        let tab_h = ((11.0 + emphasis * 5.0) * chrome_scale).clamp(5.0, 16.0);
        let tab_w = outside_extent;
        let outer_r = (tab_h * 0.5).min(tab_w * 0.5);

        let (tab_x, focus_x, anchor_x, radius) = match port.side {
            PortSide::Output => (
                node_edge_x,
                node_edge_x + outside_extent * 0.5,
                node_edge_x + outside_extent,
                [0.0_f32, outer_r, outer_r, 0.0],
            ),
            PortSide::Input => (
                node_edge_x - outside_extent,
                node_edge_x - outside_extent * 0.5,
                node_edge_x - outside_extent,
                [outer_r, 0.0_f32, 0.0, outer_r],
            ),
        };
        (
            Rectangle {
                x: tab_x,
                y: center_y - tab_h * 0.5,
                width: tab_w,
                height: tab_h,
            },
            radius,
            Point::new(focus_x, center_y),
            Point::new(anchor_x, center_y),
        )
    }

    fn port_hit_bounds(
        node: &FlowNode,
        port: &PortDef,
        rect: Rectangle,
        node_height: f32,
        chrome_scale: f32,
    ) -> Rectangle {
        let (bounds, _, _, _) =
            Self::port_tab_geometry(node, port, rect, node_height, chrome_scale, 1.0);
        let pad_x = (2.0 * chrome_scale).max(2.0);
        let pad_y = (3.0 * chrome_scale).max(3.0);

        Rectangle {
            x: bounds.x - pad_x,
            y: bounds.y - pad_y,
            width: bounds.width + pad_x * 2.0,
            height: bounds.height + pad_y * 2.0,
        }
    }

    fn header_widget_slot(node_rect: Rectangle, header_height: f32) -> Rectangle {
        let gap = 8.0;
        let vertical_padding = 5.0;
        let min_control_width = 88.0_f32.min(node_rect.width.max(0.0));
        let max_title_width = (node_rect.width - min_control_width - gap).max(0.0);
        let min_title_width = 60.0_f32.min(max_title_width);
        let title_width = (node_rect.width * 0.52).clamp(min_title_width, max_title_width);

        Rectangle {
            x: node_rect.x + title_width,
            y: node_rect.y + vertical_padding,
            width: (node_rect.width - title_width - gap).max(0.0),
            height: (header_height - vertical_padding * 2.0).max(0.0),
        }
    }

    fn slot_visible(slot: ChildSlot, selected_primary_index: Option<usize>) -> bool {
        match slot.kind {
            ChildSlotKind::Header | ChildSlotKind::Body => true,
            ChildSlotKind::SelectedTop | ChildSlotKind::SelectedBottom => {
                Some(slot.content_idx) == selected_primary_index
            }
        }
    }

    fn selected_overlay_position(
        kind: ChildSlotKind,
        child_size: Size,
        node_rect: Rectangle,
    ) -> Point {
        let gap = 10.0;
        let x = node_rect.x;
        let y = match kind {
            ChildSlotKind::SelectedTop => node_rect.y - child_size.height - gap,
            ChildSlotKind::SelectedBottom => node_rect.y + node_rect.height + gap,
            ChildSlotKind::Header | ChildSlotKind::Body => node_rect.y,
        };

        Point::new(x, y)
    }

    fn child_screen_bounds(
        slot: ChildSlot,
        bounds: Rectangle,
        canvas_transform: Transformation,
    ) -> Rectangle {
        if slot.uses_canvas_transform() {
            bounds * canvas_transform
        } else {
            bounds
        }
    }

    fn intersect_bounds(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
        let left = a.x.max(b.x);
        let top = a.y.max(b.y);
        let right = (a.x + a.width).min(b.x + b.width);
        let bottom = (a.y + a.height).min(b.y + b.height);
        let width = right - left;
        let height = bottom - top;

        (width > 0.0 && height > 0.0).then_some(Rectangle {
            x: left,
            y: top,
            width,
            height,
        })
    }

    fn visible_bounds(bounds: Rectangle, viewport: Rectangle) -> Option<Rectangle> {
        Self::intersect_bounds(bounds, viewport)
    }

    fn cursor_on_palette(&self, cursor_screen: Point, root_bounds: Rectangle) -> bool {
        let cursor_scene = self.viewport.screen_to_scene(cursor_screen, root_bounds);

        self.context_palette.is_some_and(|palette| {
            hit_test::palette_hit(self.palette_entries, palette.position, cursor_scene).is_some()
        })
    }

    fn screen_node_index_at(
        &self,
        ordered_nodes: &[FlowNode],
        visual_node_indices: &[usize],
        root_bounds: Rectangle,
        cursor_screen: Point,
    ) -> Option<usize> {
        visual_node_indices.iter().rev().find_map(|&index| {
            let node = ordered_nodes.get(index)?;
            self.node_screen_rect(node, root_bounds)
                .contains(cursor_screen)
                .then_some(index)
        })
    }

    fn top_selected_overlay_slot_at(
        slots: &[ChildSlot],
        child_screen_bounds: &[Rectangle],
        selected_primary_index: Option<usize>,
        cursor_screen: Point,
    ) -> Option<usize> {
        slots
            .iter()
            .enumerate()
            .rev()
            .find_map(|(slot_idx, &slot)| {
                (slot.is_selected_overlay()
                    && Self::slot_visible(slot, selected_primary_index)
                    && child_screen_bounds
                        .get(slot_idx)
                        .is_some_and(|bounds| bounds.contains(cursor_screen)))
                .then_some(slot_idx)
            })
    }

    fn top_canvas_slot_at(
        &self,
        slots: &[ChildSlot],
        child_screen_bounds: &[Rectangle],
        ordered_nodes: &[FlowNode],
        visual_node_indices: &[usize],
        root_bounds: Rectangle,
        cursor_screen: Point,
    ) -> Option<usize> {
        let top_node = self.screen_node_index_at(
            ordered_nodes,
            visual_node_indices,
            root_bounds,
            cursor_screen,
        )?;

        slots
            .iter()
            .enumerate()
            .rev()
            .find_map(|(slot_idx, &slot)| {
                (!slot.is_selected_overlay()
                    && slot.content_idx == top_node
                    && child_screen_bounds
                        .get(slot_idx)
                        .is_some_and(|bounds| bounds.contains(cursor_screen)))
                .then_some(slot_idx)
            })
    }

    fn cursor_owner_at(
        &self,
        slots: &[ChildSlot],
        child_screen_bounds: &[Rectangle],
        ordered_nodes: &[FlowNode],
        visual_node_indices: &[usize],
        selected_primary_index: Option<usize>,
        root_bounds: Rectangle,
        cursor_screen: Option<Point>,
        cursor_on_palette: bool,
    ) -> Option<CursorOwner> {
        let cursor_screen = cursor_screen.filter(|_| !cursor_on_palette)?;

        if let Some(slot_idx) = Self::top_selected_overlay_slot_at(
            slots,
            child_screen_bounds,
            selected_primary_index,
            cursor_screen,
        ) {
            return Some(CursorOwner::SelectedOverlay(slots[slot_idx].content_idx));
        }

        self.screen_node_index_at(
            ordered_nodes,
            visual_node_indices,
            root_bounds,
            cursor_screen,
        )
        .map(CursorOwner::CanvasNode)
    }

    fn slot_receives_cursor(slot: ChildSlot, cursor_owner: Option<CursorOwner>) -> bool {
        match cursor_owner {
            Some(CursorOwner::CanvasNode(content_idx)) => {
                !slot.is_selected_overlay() && slot.content_idx == content_idx
            }
            Some(CursorOwner::SelectedOverlay(content_idx)) => {
                slot.is_selected_overlay() && slot.content_idx == content_idx
            }
            None => false,
        }
    }

    fn overlay_slot_order(
        slots: &[ChildSlot],
        visual_node_indices: &[usize],
        selected_primary_index: Option<usize>,
    ) -> Vec<usize> {
        let mut order = Vec::with_capacity(slots.len());

        if let Some(selected_idx) = selected_primary_index {
            order.extend(
                slots
                    .iter()
                    .enumerate()
                    .rev()
                    .filter_map(|(slot_idx, &slot)| {
                        (slot.is_selected_overlay() && slot.content_idx == selected_idx)
                            .then_some(slot_idx)
                    }),
            );
        }

        for &content_idx in visual_node_indices.iter().rev() {
            order.extend(
                slots
                    .iter()
                    .enumerate()
                    .rev()
                    .filter_map(|(slot_idx, &slot)| {
                        (!slot.is_selected_overlay() && slot.content_idx == content_idx)
                            .then_some(slot_idx)
                    }),
            );
        }

        order
    }
}

fn transform_layout_node(
    node: &layout::Node,
    parent_position: Point,
    parent_transformed_position: Point,
    transformation: Transformation,
) -> layout::Node {
    let bounds = node.bounds();
    let absolute_bounds = Rectangle {
        x: parent_position.x + bounds.x,
        y: parent_position.y + bounds.y,
        width: bounds.width,
        height: bounds.height,
    };
    let transformed_bounds = absolute_bounds * transformation;

    let children = node
        .children()
        .iter()
        .map(|child| {
            transform_layout_node(
                child,
                absolute_bounds.position(),
                transformed_bounds.position(),
                transformation,
            )
        })
        .collect();

    layout::Node::with_children(transformed_bounds.size(), children).move_to(Point::new(
        transformed_bounds.x - parent_transformed_position.x,
        transformed_bounds.y - parent_transformed_position.y,
    ))
}

fn inverse_transform_layout(
    layout: Layout<'_>,
    parent_scene_position: Point,
    inverse: Transformation,
) -> layout::Node {
    let transformed_bounds = layout.bounds();
    let scene_bounds = transformed_bounds * inverse;

    let children = layout
        .children()
        .map(|child| inverse_transform_layout(child, scene_bounds.position(), inverse))
        .collect();

    layout::Node::with_children(scene_bounds.size(), children).move_to(Point::new(
        scene_bounds.x - parent_scene_position.x,
        scene_bounds.y - parent_scene_position.y,
    ))
}

struct TransformedOverlay<'a, Message, Theme>
where
    Theme: Catalog,
{
    inner: overlay::Element<'a, Message, Theme, Renderer>,
    viewport: Rectangle,
    transformation: Transformation,
    scene_layout: Option<layout::Node>,
}

impl<'a, Message, Theme> TransformedOverlay<'a, Message, Theme>
where
    Theme: Catalog,
{
    fn new(
        inner: overlay::Element<'a, Message, Theme, Renderer>,
        viewport: Rectangle,
        transformation: Transformation,
    ) -> Self {
        Self {
            inner,
            viewport,
            transformation,
            scene_layout: None,
        }
    }

    fn sync_scene_layout(&mut self, layout: Layout<'_>) {
        self.scene_layout = Some(inverse_transform_layout(
            layout,
            Point::new(0.0, 0.0),
            self.transformation.inverse(),
        ));
    }
}

impl<Message, Theme> overlay::Overlay<Message, Theme, Renderer>
    for TransformedOverlay<'_, Message, Theme>
where
    Theme: Catalog,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let inverse = self.transformation.inverse();
        let scene_bounds = bounds * inverse;
        let scene_layout = self.inner.as_overlay_mut().layout(renderer, scene_bounds);
        self.scene_layout = Some(scene_layout.clone());

        transform_layout_node(
            &scene_layout,
            Point::new(0.0, 0.0),
            Point::new(0.0, 0.0),
            self.transformation,
        )
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let inverse = self.transformation.inverse();
        let scene_layout = inverse_transform_layout(layout, Point::new(0.0, 0.0), inverse);

        renderer.with_layer(self.viewport, |renderer| {
            renderer.with_transformation(self.transformation, |renderer| {
                self.inner.as_overlay().draw(
                    renderer,
                    theme,
                    style,
                    Layout::new(&scene_layout),
                    cursor * inverse,
                );
            });
        });
    }

    fn operate(&mut self, layout: Layout<'_>, renderer: &Renderer, operation: &mut dyn Operation) {
        self.sync_scene_layout(layout);
        let Self {
            inner,
            scene_layout,
            ..
        } = self;
        let scene_layout = Layout::new(
            scene_layout
                .as_ref()
                .expect("overlay layout should be computed before use"),
        );

        inner
            .as_overlay_mut()
            .operate(scene_layout, renderer, operation);
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let inverse = self.transformation.inverse();
        self.sync_scene_layout(layout);
        let Self {
            inner,
            scene_layout,
            ..
        } = self;
        let scene_layout = Layout::new(
            scene_layout
                .as_ref()
                .expect("overlay layout should be computed before use"),
        );

        inner.as_overlay_mut().update(
            event,
            scene_layout,
            cursor * inverse,
            renderer,
            clipboard,
            shell,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }

        let inverse = self.transformation.inverse();
        let scene_layout = inverse_transform_layout(layout, Point::new(0.0, 0.0), inverse);

        self.inner.as_overlay().mouse_interaction(
            Layout::new(&scene_layout),
            cursor * inverse,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.sync_scene_layout(layout);
        let Self {
            inner,
            scene_layout,
            viewport,
            transformation,
        } = self;
        let scene_layout = Layout::new(
            scene_layout
                .as_ref()
                .expect("overlay layout should be computed before use"),
        );

        inner
            .as_overlay_mut()
            .overlay(scene_layout, renderer)
            .map(|overlay| {
                overlay::Element::new(Box::new(TransformedOverlay::new(
                    overlay,
                    *viewport,
                    *transformation,
                )))
            })
    }

    fn index(&self) -> f32 {
        self.inner.as_overlay().index()
    }
}

// Widget implementation

impl<'a, Message: Clone + 'a, Theme: Catalog> Widget<Message, Theme, Renderer>
    for FlowEditor<'a, Message, Theme>
{
    fn children(&self) -> Vec<Tree> {
        let slots = self.child_slots();
        slots
            .iter()
            .map(|&slot| Tree::new(self.child_element(slot).as_widget()))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let slots = self.child_slots();
        let widgets: Vec<_> = slots
            .iter()
            .map(|&slot| self.child_element(slot).as_widget())
            .collect();
        tree.diff_children(&widgets);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(Length::Fill, Length::Fill, Size::ZERO);
        let root_bounds = Rectangle::with_size(size);
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);

        let header_h = hit_test::HEADER_HEIGHT;

        // Pre-compute scene-space rects for all content slots (indexed by content/z-order index).
        let ordered_rects: Vec<_> = (0..self.content.len())
            .filter_map(|index| {
                let node = self.ordered_node_at(index)?;
                Some(self.node_layout_rect(node, root_bounds))
            })
            .collect();

        let slots = self.child_slots();

        let children: Vec<layout::Node> = slots
            .iter()
            .zip(tree.children.iter_mut())
            .filter_map(|(&slot, child_tree)| {
                let rect = *ordered_rects.get(slot.content_idx)?;

                if !Self::slot_visible(slot, selected_primary_index) {
                    return Some(
                        layout::Node::new(Size::ZERO).move_to(Point::new(-10_000.0, -10_000.0)),
                    );
                }

                match slot.kind {
                    ChildSlotKind::Header => {
                        let elem = self.child_element_mut(slot);
                        // Header widgets keep their natural height and sit centered
                        // inside a smaller right-aligned header slot.
                        let slot_rect = Self::header_widget_slot(rect, header_h);
                        let child_limits = layout::Limits::new(
                            Size::ZERO,
                            Size::new(slot_rect.width, slot_rect.height),
                        );
                        let child =
                            elem.as_widget_mut()
                                .layout(child_tree, renderer, &child_limits);
                        let child_size = child.size();
                        let child_x = slot_rect.x + (slot_rect.width - child_size.width).max(0.0);
                        let child_y =
                            slot_rect.y + (slot_rect.height - child_size.height).max(0.0) * 0.5;

                        Some(child.move_to(Point::new(child_x, child_y)))
                    }
                    ChildSlotKind::Body => {
                        let elem = self.child_element_mut(slot);
                        let body_y = rect.y + header_h;
                        let body_h = (rect.height - header_h).max(0.0);
                        let child_limits = layout::Limits::new(
                            Size::new(rect.width, body_h),
                            Size::new(rect.width, body_h),
                        );
                        Some(
                            elem.as_widget_mut()
                                .layout(child_tree, renderer, &child_limits)
                                .move_to(Point::new(rect.x, body_y)),
                        )
                    }
                    ChildSlotKind::SelectedTop | ChildSlotKind::SelectedBottom => {
                        let node_rect = rect;
                        let max_size = Size::new(node_rect.width.max(0.0), 10_000.0);
                        let elem = self.child_element_mut(slot);
                        let child = elem.as_widget_mut().layout(
                            child_tree,
                            renderer,
                            &layout::Limits::new(Size::ZERO, max_size),
                        );
                        let position =
                            Self::selected_overlay_position(slot.kind, child.size(), node_rect);
                        Some(child.move_to(position))
                    }
                }
            })
            .collect();

        layout::Node::with_children(size, children)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let slots = self.child_slots();
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);

        for (&slot, (child_tree, child_layout)) in slots
            .iter()
            .zip(tree.children.iter_mut().zip(layout.children()))
        {
            if !Self::slot_visible(slot, selected_primary_index) {
                continue;
            }

            let elem = self.child_element_mut(slot);
            elem.as_widget_mut()
                .operate(child_tree, child_layout, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let root_bounds = layout.bounds();
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);
        let child_layouts: Vec<_> = layout.children().collect();
        let child_bounds: Vec<Rectangle> =
            child_layouts.iter().map(|child| child.bounds()).collect();
        let canvas_transform = self.viewport.transformation(root_bounds);
        let canvas_inverse = canvas_transform.inverse();
        let canvas_cursor = cursor * canvas_inverse;
        let canvas_viewport = *viewport * canvas_inverse;
        let slots = self.child_slots();
        let child_screen_bounds: Vec<Rectangle> = slots
            .iter()
            .zip(child_bounds.iter())
            .map(|(&slot, bounds)| Self::child_screen_bounds(slot, *bounds, canvas_transform))
            .collect();
        let vp = self.viewport;
        let visual_nodes: Vec<_> = visual_node_indices
            .iter()
            .filter_map(|&index| ordered_nodes.get(index).cloned())
            .collect();
        let cursor_screen = cursor.position();
        let cursor_on_palette = cursor_screen
            .is_some_and(|cursor_screen| self.cursor_on_palette(cursor_screen, root_bounds));
        let cursor_owner = self.cursor_owner_at(
            &slots,
            &child_screen_bounds,
            &ordered_nodes,
            &visual_node_indices,
            selected_primary_index,
            root_bounds,
            cursor_screen,
            cursor_on_palette,
        );

        let screen_port_hit = |cursor_screen: Point| -> Option<(NodeId, PortId, PortSide)> {
            visual_node_indices.iter().rev().find_map(|&index| {
                let node = ordered_nodes.get(index)?;
                let top_left = vp.scene_to_screen(node.position, root_bounds);
                let rect = Rectangle {
                    x: top_left.x,
                    y: top_left.y,
                    width: node.width * vp.zoom,
                    height: node.cached_height * vp.zoom,
                };
                let node_height = node.cached_height;
                let chrome_scale = vp.zoom.clamp(0.35, 1.0);

                node.inputs
                    .iter()
                    .chain(node.outputs.iter())
                    .find_map(|port| {
                        Self::port_hit_bounds(node, port, rect, node_height, chrome_scale)
                            .contains(cursor_screen)
                            .then_some((node.id, port.id, port.side))
                    })
            })
        };

        if self.drag.is_none()
            && self.pan.is_none()
            && self.preview.is_none()
            && self.selection.is_none()
        {
            for (slot_idx, &slot) in slots.iter().enumerate() {
                if !Self::slot_visible(slot, selected_primary_index) {
                    continue;
                }

                let elem = self.child_element_mut(slot);
                let child_cursor = if Self::slot_receives_cursor(slot, cursor_owner) {
                    if slot.uses_canvas_transform() {
                        canvas_cursor
                    } else {
                        cursor
                    }
                } else {
                    mouse::Cursor::Unavailable
                };
                let child_viewport = if slot.uses_canvas_transform() {
                    &canvas_viewport
                } else {
                    viewport
                };

                elem.as_widget_mut().update(
                    &mut tree.children[slot_idx],
                    event,
                    child_layouts[slot_idx],
                    child_cursor,
                    renderer,
                    clipboard,
                    shell,
                    child_viewport,
                );
            }
        }

        let child_captured = shell.is_event_captured();
        let cursor_on_selected_overlay =
            matches!(cursor_owner, Some(CursorOwner::SelectedOverlay(_)));

        let screen_node_index = |cursor_screen: Point| -> Option<usize> {
            self.screen_node_index_at(
                &ordered_nodes,
                &visual_node_indices,
                root_bounds,
                cursor_screen,
            )
        };
        let hovered_edge = cursor_screen.and_then(|cursor_screen| {
            if cursor_on_palette
                || cursor_on_selected_overlay
                || screen_port_hit(cursor_screen).is_some()
                || screen_node_index(cursor_screen).is_some()
            {
                return None;
            }

            self.hovered_edge_target(&ordered_nodes, root_bounds, cursor_screen)
        });

        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                self.publish(
                    shell,
                    Action::UpdateModifiers {
                        modifiers: *modifiers,
                    },
                );
                if child_captured {
                    return;
                }

                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Delete) => {
                        self.publish(shell, Action::DeleteSelected)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Home) => {
                        self.publish(shell, Action::CenterView)
                    }
                    keyboard::Key::Character("d") if modifiers.command() => {
                        self.publish(shell, Action::DuplicateSelected)
                    }
                    keyboard::Key::Character("c") if modifiers.command() => {
                        self.publish(shell, Action::CopySelected)
                    }
                    keyboard::Key::Character("x") if modifiers.command() => {
                        self.publish(shell, Action::CutSelected)
                    }
                    keyboard::Key::Character("v") if modifiers.command() => {
                        self.publish(shell, Action::Paste)
                    }
                    _ => {}
                }
            }

            Event::Keyboard(keyboard::Event::KeyReleased { modifiers, .. }) => {
                self.publish(
                    shell,
                    Action::UpdateModifiers {
                        modifiers: *modifiers,
                    },
                );
                if child_captured {
                    return;
                }
            }

            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                self.publish(
                    shell,
                    Action::UpdateModifiers {
                        modifiers: *modifiers,
                    },
                );
                if child_captured {
                    return;
                }
            }

            _ if child_captured => return,

            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
                if let Some(cursor_screen) = cursor_screen {
                    if !root_bounds.contains(cursor_screen) {
                        return;
                    }

                    let cursor_scene = self.viewport.screen_to_scene(cursor_screen, root_bounds);

                    if let Some(palette) = self.context_palette {
                        if let Some(template_id) = hit_test::palette_hit(
                            self.palette_entries,
                            palette.position,
                            cursor_scene,
                        ) {
                            self.publish(
                                shell,
                                Action::CreateNodeFromTemplate {
                                    template_id,
                                    position: palette.position,
                                },
                            );
                            return;
                        }
                        self.publish(shell, Action::CloseContextPalette);
                        return;
                    }

                    if cursor_on_selected_overlay {
                        return;
                    }

                    if let Some(hovered_edge) = hovered_edge {
                        if hovered_edge.button_hovered {
                            self.publish(shell, Action::DeleteEdge(hovered_edge.edge));
                            return;
                        }
                    }

                    if let Some((node, port, side)) = screen_port_hit(cursor_screen) {
                        self.publish(
                            shell,
                            Action::StartConnection {
                                node,
                                port,
                                side,
                                cursor_scene,
                            },
                        );
                        return;
                    }

                    if let Some(node_id) = hit_test::header_at(&visual_nodes, cursor_scene) {
                        let keep_group_selection = ordered_nodes
                            .iter()
                            .find(|node| node.id == node_id)
                            .is_some_and(|node| node.selected && !self.modifiers.shift());
                        if !keep_group_selection {
                            self.publish(shell, Action::SelectSingle(node_id));
                        }
                        self.publish(
                            shell,
                            Action::StartNodeDrag {
                                id: node_id,
                                cursor_scene,
                            },
                        );
                        return;
                    }

                    if let Some(node_id) = hit_test::body_at(&visual_nodes, cursor_scene) {
                        self.publish(shell, Action::SelectSingle(node_id));
                        return;
                    }

                    if let Some(index) = screen_node_index(cursor_screen) {
                        if let Some(node) = ordered_nodes.get(index) {
                            // Use full node rect (child_bounds is body-only after layout fix)
                            let rect = self.node_screen_rect(node, root_bounds);
                            let node_height = node.cached_height;
                            let scale_y = if node_height.abs() > f32::EPSILON {
                                rect.height / node_height
                            } else {
                                1.0
                            };
                            let header_bottom = rect.y + 42.0 * scale_y;
                            if cursor_screen.y <= header_bottom {
                                let keep_group_selection = node.selected && !self.modifiers.shift();
                                if !keep_group_selection {
                                    self.publish(shell, Action::SelectSingle(node.id));
                                }
                                self.publish(
                                    shell,
                                    Action::StartNodeDrag {
                                        id: node.id,
                                        cursor_scene,
                                    },
                                );
                            } else {
                                self.publish(shell, Action::SelectSingle(node.id));
                            }
                            return;
                        }
                    }

                    if self.modifiers.shift() {
                        self.publish(
                            shell,
                            Action::StartSelection {
                                cursor_scene,
                                additive: true,
                            },
                        );
                    } else {
                        self.publish(shell, Action::ClearSelection);
                        self.publish(shell, Action::StartPan { cursor_screen });
                    }
                }
            }

            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Right)) => {
                if let Some(cursor_screen) = cursor_screen {
                    if !root_bounds.contains(cursor_screen) {
                        return;
                    }
                    if cursor_on_selected_overlay {
                        return;
                    }
                    if screen_node_index(cursor_screen).is_some() {
                        return;
                    }
                    let cursor_scene = self.viewport.screen_to_scene(cursor_screen, root_bounds);
                    self.publish(shell, Action::OpenContextPalette { cursor_scene });
                }
            }

            Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Middle)) => {
                if let Some(cursor_screen) = cursor_screen {
                    if !root_bounds.contains(cursor_screen) {
                        return;
                    }
                    if cursor_on_selected_overlay {
                        return;
                    }
                    if screen_node_index(cursor_screen).is_some() {
                        return;
                    }
                    self.publish(shell, Action::StartPan { cursor_screen });
                }
            }

            Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
                shell.request_redraw();
                if let Some(cursor_screen) = cursor_screen {
                    let cursor_scene = self.viewport.screen_to_scene(cursor_screen, root_bounds);
                    if self.drag.is_some() {
                        self.publish(shell, Action::DragNodeTo { cursor_scene });
                    } else if self.pan.is_some() {
                        self.publish(shell, Action::PanTo { cursor_screen });
                    } else if self.selection.is_some() {
                        self.publish(shell, Action::UpdateSelection { cursor_scene });
                    } else if self.preview.is_some() && self.context_palette.is_none() {
                        self.publish(shell, Action::UpdateConnectionPreview { cursor_scene });
                    }
                }
            }

            Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                if self.drag.is_some() {
                    self.publish(shell, Action::EndNodeDrag);
                }
                if self.pan.is_some() {
                    self.publish(shell, Action::EndPan);
                }
                if self.selection.is_some() {
                    self.publish(shell, Action::FinishSelection);
                }

                if let Some(preview) = self.preview {
                    if let Some(cursor_screen) = cursor_screen {
                        let cursor_scene =
                            self.viewport.screen_to_scene(cursor_screen, root_bounds);
                        if let Some((target_node, target_port, target_side)) =
                            screen_port_hit(cursor_screen)
                        {
                            if preview.side != target_side {
                                let (from_node, from_port, to_node, to_port) = match preview.side {
                                    PortSide::Output => (
                                        preview.from_node,
                                        preview.from_port,
                                        target_node,
                                        target_port,
                                    ),
                                    PortSide::Input => (
                                        target_node,
                                        target_port,
                                        preview.from_node,
                                        preview.from_port,
                                    ),
                                };
                                self.publish(
                                    shell,
                                    Action::FinishConnection {
                                        from_node,
                                        from_port,
                                        to_node,
                                        to_port,
                                    },
                                );
                                return;
                            }
                        }

                        if screen_node_index(cursor_screen).is_none() {
                            self.publish(
                                shell,
                                Action::OpenContextPaletteFromConnection {
                                    cursor_scene,
                                    from_node: preview.from_node,
                                    from_port: preview.from_port,
                                    side: preview.side,
                                },
                            );
                            return;
                        }
                    }
                    self.publish(shell, Action::CancelConnection);
                }
            }

            Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Middle)) => {
                if self.pan.is_some() {
                    self.publish(shell, Action::EndPan);
                }
            }

            Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) => {
                if let Some(cursor_screen) = cursor_screen {
                    if !root_bounds.contains(cursor_screen) {
                        return;
                    }
                    if cursor_on_selected_overlay {
                        return;
                    }
                    let amount = match delta {
                        iced::mouse::ScrollDelta::Lines { y, .. } => *y,
                        iced::mouse::ScrollDelta::Pixels { y, .. } => *y / 40.0,
                    };
                    self.publish(
                        shell,
                        Action::ZoomAt {
                            cursor_screen,
                            root_bounds,
                            delta: amount,
                        },
                    );
                }
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let root_bounds = layout.bounds();
        let Some(clip_bounds) = Self::visible_bounds(root_bounds, *viewport) else {
            return;
        };
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);
        let editor_style = theme.style(&self.class);
        let slots = self.child_slots();

        renderer.start_layer(clip_bounds);

        // Background
        draw::quad(
            renderer,
            root_bounds,
            editor_style.canvas_bg,
            Color::TRANSPARENT,
            0.0,
            0.0,
            Shadow::default(),
        );

        // Grid
        let step = 28.0 * self.viewport.zoom.max(0.35);
        if step >= 8.0 {
            let x_offset = self.viewport.pan.x.rem_euclid(step);
            let y_offset = self.viewport.pan.y.rem_euclid(step);

            let mut x = root_bounds.x + x_offset;
            while x < root_bounds.x + root_bounds.width {
                draw::line(
                    renderer,
                    Point::new(x, root_bounds.y),
                    Point::new(x, root_bounds.y + root_bounds.height),
                    1.0,
                    editor_style.grid_line,
                );
                x += step;
            }

            let mut y = root_bounds.y + y_offset;
            while y < root_bounds.y + root_bounds.height {
                draw::line(
                    renderer,
                    Point::new(root_bounds.x, y),
                    Point::new(root_bounds.x + root_bounds.width, y),
                    1.0,
                    editor_style.grid_line,
                );
                y += step;
            }
        }

        let cursor_screen = cursor.position();

        // Edges
        for edge in self.edges {
            if let (Some((from, from_side, from_rect)), Some((to, to_side, to_rect))) = (
                self.scene_port_info(&ordered_nodes, edge.from_node, edge.from_port),
                self.scene_port_info(&ordered_nodes, edge.to_node, edge.to_port),
            ) {
                let points = self.edge_screen_path_points(
                    root_bounds,
                    from,
                    from_side,
                    from_rect,
                    to,
                    to_side,
                    to_rect,
                );
                draw::edge_path(renderer, &points, editor_style.edge);
            }
        }

        let pending_palette_connection = self
            .context_palette
            .and_then(|palette| palette.pending_connection);

        // Connection preview
        if pending_palette_connection.is_none() {
            if let Some(preview) = self.preview {
                if let Some((from, from_side, from_rect)) =
                    self.scene_port_info(&ordered_nodes, preview.from_node, preview.from_port)
                {
                    let to = preview.cursor_scene;
                    let to_side = match from_side {
                        PortSide::Output => PortSide::Input,
                        PortSide::Input => PortSide::Output,
                    };
                    let points = self.edge_screen_path_points(
                        root_bounds,
                        from,
                        from_side,
                        from_rect,
                        to,
                        to_side,
                        Rectangle {
                            x: to.x,
                            y: to.y,
                            width: 0.0,
                            height: 0.0,
                        },
                    );
                    draw::edge_path(renderer, &points, editor_style.edge_preview);
                }
            }
        }

        if let Some(pending) = pending_palette_connection {
            if let Some((from, from_side, from_rect)) =
                self.scene_port_info(&ordered_nodes, pending.source_node, pending.source_port)
            {
                let to = pending.drop_point;
                let to_side = match from_side {
                    PortSide::Output => PortSide::Input,
                    PortSide::Input => PortSide::Output,
                };
                let points = self.edge_screen_path_points(
                    root_bounds,
                    from,
                    from_side,
                    from_rect,
                    to,
                    to_side,
                    Rectangle {
                        x: to.x,
                        y: to.y,
                        width: 0.0,
                        height: 0.0,
                    },
                );
                draw::edge_path(renderer, &points, editor_style.edge_preview);
            }
        }

        // Collect layout children as a random-access vec for lookup by slot index.
        let child_layouts: Vec<_> = layout.children().collect();
        let canvas_transform = self.viewport.transformation(root_bounds);
        let canvas_inverse = canvas_transform.inverse();
        let canvas_cursor = cursor * canvas_inverse;
        let canvas_viewport = *viewport * canvas_inverse;
        let child_screen_bounds: Vec<_> = slots
            .iter()
            .zip(child_layouts.iter())
            .map(|(&slot, child)| Self::child_screen_bounds(slot, child.bounds(), canvas_transform))
            .collect();
        let cursor_on_palette = cursor_screen
            .is_some_and(|cursor_screen| self.cursor_on_palette(cursor_screen, root_bounds));
        let cursor_owner = self.cursor_owner_at(
            &slots,
            &child_screen_bounds,
            &ordered_nodes,
            &visual_node_indices,
            selected_primary_index,
            root_bounds,
            cursor_screen,
            cursor_on_palette,
        );
        let hovered_node_index = match cursor_owner {
            Some(CursorOwner::CanvasNode(content_idx)) => Some(content_idx),
            Some(CursorOwner::SelectedOverlay(_)) | None => None,
        };
        let hovered_port = cursor_screen.and_then(|cursor_screen| {
            if cursor_on_palette || matches!(cursor_owner, Some(CursorOwner::SelectedOverlay(_))) {
                return None;
            }

            let mut best_hit: Option<((usize, PortId), f32)> = None;

            for &ci in visual_node_indices.iter().rev() {
                let Some(node) = ordered_nodes.get(ci) else {
                    continue;
                };
                let rect = self.node_screen_rect(node, root_bounds);
                let chrome_scale = self.viewport.zoom.clamp(0.35, 1.0);

                for port in node.inputs.iter().chain(node.outputs.iter()) {
                    let hit_bounds =
                        Self::port_hit_bounds(node, port, rect, node.cached_height, chrome_scale);
                    let (_, _, focus_center, _) = Self::port_tab_geometry(
                        node,
                        port,
                        rect,
                        node.cached_height,
                        chrome_scale,
                        0.0,
                    );
                    let score = if hit_bounds.contains(cursor_screen) {
                        2.0
                    } else {
                        Self::port_emphasis(Some(cursor_screen), focus_center)
                    };

                    if score > best_hit.map_or(0.0, |(_, best)| best) {
                        best_hit = Some(((ci, port.id), score));
                    }
                }
            }

            best_hit
                .filter(|(_, score)| *score > 0.0)
                .map(|(port, _)| port)
        });
        let cursor_on_port_hit = cursor_screen.and_then(|cursor_screen| {
            visual_node_indices.iter().rev().find_map(|&ci| {
                let node = ordered_nodes.get(ci)?;
                let rect = self.node_screen_rect(node, root_bounds);
                let chrome_scale = self.port_chrome_scale();

                node.inputs
                    .iter()
                    .chain(node.outputs.iter())
                    .find_map(|port| {
                        Self::port_hit_bounds(node, port, rect, node.cached_height, chrome_scale)
                            .contains(cursor_screen)
                            .then_some((ci, port.id))
                    })
            })
        });
        let hovered_edge = cursor_screen.and_then(|cursor_screen| {
            if cursor_on_palette
                || matches!(cursor_owner, Some(CursorOwner::SelectedOverlay(_)))
                || hovered_node_index.is_some()
                || cursor_on_port_hit.is_some()
            {
                return None;
            }

            self.hovered_edge_target(&ordered_nodes, root_bounds, cursor_screen)
        });
        let draw_node_ports = |renderer: &mut Renderer,
                               ci: usize,
                               node: &FlowNode,
                               rect: Rectangle,
                               node_height: f32,
                               chrome_scale: f32| {
            for port in node.inputs.iter().chain(node.outputs.iter()) {
                let hovered = hovered_port == Some((ci, port.id));
                let (_, _, focus_center, _) =
                    Self::port_tab_geometry(node, port, rect, node_height, chrome_scale, 0.0);
                let emphasis = Self::port_emphasis(
                    if hovered || hovered_node_index == Some(ci) {
                        cursor_screen
                    } else {
                        None
                    },
                    focus_center,
                );
                let (bounds, radius, _, _) =
                    Self::port_tab_geometry(node, port, rect, node_height, chrome_scale, emphasis);
                let port_color = match port.side {
                    PortSide::Input => editor_style.port_input,
                    PortSide::Output => editor_style.port_output,
                };
                draw::port_tab(renderer, bounds, port_color, radius);
            }
        };

        // Nodes clipped to canvas bounds so nothing renders outside the editor area.
        renderer.start_layer(clip_bounds);
        for &ci in &visual_node_indices {
            let Some(node) = ordered_nodes.get(ci) else {
                continue;
            };
            // Full node rect for background/header/port drawing
            let rect = self.node_screen_rect(node, root_bounds);
            let node_height = node.cached_height;
            let node_scale_y = if node_height.abs() > f32::EPSILON {
                rect.height / node_height
            } else {
                1.0
            };
            let header_band = Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: 42.0 * node_scale_y,
            };
            let card_radius = (18.0 * self.viewport.zoom)
                .min((rect.width * 0.5).max(0.0))
                .min((rect.height * 0.5).max(0.0));
            let chrome_scale = self.viewport.zoom.clamp(0.35, 1.0);
            let header_radius = card_radius.min(header_band.height.max(0.0));
            let card_border = if node.selected {
                editor_style.node_border_selected
            } else {
                editor_style.node_border
            };
            let header_bg = if node.selected {
                editor_style.header_bg_selected
            } else {
                editor_style.header_bg
            };
            let node_fill = Self::composite_color(editor_style.canvas_bg, editor_style.node_bg);
            let header_fill = Self::composite_color(node_fill, header_bg);
            let border_width: f32 = if node.selected { 1.5 } else { 1.0 };
            let accent_visible_width = if node.accent_color.a > 0.0 {
                (8.0 * chrome_scale)
                    .clamp(3.0, 8.0)
                    .min(rect.width.max(0.0))
            } else {
                0.0
            };

            let node_bounds = Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width.max(0.0),
                height: rect.height.max(0.0),
            };
            let Some(node_clip) = Self::intersect_bounds(node_bounds, clip_bounds) else {
                continue;
            };
            let header_clip = Rectangle {
                x: rect.x,
                y: rect.y,
                width: rect.width.max(0.0),
                height: header_band.height.max(0.0),
            };
            let body_clip = Rectangle {
                x: rect.x,
                y: rect.y + header_band.height,
                width: rect.width.max(0.0),
                height: (rect.height - header_band.height).max(0.0),
            };
            let header_clip = Self::intersect_bounds(header_clip, node_clip);
            let body_clip = Self::intersect_bounds(body_clip, node_clip);

            // Port handles Postman-style flat tab protruding from node edge.
            // At rest: thin rectangle flush with edge. On hover: wider + fully-rounded outer tip.
            // Card background + shadow (no border in this pass).
            // Border is rendered in a second inset pass so it stays fully inside `rect`.
            draw::quad(
                renderer,
                rect,
                node_fill,
                Color::TRANSPARENT,
                0.0,
                card_radius,
                Shadow {
                    color: Color::from_rgba8(0, 0, 0, 0.26),
                    offset: Vector::new(0.0, 8.0),
                    blur_radius: 22.0,
                },
            );

            renderer.start_layer(node_clip);

            // Repaint the card fill inside the node layer so semi-transparent
            // child widgets composite against the node itself instead of the
            // canvas behind overlapping nodes.
            draw::quad(
                renderer,
                rect,
                node_fill,
                Color::TRANSPARENT,
                0.0,
                card_radius,
                Shadow::default(),
            );

            let header_inset = border_width.max(1.0);
            let header = Rectangle {
                x: header_band.x + header_inset,
                y: header_band.y + header_inset,
                width: (header_band.width - header_inset * 2.0).max(0.0),
                height: (header_band.height - header_inset).max(0.0),
            };
            draw::quad(
                renderer,
                header,
                header_fill,
                Color::TRANSPARENT,
                0.0,
                header_radius,
                Shadow::default(),
            );

            let has_body = node.cached_height > hit_test::HEADER_HEIGHT + f32::EPSILON;
            let header_flat = Rectangle {
                x: header.x,
                y: header.y + header_radius,
                width: header.width,
                height: (header.height - header_radius).max(0.0),
            };
            if has_body && header_flat.height > 0.0 {
                draw::quad(
                    renderer,
                    header_flat,
                    header_fill,
                    Color::TRANSPARENT,
                    0.0,
                    0.0,
                    Shadow::default(),
                );
            }

            // Accent colour stripe 8 px left-edge bar spanning full node height.
            if node.accent_color.a > 0.0 {
                let accent_inset = border_width.max(1.0);
                let accent_radius = (card_radius - accent_inset).max(0.0);
                let accent_bounds = Rectangle {
                    x: rect.x + accent_inset,
                    y: rect.y + accent_inset,
                    width: accent_visible_width.min((rect.width - accent_inset * 2.0).max(0.0)),
                    height: (rect.height - accent_inset * 2.0).max(0.0),
                };

                // Draw an oversized rounded fill, clipped to the accent width, so the visible
                // accent sits inside the outline while keeping card-matching outer corners
                // and a straight inner edge.
                if let Some(accent_clip) = Self::intersect_bounds(accent_bounds, node_clip) {
                    renderer.start_layer(accent_clip);
                    draw::quad(
                        renderer,
                        Rectangle {
                            x: accent_bounds.x,
                            y: accent_bounds.y,
                            // Keep source wide enough to avoid radius clamping.
                            width: accent_bounds.width + accent_radius * 2.0,
                            height: accent_bounds.height,
                        },
                        node.accent_color,
                        Color::TRANSPARENT,
                        0.0,
                        accent_radius,
                        Shadow::default(),
                    );
                    renderer.end_layer();
                }
            }

            // Node title (always drawn; max width shrinks when a header widget is present)
            let zoom = self.viewport.zoom.clamp(0.35, 1.9);
            let title_x = rect.x + 14.0 * zoom;
            let title_size = (13.0 * zoom).max(4.0);
            let has_header_widget = self.content.get(ci).map_or(false, |c| c.header.is_some());
            let title_max_w = if has_header_widget {
                let header_slot = Self::header_widget_slot(rect, header.height);
                (header_slot.x - rect.x - 14.0 * zoom).max(0.0)
            } else {
                rect.width - 28.0
            };
            if let Some(header_clip) = header_clip {
                draw::label(
                    renderer,
                    header_clip,
                    Point::new(title_x, header.y + (header.height - title_size) * 0.5),
                    title_max_w,
                    &node.title,
                    title_size,
                    editor_style.title_text,
                );
            }

            // Kind label only drawn when there is no header widget for this node.
            if !has_header_widget {
                let kind_size = (10.0 * zoom).max(3.5);
                if let Some(header_clip) = header_clip {
                    draw::label(
                        renderer,
                        header_clip,
                        Point::new(
                            rect.x + rect.width - 90.0 * zoom,
                            header.y + (header.height - kind_size) * 0.5,
                        ),
                        85.0 * zoom,
                        &node.kind_label,
                        kind_size,
                        editor_style.kind_label_text,
                    );
                }
            }

            // Header widget (right side of header, replaces kind_label)
            if let Some(header_slot) = self.child_index(&slots, ci, ChildSlotKind::Header) {
                if let (Some(child_tree), Some(child_layout)) = (
                    tree.children.get(header_slot),
                    child_layouts.get(header_slot),
                ) {
                    let child_cursor =
                        if Self::slot_receives_cursor(slots[header_slot], cursor_owner) {
                            canvas_cursor
                        } else {
                            mouse::Cursor::Unavailable
                        };
                    if header_clip.is_some() {
                        renderer.with_transformation(canvas_transform, |renderer| {
                            self.content[ci].header.as_ref().unwrap().as_widget().draw(
                                child_tree,
                                renderer,
                                theme,
                                style,
                                *child_layout,
                                child_cursor,
                                &canvas_viewport,
                            );
                        });
                    }
                }
            }

            // Body widget
            if let Some(body_slot) = self.child_index(&slots, ci, ChildSlotKind::Body) {
                if let (Some(child_tree), Some(child_layout)) =
                    (tree.children.get(body_slot), child_layouts.get(body_slot))
                {
                    let child_cursor = if Self::slot_receives_cursor(slots[body_slot], cursor_owner)
                    {
                        canvas_cursor
                    } else {
                        mouse::Cursor::Unavailable
                    };
                    if body_clip.is_some() {
                        renderer.with_transformation(canvas_transform, |renderer| {
                            self.content[ci].body.as_ref().unwrap().as_widget().draw(
                                child_tree,
                                renderer,
                                theme,
                                style,
                                *child_layout,
                                child_cursor,
                                &canvas_viewport,
                            );
                        });
                    }
                }
            }

            // Final border pass on top of node content so embedded widgets do not
            // visually eat the right/bottom edge border.
            if border_width > 0.0 {
                let border_inset = border_width * 0.5;
                let border_rect = Rectangle {
                    x: rect.x + border_inset,
                    y: rect.y + border_inset,
                    width: (rect.width - border_width).max(0.0),
                    height: (rect.height - border_width).max(0.0),
                };

                if border_rect.width > 0.0 && border_rect.height > 0.0 {
                    draw::quad(
                        renderer,
                        border_rect,
                        Color::TRANSPARENT,
                        card_border,
                        border_width,
                        (card_radius - border_inset).max(0.0),
                        Shadow::default(),
                    );
                }
            }

            renderer.end_layer();

            // Non-selected node ports stay in the main node pass. Selected
            // node ports are drawn in the final top layer with the rest of the
            // selected-node chrome.
            if !node.selected {
                draw_node_ports(renderer, ci, node, rect, node_height, chrome_scale);
            }
        }
        renderer.end_layer();

        renderer.start_layer(clip_bounds);

        for &ci in &visual_node_indices {
            let Some(node) = ordered_nodes.get(ci) else {
                continue;
            };
            if !node.selected {
                continue;
            }

            let rect = self.node_screen_rect(node, root_bounds);
            let chrome_scale = self.viewport.zoom.clamp(0.35, 1.0);
            draw_node_ports(renderer, ci, node, rect, node.cached_height, chrome_scale);
        }

        // Selection rectangle
        if let Some(selection) = self.selection {
            let rect = Rectangle {
                x: selection.start.x.min(selection.current.x),
                y: selection.start.y.min(selection.current.y),
                width: (selection.current.x - selection.start.x).abs(),
                height: (selection.current.y - selection.start.y).abs(),
            };
            draw::selection_rect(
                renderer,
                self.viewport.scene_rect_to_screen(rect, root_bounds),
                editor_style.selection_fill,
                editor_style.selection_border,
            );
        }

        for (slot_idx, &slot) in slots.iter().enumerate() {
            if !slot.is_selected_overlay() || !Self::slot_visible(slot, selected_primary_index) {
                continue;
            }

            let (Some(child_tree), Some(child_layout)) =
                (tree.children.get(slot_idx), child_layouts.get(slot_idx))
            else {
                continue;
            };

            let child_cursor = if Self::slot_receives_cursor(slot, cursor_owner) {
                if slot.uses_canvas_transform() {
                    canvas_cursor
                } else {
                    cursor
                }
            } else {
                mouse::Cursor::Unavailable
            };

            if slot.uses_canvas_transform() {
                renderer.with_transformation(canvas_transform, |renderer| {
                    self.child_element(slot).as_widget().draw(
                        child_tree,
                        renderer,
                        theme,
                        style,
                        *child_layout,
                        child_cursor,
                        &canvas_viewport,
                    );
                });
            } else {
                self.child_element(slot).as_widget().draw(
                    child_tree,
                    renderer,
                    theme,
                    style,
                    *child_layout,
                    child_cursor,
                    viewport,
                );
            }
        }

        if let Some(hovered_edge) = hovered_edge {
            draw::edge_delete_button(
                renderer,
                hovered_edge.button_bounds,
                editor_style.palette_bg,
                if hovered_edge.button_hovered {
                    editor_style.edge_preview
                } else {
                    editor_style.edge
                },
                editor_style.title_text,
            );
        }

        // Context palette
        if let Some(ctx_palette) = self.context_palette {
            draw::palette(
                renderer,
                self.palette_entries,
                self.viewport,
                root_bounds,
                ctx_palette.position,
                &editor_style,
            );
        }

        renderer.end_layer();
        renderer.end_layer();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let Some(visible_bounds) = Self::visible_bounds(bounds, *viewport) else {
            return mouse::Interaction::default();
        };

        if !cursor.is_over(visible_bounds) {
            return mouse::Interaction::default();
        }

        if self.drag.is_some() || self.pan.is_some() {
            return mouse::Interaction::Grabbing;
        }

        let canvas_transform = self.viewport.transformation(bounds);
        let canvas_inverse = canvas_transform.inverse();
        let canvas_cursor = cursor * canvas_inverse;
        let canvas_viewport = *viewport * canvas_inverse;
        let child_layouts: Vec<_> = layout.children().collect();
        let slots = self.child_slots();
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);
        let child_screen_bounds: Vec<_> = slots
            .iter()
            .zip(child_layouts.iter())
            .map(|(&slot, child)| Self::child_screen_bounds(slot, child.bounds(), canvas_transform))
            .collect();
        let screen_port_hit = |cursor_screen: Point| -> bool {
            visual_node_indices.iter().rev().any(|&index| {
                let Some(node) = ordered_nodes.get(index) else {
                    return false;
                };
                let rect = self.node_screen_rect(node, layout.bounds());
                let chrome_scale = self.viewport.zoom.clamp(0.35, 1.0);

                node.inputs.iter().chain(node.outputs.iter()).any(|port| {
                    Self::port_hit_bounds(node, port, rect, node.cached_height, chrome_scale)
                        .contains(cursor_screen)
                })
            })
        };

        if let Some(cursor_screen) = cursor.position() {
            if self.cursor_on_palette(cursor_screen, bounds) {
                return mouse::Interaction::Pointer;
            }
            let cursor_over_node = self
                .screen_node_index_at(&ordered_nodes, &visual_node_indices, bounds, cursor_screen)
                .is_some();

            if let Some(slot_idx) = Self::top_selected_overlay_slot_at(
                &slots,
                &child_screen_bounds,
                selected_primary_index,
                cursor_screen,
            ) {
                let slot = slots[slot_idx];
                let interaction = self.child_element(slot).as_widget().mouse_interaction(
                    &tree.children[slot_idx],
                    child_layouts[slot_idx],
                    if slot.uses_canvas_transform() {
                        canvas_cursor
                    } else {
                        cursor
                    },
                    if slot.uses_canvas_transform() {
                        &canvas_viewport
                    } else {
                        viewport
                    },
                    renderer,
                );

                return if interaction != mouse::Interaction::default() {
                    interaction
                } else {
                    mouse::Interaction::Pointer
                };
            }

            if screen_port_hit(cursor_screen) {
                return mouse::Interaction::Pointer;
            }

            if !cursor_over_node
                && self
                    .hovered_edge_target(&ordered_nodes, bounds, cursor_screen)
                    .is_some_and(|hovered_edge| hovered_edge.button_hovered)
            {
                return mouse::Interaction::Pointer;
            }

            if let Some(slot_idx) = self.top_canvas_slot_at(
                &slots,
                &child_screen_bounds,
                &ordered_nodes,
                &visual_node_indices,
                layout.bounds(),
                cursor_screen,
            ) {
                let slot = slots[slot_idx];
                let interaction = self.child_element(slot).as_widget().mouse_interaction(
                    &tree.children[slot_idx],
                    child_layouts[slot_idx],
                    canvas_cursor,
                    &canvas_viewport,
                    renderer,
                );

                if interaction != mouse::Interaction::default() {
                    return interaction;
                }
            }

            if !cursor_over_node
                && self
                    .hovered_edge_target(&ordered_nodes, bounds, cursor_screen)
                    .is_some()
            {
                return mouse::Interaction::Pointer;
            }
        }

        mouse::Interaction::default()
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let ordered_nodes = self.ordered_nodes();
        let visual_node_indices = Self::visual_node_indices(&ordered_nodes);
        let selected_primary_index =
            Self::selected_primary_index(&ordered_nodes, &visual_node_indices);
        let canvas_transform = Transformation::translate(translation.x, translation.y)
            * self.viewport.transformation(layout.bounds());
        let canvas_inverse = canvas_transform.inverse();
        let transformed_viewport = *viewport * canvas_inverse;
        let child_layouts: Vec<_> = layout.children().collect();
        let slots = self.child_slots();
        let slot_order =
            Self::overlay_slot_order(&slots, &visual_node_indices, selected_primary_index);
        let mut flat_elems: Vec<Option<(ChildSlot, &'b mut Element<'a, Message, Theme>)>> = self
            .content
            .iter_mut()
            .enumerate()
            .flat_map(|(content_idx, content)| {
                let mut elems: Vec<(ChildSlot, &mut Element<'a, Message, Theme>)> =
                    Vec::with_capacity(4);

                if let Some(header) = content.header.as_mut() {
                    elems.push((
                        ChildSlot {
                            content_idx,
                            kind: ChildSlotKind::Header,
                        },
                        header,
                    ));
                }
                if let Some(body) = content.body.as_mut() {
                    elems.push((
                        ChildSlot {
                            content_idx,
                            kind: ChildSlotKind::Body,
                        },
                        body,
                    ));
                }
                if let Some(selected_top) = content.selected_top.as_mut() {
                    elems.push((
                        ChildSlot {
                            content_idx,
                            kind: ChildSlotKind::SelectedTop,
                        },
                        selected_top,
                    ));
                }
                if let Some(selected_bottom) = content.selected_bottom.as_mut() {
                    elems.push((
                        ChildSlot {
                            content_idx,
                            kind: ChildSlotKind::SelectedBottom,
                        },
                        selected_bottom,
                    ));
                }

                elems.into_iter().map(Some)
            })
            .collect();
        let mut child_trees: Vec<Option<&'b mut Tree>> =
            tree.children.iter_mut().map(Some).collect();

        slot_order
            .into_iter()
            .filter_map(|slot_idx| {
                let slot = slots.get(slot_idx).copied()?;
                if !Self::slot_visible(slot, selected_primary_index) {
                    return None;
                }

                Some((
                    slot,
                    flat_elems.get_mut(slot_idx)?.take()?.1,
                    child_trees.get_mut(slot_idx)?.take()?,
                    *child_layouts.get(slot_idx)?,
                ))
            })
            .find_map(|(slot, elem, child_tree, child_layout)| {
                elem.as_widget_mut()
                    .overlay(
                        child_tree,
                        child_layout,
                        renderer,
                        if slot.uses_canvas_transform() {
                            &transformed_viewport
                        } else {
                            viewport
                        },
                        if slot.uses_canvas_transform() {
                            Vector::ZERO
                        } else {
                            translation
                        },
                    )
                    .map(|overlay| {
                        if slot.uses_canvas_transform() {
                            overlay::Element::new(Box::new(TransformedOverlay::new(
                                overlay,
                                *viewport,
                                canvas_transform,
                            )))
                        } else {
                            overlay
                        }
                    })
            })
    }
}

impl<'a, Message: Clone + 'a, Theme: Catalog + 'a> From<FlowEditor<'a, Message, Theme>>
    for Element<'a, Message, Theme>
{
    fn from(editor: FlowEditor<'a, Message, Theme>) -> Self {
        Element::new(editor)
    }
}
