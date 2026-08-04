use iced::{Color, Element, Point, Rectangle, Theme, Vector};

// ─── Style ───────────────────────────────────────────────────────────────────

/// All colours used by the flow editor canvas.
/// Obtain one by implementing [`Catalog`] for your theme, derive one from
/// an [`iced::Theme`] with [`Style::from_theme`], or use [`Style::dark()`]
/// for the legacy palette.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Main canvas background fill.
    pub canvas_bg: Color,
    /// Subtle grid line colour (use low alpha).
    pub grid_line: Color,
    /// Node card body background.
    pub node_bg: Color,
    /// Node card border when unselected.
    pub node_border: Color,
    /// Node card border when selected.
    pub node_border_selected: Color,
    /// Header background when unselected.
    pub header_bg: Color,
    /// Header background when selected.
    pub header_bg_selected: Color,
    /// Title text inside node header.
    pub title_text: Color,
    /// Secondary kind-label text (used when no header widget is set).
    pub kind_label_text: Color,
    /// Colour of input port tabs.
    pub port_input: Color,
    /// Colour of output port tabs.
    pub port_output: Color,
    /// Edge / connection line colour.
    pub edge: Color,
    /// In-progress connection preview colour.
    pub edge_preview: Color,
    /// Selection rectangle fill (use low alpha).
    pub selection_fill: Color,
    /// Selection rectangle border (use medium alpha).
    pub selection_border: Color,
    /// Context palette item background.
    pub palette_bg: Color,
    /// Context palette item border.
    pub palette_item_border: Color,
    /// Context palette item label text.
    pub palette_text: Color,
}

impl Style {
    /// Legacy dark palette matching the original hard-coded colour scheme.
    pub fn dark() -> Self {
        Self {
            canvas_bg: Color::from_rgb8(11, 14, 20),
            grid_line: Color::from_rgba8(45, 52, 66, 0.28),
            node_bg: Color::from_rgb8(18, 23, 31),
            node_border: Color::from_rgb8(38, 46, 60),
            node_border_selected: Color::from_rgb8(82, 155, 255),
            header_bg: Color::from_rgb8(24, 29, 40),
            header_bg_selected: Color::from_rgb8(30, 43, 69),
            title_text: Color::from_rgb8(210, 224, 245),
            kind_label_text: Color::from_rgb8(120, 145, 178),
            port_input: Color::from_rgb8(112, 197, 146),
            port_output: Color::from_rgb8(99, 179, 255),
            edge: Color::from_rgb8(99, 179, 255),
            edge_preview: Color::from_rgb8(255, 190, 92),
            selection_fill: Color::from_rgba8(82, 155, 255, 0.12),
            selection_border: Color::from_rgba8(82, 155, 255, 0.55),
            palette_bg: Color::from_rgb8(24, 29, 40),
            palette_item_border: Color::from_rgb8(49, 59, 77),
            palette_text: Color::from_rgb8(210, 222, 240),
        }
    }

    /// Derive a default flow-editor palette from the active Iced theme.
    pub fn from_theme(theme: &Theme) -> Self {
        let palette = theme.palette();

        Self {
            canvas_bg: palette.background.base.color,
            grid_line: Color {
                a: 0.12,
                ..palette.background.strong.color
            },
            node_bg: palette.background.weak.color,
            node_border: palette.background.strong.color,
            node_border_selected: palette.primary.strong.color,
            header_bg: palette.background.strong.color,
            header_bg_selected: palette.primary.weak.color,
            title_text: palette.background.strong.text,
            kind_label_text: palette.background.weak.text,
            port_input: palette.success.base.color,
            port_output: palette.primary.base.color,
            edge: palette.primary.base.color,
            edge_preview: palette.warning.base.color,
            selection_fill: Color {
                a: 0.12,
                ..palette.primary.strong.color
            },
            selection_border: Color {
                a: 0.55,
                ..palette.primary.strong.color
            },
            palette_bg: palette.background.strong.color,
            palette_item_border: palette.background.stronger.color,
            palette_text: palette.background.strong.text,
        }
    }
}

