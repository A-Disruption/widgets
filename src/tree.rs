use iced::{
    advanced::{
        layout,
        renderer,
        text::Renderer as _,
        widget::{self, tree::Tree},
        Clipboard, Layout, Shell, Widget,
    }, border::Radius, keyboard, mouse, widget::text::Alignment, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Size, Vector
};
use std::collections::{HashSet, HashMap};

// Constants for layout
const LINE_HEIGHT: f32 = 32.0;       
const ARROW_X_PAD: f32 = 4.0;       
const ARROW_W: f32 = 16.0;          
const HANDLE_HOVER_W: f32 = 24.0;   
const HANDLE_STRIPE_W: f32 = 2.0;   
const CONTENT_GAP: f32 = 14.0;       
const DRAG_THRESHOLD: f32 = 5.0;     // Minimum distance to start drag

/// Creates a new [`TreeHandle`] with the given root branches.
pub fn tree_handle<'a, Message, Theme, Renderer>(
    roots: impl IntoIterator<Item = Branch<'a, Message, Theme, Renderer>>,
) -> TreeHandle<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer,
{
    TreeHandle::new(roots)
}

/// Creates a new [`Branch`] with the given content element.
pub fn branch<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Branch<'a, Message, Theme, Renderer>
{
    Branch {
        content: content.into(),
        children: Vec::new(),
        external_id: 0,
        align_x: iced::Alignment::Start,
        align_y: iced::Alignment::Center,
        accepts_drops: false,
        draggable: true,
    }
}

#[derive(Debug, Clone)]
pub struct DropInfo{
    pub dragged_ids: Vec<usize>,
    pub target_id: Option<usize>,
    pub position: DropPosition,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DropPosition {
    Before,
    After, 
    Into,
}

#[allow(missing_debug_implementations)]
pub struct TreeHandle<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> 
where 
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    branches: Vec<Branch_>,
    branch_content: Vec<Element<'a, Message, Theme, Renderer>>, 
    width: Length, 
    height: Length,
    spacing: f32, 
    indent: f32, 
    padding_x: f32,
    padding_y: f32,
    on_drop: Option<Box<dyn Fn(DropInfo) -> Message + 'a>>,
    on_select: Option<Box< dyn Fn(HashSet<usize>) -> Message + 'a>>,
    force_reset_order: bool,
    ext_to_int: HashMap<usize, usize>,
    int_to_ext: Vec<usize>, // index is internal id; value is external id or 0
    class: Theme::Class<'a>,
}

#[derive(Clone, Debug)]
struct Branch_ {
    id: usize,
    external_id: usize,
    parent_id: Option<usize>,
    depth: u16,
    has_children: bool,
    accepts_drops: bool,
    draggable: bool,
    align_x: iced::Alignment,
    align_y: iced::Alignment,
}

#[derive(Clone, Debug)]
struct BranchState {
    id: usize,
    parent_id: Option<usize>,
    depth: u16,
}

// Combined state structure
#[derive(Default)]
struct TreeState {
    // Visual state
    expanded: HashSet<usize>,
    branch_heights: Vec<f32>,
    branch_widths: Vec<f32>,
    visible_branches: Vec<bool>,
    
    // Interaction state
    selected: HashSet<usize>,
    focused: Option<usize>,
    hovered: Option<usize>,
    hovered_handle: Option<usize>,
    
    // Drag state
    drag_pending: Option<DragPending>,
    drag_active: Option<DragActive>,
    
    // Tree structure state (for reordering)
    branch_order: Option<Vec<BranchState>>,

    // Track keyboard modifiers
    current_modifiers: keyboard::Modifiers,
}

#[derive(Debug, Clone)]
struct DragPending {
    start_position: Point,
    branch_ids: Vec<usize>,
    primary_branch_id: usize, // Actual dragged branch, for overlay rendering
    branch_bounds: Rectangle,
    click_offset: Vector,
}

#[derive(Debug, Clone)]
struct DragActive {
    dragged_nodes: Vec<usize>,
    primary_node: usize, // Actual dragged branch, for overlay rendering
    drag_start_bounds: Rectangle,
    click_offset: Vector,
    current_position: Point,
    drop_target: Option<usize>,
    drop_position: DropPosition,
}

impl<'a, Message, Theme, Renderer> 
    TreeHandle<'a, Message, Theme, Renderer>
