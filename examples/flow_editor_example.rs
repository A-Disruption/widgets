//! # Flow Editor Example
//!
//! Two node types wired together on a pannable / zoomable canvas.
//!
//! ## Node types
//!
//! ### HTTP Request  (`NodeContent::body(…).with_header(…)`)
//! - **Header widget**: a `pick_list` for the HTTP method (GET / POST / PUT / DELETE).
//!   Styled with `flow_editor::styles::header_pick_list_style` so it blends into
//!   the node header — no visible border at rest, subtle highlight on hover/open.
//!   Replaces the `kind_label` canvas-text draw call entirely.
//! - **Body widget**: a `text_input` for the request URL, padded inside the node.
//! - **Ports**: one Flow-in (left) and one Flow-out (right) at header centre.
//! - **Accent**: teal / cyan bar on the left edge of the header.
//!
//! ### Transform  (`NodeContent::empty()`)
//! - No iced widgets at all — title and kind_label are drawn as canvas primitives.
//! - **Ports**: one Flow-in (left) and one Flow-out (right).
//! - **Accent**: amber bar on the left edge of the header.
//!
//! ## Features demonstrated
//!
//! | Feature | Where |
//! |---|---|
//! | `NodeContent::body(…).with_header(…)` | HTTP Request node |
//! | `NodeContent::empty()` | Transform node |
//! | `header_pick_list_style` | Method pick_list in the HTTP header |
//! | `.style(…)` theme integration | `FlowEditor::new(…).style(…)` |
//! | Pre-wired edge | HTTP Request out → Transform in |
//! | Right-click context palette | two entries |
//! | Drag, pan, zoom, marquee, connect, delete, duplicate | action handler |

use std::collections::HashMap;

use iced::keyboard;
use iced::widget::{Stack, button, column, container, pick_list, row, text, text_input};
use iced::{Color, Element, Length, Point, Rectangle, Theme, Vector};

use widgets::flow_editor::{
    self, ConnectionPreview, ContextPalette, DragState, Edge, FlowEditorAction, FlowNode,
    NodeContent, NodeId, PaletteEntry, PanState, PendingConnection, PortDef, PortId, PortSide,
    PortType, SelectionRect, Style, Viewport2D,
};

// ─── Geometry constants ───────────────────────────────────────────────────────
//
// HEADER_H must match `hit_test::HEADER_HEIGHT` (42.0) — the editor uses that
// value internally for header hit-testing and layout.

const HEADER_H: f32 = 42.0;
const NODE_W: f32 = 220.0;

// Body dimensions for the HTTP Request node.
const BODY_PAD: f32 = 8.0; // top + bottom padding inside the body area
const TEXT_INPUT_H: f32 = 36.0; // approximate rendered height of a text_input
const HTTP_BODY_H: f32 = BODY_PAD * 2.0 + TEXT_INPUT_H;

// ─── Fixed node / port IDs ────────────────────────────────────────────────────

const HTTP_ID: u64 = 1;
const HTTP_PORT_IN: u64 = 10;
const HTTP_PORT_OUT: u64 = 11;

const XFORM_ID: u64 = 2;
const XFORM_PORT_IN: u64 = 20;
#[allow(dead_code)]
const XFORM_PORT_OUT: u64 = 21;

// Context-palette template IDs.
const TMPL_HTTP: u64 = 100;
const TMPL_XFORM: u64 = 101;

// ─── Application data per node ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    const ALL: &'static [HttpMethod] = &[
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Delete,
    ];
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::Get => f.write_str("GET"),
            HttpMethod::Post => f.write_str("POST"),
            HttpMethod::Put => f.write_str("PUT"),
            HttpMethod::Delete => f.write_str("DELETE"),
        }
    }
}

/// Semantic data owned by the host app for each node.
/// The `FlowNode` struct only stores geometry and display state.
#[derive(Debug, Clone)]
enum NodeKind {
    HttpRequest { method: HttpMethod, url: String },
    Transform { input_mapping: String },
}

// ─── Messages ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Message {
    /// All canvas interactions arrive as a single `FlowEditorAction`.
    FlowAction(FlowEditorAction),

    /// HTTP Request — header pick_list changed.
    MethodChanged(NodeId, HttpMethod),

    /// HTTP Request — body text_input changed.
    UrlChanged(NodeId, String),

    /// Selected overlay panel text_input changed.
    InputMappingChanged(NodeId, String),
}