/// Backwards-compatible alias. Prefer [`Style`] in new code.
pub type FlowEditorPalette = Style;

// ─── Catalog ─────────────────────────────────────────────────────────────────

/// Theming trait for [`FlowEditor`](crate::flow_editor::FlowEditor).
///
/// Implement this for your own theme type, or rely on the provided
/// implementation for [`iced::Theme`] which derives from
/// [`Theme::extended_palette`](iced::Theme::extended_palette).
pub trait Catalog {
    /// The style class type accepted by the `.style()` / `.class()` builders.
    type Class<'a>;
    /// The default class used when no explicit style is set.
    fn default<'a>() -> Self::Class<'a>;
    /// Produce a [`Style`] from the theme for the given class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A boxed style closure — the default `Class` type for `iced::Theme`.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(Style::from_theme)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

// ─── NodeContent ─────────────────────────────────────────────────────────────

/// The iced widget content for a single node in the flow editor.
///
/// Aligned 1-to-1 with `z_order` when passed to `FlowEditor::new`.
/// Header/body widgets are authored in the node's unscaled scene space.
/// The flow editor applies the active viewport pan + zoom when rendering and
/// hit-testing them, so host applications should not manually scale them.
/// Selected top/bottom overlays are authored in ordinary screen space and are
/// anchored above / below the primary selected node by the widget itself.
///
/// ```rust,ignore
/// // Body only
/// NodeContent::body(column![text_input("...", &val)])
///
/// // Body + header widget (replaces the kind_label text)
/// NodeContent::body(my_body)
///     .with_header(pick_list(&opts, selected, Msg::Changed))
///
/// // Optional widgets shown only when this node is the primary selection
/// NodeContent::empty()
///     .with_selected_top(my_toolbar)
///     .with_selected_bottom(my_details_panel)
/// ```
pub struct NodeContent<'a, Message, Theme = iced::Theme> {
    /// Widget rendered in the right portion of the node header.
    /// When `Some`, replaces the `kind_label` text draw call.
    pub header: Option<Element<'a, Message, Theme>>,
    /// Widget rendered in the node body (below the header line).
    pub body: Option<Element<'a, Message, Theme>>,
    /// Optional widget anchored above the node while it is the primary
    /// selection. It follows the node in scene-space, scales with zoom,
    /// and is width-limited to the node width.
    pub selected_top: Option<Element<'a, Message, Theme>>,
    /// Optional widget anchored below the node while it is the primary
    /// selection. It follows the node in scene-space, scales with zoom,
    /// and is width-limited to the node width.
    pub selected_bottom: Option<Element<'a, Message, Theme>>,
}

impl<'a, Message, Theme> NodeContent<'a, Message, Theme> {
    /// Content with a body widget only.
    pub fn body(widget: impl Into<Element<'a, Message, Theme>>) -> Self {
        Self {
            header: None,
            body: Some(widget.into()),
            selected_top: None,
            selected_bottom: None,
        }
    }

    /// Content with no widgets (canvas-only node).
    pub fn empty() -> Self {
        Self {
            header: None,
            body: None,
            selected_top: None,
            selected_bottom: None,
        }
    }

    /// Attach a widget to the right portion of the header.
    pub fn with_header(mut self, widget: impl Into<Element<'a, Message, Theme>>) -> Self {
        self.header = Some(widget.into());
        self
    }

    /// Attach a widget above the node while it is the primary selection.
    pub fn with_selected_top(mut self, widget: impl Into<Element<'a, Message, Theme>>) -> Self {
        self.selected_top = Some(widget.into());
        self
    }

    /// Attach a widget below the node while it is the primary selection.
    pub fn with_selected_bottom(mut self, widget: impl Into<Element<'a, Message, Theme>>) -> Self {
        self.selected_bottom = Some(widget.into());
        self
    }
}

// ─── Identifiers ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortId(pub u64);

