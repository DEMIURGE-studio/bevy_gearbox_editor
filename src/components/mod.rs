pub mod entity_node;
pub mod leaf_node;
pub mod parent_node;

use egui::Pos2;
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

    pub fn position(&self) -> Pos2 {
        match self {
            NodeType::Leaf(leaf_node) => leaf_node.entity_node.position,
            NodeType::Parent(parent_node) => parent_node.entity_node.position,
        }
    }
}

pub fn draw_dotted_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    _radius: egui::CornerRadius,
    stroke: egui::Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    // Approximate rounded rect with 4 sides (ignore arcs for simplicity)
    let draw_segmented = |a: egui::Pos2, b: egui::Pos2| {
        let dir = b - a;
        let len = dir.length();
        if len <= 0.0 { return; }
        let step = dash_len + gap_len;
        let n = (len / step).ceil() as i32;
        let dirn = dir / len;
        for i in 0..n {
            let start = a + dirn * (i as f32 * step);
            let end = (start + dirn * dash_len).min(b);
            painter.line_segment([start, end], stroke);
        }
    };

    let r = rect;
    draw_segmented(r.min, egui::pos2(r.max.x, r.min.y)); // top
    draw_segmented(egui::pos2(r.max.x, r.min.y), r.max); // right
    draw_segmented(egui::pos2(r.min.x, r.max.y), r.max); // bottom (right to left will be drawn via min..max; acceptable)
    draw_segmented(r.min, egui::pos2(r.min.x, r.max.y)); // left
}