// ─── Application state ────────────────────────────────────────────────────────

struct App {
    // ── Flow graph ────────────────────────────────────────────────────────────
    nodes: Vec<FlowNode>,
    /// Application-level data indexed by NodeId.0.
    /// The `FlowNode` only knows geometry; semantic data lives here.
    node_data: HashMap<u64, NodeKind>,
    z_order: Vec<NodeId>,
    edges: Vec<Edge>,

    // ── Viewport ──────────────────────────────────────────────────────────────
    viewport: Viewport2D,

    // ── Interaction sub-state (passed straight into FlowEditor::new) ──────────
    //
    // These are kept in the host app so the widget is stateless — it reads
    // them, emits actions, and the app updates them.  This pattern means
    // you can serialize / inspect the full editor state at any time.
    drag: Option<DragState>,
    pan: Option<PanState>,
    selection: Option<SelectionRect>,
    connection_preview: Option<ConnectionPreview>,
    context_palette: Option<ContextPalette>,
    modifiers: keyboard::Modifiers,

    // ── ID counter for dynamically created nodes ──────────────────────────────
    next_id: u64,

    // ── Context-palette template list ─────────────────────────────────────────
    //
    // Stored here so the slice reference in `view()` lives long enough.
    palette_entries: Vec<PaletteEntry>,
}

impl App {
    fn new() -> Self {
        let http = build_http_node(NodeId(HTTP_ID), Point::new(80.0, 120.0), HTTP_PORT_IN);
        let xform = build_transform_node(NodeId(XFORM_ID), Point::new(380.0, 140.0), XFORM_PORT_IN);

        let mut node_data = HashMap::new();
        node_data.insert(
            HTTP_ID,
            NodeKind::HttpRequest {
                method: HttpMethod::Get,
                url: "https://api.example.com/data".into(),
            },
        );
        node_data.insert(
            XFORM_ID,
            NodeKind::Transform {
                input_mapping: "in".into(),
            },
        );

        // Pre-wire HTTP output → Transform input.
        let edges = vec![Edge::new(
            NodeId(HTTP_ID),
            PortId(HTTP_PORT_OUT),
            NodeId(XFORM_ID),
            PortId(XFORM_PORT_IN),
        )];

        App {
            nodes: vec![http, xform],
            node_data,
            z_order: vec![NodeId(HTTP_ID), NodeId(XFORM_ID)],
            edges,
            viewport: Viewport2D::default(),
            drag: None,
            pan: None,
            selection: None,
            connection_preview: None,
            context_palette: None,
            modifiers: keyboard::Modifiers::default(),
            next_id: 200,
            palette_entries: vec![
                PaletteEntry {
                    id: TMPL_HTTP,
                    label: "HTTP Request",
                    kind_label: "io",
                    accent_color: Color::from_rgb8(0, 188, 212),
                },
                PaletteEntry {
                    id: TMPL_XFORM,
                    label: "Transform",
                    kind_label: "transform",
                    accent_color: Color::from_rgb8(255, 167, 38),
                },
            ],
        }
    }