where 
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer  + iced::advanced::text::Renderer,
{
    /// Creates a new [`TreeHandle`] from root branches.
    pub fn new<'b>( 
        roots: impl IntoIterator<Item = Branch<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let roots = roots.into_iter();

        let mut width = Length::Fill;
        let mut height = Length::Shrink;

        let mut branches = Vec::new();
        let mut branch_content = Vec::new();
        let mut next_id = 0usize;

        // Flatten the tree structure into arrays
        fn flatten_branch<'a, Message, Theme, Renderer>(
            branch: Branch<'a, Message, Theme, Renderer>,
            parent_id: Option<usize>,
            depth: u16,
            next_id: &mut usize,
            branches: &mut Vec<Branch_>,
            branch_content: &mut Vec<Element<'a, Message, Theme, Renderer>>,
            width: &mut Length,
            height: &mut Length,
        ) where
            Renderer: iced::advanced::Renderer  + iced::advanced::text::Renderer,
        {
            let current_id = *next_id;
            *next_id += 1;
            
            let has_children = !branch.children.is_empty();
            
            branches.push(Branch_ {
                id: current_id,
                external_id: branch.external_id,
                parent_id,
                depth,
                has_children,
                accepts_drops: branch.accepts_drops,
                draggable: branch.draggable,
                align_x: branch.align_x,
                align_y: branch.align_y,
            });
            
            let size_hint = branch.content.as_widget().size_hint();
            *width = width.enclose(size_hint.width);
            *height = height.enclose(size_hint.height);
            branch_content.push(branch.content);
            
            for child in branch.children {
                flatten_branch(
                    child,
                    Some(current_id),
                    depth + 1,
                    next_id,
                    branches,
                    branch_content,
                    width,
                    height,
                );
            }
        }

        for root in roots {
            flatten_branch(
                root,
                None,
                0,
                &mut next_id,
                &mut branches,
                &mut branch_content,
                &mut width,
                &mut height,
            );
        }

        let mut ext_to_int = HashMap::new();
        let mut int_to_ext = vec![0usize; branches.len()];

        for b in &branches {
            if b.external_id != 0 {
                // Use debug_assert for development, or handle duplicates gracefully
                if ext_to_int.contains_key(&b.external_id) {
                    eprintln!("Warning: duplicate external_id {} found", b.external_id);
                }
                ext_to_int.insert(b.external_id, b.id);
                int_to_ext[b.id] = b.external_id;
            }
        }

        Self {
            branches,
            branch_content,
            width,
            height,
            spacing: 4.0,
            indent: 20.0,
            padding_x: 10.0,
            padding_y: 5.0,
            on_drop: None,
            on_select: None,
            force_reset_order: false,
            ext_to_int,
            int_to_ext,
            class: Theme::default(),
        }
    }

    /// Sets the message to emit when a drop occurs
    pub fn on_drop<F>(mut self, f: F) -> Self 
    where
        F: Fn(DropInfo) -> Message + 'a,
    {
        self.on_drop = Some(Box::new(f));
        self
    }

    /// Sets the message emit when a branch is selected
    pub fn on_select<F>(mut self, f: F) -> Self
    where 
        F: Fn(HashSet<usize>) -> Message + 'a
    {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Forces the tree to reset its internal ordering state.
    /// This is useful when the external structure has changed and
    /// the tree needs to reflect the new hierarchy based on external IDs.
    pub fn reset_order_state(mut self) -> Self {
        self.force_reset_order = true;
        self
    }

    /// Sets the width of the [`Tree`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Tree`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the indent of the [`Tree`].
    pub fn indent(mut self, px: f32) -> Self { 
        self.indent = px; 
        self 
    }

    /// Sets the spacing of the [`Tree`].
    pub fn spacing(mut self, px: f32) -> Self { 
        self.spacing = px; 
        self 
    }

    /// Sets the class of the [`Tree`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self { 
        self.class = class.into(); 
        self 
    }

    /// Sets the padding of the cells of the [`Tree`].
    pub fn padding(self, padding: impl Into<Pixels>) -> Self {
        let padding = padding.into();
        self.padding_x(padding).padding_y(padding)
    }

    /// Sets the horizontal padding of the cells of the [`Tree`].
    pub fn padding_x(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding_x = padding.into().0;
        self
    }

    /// Sets the vertical padding of the cells of the [`Tree`].
    pub fn padding_y(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding_y = padding.into().0;
        self
    }

    // Helper to get ordered indices from saved state
    fn get_ordered_indices(&self, state: &TreeState) -> Vec<usize> {
        if let Some(ref branch_order) = state.branch_order {
            let mut indices = Vec::new();
            
            for bs in branch_order {
                if let Some(idx) = self.branches.iter().position(|b| b.id == bs.id) {
                    indices.push(idx);
                }
            }
            
            // Add any new branches not in saved state
            for (i, branch) in self.branches.iter().enumerate() {
                if !branch_order.iter().any(|bs| bs.id == branch.id) {
                    indices.push(i);
                }
            }
            
            indices
        } else {
            (0..self.branches.len()).collect()
        }
    }
    
    // Helper to get effective branch info
    fn get_branch_info(&self, index: usize, state: &TreeState) -> (usize, Option<usize>, u16) {
        let branch = &self.branches[index];
        
        if let Some(ref branch_order) = state.branch_order {
            if let Some(bs) = branch_order.iter().find(|bs| bs.id == branch.id) {
                return (branch.id, bs.parent_id, bs.depth);
            }
        }
        
        (branch.id, branch.parent_id, branch.depth)
    }
    
    // Determines if a branch is visible
    fn is_branch_visible(&self, index: usize, state: &TreeState) -> bool {
        if index >= self.branches.len() {
            return false;
        }

        let (id, parent_id, _) = self.get_branch_info(index, state);
        
        // Check if being dragged
        if let Some(ref drag) = state.drag_active {
            if drag.dragged_nodes.contains(&id) {
                return false;
            }
            
            // Check if parent is being dragged
            if let Some(parent_id) = parent_id {
                if drag.dragged_nodes.contains(&parent_id) {
                    return false;
                }
                
                // Check ancestors
                let mut current_parent = parent_id;
                while let Some(parent_idx) = self.branches.iter().position(|b| b.id == current_parent) {
                    if drag.dragged_nodes.contains(&current_parent) {
                        return false;
                    }
                    let (_, next_parent, _) = self.get_branch_info(parent_idx, state);
                    if let Some(np) = next_parent {
                        current_parent = np;
                    } else {
                        break;
                    }
                }
            }
        }
        
        // Root level items are always visible
        if parent_id.is_none() {
            return true;
        }
        
        // Check if parent is expanded
        if let Some(parent_id) = parent_id {
            if let Some(parent_index) = self.branches.iter().position(|b| b.id == parent_id) {
                return self.is_branch_visible(parent_index, state) 
                    && state.expanded.contains(&parent_id);
            }
        }
        
        false
    }

    /// Calculate drop position based on mouse position
    fn calculate_drop_position(
        &self, 
        mouse_y: f32, 
        branch_bounds: Rectangle, 
        has_children: bool, 
        expanded: bool, 
        accepts_drops: bool  // Add this parameter
    ) -> DropPosition {
        let relative_y = mouse_y - branch_bounds.y;
        let third_height = branch_bounds.height / 3.0;
        
        let can_drop_into = (has_children && expanded) || accepts_drops;
        
        if relative_y < third_height {
            DropPosition::Before
        } else if relative_y > branch_bounds.height - third_height {
            DropPosition::After
        } else if can_drop_into {
            DropPosition::Into
        } else {
            if relative_y < branch_bounds.height / 2.0 {
                DropPosition::Before
            } else {
                DropPosition::After
            }
        }
    }

    fn update_has_children(&mut self, state: &TreeState) -> Vec<usize> {
        // Track which branches are gaining children for the first time
        let mut newly_has_children = Vec::new();
        
        // Store current has_children state
        let previous_state: Vec<(usize, bool)> = self.branches
            .iter()
            .map(|b| (b.id, b.has_children))
            .collect();
        
        // Reset all to false
        for branch in &mut self.branches {
            branch.has_children = false;
        }
        
        // Check actual parent-child relationships from state
        if let Some(ref branch_order) = state.branch_order {
            let parent_ids: HashSet<usize> = branch_order
                .iter()
                .filter_map(|bs| bs.parent_id)
                .collect();
            
            for branch in &mut self.branches {
                if parent_ids.contains(&branch.id) {
                    branch.has_children = true;
                    
                    // Check if this branch didn't have children before
                    if let Some((_, prev_has_children)) = previous_state.iter()
                        .find(|(id, _)| *id == branch.id) {
                        if !prev_has_children {
                            newly_has_children.push(branch.id);
                        }
                    }
                }
            }
        } else {
            // Fallback to original parent_ids
            let parent_ids: HashSet<usize> = self.branches
                .iter()
                .filter_map(|b| b.parent_id)
                .collect();
                
            for branch in &mut self.branches {
                if parent_ids.contains(&branch.id) {
                    branch.has_children = true;
                    
                    // Check if this branch didn't have children before
                    if let Some((_, prev_has_children)) = previous_state.iter()
                        .find(|(id, _)| *id == branch.id) {
                        if !prev_has_children {
                            newly_has_children.push(branch.id);
                        }
                    }
                }
            }
        }
        
        newly_has_children
    }

    #[inline]
    fn preferred_id(&self, internal_id: usize) -> usize {
        // Always prefer the external ID if it exists
        self.int_to_ext.get(internal_id).copied().unwrap_or(internal_id)
    }

}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TreeHandle<'a, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<TreeState>()
    }

    fn state(&self) -> widget::tree::State {
        let mut expanded = HashSet::new();
        
        for branch in &self.branches {
            if branch.has_children {
                expanded.insert(branch.id.clone());
            }
        }
        
        widget::tree::State::new(TreeState {
            expanded,
            branch_heights: Vec::new(),
            branch_widths: Vec::new(),
            visible_branches: Vec::new(),
            selected: HashSet::new(),
            focused: None,
            hovered: None,
            hovered_handle: None,
            drag_pending: None,
            drag_active: None,
            branch_order: None,
            current_modifiers: keyboard::Modifiers::empty(),
        })
    }

    fn children(&self) -> Vec<widget::Tree> {
        self.branch_content
            .iter()
            .map(|branch| widget::Tree::new(branch.as_widget()))
            .collect()
    }

    fn diff(&self, state: &mut widget::Tree) {
        state.diff_children(&self.branch_content);
    }

    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<TreeState>();

        // Check if we need to force reset the order
        if self.force_reset_order {
            state.branch_order = None;
            self.force_reset_order = false;
        }

        // Initialize branch order if not present
        if state.branch_order.is_none() {
            state.branch_order = Some(
                self.branches
                    .iter()
                    .map(|b| BranchState {
                        id: b.id,
                        parent_id: b.parent_id,
                        depth: b.depth,
                    })
                    .collect(),
            );
        }

        // Update has_children flags based on current state and get newly parented branches
        let newly_has_children = self.update_has_children(state);

        // Auto-expand branches that just gained children
        for branch_id in newly_has_children {
            state.expanded.insert(branch_id);
        }

        let ordered_indices = self.get_ordered_indices(state);
        let branch_count = self.branches.len();

        let limits = limits.width(self.width).height(self.height);
        let available = limits.max();
        let tree_fluid = self.width.fluid();

        // Update visibility
        state.visible_branches = vec![false; branch_count];
        for i in 0..branch_count {
            state.visible_branches[i] = self.is_branch_visible(i, state);
        }

        let mut cells = Vec::with_capacity(branch_count);
        cells.resize(branch_count, layout::Node::default());

        state.branch_heights = vec![0.0; branch_count];
        state.branch_widths = vec![0.0; branch_count];

        // Layout passes
        let mut y = self.padding_y;

        let mut width_fill_factors = vec![0u16; branch_count];
        let mut row_fill_factors = vec![0u16; branch_count];

        let mut max_content_width = 0.0f32;
        let mut total_nonfluid_height = 0.0;

        // FIRST PASS — layout non-fluid branches, collect factors for fluid ones
        for index in 0..ordered_indices.len() {
            let i = ordered_indices[index];
            if i >= self.branches.len() {
                continue;
            }

            // For invisible branches, keep default height so rows still occupy space for hover math
            if !state.visible_branches[i] {
                cells[i] = layout::Node::new(Size::ZERO);
                state.branch_heights[i] = LINE_HEIGHT;
                state.branch_widths[i] = 0.0;
            }

            let (_, _, effective_depth) = self.get_branch_info(i, state);
            let child_state = &mut tree.children[i];
            let content = &mut self.branch_content[i];

            let size_hint = content.as_widget().size();
            let h_factor = size_hint.height.fill_factor();
            let w_factor = size_hint.width.fill_factor();

            let is_width_fluid = w_factor != 0 || size_hint.width.is_fill();
            let is_height_fluid = h_factor != 0;

            if is_width_fluid || is_height_fluid {
                row_fill_factors[i] = h_factor;
                width_fill_factors[i] = w_factor.max(if size_hint.width.is_fill() { 1 } else { 0 });
                continue; // defer layout to second pass
            }

            // Non-fluid: lay out immediately with the full remaining content width
            let indent_x = self.padding_x + (effective_depth as f32 * self.indent);
            let content_x = indent_x + ARROW_W + CONTENT_GAP;
            let avail_w = (available.width - content_x - self.padding_x).max(0.0);

            let content_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(avail_w, (available.height - y).max(0.0)),
            )
            .max_width(avail_w);

            let content_layout = content.as_widget_mut().layout(child_state, renderer, &content_limits);
            let content_size =
                content_limits.resolve(Length::Shrink, Length::Shrink, content_layout.size());

            state.branch_heights[i] = content_size.height.max(LINE_HEIGHT);
            state.branch_widths[i] = content_size.width;

            let total_w = content_x + content_size.width;
            max_content_width = max_content_width.max(total_w);

            cells[i] = content_layout;
        }

        // Sum up non-fluid heights
        for (i, &h) in state.branch_heights.iter().enumerate() {
            if state.visible_branches[i] && row_fill_factors[i] == 0 {
                if let Some(ref drag) = state.drag_active {
                    if !drag.dragged_nodes.contains(&self.branches[i].id) {
                        total_nonfluid_height += h;
                    }
                } else {
                    total_nonfluid_height += h;
                }
            }
        }

        // SECOND PASS — lay out fluid branches (width and/or height)
        let total_height_fill: u16 = row_fill_factors
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                state.visible_branches.get(*i).copied().unwrap_or(false)
                    && state
                        .drag_active
                        .as_ref()
                        .map_or(true, |drag| !drag.dragged_nodes.contains(&self.branches[*i].id))
            })
            .map(|(_, &f)| f)
            .sum();

        let total_width_fill: u16 = width_fill_factors
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                state.visible_branches.get(*i).copied().unwrap_or(false)
                    && state
                        .drag_active
                        .as_ref()
                        .map_or(true, |drag| !drag.dragged_nodes.contains(&self.branches[*i].id))
            })
            .map(|(_, &f)| f)
            .sum();

        let do_height_pass = total_height_fill > 0;
        let do_width_pass = total_width_fill > 0;

        if do_height_pass || do_width_pass {
            let available_fluid_height = (available.height
                - total_nonfluid_height
                - self.padding_y * 2.0
                - self.spacing
                    * state
                        .visible_branches
                        .iter()
                        .filter(|&&v| v)
                        .count()
                        .saturating_sub(1) as f32)
                .max(0.0);

            let height_unit = if total_height_fill > 0 {
                available_fluid_height / total_height_fill as f32
            } else {
                0.0
            };

            for &i in &ordered_indices {
                if i >= self.branches.len() || !state.visible_branches[i] {
                    continue;
                }

                if let Some(ref drag) = state.drag_active {
                    if drag.dragged_nodes.contains(&self.branches[i].id) {
                        continue;
                    }
                }

                // Handle fluid rows
                if row_fill_factors[i] == 0 && width_fill_factors[i] == 0 {
                    continue;
                }

                let (_, _, effective_depth) = self.get_branch_info(i, state);
                let child_state = &mut tree.children[i];
                let content = &mut self.branch_content[i];
                let size_hint = content.as_widget().size();

                let w_factor = size_hint.width.fill_factor();
                let is_width_fluid = w_factor != 0 || size_hint.width.is_fill();

                let indent_x = self.padding_x + (effective_depth as f32 * self.indent);
                let content_x = indent_x + ARROW_W + CONTENT_GAP;
                let avail_w = (available.width - content_x - self.padding_x).max(0.0);

                let max_h = if row_fill_factors[i] == 0 {
                    if size_hint.height.is_fill() {
                        state.branch_heights[i]
                    } else {
                        (available.height - y).max(0.0)
                    }
                } else {
                    height_unit * row_fill_factors[i] as f32
                };

                let content_limits =
                    layout::Limits::new(Size::ZERO, Size::new(avail_w, max_h)).max_width(avail_w);

                let content_layout = content.as_widget_mut().layout(child_state, renderer, &content_limits);

                let content_size = content_limits.resolve(
                    if is_width_fluid { tree_fluid } else { Length::Shrink },
                    Length::Shrink,
                    content_layout.size(),
                );

                state.branch_heights[i] = state.branch_heights[i].max(content_size.height);
                state.branch_widths[i] = state.branch_widths[i].max(content_size.width);
                cells[i] = content_layout;

                let total_w = content_x + content_size.width;
                max_content_width = max_content_width.max(total_w);
            }
        }

        // THIRD PASS — position each visible branch
        y = self.padding_y;

        let drop_indicator_space = if state.drag_active.is_some() {
            LINE_HEIGHT + self.spacing
        } else {
            0.0
        };

        for &i in &ordered_indices {
            if i >= self.branches.len() || !state.visible_branches[i] {
                continue;
            }

            let branch = &self.branches[i];

            if let Some(ref drag) = state.drag_active {
                if drag.dragged_nodes.contains(&branch.id) {
                    continue;
                }

                if drag.drop_target == Some(branch.id)
                    && drag.drop_position == DropPosition::Before
                {
                    y += drop_indicator_space;
                }
            }

            let (_, _, effective_depth) = self.get_branch_info(i, state);

            let indent_x = self.padding_x + (effective_depth as f32 * self.indent);
            let content_x = indent_x + ARROW_W + CONTENT_GAP;

            cells[i].move_to_mut((content_x, y));

            let Branch_ { align_x, align_y, .. } = branch;
            cells[i].align_mut(
                *align_x,
                *align_y,
                Size::new(state.branch_widths[i], state.branch_heights[i]),
            );

            y += state.branch_heights[i] + self.spacing;

            if let Some(ref drag) = state.drag_active {
                if drag.drop_target == Some(branch.id) && drag.drop_position == DropPosition::Into {
                    if state.expanded.contains(&branch.id) {
                        y += drop_indicator_space;
                    }
                }

                if drag.drop_target == Some(branch.id) && drag.drop_position == DropPosition::After {
                    y += drop_indicator_space;
                }
            }
        }

        let intrinsic = limits.resolve(
            self.width,
            self.height,
            Size::new(
                max_content_width + self.padding_x,
                y - self.spacing + self.padding_y,
            ),
        );

        layout::Node::with_children(intrinsic, cells)
    }


    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<TreeState>();
        let ordered_indices = self.get_ordered_indices(state);
        
        // Update all visible children
        for &i in &ordered_indices {
            if i >= self.branches.len() || 
               i >= state.visible_branches.len() || 
               !state.visible_branches[i] {
                continue;
            }
            
            if let Some(ref drag) = state.drag_active {
                if drag.dragged_nodes.contains(&self.branches[i].id) {
                    continue;
                }
            }
            
            let branch = &mut self.branch_content[i];
            let child_state = &mut tree.children[i];
            let child_layout = layout.children().nth(i).unwrap();
            
            branch.as_widget_mut().update(
                child_state, event, child_layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        
        // Handle tree-specific events
        match event {
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.current_modifiers = *modifiers;
            }

            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position() {
                    let bounds = layout.bounds();
                    let mut y = bounds.y + self.padding_y;
                    
                    for &i in &ordered_indices {
                        if i >= self.branches.len() || 
                           i >= state.visible_branches.len() || 
                           !state.visible_branches[i] {
                            continue;
                        }
                        
                        let branch = &self.branches[i];
                        let (_, _, effective_depth) = self.get_branch_info(i, state);
                        
                        if let Some(ref drag) = state.drag_active {
                            if drag.dragged_nodes.contains(&branch.id) {
                                continue;
                            }
                        }
                        
                        let indent_x = bounds.x + self.padding_x + (effective_depth as f32 * self.indent);
                        let branch_height = state.branch_heights[i];
                        let branch_bounds = Rectangle {
                            x: bounds.x,
                            y,
                            width: bounds.width,
                            height: branch_height,
                        };
                        
                        // Check if clicking on arrow
                        if branch.has_children {
                            let arrow_bounds = Rectangle {
                                x: indent_x,
                                y,
                                width: ARROW_W,
                                height: branch_height,
                            };
                            
                            if arrow_bounds.contains(position) {
                                if state.expanded.contains(&branch.id) {
                                    state.expanded.remove(&branch.id);
                                } else {
                                    state.expanded.insert(branch.id);
                                }
                                shell.invalidate_layout();
                                shell.request_redraw();
                                return;
                            }
                        }

                        if branch_bounds.contains(position) {

                            if !branch.draggable {
                                // Branch is not draggable - only allow selection
                                if state.current_modifiers.control() || state.current_modifiers.command() {
                                    if state.selected.contains(&branch.id) {
                                        state.selected.remove(&branch.id);
                                    } else {
                                        state.selected.insert(branch.id);
                                    }
                                } else {
                                    state.selected.clear();
                                    state.selected.insert(branch.id);
                                }
                                state.focused = Some(branch.id);

                                if let Some(ref on_select) = self.on_select {
                                    let external_ids: HashSet<usize> = state
                                        .selected
                                        .iter()
                                        .map(|&internal| self.preferred_id(internal))
                                        .collect();
                                    shell.publish(on_select(external_ids));
                                }

                                shell.invalidate_widgets();
                                shell.request_redraw();
                                return;
                            }

                            // Set up pending drag
                            let selected_for_drag = if state.selected.contains(&branch.id) {
                                filter_redundant_selections(
                                    &state.selected.iter().cloned().collect::<Vec<_>>(),
                                    &self.branches,
                                    &state.branch_order
                                )
                            } else {
                                vec![branch.id]
                            };
                            
                            let click_offset = Vector::new(
                                position.x - branch_bounds.x,
                                position.y - branch_bounds.y,
                            );
                            
                            state.drag_pending = Some(DragPending {
                                start_position: position,
                                branch_ids: selected_for_drag,
                                primary_branch_id: branch.id,
                                branch_bounds,
                                click_offset,
                            });
                            
                            if state.current_modifiers.control() || state.current_modifiers.command() {
                                if state.selected.contains(&branch.id) {
                                    state.selected.remove(&branch.id);
                                } else {
                                    state.selected.insert(branch.id);
                                }
                            } else {
                                state.selected.clear();
                                state.selected.insert(branch.id);
                            }
                            state.focused = Some(branch.id);

                            if let Some(ref on_select) = self.on_select {
                                let external_ids: HashSet<usize> = state
                                    .selected
                                    .iter()
                                    .map(|&internal| self.preferred_id(internal))
                                    .collect();
                                shell.publish(on_select(external_ids));
                            }
                        }
                        
                        y += branch_height + self.spacing;
                    }
                }
            }

            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag_pending = None;
            }

            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(position) = cursor.position() {
                    // Check if we should start dragging
                    if let Some(ref pending) = state.drag_pending {
                        let distance = ((position.x - pending.start_position.x).powi(2) + 
                                       (position.y - pending.start_position.y).powi(2)).sqrt();
                        
                        if distance >= DRAG_THRESHOLD {
                            // Start actual drag
                            state.drag_active = Some(DragActive {
                                dragged_nodes: pending.branch_ids.clone(),
                                primary_node: pending.primary_branch_id,
                                drag_start_bounds: pending.branch_bounds,
                                click_offset: pending.click_offset,
                                current_position: position,
                                drop_target: None,
                                drop_position: DropPosition::Before,
                            });
                            state.drag_pending = None;
                            shell.invalidate_layout();
                            shell.request_redraw();
                        }
                    } else if state.drag_active.is_none() {
                        // Handle hover states
                        let bounds = layout.bounds();
                        let mut y = bounds.y + self.padding_y;
                        let mut new_hovered = None;
                        let mut new_hovered_handle = None;
                        
                        for &i in &ordered_indices {
                            if i >= self.branches.len() || 
                               i >= state.visible_branches.len() || 
                               !state.visible_branches[i] {
                                continue;
                            }
                            
                            let branch = &self.branches[i];
                            let branch_height = state.branch_heights[i];
                            let branch_bounds = Rectangle {
                                x: bounds.x,
                                y,
                                width: bounds.width,
                                height: branch_height,
                            };
                            
                            if branch_bounds.contains(position) {
                                new_hovered = Some(branch.id);
                                
                                let (_, _, effective_depth) = self.get_branch_info(i, state);
                                let indent_x = bounds.x + self.padding_x + (effective_depth as f32 * self.indent);
                                let handle_x = indent_x + ARROW_W;
                                let handle_bounds = Rectangle {
                                    x: handle_x,
                                    y: branch_bounds.y,
                                    width: HANDLE_HOVER_W,
                                    height: branch_bounds.height,
                                };
                                
                                if handle_bounds.contains(position) {
                                    new_hovered_handle = Some(branch.id);
                                }
                                break;
                            }
                            
                            y += branch_height + self.spacing;
                        }
                        
                        if new_hovered != state.hovered || new_hovered_handle != state.hovered_handle {
                            state.hovered = new_hovered;
                            state.hovered_handle = new_hovered_handle;
                            shell.request_redraw();
                        }
                    }
                }
            }

            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                if let Some(focused) = state.focused {
                    let visible_ordered: Vec<usize> = ordered_indices.iter()
                        .filter(|&&i| i < state.visible_branches.len() && state.visible_branches[i])
                        .map(|&i| self.branches[i].id)
                        .collect();

                    match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            if let Some(current_pos) = visible_ordered.iter().position(|&id| id == focused) {
                                if current_pos > 0 {
                                    state.focused = Some(visible_ordered[current_pos - 1]);
                                    shell.invalidate_widgets();
                                    shell.request_redraw();
                                }
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            if let Some(current_pos) = visible_ordered.iter().position(|&id| id == focused) {
                                if current_pos < visible_ordered.len() - 1 {
                                    state.focused = Some(visible_ordered[current_pos + 1]);
                                    shell.invalidate_widgets();
                                    shell.request_redraw();
                                }
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            if let Some(branch) = self.branches.iter().find(|b| b.id == focused) {
                                if branch.has_children && state.expanded.contains(&focused) {
                                    state.expanded.remove(&focused);
                                    shell.invalidate_layout();
                                    shell.request_redraw();
                                }
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            if let Some(branch) = self.branches.iter().find(|b| b.id == focused) {
                                if branch.has_children && !state.expanded.contains(&focused) {
                                    state.expanded.insert(focused);
                                    shell.invalidate_layout();
                                    shell.request_redraw();
                                }
                            }
                        }
                        keyboard::Key::Named(keyboard::key::Named::Space) => {
                            if modifiers.control() || modifiers.command() {
                                if state.selected.contains(&focused) {
                                    state.selected.remove(&focused);
                                } else {
                                    state.selected.insert(focused);
                                }
                            } else {
                                state.selected.clear();
                                state.selected.insert(focused);
                            }

                            if let Some(ref on_select) = self.on_select {
                                let external_ids: HashSet<usize> = state
                                    .selected
                                    .iter()
                                    .map(|&internal| self.preferred_id(internal))
                                    .collect();
                                shell.publish(on_select(external_ids));
                            }

                            shell.invalidate_widgets();
                            shell.request_redraw();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        for ((child, state), layout) in self
            .branch_content
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child.as_widget_mut().update(
                state, event, layout, cursor, renderer, clipboard, shell,
                viewport,
            );
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_ref::<TreeState>();
        let ordered_indices = self.get_ordered_indices(state);
        let tree_style = theme.style(&self.class);
        
        let mut y = bounds.y + self.padding_y;

        // Helper to draw drop preview
        let draw_drop_preview = |renderer: &mut Renderer, y: f32, depth: u16, width: f32| {
            let preview_indent = bounds.x + self.padding_x + (depth as f32 * self.indent);
            let preview_height = LINE_HEIGHT;
            
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: preview_indent,
                        y,
                        width: width - preview_indent + bounds.x,
                        height: preview_height,
                    },
                    border: Border {
                        color: tree_style.accept_drop_indicator_color,
                        width: 2.0,
                        radius: Radius::from(4.0),
                    },
                    ..Default::default()
                },
                Color::from_rgba(
                    tree_style.accept_drop_indicator_color.r,
                    tree_style.accept_drop_indicator_color.g,
                    tree_style.accept_drop_indicator_color.b,
                    0.1
                ),
            );
            
            let handle_x = preview_indent + ARROW_W;
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: handle_x,
                        y: y + 2.0,
                        width: HANDLE_STRIPE_W,
                        height: preview_height - 4.0,
                    },
                    border: Border::default(),
                    ..Default::default()
                },
                Color::from_rgba(
                    tree_style.line_color.r,
                    tree_style.line_color.g,
                    tree_style.line_color.b,
                    0.3
                ),
            );
        };

        let mut pending_into_adjustment = false;
        
        for &i in &ordered_indices {
            if i >= self.branches.len() || 
               i >= state.visible_branches.len() || 
               !state.visible_branches[i] ||
               i >= state.branch_heights.len() {
                continue;
            }
            
            let branch = &self.branches[i];
            let (id, parent_id, effective_depth) = self.get_branch_info(i, state);

            if let Some(ref drag) = state.drag_active {
                if drag.dragged_nodes.contains(&id) {
                    continue;
                }
                
                if drag.drop_target == Some(id) && drag.drop_position == DropPosition::Before {
                    let preview_depth = effective_depth;
                    draw_drop_preview(renderer, y, preview_depth, bounds.width);
                    y += LINE_HEIGHT + self.spacing;
                }
            }

            if pending_into_adjustment {
                y += LINE_HEIGHT + self.spacing;
                pending_into_adjustment = false;
            }
            
            let indent_x = bounds.x + self.padding_x + (effective_depth as f32 * self.indent);
            let branch_height = state.branch_heights[i];
            let branch_y = y;

            if let Some(ref drag) = state.drag_active {
                if drag.drop_target == Some(id) && drag.drop_position == DropPosition::Into {
                    if state.expanded.contains(&id) {
                        pending_into_adjustment = true;
                    } else {
                        let indicator_width = 30.0;
                        let indicator_x = bounds.x + bounds.width - indicator_width - 10.0;
                        
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle {
                                    x: indicator_x,
                                    y: y + branch_height / 2.0 - 1.5,
                                    width: indicator_width,
                                    height: 3.0,
                                },
                                border: Border::default(),
                                ..Default::default()
                            },
                            tree_style.accept_drop_indicator_color,
                        );
                        
                        renderer.fill_text(
                            iced::advanced::Text {
                                content: "→".into(),
                                bounds: Size::new(20.0, branch_height),
                                size: Pixels(16.0),
                                font: iced::Font::default(),
                                align_x: Alignment::Center,
                                align_y: iced::alignment::Vertical::Center,
                                line_height: iced::advanced::text::LineHeight::default(),
                                shaping: iced::advanced::text::Shaping::Advanced,
                                wrapping: iced::advanced::text::Wrapping::default(),
                            },
                            Point::new(indicator_x - 20.0, y + (branch_height / 2.0)),
                            tree_style.accept_drop_indicator_color,
                            *viewport,
                        );
                    }
                }
            }

            // Draw selection background
            if state.selected.contains(&id) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x,
                            y,
                            width: bounds.width,
                            height: branch_height,
                        },
                        border: Border::default(),
                        ..Default::default()
                    },
                    tree_style.selection_background,
                );
            }

            // Draw drop-into indicator border
            if let Some(ref drag) = state.drag_active {
                if drag.drop_target == Some(id) && drag.drop_position == DropPosition::Into {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle {
                                x: bounds.x,
                                y,
                                width: bounds.width,
                                height: branch_height,
                            },
                            border: Border {
                                color: tree_style.accept_drop_indicator_color,
                                width: 2.0,
                                radius: Radius::from(4.0),
                            },
                            ..Default::default()
                        },
                        Color::from_rgba(
                            tree_style.accept_drop_indicator_color.r,
                            tree_style.accept_drop_indicator_color.g,
                            tree_style.accept_drop_indicator_color.b,
                            0.1
                        ),
                    );
                }
            }
            
            // Draw hover/focus border
            if state.focused == Some(id) || state.hovered == Some(id) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: bounds.x,
                            y,
                            width: bounds.width,
                            height: branch_height,
                        },
                        border: Border {
                            color: tree_style.focus_border,
                            width: 1.0,
                            radius: Radius::from(2.0),
                        },
                        ..Default::default()
                    },
                    iced::Background::Color(Color::TRANSPARENT),
                );
            }
            
            // Draw expand/collapse arrow
            if branch.has_children {
                let arrow = if state.expanded.contains(&id) { "🠻" } else { "🠺" };
                
                renderer.fill_text(
                    iced::advanced::Text {
                        content: arrow.into(),
                        bounds: Size::new(ARROW_W, branch_height),
                        size: Pixels(24.0),
                        font: iced::Font::default(),
                        align_x: Alignment::Center,
                        align_y: iced::alignment::Vertical::Center,
                        line_height: iced::advanced::text::LineHeight::default(),
                        shaping: iced::advanced::text::Shaping::Advanced,
                        wrapping: iced::advanced::text::Wrapping::default(),
                    },
                    Point::new(indent_x + ARROW_X_PAD, y + (branch_height / 2.0)),
                    tree_style.arrow_color,
                    *viewport,
                );
            }
            
            // Draw handle/drag area
            let handle_x = indent_x + ARROW_W;
            let handle_width = HANDLE_STRIPE_W;
            
            let handle_color = if state.hovered_handle == Some(id) {
                Color::from_rgba(
                    tree_style.line_color.r,
                    tree_style.line_color.g,
                    tree_style.line_color.b,
                    0.3,
                )
            } else {
                tree_style.line_color
            };
            
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: handle_x,
                        y: branch_y + 2.0,
                        width: handle_width,
                        height: branch_height - 4.0,
                    },
                    border: Border::default(),
                    ..Default::default()
                },
                handle_color,
            );
            
            // Draw the branch content HERE for this specific branch
            if let Some(ref drag) = state.drag_active {
                if !drag.dragged_nodes.contains(&id) {
                    let child_state = &tree.children[i];
                    let child_layout = layout.children().nth(i).unwrap();
                    self.branch_content[i].as_widget().draw(
                        child_state, renderer, theme, style, child_layout, cursor, viewport,
                    );
                }
            } else {
                let child_state = &tree.children[i];
                let child_layout = layout.children().nth(i).unwrap();
                self.branch_content[i].as_widget().draw(
                    child_state, renderer, theme, style, child_layout, cursor, viewport,
                );
            }
            
            y += branch_height + self.spacing;

            if let Some(ref drag) = state.drag_active {
                if drag.drop_target == Some(id) && 
                   drag.drop_position == DropPosition::Into && 
                   state.expanded.contains(&id) {
                    let child_preview_y = branch_y + branch_height + self.spacing;
                    let child_depth = effective_depth + 1;
                    draw_drop_preview(renderer, child_preview_y, child_depth, bounds.width);
                }
            }

            if let Some(ref drag) = state.drag_active {
                if drag.drop_target == Some(id) && drag.drop_position == DropPosition::After {
                    let is_last_visible_item = !ordered_indices.iter()
                        .skip_while(|&&j| j != i)
                        .skip(1)
                        .any(|&j| j < self.branches.len() && state.visible_branches[j]);
                    
                    let preview_depth = if parent_id.is_some() && is_last_visible_item {
                        0
                    } else {
                        effective_depth
                    };
                    
                    draw_drop_preview(renderer, y, preview_depth, bounds.width);
                    y += LINE_HEIGHT + self.spacing;
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<TreeState>();
        
        // First check children interactions - prioritize them
        let child_interaction = self.branch_content
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .enumerate()
            .filter(|(i, _)| state.visible_branches.get(*i).copied().unwrap_or(false))
            .map(|(_, ((branch, child_state), child_layout))| {
                branch.as_widget().mouse_interaction(
                    child_state, child_layout, cursor, viewport, renderer,
                )
            })
            .find(|&interaction| interaction != mouse::Interaction::None);
        
        // If a child has a specific interaction, use it
        if let Some(interaction) = child_interaction {
            return interaction;
        }
        
        // Only show grabbing cursor if actively dragging
        if state.drag_active.is_some() {
            return mouse::Interaction::Grabbing;
        }
        
        // Default to child interactions or None
        mouse::Interaction::None
    }

    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        for i in 0..self.branch_content.len() {
            if let Some(child_layout) = layout.children().nth(i) {
                self.branch_content[i].as_widget_mut().operate(
                    &mut tree.children[i],
                    child_layout,
                    renderer,
                    operation
                );
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let drag_active_clone = {
            let state = tree.state.downcast_mut::<TreeState>();
            state.drag_active.clone()
        };

        if let Some(ref drag_active) = drag_active_clone {
            let dragged_indices: Vec<usize> = self.branches
                .iter()
                .enumerate()
                .filter(|(_, b)| drag_active.dragged_nodes.contains(&b.id))
                .map(|(i, _)| i)
                .collect();

            for (i, ((branch, child_state), child_layout)) in self
                .branch_content
                .iter_mut()
                .zip(&mut tree.children)
                .zip(layout.children())
                .enumerate()
            {
                if dragged_indices.contains(&i) {
                    return Some(iced::advanced::overlay::Element::new(Box::new(DragOverlay {
                        tree_handle: self,
                        state: tree,
                        layout: child_layout,
                        tree_layout: layout,
                        viewport: *viewport,
                        dragged_indices,
                        translation,
                    })));
                }
            }
        } else {
            return iced::advanced::overlay::from_children(
                &mut self.branch_content,
                tree,
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        None
    }
}

// Custom overlay for rendering dragged items
struct DragOverlay<'a, 'b, Message, Theme, Renderer>
where 
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer,
{
    tree_handle: &'a mut TreeHandle<'b, Message, Theme, Renderer>,
    state: &'a mut widget::Tree,
    layout: Layout<'a>,
    tree_layout: Layout<'a>,
    viewport: Rectangle,
    dragged_indices: Vec<usize>,
    translation: Vector,
}

impl<'a, Message, Theme, Renderer> iced::advanced::overlay::Overlay<Message, Theme, Renderer> 
    for DragOverlay<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        let state = self.state.state.downcast_ref::<TreeState>();
        
        let position = if let Some(ref drag) = state.drag_active {
            Point::new(
                drag.current_position.x - drag.click_offset.x,
                drag.current_position.y - drag.click_offset.y,
            )
        } else {
            Point::ORIGIN
        };

        let (width, height) = if let Some(ref drag) = state.drag_active {
            let mut max_width = 0.0f32;
            let mut total_height = 0.0f32;
            
            for &i in &self.dragged_indices {
                if i < state.branch_widths.len() {
                    let effective_depth = if let Some(ref branch_order) = state.branch_order {
                        branch_order.iter()
                            .find(|bs| self.tree_handle.branches.get(i).map(|b| b.id == bs.id).unwrap_or(false))
                            .map(|bs| bs.depth)
                            .unwrap_or(self.tree_handle.branches[i].depth)
                    } else {
                        self.tree_handle.branches[i].depth
                    };
                    
                    let indent_x = effective_depth as f32 * self.tree_handle.indent;
                    let content_width = indent_x + ARROW_W + CONTENT_GAP + state.branch_widths[i] + self.tree_handle.padding_x;
                    max_width = max_width.max(content_width);
                    
                    total_height += state.branch_heights[i].max(LINE_HEIGHT);
                    if i < self.dragged_indices.len() - 1 {
                        total_height += self.tree_handle.spacing;
                    }
                }
            }
            
            (max_width.max(200.0), total_height.max(LINE_HEIGHT))
        } else {
            (300.0, LINE_HEIGHT)
        };      

        layout::Node::new(Size::new(width, height))
            .move_to(position)
    }

    fn update(
        &mut self,
        event: &Event,
        _layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(position) = cursor.position() {
                    let state = self.state.state.downcast_mut::<TreeState>();
                    let ordered_indices = self.tree_handle.get_ordered_indices(state);
                    
                    let branch_infos: Vec<_> = ordered_indices.iter()
                        .filter_map(|&i| {
                            if i >= self.tree_handle.branches.len() || 
                               i >= state.visible_branches.len() || 
                               !state.visible_branches[i] {
                                return None;
                            }
                            
                            let branch = &self.tree_handle.branches[i];
                            let (id, parent_id, depth) = self.tree_handle.get_branch_info(i, state);
                            
                            let branch_height = if i < state.branch_heights.len() {
                                state.branch_heights[i]
                            } else {
                                LINE_HEIGHT
                            };
                            
                            Some((
                                id,
                                parent_id,
                                depth,
                                branch_height,
                                branch.has_children,
                                state.expanded.contains(&id),
                                branch.accepts_drops
                            ))
                        })
                        .collect();
                    
                    if let Some(ref mut drag) = state.drag_active {
                        drag.current_position = position;
                        
                        let tree_bounds = self.tree_layout.bounds();
                        let mut new_drop_target = drag.drop_target;
                        let mut new_drop_position = drag.drop_position.clone();
                        
                        let mut branch_positions = Vec::new();
                        let mut y = tree_bounds.y + self.tree_handle.padding_y;
                        
                        for (id, parent_id, depth, branch_height, has_children, is_expanded, accepts_drops) in &branch_infos {
                            if drag.dragged_nodes.contains(id) {
                                continue;
                            }
                            
                            branch_positions.push((
                                *id,
                                *parent_id,
                                *depth,
                                y,
                                *branch_height,
                                *has_children,
                                *is_expanded,
                                *accepts_drops
                            ));
                            
                            y += branch_height + self.tree_handle.spacing;
                        }
                        
                        let mut found_target = false;
                        for (id, parent_id, depth, branch_y, height, has_children, is_expanded, accepts_drops) in &branch_positions {
                            let row_bounds = Rectangle {
                                x: tree_bounds.x,
                                y: *branch_y,
                                width: tree_bounds.width,
                                height: *height,
                            };
                            
                            let expanded_bounds = Rectangle {
                                x: row_bounds.x,
                                y: row_bounds.y - 2.0,
                                width: row_bounds.width,
                                height: row_bounds.height + 4.0,
                            };
                            
                            if expanded_bounds.contains(position) {
                                found_target = true;
                                new_drop_target = Some(*id);
                                
                                new_drop_position = self.tree_handle.calculate_drop_position(
                                    position.y,
                                    row_bounds,
                                    *has_children,
                                    *is_expanded,
                                    *accepts_drops  // Pass accepts_drops
                                );
                                break;
                            }
                        }

                        if !found_target && position.y > tree_bounds.y && !branch_positions.is_empty() {
                            let (last_id, _, _, last_y, last_height, _, _, _) = 
                                branch_positions.last().unwrap();
                            
                            if position.y > last_y + last_height {
                                new_drop_target = Some(*last_id);
                                new_drop_position = DropPosition::After;
                            }
                        }
                        
                        let changed = new_drop_target != drag.drop_target || 
                                      new_drop_position != drag.drop_position;
                        
                        if changed {
                            drag.drop_target = new_drop_target;
                            drag.drop_position = new_drop_position;
                            shell.invalidate_layout();
                        }
                        
                        shell.request_redraw();
                    }
                }
            }
            
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let (drop_target, drop_position, dragged_nodes, dragged_external, target_external) = {
                    let state = self.state.state.downcast_ref::<TreeState>();
                    if let Some(ref drag) = state.drag_active {
                        // Convert internal IDs to external IDs while we have access to everything
                        let dragged_ext: Vec<usize> = drag
                            .dragged_nodes
                            .iter()
                            .map(|&internal| self.tree_handle.preferred_id(internal))
                            .collect();
                        
                        let target_ext = drag.drop_target.map(|internal| self.tree_handle.preferred_id(internal));
                        
                        (
                            drag.drop_target, 
                            drag.drop_position.clone(), 
                            drag.dragged_nodes.clone(),
                            dragged_ext,
                            target_ext
                        )
                    } else {
                        (None, DropPosition::Before, vec![], vec![], None)
                    }
                };
                
                if let Some(target_id) = drop_target {
                    // Use internal IDs for reordering
                    self.reorder_branches(&dragged_nodes, target_id, &drop_position);
                    
                    // Use external IDs for the callback
                    if let Some(ref on_drop) = self.tree_handle.on_drop {
                        if let Some(target_ext) = target_external {
                            let drop_info = DropInfo {
                                dragged_ids: dragged_external,
                                target_id: Some(target_ext),
                                position: drop_position,
                            };
                            shell.publish(on_drop(drop_info));
                        }
                    }
                }
                
                let state = self.state.state.downcast_mut::<TreeState>();
                state.drag_active = None;
                shell.invalidate_layout();
                shell.request_redraw();
            }
            
            Event::Mouse(mouse::Event::CursorLeft) => {
                let state = self.state.state.downcast_mut::<TreeState>();
                state.drag_active = None;
                shell.invalidate_layout();
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let state = self.state.state.downcast_ref::<TreeState>();
        let drag_bounds = layout.bounds();
        let tree_style = theme.style(&self.tree_handle.class);
        
        renderer.with_layer(self.viewport, |renderer| {
            // We need to draw each dragged item properly
            let mut y_offset = 0.0;

            let primary_index = self.tree_handle.branches
                .iter()
                .position(|b| {
                    if let Some(ref drag) = state.drag_active {
                        b.id == drag.primary_node
                    } else {
                        false
                    }
                })
                .unwrap_or_else(|| self.dragged_indices.first().copied().unwrap_or(0));
            
            if primary_index >= self.tree_handle.branch_content.len() {
                return;
            }
            
            let branch_height = if primary_index < state.branch_heights.len() {
                state.branch_heights[primary_index].max(LINE_HEIGHT)
            } else {
                LINE_HEIGHT
            };

            let effective_depth = if let Some(ref branch_order) = state.branch_order {
                branch_order.iter()
                    .find(|bs| self.tree_handle.branches.get(primary_index).map(|b| b.id == bs.id).unwrap_or(false))
                    .map(|bs| bs.depth)
                    .unwrap_or_else(|| self.tree_handle.branches.get(primary_index).map(|b| b.depth).unwrap_or(0))
            } else {
                self.tree_handle.branches.get(primary_index).map(|b| b.depth).unwrap_or(0)
            };

            let branch_bounds = Rectangle {
                x: drag_bounds.x,
                y: drag_bounds.y,
                width: state.drag_active.as_ref().unwrap().drag_start_bounds.width,
                height: branch_height,
            };

            // Draw the branch background with decorations
            renderer.fill_quad(
                renderer::Quad {
                    bounds: branch_bounds,
                    border: Border {
                        color: Color::from_rgba(
                            tree_style.selection_border.r,
                            tree_style.selection_border.g,
                            tree_style.selection_border.b,
                            0.9
                        ),
                        width: 2.0,
                        radius: Radius::from(2.0),
                    },
                    ..Default::default()
                },
                Color::from_rgba(
                    tree_style.selection_background.r,
                    tree_style.selection_background.g,
                    tree_style.selection_background.b,
                    0.9
                ),
            );

            // Draw the handle stripe
            let indent_x = self.tree_handle.padding_x + (effective_depth as f32 * self.tree_handle.indent);
            let handle_x = drag_bounds.x + indent_x + ARROW_W;
            
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        x: handle_x,
                        y: drag_bounds.y + 2.0,
                        width: HANDLE_STRIPE_W,
                        height: branch_height - 4.0,
                    },
                    border: Border::default(),
                    ..Default::default()
                },
                Color::from_rgba(
                    tree_style.line_color.r,
                    tree_style.line_color.g,
                    tree_style.line_color.b,
                    0.7
                ),
            );
            
            // Draw the content
            let content_x = indent_x + ARROW_W + CONTENT_GAP;
            let translation = Vector::new(
                (drag_bounds.x + content_x) - self.layout.bounds().x,
                drag_bounds.y - self.layout.bounds().y,
            );
            
            let transparent_style = renderer::Style {
                text_color: Color::from_rgba(
                    style.text_color.r,
                    style.text_color.g,
                    style.text_color.b,
                    0.9
                ),
            };

            renderer.with_translation(translation, |renderer| {
                if let Some(branch_content) = self.tree_handle.branch_content.get(primary_index) {
                    let branch_tree = &self.state.children[primary_index];
                    
                    // Get the correct layout for the primary branch
                    let primary_layout = if self.dragged_indices.contains(&primary_index) {
                        // Find the position of primary_index in dragged_indices
                        if let Some(pos) = self.dragged_indices.iter().position(|&i| i == primary_index) {
                            if pos == 0 {
                                self.layout
                            } else {
                                self.tree_layout.children().nth(primary_index).unwrap_or(self.layout)
                            }
                        } else {
                            self.layout
                        }
                    } else {
                        self.tree_layout.children().nth(primary_index).unwrap_or(self.layout)
                    };
                    
                    branch_content.as_widget().draw(
                        branch_tree,
                        renderer,
                        theme,
                        &transparent_style,
                        primary_layout,
                        cursor,
                        &primary_layout.bounds()
                    );
                }
            });
        });
    }

    fn mouse_interaction(
        &self,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::Grabbing
    }

    fn index(&self) -> f32 {
        f32::MAX
    }
}

