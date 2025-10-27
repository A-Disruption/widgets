use iced::{
    widget::{button, column, container, space::horizontal as horizontal_space, row, scrollable, text, text_input},
    Element, Task, Theme,
};
use std::collections::{HashSet, HashMap};
use widgets::tree::{branch, tree_handle, Branch, DropInfo, DropPosition, TreeId};
use widgets::tree::tree_node::*;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum Message {
    TreeToggle(Uuid, bool),
    TreeSelect(HashSet<Uuid>),
    InputChanged(String),
    AddChild,
    ButtonPressed,
    HandleBranchDropped(DropInfo<Uuid>),
}

pub struct App {
    tree_roots: Vec<TreeNode<Uuid>>,
    node_labels: HashMap<Uuid, String>,
    expanded_branches: HashSet<Uuid>,
    selected_items: HashSet<Uuid>,
    input_text: String,
}

impl App {
    pub fn new() -> Self {
        let root_id = Uuid::new_v4();
        let child1_id = Uuid::new_v4();
        let child2_id = Uuid::new_v4();
        
        let tree_roots = vec![
            treenode(root_id)
                .expanded(true)
                .accepts_drops()
                .block_dragging()
                .with_children(vec![
                    treenode(child1_id),
                    treenode(child2_id),
                ])
        ];
        
        let mut node_labels = HashMap::new();
        node_labels.insert(root_id, "Root Branch".to_string());
        node_labels.insert(child1_id, "Child 1".to_string());
        node_labels.insert(child2_id, "Child 2".to_string());

        Self {
            tree_roots,
            node_labels,
            expanded_branches: HashSet::from([root_id]),  // Start expanded
            selected_items: HashSet::new(),
            input_text: String::new(),
        }
    }
    
    fn theme(&self) -> Theme {
        iced::Theme::Dark
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        println!("🚀 APP.update called with message: {:?}", message);
        match message {
            Message::TreeToggle(id, is_expanded) => {
                for root in &mut self.tree_roots {
                    if let Some(parent) = root.find_mut(id) {
                        parent.expanded = is_expanded;
                    }
                    break;
                }

                println!("Toggled ID {} to expanded: {}", id, is_expanded);
            }
            Message::TreeSelect(selected_ids) => {
                self.selected_items = selected_ids.clone();
                println!("Selected: {:?}", selected_ids);
            }
            Message::InputChanged(text) => {
                self.input_text = text;
            }
            Message::AddChild => {
                if let Some(&parent_id) = self.selected_items.iter().next() {
                    if !self.input_text.trim().is_empty() {  // Add this check!
                        let new_id = Uuid::new_v4();
                        let new_node = TreeNode::new(new_id);
                        
                        for root in &mut self.tree_roots {
                            if root.add_child_to(parent_id, new_node.clone()) {
                                self.node_labels.insert(new_id, self.input_text.clone());

                                if let Some(parent) = root.find_mut(parent_id) {
                                    parent.expanded = true;
                                }
                                
                                self.input_text.clear();  // Clear the input!
                                println!("✅ Added child {} to parent {}", new_id, parent_id);
                                println!("parent {} added to expanded group", parent_id);
                                println!("Expanded: {:?}", self.expanded_branches);
                                break;
                            }
                        }
                    } else {
                        println!("⚠️ Input text is empty");
                    }
                } else {
                    println!("⚠️ No parent selected");
                }
            }
            Message::ButtonPressed => {
                println!("🎉 BUTTON WAS PRESSED! 🎉");
            }
            Message::HandleBranchDropped(drop_info) => {
                // Move the node in your data structure
                for root in &mut self.tree_roots {
                    if let Some(target_id) = drop_info.target_id {
                        for &dragged_id in &drop_info.dragged_ids {
                            root.move_node(dragged_id, target_id);
                        }
                    }
                }
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {

        // Convert TreeNodes to Branches in view
        let branches: Vec<Branch<Message, Theme, iced::Renderer, Uuid>> = self.tree_roots.iter()
            .map(|node| tree_node_to_branch(node, &|id| {
                let label = self.node_labels.get(&id).expect("Error, sry");
                text(label).into()
            }))
            .collect();
        
        let tree_widget = tree_handle(branches)
            .on_expand(Message::TreeToggle)
            .on_drop(Message::HandleBranchDropped)
            .on_select(Message::TreeSelect)
            .reset_order_state();

        let input_section = row![
            text_input("Enter branch text", &self.input_text)
                .on_input(Message::InputChanged)
                .on_submit(Message::AddChild),
            button("Add").on_press(Message::AddChild),
        ].spacing(10).padding(5);

        scrollable(
            column![
                iced::widget::text("Tree Widget Example - Add Children").size(24),
                input_section,
                tree_widget,
            ]
            .width(400)
            .spacing(20)
            .padding(20)
        )
        .into()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .theme(App::theme)
        .run()
}