    fn title(&self) -> String {
        "Flow Editor Example".into()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn port_def(&self, node_id: NodeId, port_id: PortId) -> Option<&PortDef> {
        self.nodes
            .iter()
            .find(|node| node.id == node_id)
            .and_then(|node| {
                node.inputs
                    .iter()
                    .chain(node.outputs.iter())
                    .find(|port| port.id == port_id)
            })
    }

    fn port_types_compatible(a: &PortType, b: &PortType) -> bool {
        a == b
    }

    fn compatible_port(node: &FlowNode, side: PortSide, kind: &PortType) -> Option<PortId> {
        node.inputs
            .iter()
            .chain(node.outputs.iter())
            .find(|port| port.side == side && Self::port_types_compatible(&port.kind, kind))
            .map(|port| port.id)
    }

    // ─── update ───────────────────────────────────────────────────────────────

    fn update(&mut self, message: Message) {
        match message {
            Message::FlowAction(action) => self.handle_action(action),

            Message::MethodChanged(id, method) => {
                if let Some(NodeKind::HttpRequest { method: m, .. }) = self.node_data.get_mut(&id.0)
                {
                    *m = method;
                }
            }

            Message::UrlChanged(id, url) => {
                if let Some(NodeKind::HttpRequest { url: u, .. }) = self.node_data.get_mut(&id.0) {
                    *u = url;
                }
            }

            Message::InputMappingChanged(id, input_mapping) => {
                if let Some(NodeKind::Transform {
                    input_mapping: mapping,
                }) = self.node_data.get_mut(&id.0)
                {
                    *mapping = input_mapping;
                }
            }
        }
    }

    fn handle_action(&mut self, action: FlowEditorAction) {
        use FlowEditorAction as A;
        match action {
            // ── Keyboard modifiers ────────────────────────────────────────────
            A::UpdateModifiers { modifiers } => {
                self.modifiers = modifiers;
            }

            // ── Selection ─────────────────────────────────────────────────────
            //
            // Shift-click is additive; plain click deselects others.
            A::SelectSingle(id) => {
                let additive = self.modifiers.shift();
                for node in &mut self.nodes {
                    if node.id == id {
                        node.selected = true;
                    } else if !additive {
                        node.selected = false;
                    }
                }
                // Bring clicked node to front in the z-order.
                self.z_order.retain(|&z| z != id);
                self.z_order.push(id);
            }

            A::ClearSelection => {
                for node in &mut self.nodes {
                    node.selected = false;
                }
                self.selection = None;
            }

            // ── Node dragging ──────────────────────────────────────────────────
            //
            // `DragState::from_nodes` records the starting positions of every
            // selected node so we can compute absolute positions during drag
            // rather than accumulating floating-point deltas.
            A::StartNodeDrag { id, cursor_scene } => {
                // If the dragged node isn't already selected, select it alone.
                if !self.nodes.iter().any(|n| n.id == id && n.selected) {
                    for node in &mut self.nodes {
                        node.selected = node.id == id;
                    }
                }
                self.drag = Some(DragState::from_nodes(id, cursor_scene, &self.nodes));
            }

            A::DragNodeTo { cursor_scene } => {
                if let Some(drag) = &self.drag {
                    let delta = cursor_scene - drag.cursor_start_scene;
                    for node in &mut self.nodes {
                        for i in 0..drag.selected_count {
                            let (dragged_id, origin) = drag.selected_origins[i];
                            if node.id == dragged_id {
                                node.position = origin + delta;
                            }
                        }
                    }
                }
            }

            A::EndNodeDrag => {
                self.drag = None;
            }

            // ── Canvas panning ─────────────────────────────────────────────────
            A::StartPan { cursor_screen } => {
                self.pan = Some(PanState {
                    cursor_start_screen: cursor_screen,
                    pan_start: self.viewport.pan,
                });
            }

            A::PanTo { cursor_screen } => {
                if let Some(pan) = &self.pan {
                    let delta = cursor_screen - pan.cursor_start_screen;
                    self.viewport.pan = pan.pan_start + delta;
                }
            }

            A::EndPan => {
                self.pan = None;
            }

            // ── Marquee selection ──────────────────────────────────────────────
            A::StartSelection {
                cursor_scene,
                additive,
            } => {
                if !additive {
                    for node in &mut self.nodes {
                        node.selected = false;
                    }
                }
                self.selection = Some(SelectionRect {
                    start: cursor_scene,
                    current: cursor_scene,
                    additive,
                });
            }

            A::UpdateSelection { cursor_scene } => {
                if let Some(sel) = &mut self.selection {
                    sel.current = cursor_scene;
                    let rect = aabb(sel.start, sel.current);
                    for node in &mut self.nodes {
                        let hit = node.rect().intersects(&rect);
                        if sel.additive {
                            node.selected |= hit;
                        } else {
                            node.selected = hit;
                        }
                    }
                }
            }

            A::FinishSelection => {
                self.selection = None;
            }

            // ── Port connections ───────────────────────────────────────────────
            //
            // The editor draws a preview curve while the user drags from a port.
            // On drop onto another port, `FinishConnection` is emitted.
            A::StartConnection {
                node,
                port,
                side,
                cursor_scene,
            } => {
                self.connection_preview = Some(ConnectionPreview {
                    from_node: node,
                    from_port: port,
                    side,
                    cursor_scene,
                });
            }

            A::UpdateConnectionPreview { cursor_scene } => {
                if let Some(p) = &mut self.connection_preview {
                    p.cursor_scene = cursor_scene;
                }
            }

            A::FinishConnection {
                from_node,
                from_port,
                to_node,
                to_port,
            } => {
                self.connection_preview = None;
                // Avoid duplicate edges.
                let exists = self.edges.iter().any(|e| {
                    e.from_node == from_node
                        && e.from_port == from_port
                        && e.to_node == to_node
                        && e.to_port == to_port
                });
                if !exists {
                    self.edges
                        .push(Edge::new(from_node, from_port, to_node, to_port));
                }
            }

            A::CancelConnection => {
                self.connection_preview = None;
            }

            // ── Zoom ──────────────────────────────────────────────────────────
            //
            // We keep the scene point under the cursor fixed by recomputing pan
            // after changing zoom:
            //   screen_to_scene(cursor) = scene_point_before
            //   => pan.x = cursor_screen.x - bounds.x - scene_before.x * new_zoom
            A::ZoomAt {
                cursor_screen,
                root_bounds,
                delta,
            } => {
                let scene_before = self.viewport.screen_to_scene(cursor_screen, root_bounds);
                let factor = 1.0 + delta * 0.12;
                self.viewport.zoom = (self.viewport.zoom * factor).clamp(0.2, 3.5);
                self.viewport.pan = Vector::new(
                    cursor_screen.x - root_bounds.x - scene_before.x * self.viewport.zoom,
                    cursor_screen.y - root_bounds.y - scene_before.y * self.viewport.zoom,
                );
            }

            // ── Delete / duplicate ─────────────────────────────────────────────
            A::DeleteSelected => {
                let ids: Vec<NodeId> = self
                    .nodes
                    .iter()
                    .filter(|n| n.selected)
                    .map(|n| n.id)
                    .collect();
                self.nodes.retain(|n| !n.selected);
                self.z_order.retain(|id| !ids.contains(id));
                self.edges
                    .retain(|e| !ids.contains(&e.from_node) && !ids.contains(&e.to_node));
                for id in &ids {
                    self.node_data.remove(&id.0);
                }
            }

            A::DeleteEdge(edge) => {
                self.edges.retain(|existing| *existing != edge);
            }

            A::DuplicateSelected => {
                let originals: Vec<FlowNode> =
                    self.nodes.iter().filter(|n| n.selected).cloned().collect();
                for node in &mut self.nodes {
                    node.selected = false;
                }
                let offset = Vector::new(30.0, 30.0);
                for orig in originals {
                    let new_id = NodeId(self.next_id);
                    self.next_id += 100;

                    if let Some(data) = self.node_data.get(&orig.id.0).cloned() {
                        self.node_data.insert(new_id.0, data);
                    }

                    let mut clone = orig.clone();
                    clone.id = new_id;
                    clone.position = orig.position + offset;
                    clone.selected = true;
                    clone.inputs = renumber_ports(orig.inputs, self.next_id);
                    clone.outputs = renumber_ports(orig.outputs, self.next_id + 50);
                    self.next_id += 100;

                    self.z_order.push(new_id);
                    self.nodes.push(clone);
                }
            }

            A::ToggleSelectedEnabled => {
                for node in &mut self.nodes {
                    if node.selected {
                        node.enabled = !node.enabled;
                    }
                }
            }

            A::CenterView => {
                self.viewport = Viewport2D::default();
            }

            // Clipboard not implemented in this minimal example.
            A::CopySelected | A::CutSelected | A::Paste => {}

            // ── Context palette ────────────────────────────────────────────────
            //
            // Right-clicking the canvas opens a floating palette. The palette
            // entries are defined below (`PALETTE_ENTRIES`) and rendered by the
            // editor. When the user clicks an entry, `CreateNodeFromTemplate` is
            // emitted with the entry's `template_id`.
            A::OpenContextPalette { cursor_scene } => {
                self.context_palette = Some(ContextPalette {
                    position: cursor_scene,
                    pending_connection: None,
                });
            }

            A::OpenContextPaletteFromConnection {
                cursor_scene,
                from_node,
                from_port,
                side,
            } => {
                self.context_palette = Some(ContextPalette {
                    position: cursor_scene,
                    pending_connection: Some(PendingConnection {
                        source_node: from_node,
                        source_port: from_port,
                        source_side: side,
                        drop_point: cursor_scene,
                    }),
                });
                self.connection_preview = None;
            }

            A::CloseContextPalette => {
                self.context_palette = None;
            }

            A::CreateNodeFromTemplate {
                template_id,
                position,
            } => {
                let pending_connection = self
                    .context_palette
                    .and_then(|palette| palette.pending_connection);
                self.context_palette = None;
                let new_id = NodeId(self.next_id);
                let port_base = self.next_id + 1;
                self.next_id += 100;

                let node = match template_id {
                    TMPL_HTTP => {
                        self.node_data.insert(
                            new_id.0,
                            NodeKind::HttpRequest {
                                method: HttpMethod::Get,
                                url: String::new(),
                            },
                        );
                        build_http_node(new_id, position, port_base)
                    }
                    _ => {
                        self.node_data.insert(
                            new_id.0,
                            NodeKind::Transform {
                                input_mapping: "in".into(),
                            },
                        );
                        build_transform_node(new_id, position, port_base)
                    }
                };

                let pending_edge = pending_connection
                    .and_then(|pending| {
                        self.port_def(pending.source_node, pending.source_port)
                            .map(|source_port| (pending, source_port.kind.clone()))
                    })
                    .and_then(|(pending, source_kind)| {
                        let new_port_side = match pending.source_side {
                            PortSide::Output => PortSide::Input,
                            PortSide::Input => PortSide::Output,
                        };
                        Self::compatible_port(&node, new_port_side, &source_kind)
                            .map(|new_port| (pending, new_port))
                    });

                self.z_order.push(new_id);
                self.nodes.push(node);

                if let Some((pending, new_port)) = pending_edge {
                    let (from_node, from_port, to_node, to_port) = match pending.source_side {
                        PortSide::Output => {
                            (pending.source_node, pending.source_port, new_id, new_port)
                        }
                        PortSide::Input => {
                            (new_id, new_port, pending.source_node, pending.source_port)
                        }
                    };

                    let exists = self.edges.iter().any(|edge| {
                        edge.from_node == from_node
                            && edge.from_port == from_port
                            && edge.to_node == to_node
                            && edge.to_port == to_port
                    });

                    if !exists {
                        self.edges
                            .push(Edge::new(from_node, from_port, to_node, to_port));
                    }
                }
            }
        }
    }

    // ─── view ─────────────────────────────────────────────────────────────────

    fn view(&self) -> Element<'_, Message> {
        // ── Build NodeContent for every node ──────────────────────────────────
        //
        // `content` must be 1-to-1 with `self.z_order` (same length, same order).
        // The FlowEditor maps content[i] to the node at z_order[i].
        //
        // Each NodeContent can carry regular node widgets plus optional selected overlays:
        //   - `.with_header(widget)` → rendered in the right portion of the header,
        //     replacing the `kind_label` canvas text. Good for pick_lists, badges.
        //   - `NodeContent::body(widget)` → rendered below the header divider.
        //   - `.with_selected_top(widget)` / `.with_selected_bottom(widget)` →
        //     anchored above or below the primary selected node.
        //   - `NodeContent::empty()` → no regular node widgets; pure canvas node.
        let content: Vec<NodeContent<'_, Message>> = self
            .z_order
            .iter()
            .filter_map(|id| self.nodes.iter().find(|node| node.id == *id))
            .map(|node| match self.node_data.get(&node.id.0) {
                Some(NodeKind::HttpRequest { method, url }) => {
                    let node_id = node.id;

                    // ── Header widget: HTTP method pick_list ──────────────────
                    //
                    // `header_pick_list_style` gives the pick_list a transparent
                    // background so it visually merges with the node header.
                    // A subtle border appears on hover / when the dropdown is open.
                    // Match the active Iced theme so the embedded header control
                    // stays in sync with the default flow-editor palette.
                    let header = pick_list(HttpMethod::ALL, Some(*method), move |m| {
                        Message::MethodChanged(node_id, m)
                    })
                    .width(Length::Fill)
                    .padding([5, 8])
                    .text_size(13.0)
                    .style(|theme, status| {
                        flow_editor::styles::header_pick_list_style(
                            theme,
                            status,
                            &Style::from_theme(theme),
                        )
                    });

                    // ── Body widget: URL text_input ───────────────────────────
                    //
                    // Wrapped in a container for consistent padding inside the
                    // node body area.
                    let url_val = url.clone();
                    let body = container(
                        text_input("https://...", &url_val)
                            .on_input(move |s| Message::UrlChanged(node_id, s))
                            .width(Length::Fill)
                            .padding([7, 10])
                            .size(12.0),
                    )
                    .padding(BODY_PAD as u16);

                    // Combine: body widget below header, pick_list in the header,
                    // plus a selected-state toolbar rendered above the node.
                    NodeContent::body(body)
                        .with_header(header)
                        .with_selected_top(selected_toolbar(node))
                }

                // ── Transform: canvas-only node ───────────────────────────────
                //
                // `NodeContent::empty()` tells the editor there are no child
                // widgets. The editor draws `node.title` and `node.kind_label`
                // as canvas text primitives instead.
                Some(NodeKind::Transform { input_mapping }) => NodeContent::empty()
                    .with_selected_top(selected_toolbar(node))
                    .with_selected_bottom(selected_mapping_panel(node.id, input_mapping)),

                None => NodeContent::empty(),
            })
            .collect();

        // ── FlowEditor widget ──────────────────────────────────────────────────
        //
        // `.style(...)` integrates with Iced's theme system. The closure receives
        // the active `Theme` and returns a `Style` struct. Here we tweak a few
        // fields while inheriting the rest from `Style::from_theme(theme)`.
        //
        // `.on_action(…)` maps every canvas interaction to `Message::FlowAction`
        // so the app's `update` function can handle it.
        let editor: Element<'_, Message> = flow_editor::FlowEditor::new(
            &self.nodes,
            &self.z_order,
            &self.edges,
            self.viewport,
            self.drag,
            self.pan,
            self.selection,
            self.connection_preview,
            self.context_palette,
            self.modifiers,
            &self.palette_entries,
            content,
        )
        .style(|theme| {
            let p = theme.extended_palette();
            Style {
                canvas_bg: p.background.base.color,
                grid_line: Color {
                    a: 0.10,
                    ..p.background.strong.color
                },
                node_bg: p.background.weak.color,
                node_border: p.background.strong.color,
                // All other fields follow the active Iced theme by default.
                ..Style::from_theme(theme)
            }
        })
        .on_action(Message::FlowAction)
        .into();