impl<'a, 'b, Message, Theme, Renderer> DragOverlay<'a, 'b, Message, Theme, Renderer>
where 
    Message: Clone,
    Theme: Catalog,
    Renderer: iced::advanced::text::Renderer<Font = iced::Font>,
{
    fn reorder_branches(
        &mut self,
        dragged_ids: &[usize],
        target_id: usize,
        drop_position: &DropPosition,
    ) {
        // Get the current order before we borrow state mutably
        let (current_order, branches_copy) = {
            let state = self.state.state.downcast_ref::<TreeState>();
            let order = if let Some(ref branch_order) = state.branch_order {
                branch_order.clone()
            } else {
                self.tree_handle.branches.iter().map(|b| BranchState {
                    id: b.id,
                    parent_id: b.parent_id,
                    depth: b.depth,
                }).collect()
            };
            (order, self.tree_handle.branches.clone())
        };
        
        let state_map: HashMap<usize, BranchState> = current_order.iter()
            .map(|bs| (bs.id, bs.clone()))
            .collect();
        
        // Use standalone function to avoid borrow issues
        let mut items_to_move = HashSet::new();
        for &id in dragged_ids {
            collect_branch_and_descendants(id, &mut items_to_move, &current_order);
        }
        
        let target_state = state_map.get(&target_id)
            .cloned()
            .unwrap_or_else(|| BranchState {
                id: target_id,
                parent_id: None,
                depth: 0,
            });
        
        let mut new_order: Vec<BranchState> = Vec::new();
        let mut removed_items: Vec<BranchState> = Vec::new();
        
        for bs in current_order {
            if items_to_move.contains(&bs.id) {
                removed_items.push(bs);
            } else {
                new_order.push(bs);
            }
        }
        
        let (new_parent_id, new_base_depth) = match drop_position {
            DropPosition::Before => (target_state.parent_id, target_state.depth),
            DropPosition::After => {
                let is_last_item = new_order.iter()
                    .rposition(|bs| bs.id == target_id)
                    .map(|idx| idx == new_order.len() - 1 || 
                        !new_order[idx + 1..].iter().any(|bs| bs.parent_id == target_state.parent_id))
                    .unwrap_or(false);
                
                if is_last_item && target_state.parent_id.is_some() {
                    let has_root_siblings_after = new_order.iter()
                        .skip_while(|bs| bs.id != target_id)
                        .skip(1)
                        .any(|bs| bs.parent_id.is_none());
                    
                    if !has_root_siblings_after {
                        (None, 0)
                    } else {
                        (target_state.parent_id, target_state.depth)
                    }
                } else {
                    (target_state.parent_id, target_state.depth)
                }
            }
            DropPosition::Into => (Some(target_id), target_state.depth + 1),
        };
        
        let insertion_index = match drop_position {
            DropPosition::Before => {
                new_order.iter().position(|bs| bs.id == target_id)
                    .unwrap_or(new_order.len())
            }
            DropPosition::Into => {
                let parent_pos = new_order.iter().position(|bs| bs.id == target_id)
                    .unwrap_or(new_order.len());
                parent_pos + 1
            }
            DropPosition::After => {
                let mut idx = new_order.iter().position(|bs| bs.id == target_id)
                    .map(|i| i + 1)
                    .unwrap_or(new_order.len());
                
                while idx < new_order.len() {
                    let current = &new_order[idx];
                    if is_descendant_of(current.id, target_id, &new_order) {
                        idx += 1;
                    } else {
                        break;
                    }
                }
                idx
            }
        };
        
        let old_depth = removed_items.iter()
            .find(|bs| dragged_ids.contains(&bs.id))
            .map(|bs| bs.depth)
            .unwrap_or(0);
        let depth_change = new_base_depth as i32 - old_depth as i32;
        
        let mut insert_offset = 0;
        for mut bs in removed_items {
            if dragged_ids.contains(&bs.id) {
                bs.parent_id = new_parent_id;
                bs.depth = new_base_depth;
            } else {
                bs.depth = (bs.depth as i32 + depth_change).max(0) as u16;
            }
            new_order.insert(insertion_index + insert_offset, bs);
            insert_offset += 1;
        }
        
        // Now update the state
        let state = self.state.state.downcast_mut::<TreeState>();
        state.branch_order = Some(new_order);
        
        self.tree_handle.update_has_children(state);
    }
}

