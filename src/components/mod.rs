pub mod entity_node;
pub mod leaf_node;
pub mod parent_node;

pub use entity_node::*;
pub use leaf_node::*;
pub use parent_node::*;

/// Enum representing the different types of nodes in the editor
#[derive(Debug)]
pub enum NodeType {
    Leaf(LeafNode),
    Parent(ParentNode),
}

impl NodeType {
    /// Get the current rectangle of the node
    pub fn current_rect(&self) -> egui::Rect {
        match self {
            NodeType::Leaf(leaf_node) => leaf_node.entity_node.current_rect(),
            NodeType::Parent(parent_node) => parent_node.entity_node.current_rect(),
        }
    }
}