        let zoom_overlay: Element<'_, Message> = container(
            container(text(format!("Zoom: {:>3.0}%", self.viewport.zoom * 100.0)).size(13.0))
                .padding([4, 8])
                .style(|theme: &Theme| {
                    let p = theme.extended_palette();
                    iced::widget::container::Style::default()
                        .background(Color {
                            a: 0.72,
                            ..p.background.base.color
                        })
                        .border(iced::Border {
                            color: Color {
                                a: 0.45,
                                ..p.background.strong.color
                            },
                            width: 1.0,
                            radius: iced::border::radius(8.0),
                        })
                }),
        )
        .padding(10)
        .align_left(Length::Fill)
        .align_bottom(Length::Fill)
        .into();

        Stack::new()
            .width(Length::Fill)
            .height(Length::Fill)
            .push(editor)
            .push(zoom_overlay)
            .into()
    }
}

// ─── Node builder helpers ─────────────────────────────────────────────────────

fn selected_toolbar(node: &FlowNode) -> Element<'_, Message> {
    let toggle_label = if node.enabled { "Disable" } else { "Enable" };

    container(
        row![
            button(text("Duplicate").size(12.0))
                .padding([4, 10])
                .on_press(Message::FlowAction(FlowEditorAction::DuplicateSelected)),
            button(text("Delete").size(12.0))
                .padding([4, 10])
                .style(flow_editor::styles::danger_icon_button_style)
                .on_press(Message::FlowAction(FlowEditorAction::DeleteSelected)),
            button(text(toggle_label).size(12.0))
                .padding([4, 10])
                .on_press(Message::FlowAction(FlowEditorAction::ToggleSelectedEnabled)),
        ]
        .spacing(8),
    )
    .padding([8, 10])
    .style(|theme: &Theme| {
        let editor_style = Style::from_theme(theme);
        container::Style::default()
            .background(Color {
                a: 0.96,
                ..editor_style.header_bg
            })
            .color(editor_style.title_text)
            .border(iced::Border {
                color: Color {
                    a: 0.80,
                    ..editor_style.node_border_selected
                },
                width: 1.0,
                radius: iced::border::radius(14.0),
            })
    })
    .into()
}