// Standalone helper functions to avoid borrow issues
fn is_descendant_of(potential_child: usize, potential_ancestor: usize, states: &[BranchState]) -> bool {
    let mut current_id = Some(potential_child);
    
    while let Some(id) = current_id {
        if let Some(bs) = states.iter().find(|s| s.id == id) {
            if bs.parent_id == Some(potential_ancestor) {
                return true;
            }
            current_id = bs.parent_id;
        } else {
            break;
        }
    }
    false
}

fn collect_branch_and_descendants(branch_id: usize, result: &mut HashSet<usize>, current_order: &[BranchState]) {
    result.insert(branch_id);
    
    let children: Vec<usize> = current_order.iter()
        .filter(|bs| bs.parent_id == Some(branch_id))
        .map(|bs| bs.id)
        .collect();
    
    for child_id in children {
        collect_branch_and_descendants(child_id, result, current_order);
    }
}

fn filter_redundant_selections(selected_ids: &[usize], branches: &[Branch_], branch_order: &Option<Vec<BranchState>>) -> Vec<usize> {
    let mut filtered = Vec::new();
    
    for &id in selected_ids {
        let mut should_include = true;
        
        // Check if any ancestor is also selected
        let mut current_id = id;
        while let Some(branch) = branches.iter().find(|b| b.id == current_id) {
            let parent_id = if let Some(order) = branch_order {
                order.iter()
                    .find(|bs| bs.id == current_id)
                    .and_then(|bs| bs.parent_id)
            } else {
                branch.parent_id
            };
            
            if let Some(parent) = parent_id {
                if selected_ids.contains(&parent) {
                    should_include = false;
                    break;
                }
                current_id = parent;
            } else {
                break;
            }
        }
        
        if should_include {
            filtered.push(id);
        }
    }
    
    filtered
}