// ─── Port definitions ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSide {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Object,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PortType {
    Flow,
    Data(VariableType),
}

/// A single port on a node.
///
/// `y_offset` is the vertical distance from the node's top-left corner
/// (in scene space) to the port's centre. Computed by the host application
/// in `rebuild_ports()` so the library does not need to know node templates.
#[derive(Debug, Clone)]
pub struct PortDef {
    pub id: PortId,
    pub label: String,
    pub side: PortSide,
    pub slot: usize,
    pub kind: PortType,
    /// Vertical offset from node.position.y to this port's centre, scene units.
    pub y_offset: f32,
}

impl PortDef {
    pub fn new(
        id: u64,
        label: impl Into<String>,
        side: PortSide,
        slot: usize,
        kind: PortType,
        y_offset: f32,
    ) -> Self {
        Self {
            id: PortId(id),
            label: label.into(),
            side,
            slot,
            kind,
            y_offset,
        }
    }
}

// ─── Node ────────────────────────────────────────────────────────────────────

/// All geometry and display state for a single node.
/// Semantic data (what the node *does*) is managed by the host application.
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: NodeId,
    pub position: Point,
    pub width: f32,
    /// Pre-computed total height. Host calls `rebuild_ports()` which also
    /// recomputes this value.
    pub cached_height: f32,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    pub selected: bool,
    pub enabled: bool,
    pub title: String,
    pub kind_label: String,
    /// Optional accent colour drawn as a 4 px left-edge bar in the header.
    /// Set to `Color::TRANSPARENT` (default) to hide.
    pub accent_color: Color,
}

impl FlowNode {
    pub fn rect(&self) -> Rectangle {
        Rectangle {
            x: self.position.x,
            y: self.position.y,
            width: self.width,
            height: self.cached_height,
        }
    }
}

// ─── Edge ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub from_node: NodeId,
    pub from_port: PortId,
    pub to_node: NodeId,
    pub to_port: PortId,
}

impl Edge {
    pub fn new(from_node: NodeId, from_port: PortId, to_node: NodeId, to_port: PortId) -> Self {
        Self {
            from_node,
            from_port,
            to_node,
            to_port,
        }
    }
}

// ─── Palette ─────────────────────────────────────────────────────────────────

/// One entry in the context-menu palette shown when right-clicking.
/// The host application defines these to describe available node templates.
#[derive(Debug, Clone)]
pub struct PaletteEntry {
    pub id: u64,
    pub label: &'static str,
    pub kind_label: &'static str,
    pub accent_color: Color,
}

// ─── Editor interaction state ────────────────────────────────────────────────
// These structs are stored by the host application and passed into FlowEditor.

#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub lead_node: NodeId,
    pub cursor_start_scene: Point,
    pub selected_origins: [(NodeId, Point); 32],
    pub selected_count: usize,
}

impl DragState {
    pub fn from_nodes(lead_node: NodeId, cursor_start_scene: Point, nodes: &[FlowNode]) -> Self {
        let mut selected_origins = [(NodeId(0), Point::ORIGIN); 32];
        let mut selected_count = 0;

        for node in nodes.iter().filter(|n| n.selected) {
            if selected_count < selected_origins.len() {
                selected_origins[selected_count] = (node.id, node.position);
                selected_count += 1;
            }
        }

        Self {
            lead_node,
            cursor_start_scene,
            selected_origins,
            selected_count,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PanState {
    pub cursor_start_screen: Point,
    pub pan_start: Vector,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectionRect {
    pub start: Point,
    pub current: Point,
    pub additive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionPreview {
    pub from_node: NodeId,
    pub from_port: PortId,
    pub side: PortSide,
    pub cursor_scene: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct PendingConnection {
    pub source_node: NodeId,
    pub source_port: PortId,
    pub source_side: PortSide,
    pub drop_point: Point,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextPalette {
    pub position: Point,
    pub pending_connection: Option<PendingConnection>,
}
