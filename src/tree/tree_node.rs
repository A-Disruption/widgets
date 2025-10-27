use iced::Element;
use super::{TreeId, Branch, branch};

/// Creates a new [`TreeHandle`] with the given root branches.
pub fn treenode<Id>(
    id: Id,
) -> TreeNode<Id>
where
    Id: TreeId,
{
    TreeNode::new(id)
}

/// A lifetime-free tree node structure that can be stored in app state.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeNode<Id = usize> 
where
    Id: TreeId,
{
    pub id: Id,
    pub children: Vec<TreeNode<Id>>,
    pub accepts_drops: bool,
    pub draggable: bool,
    pub expanded: bool,
}

impl<Id: TreeId> TreeNode<Id> {
    /// Creates a new tree node with the given ID
    pub fn new(id: Id) -> Self {
        Self {
            id,
            children: Vec::new(),
            accepts_drops: false,
            draggable: false,
            expanded: false,
        }
    }
    
    /// Sets children for this node
    pub fn with_children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }
    
    /// Marks this node as accepting drops
    pub fn accepts_drops(mut self) -> Self {
        self.accepts_drops = true;
        self
    }
    
    /// Marks this node as non-draggable
    pub fn block_dragging(mut self) -> Self {
        self.draggable = false;
        self
    }
    
    /// Adds a child to this node
    pub fn add_child(&mut self, child: TreeNode<Id>) {
        self.children.push(child);
    }
    
    /// Recursively finds a node by ID and adds a child to it
    pub fn add_child_to(&mut self, parent_id: Id, child: TreeNode<Id>) -> bool {
        if self.id == parent_id {
            self.children.push(child);
            return true;
        }
        for child_node in &mut self.children {
            if child_node.add_child_to(parent_id, child.clone()) {
                return true;
            }
        }
        false
    }
    
    /// Recursively finds and removes a node by ID, returns the removed node
    pub fn remove_node(&mut self, id: Id) -> Option<TreeNode<Id>> {
        if let Some(pos) = self.children.iter().position(|n| n.id == id) {
            return Some(self.children.remove(pos));
        }
        
        for child in &mut self.children {
            if let Some(removed) = child.remove_node(id) {
                return Some(removed);
            }
        }
        None
    }
    
    /// Finds a node by ID (immutable)
    pub fn find(&self, id: Id) -> Option<&TreeNode<Id>> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }
    
    /// Finds a node by ID (mutable)
    pub fn find_mut(&mut self, id: Id) -> Option<&mut TreeNode<Id>> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }
    
    /// Moves a node from one parent to another
    pub fn move_node(&mut self, node_id: Id, new_parent_id: Id) -> bool {
        // Remove from current location
        if let Some(removed) = self.remove_node(node_id) {
            // Add to new parent
            return self.add_child_to(new_parent_id, removed);
        }
        false
    }
    
    /// Collects all IDs in the tree (depth-first)
    pub fn collect_ids(&self) -> Vec<Id> {
        let mut ids = vec![self.id];
        for child in &self.children {
            ids.extend(child.collect_ids());
        }
        ids
    }

    /// Sets initial expanded state
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }
    
    /// Toggle expanded state
    pub fn toggle_expanded(&mut self) {
        self.expanded = !self.expanded;
    }
    
    /// Recursively find and toggle a node's expanded state
    pub fn toggle_expanded_at(&mut self, id: Id) -> bool {
        if self.id == id {
            self.expanded = !self.expanded;
            return true;
        }
        for child in &mut self.children {
            if child.toggle_expanded_at(id) {
                return true;
            }
        }
        false
    }

}


/// Converts a TreeNode into a Branch for rendering.
/// 
/// This is a standalone function so TreeNode doesn't need lifetimes.
/// 
/// # Arguments
/// * `node` - The tree node to convert
/// * `content_fn` - Function that creates widget content for each node's ID
/// 
/// # Example
/// ```
/// let branch = tree_node_to_branch(&my_node, &|id| {
///     text(labels.get(&id).unwrap_or(&"Unknown")).into()
/// });
/// ```
pub fn tree_node_to_branch<'a, Message, Theme, Renderer, Id, F>(
    node: &TreeNode<Id>,
    content_fn: &F,
) -> Branch<'a, Message, Theme, Renderer, Id>
where
    Id: TreeId,
    F: Fn(Id) -> Element<'a, Message, Theme, Renderer>,
{
    let mut b = branch(content_fn(node.id))
        .with_id(node.id)
        .expanded(node.expanded);
    
    if !node.draggable {
        b = b.block_dragging();
    }
    
    if node.accepts_drops {
        b = b.accepts_drops();
    }
    
    if !node.children.is_empty() {
        let children = node.children.iter()
            .map(|child| tree_node_to_branch(child, content_fn))
            .collect();
        b = b.with_children(children);
    }
    
    b
}

/// Convenience function to convert multiple tree nodes
pub fn tree_nodes_to_branches<'a, Message, Theme, Renderer, Id, F>(
    nodes: &[TreeNode<Id>],
    content_fn: &F,
) -> Vec<Branch<'a, Message, Theme, Renderer, Id>>
where
    Id: TreeId,
    F: Fn(Id) -> Element<'a, Message, Theme, Renderer>,
{
    nodes.iter()
        .map(|node| tree_node_to_branch(node, content_fn))
        .collect()
}