impl<'a, Message, Theme, Renderer> From<TreeHandle<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: iced::advanced::Renderer + iced::advanced::text::Renderer<Font = iced::Font> + 'a,
{
    fn from(tree: TreeHandle<'a, Message, Theme, Renderer>) -> Self {
        Element::new(tree)
    }
}

/// A branch in a tree that contains content and can have children.
#[allow(missing_debug_implementations)]
pub struct Branch<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub content: Element<'a, Message, Theme, Renderer>,
    pub children: Vec<Branch<'a, Message, Theme, Renderer>>,
    pub external_id: usize, 
    pub align_x: iced::Alignment,
    pub align_y: iced::Alignment,
    pub accepts_drops: bool,
    pub draggable: bool, 
}

impl<'a, Message, Theme, Renderer> 
    Branch<'a, Message, Theme, Renderer> {

    /// Adds children to this branch
    pub fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    pub fn accepts_drops(mut self) -> Self {
        self.accepts_drops = true;
        self
    }

    pub fn align_x(mut self, alignment: impl Into<iced::Alignment>) -> Self {
        self.align_x = alignment.into();
        self
    }

    pub fn align_y(mut self, alignment: impl Into<iced::Alignment>) -> Self {
        self.align_y = alignment.into();
        self
    }

    pub fn block_dragging(mut self) -> Self {
        self.draggable = false;
        self
    }

    pub fn with_id(mut self, id: usize) -> Self {
        self.external_id = id;
        self
    }
}