fn selected_mapping_panel<'a>(node_id: NodeId, input_mapping: &'a str) -> Element<'a, Message> {
    container(
        column![
            text("Input Mapping").size(13.0),
            text_input("Input name", input_mapping)
                .on_input(move |value| Message::InputMappingChanged(node_id, value))
                .padding([6, 10])
                .size(13.0)
                .width(Length::Fill),
        ]
        .spacing(8),
    )
    .width(Length::Fixed(NODE_W))
    .padding(12)
    .style(|theme: &Theme| {
        let editor_style = Style::from_theme(theme);
        container::Style::default()
            .background(editor_style.node_bg)
            .color(editor_style.title_text)
            .border(iced::Border {
                color: editor_style.node_border,
                width: 1.0,
                radius: iced::border::radius(12.0),
            })
    })
    .into()
}

/// HTTP Request node: body text_input + header pick_list + two Flow ports.
///
/// Port y_offset is the vertical distance from `node.position.y` to the port
/// centre in scene space. Positioning ports at `HEADER_H / 2.0` centres them
/// in the header row — the typical convention for flow ports.
fn build_http_node(id: NodeId, position: Point, port_base: u64) -> FlowNode {
    let port_y = HEADER_H / 2.0;
    FlowNode {
        id,
        position,
        width: NODE_W,
        // cached_height = header + body. The host keeps this in sync whenever
        // it changes node content (e.g. after adding/removing ports).
        cached_height: HEADER_H + HTTP_BODY_H,
        selected: false,
        enabled: true,
        title: "HTTP Request".into(),
        // kind_label is skipped when a header widget is present (the widget
        // replaces it), but it's still useful as a tooltip / accessibility label.
        kind_label: "io".into(),
        accent_color: Color::from_rgb8(0, 188, 212), // teal accent bar
        inputs: vec![PortDef::new(
            port_base,
            "in",
            PortSide::Input,
            0,
            PortType::Flow,
            port_y,
        )],
        outputs: vec![PortDef::new(
            port_base + 1,
            "out",
            PortSide::Output,
            0,
            PortType::Flow,
            port_y,
        )],
    }
}

