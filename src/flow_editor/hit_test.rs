use iced::{Point, Rectangle};

use crate::flow_editor::model::{FlowNode, NodeId, PaletteEntry, PortDef, PortId, PortSide};

pub const HEADER_HEIGHT: f32 = 42.0;
pub const PORT_BASE_PROTRUSION: f32 = 5.0;
pub const PORT_HIT_RADIUS: f32 = 13.0;

#[derive(Debug, Clone, Copy)]
pub struct PaletteItem {
    pub template_id: u64,
    pub bounds: Rectangle,
}

pub fn header_rect(node: &FlowNode) -> Rectangle {
    Rectangle {
        x: node.position.x,
        y: node.position.y,
        width: node.width,
        height: HEADER_HEIGHT,
    }
}

pub fn body_rect(node: &FlowNode) -> Rectangle {
    Rectangle {
        x: node.position.x,
        y: node.position.y + HEADER_HEIGHT,
        width: node.width,
        height: (node.cached_height - HEADER_HEIGHT).max(0.0),
    }
}

/// Computes the scene-space centre of a port using its pre-computed y_offset.
pub fn port_center(node: &FlowNode, port: &PortDef) -> Point {
    let x = match port.side {
        PortSide::Input => node.position.x - PORT_BASE_PROTRUSION,
        PortSide::Output => node.position.x + node.width + PORT_BASE_PROTRUSION,
    };
    Point::new(x, node.position.y + port.y_offset)
}

pub fn header_at(nodes: &[FlowNode], cursor_scene: Point) -> Option<NodeId> {
    nodes
        .iter()
        .rev()
        .find_map(|node| header_rect(node).contains(cursor_scene).then_some(node.id))
}

pub fn body_at(nodes: &[FlowNode], cursor_scene: Point) -> Option<NodeId> {
    nodes
        .iter()
        .rev()
        .find_map(|node| body_rect(node).contains(cursor_scene).then_some(node.id))
}

pub fn port_at(nodes: &[FlowNode], cursor_scene: Point) -> Option<(NodeId, PortId, PortSide)> {
    nodes.iter().rev().find_map(|node| {
        node.inputs
            .iter()
            .chain(node.outputs.iter())
            .find_map(|port| {
                let center = port_center(node, port);
                let dx = cursor_scene.x - center.x;
                let dy = cursor_scene.y - center.y;
                let hit = dx * dx + dy * dy <= PORT_HIT_RADIUS.powi(2);
                hit.then_some((node.id, port.id, port.side))
            })
    })
}

pub fn palette_items(entries: &[PaletteEntry], origin_scene: Point) -> Vec<PaletteItem> {
    let width = 210.0;
    let height = 38.0;

    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| PaletteItem {
            template_id: entry.id,
            bounds: Rectangle {
                x: origin_scene.x,
                y: origin_scene.y + index as f32 * (height + 6.0),
                width,
                height,
            },
        })
        .collect()
}

pub fn palette_hit(
    entries: &[PaletteEntry],
    origin_scene: Point,
    cursor_scene: Point,
) -> Option<u64> {
    palette_items(entries, origin_scene)
        .into_iter()
        .find_map(|item| {
            item.bounds
                .contains(cursor_scene)
                .then_some(item.template_id)
        })
}