/// The theme catalog for the tree widget
pub trait Catalog {
    /// The style class
    type Class<'a>;
    
    /// Default style
    fn default<'a>() -> Self::Class<'a>;
    
    /// Get the style for a class
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// Style for the tree widget
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Text color
    pub text: Color,
    /// Selection background color
    pub selection_background: Color,
    /// Selection text color
    pub selection_text: Color,
    /// Selection border color
    pub selection_border: Color,
    /// Focus border color
    pub focus_border: Color,
    /// Arrow color
    pub arrow_color: Color,
    /// Line color for connecting lines
    pub line_color: Color,
    /// Drop indicator color - Accept
    pub accept_drop_indicator_color: Color,
    /// Drop indicator color - Deny
    pub deny_drop_indicator_color: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            text: Color::BLACK,
            selection_background: Color::from_rgba(0.0, 0.0, 0.0, 0.05),
            selection_text: Color::BLACK,
            selection_border: Color::from_rgb(0.0, 0.5, 1.0),
            focus_border: Color::from_rgba(0.0, 0.5, 1.0, 0.5),
            arrow_color: Color::from_rgb(0.3, 0.3, 0.3),
            line_color: Color::from_rgb(0.3, 0.3, 0.3),
            accept_drop_indicator_color: Color::from_rgb(0.0, 0.8, 0.0),
            deny_drop_indicator_color: Color::from_rgb(1.0, 0.0, 0.0),
        }
    }
}

/// Styling function
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;
    
    fn default<'a>() -> Self::Class<'a> {
        Box::new(|theme| {
            let palette = theme.extended_palette();
            
            Style {
                text: palette.background.base.text,
                selection_background: palette.background.weakest.color,
                selection_text: palette.background.base.text,
                selection_border: palette.secondary.base.color,
                focus_border: Color::from_rgba(
                    palette.secondary.base.color.r,
                    palette.secondary.base.color.g,
                    palette.secondary.base.color.b,
                    0.5
                ),
                arrow_color: palette.background.strong.color,
                line_color: palette.primary.weak.color,
                accept_drop_indicator_color: palette.primary.strong.color,
                deny_drop_indicator_color: palette.danger.strong.color,
            }
        })
    }
    
    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}