/// Transform node: canvas-only, no body/header widgets.
fn build_transform_node(id: NodeId, position: Point, port_base: u64) -> FlowNode {
    let port_y = HEADER_H / 2.0;
    FlowNode {
        id,
        position,
        width: NODE_W,
        // No body — height equals the header only.
        cached_height: HEADER_H,
        selected: false,
        enabled: true,
        title: "Transform".into(),
        kind_label: "transform".into(),
        accent_color: Color::from_rgb8(255, 167, 38), // amber accent bar
        inputs: vec![PortDef::new(
            port_base,
            "in",
            PortSide::Input,
            0,
            PortType::Flow,
            port_y,
        )],
        outputs: vec![PortDef::new(
            port_base + 1,
            "out",
            PortSide::Output,
            0,
            PortType::Flow,
            port_y,
        )],
    }
}

// ─── Misc helpers ─────────────────────────────────────────────────────────────

/// Build an axis-aligned rectangle from two arbitrary points.
fn aabb(a: Point, b: Point) -> Rectangle {
    Rectangle {
        x: a.x.min(b.x),
        y: a.y.min(b.y),
        width: (a.x - b.x).abs(),
        height: (a.y - b.y).abs(),
    }
}

/// Assign new port IDs when duplicating a node (so IDs remain unique).
fn renumber_ports(ports: Vec<PortDef>, base: u64) -> Vec<PortDef> {
    ports
        .into_iter()
        .enumerate()
        .map(|(i, p)| PortDef {
            id: PortId(base + i as u64),
            ..p
        })
        .collect()
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .title(App::title)
        .run()
}
