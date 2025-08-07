use egui::{Pos2, Rect, Vec2};
use super::{EntityNode, NodeResponse};

/// A draggable, selectable leaf node component for terminal state machine nodes
#[derive(Debug, Clone)]
pub struct LeafNode {
    /// Shared node properties
    pub entity_node: EntityNode,
}

impl LeafNode {
    /// Create a new leaf node at the specified position
    pub fn new(position: Pos2) -> Self {
        Self {
            entity_node: EntityNode::new(position),
        }
    }
    
    /// Show the leaf node UI and handle interactions
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        text: &str,
        entity_id: Option<&str>,
    ) -> NodeResponse {
        // Calculate text dimensions
        let main_font_id = self.entity_node.main_font_id();
        let main_text_galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), main_font_id, self.entity_node.text_color));
        
        let subscript_galley = entity_id.map(|id| {
            let subscript_font_id = self.entity_node.subscript_font_id();
            ui.fonts(|f| f.layout_no_wrap(id.to_string(), subscript_font_id, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180)))
        });
        
        // Calculate total text dimensions
        let main_text_size = main_text_galley.size();
        let subscript_size = subscript_galley.as_ref().map(|g| g.size()).unwrap_or(Vec2::ZERO);
        let text_gap = 2.0;
        
        let total_text_width = main_text_size.x.max(subscript_size.x);
        let total_text_height = main_text_size.y + subscript_size.y + text_gap;
        
        // Calculate node size with padding
        let content_size = Vec2::new(total_text_width, total_text_height);
        let node_size = content_size + self.entity_node.padding * 2.0;
        
        // Apply size constraints
        let constrained_size = Vec2::new(
            node_size.x.clamp(self.entity_node.min_size.x, self.entity_node.max_size.x),
            node_size.y.clamp(self.entity_node.min_size.y, self.entity_node.max_size.y),
        );
        
        // Update the current size
        self.entity_node.current_size = constrained_size;
        
        // Create the node rectangle
        let rect = Rect::from_min_size(self.entity_node.position, constrained_size);
        
        // Handle UI interaction
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        
        let mut node_response = NodeResponse::default();
        
        // Handle drag state tracking
        if response.drag_started_by(egui::PointerButton::Primary) {
            self.entity_node.is_being_dragged_by_primary = true;
        } else if response.drag_stopped() {
            self.entity_node.is_being_dragged_by_primary = false;
        }
        
        // Check for dragging - only if started by primary button
        if response.dragged() && self.entity_node.is_being_dragged_by_primary {
            self.entity_node.position += response.drag_delta();
            node_response.dragged = true;
            node_response.drag_delta = response.drag_delta();
        }
        
        // Handle clicking (for selection)
        if response.clicked_by(egui::PointerButton::Primary) {
            node_response.clicked = true;
        }
        
        // Handle right-clicking (for context menu)
        if response.clicked_by(egui::PointerButton::Secondary) {
            node_response.right_clicked = true;
        }
        
        node_response.hovered = response.hovered();
        
        // Draw the leaf node
        self.draw_node(ui, rect, &main_text_galley, subscript_galley.as_ref().map(|v| &**v), text_gap);
        
        node_response
    }
    
    /// Draw the leaf node with rounded rectangle background and text
    fn draw_node(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        main_text_galley: &egui::Galley,
        subscript_galley: Option<&egui::Galley>,
        text_gap: f32,
    ) {
        let painter = ui.painter();
        
        // Draw background
        let bg_color = self.entity_node.current_bg_color();
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(10),
            bg_color,
        );
        
        // Draw border
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(10),
            egui::Stroke::new(1.5, self.entity_node.border_color),
            egui::StrokeKind::Outside,
        );
        
        // Calculate text positioning
        let main_text_size = main_text_galley.size();
        let subscript_size = subscript_galley.map(|g| g.size()).unwrap_or(Vec2::ZERO);
        let total_text_height = main_text_size.y + subscript_size.y + text_gap;
        
        // Center the text block vertically in the node
        let text_block_start_y = rect.center().y - total_text_height * 0.5;
        
        // Draw main text (centered horizontally)
        let main_text_pos = Pos2::new(
            rect.center().x - main_text_size.x * 0.5,
            text_block_start_y,
        );
        painter.galley(main_text_pos, main_text_galley.clone().into(), self.entity_node.text_color);
        
        // Draw subscript text if present (centered horizontally, below main text)
        if let Some(subscript_galley) = subscript_galley {
            let subscript_pos = Pos2::new(
                rect.center().x - subscript_size.x * 0.5,
                text_block_start_y + main_text_size.y + text_gap,
            );
            painter.galley(subscript_pos, subscript_galley.clone().into(), egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180));
        }
    }
}