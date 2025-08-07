use bevy::prelude::*;
use egui::{Color32, Pos2, Rect, Vec2};
use super::{EntityNode, NodeResponse};

/// Component for nodes that contain children (have InitialState or Children components)
#[derive(Debug, Clone)]
pub struct ParentNode {
    /// Shared node properties
    pub entity_node: EntityNode,
    /// Title bar height
    pub title_bar_height: f32,
    /// Minimum container size to accommodate children
    pub min_content_size: Vec2,
    /// Margin around children
    pub child_margin: Vec2,
}

impl ParentNode {
    /// Create a new ParentNode with default styling
    pub fn new(position: Pos2) -> Self {
        let mut parent = Self {
            entity_node: EntityNode::new(position),
            title_bar_height: 30.0,
            min_content_size: Vec2::new(150.0, 80.0),
            child_margin: Vec2::new(10.0, 10.0),
        };
        // Set initial size
        parent.entity_node.current_size = Vec2::new(200.0, 120.0);
        parent
    }
    
    /// Get the rectangle for the entire parent node
    pub fn rect(&self) -> Rect {
        Rect::from_min_size(self.entity_node.position, self.entity_node.current_size)
    }
    
    /// Get the rectangle for the title bar
    pub fn title_bar_rect(&self) -> Rect {
        Rect::from_min_size(
            self.entity_node.position,
            Vec2::new(self.entity_node.current_size.x, self.title_bar_height),
        )
    }
    
    /// Get the rectangle for the content area (below title bar)
    pub fn content_rect(&self) -> Rect {
        let content_start = self.entity_node.position + Vec2::new(0.0, self.title_bar_height);
        let content_size = Vec2::new(self.entity_node.current_size.x, self.entity_node.current_size.y - self.title_bar_height);
        Rect::from_min_size(content_start, content_size)
    }
    
    /// Calculate the bounding box that should contain all child rectangles
    /// Parents only expand right and down, never left or up
    pub fn calculate_size_for_children(&mut self, child_rects: &[Rect]) {
        if child_rects.is_empty() {
            // If no children, use minimum size
            self.entity_node.current_size = Vec2::new(
                self.min_content_size.x,
                self.min_content_size.y + self.title_bar_height,
            );
            return;
        }
        
        // Get current content area bounds
        let content_rect = self.content_rect();
        let content_start = content_rect.min + self.child_margin;
        
        // Find the maximum extent of children relative to content start
        let mut max_x = content_start.x + self.min_content_size.x - self.child_margin.x * 2.0;
        let mut max_y = content_start.y + self.min_content_size.y - self.child_margin.y * 2.0;
        
        for rect in child_rects {
            // Only consider expansion to the right and down
            max_x = max_x.max(rect.max.x);
            max_y = max_y.max(rect.max.y);
        }
        
        // Calculate required content size based on maximum extents
        // Add extra margin to bottom and right edges so children aren't right against the border
        let bottom_right_margin = 30.0;
        let required_content_width = (max_x - content_start.x) + self.child_margin.x + bottom_right_margin;
        let required_content_height = (max_y - content_start.y) + self.child_margin.y + bottom_right_margin;
        
        // Apply minimum size constraints
        let final_content_width = required_content_width.max(self.min_content_size.x);
        let final_content_height = required_content_height.max(self.min_content_size.y);
        
        // Set the new size (content + title bar)
        self.entity_node.current_size = Vec2::new(
            final_content_width,
            final_content_height + self.title_bar_height,
        );
    }
    
    /// Show the parent node UI and handle interactions
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        entity_id: Option<&str>,
    ) -> NodeResponse {
        let rect = self.rect();
        let title_rect = self.title_bar_rect();
        
        // Allocate the entire rectangle for interaction
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
        
        // Draw the parent node
        self.draw_parent_node(ui, rect, title_rect, name, entity_id);
        
        node_response
    }
    
    /// Draw the parent node with title bar and content area
    fn draw_parent_node(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        title_rect: Rect,
        name: &str,
        entity_id: Option<&str>,
    ) {
        let painter = ui.painter();
        let bg_color = self.entity_node.current_bg_color();
        
        // Draw main container background
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(8),
            bg_color,
        );
        
        // Draw container border
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(1.5, self.entity_node.border_color),
            egui::StrokeKind::Outside,
        );
        
        // Draw title bar background (slightly darker)
        let title_bg_color = Color32::from_rgba_unmultiplied(
            bg_color.r().saturating_sub(10),
            bg_color.g().saturating_sub(10),
            bg_color.b().saturating_sub(10),
            bg_color.a(),
        );
        
        painter.rect_filled(
            title_rect,
            egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            },
            title_bg_color,
        );
        
        // Draw title bar separator line
        let separator_y = title_rect.max.y;
        painter.line_segment(
            [
                egui::Pos2::new(rect.min.x + 5.0, separator_y),
                egui::Pos2::new(rect.max.x - 5.0, separator_y),
            ],
            egui::Stroke::new(1.0, self.entity_node.border_color),
        );
        
        // Draw title text (name and entity ID side by side)
        let font_id = self.entity_node.main_font_id();
        let name_galley = ui.fonts(|f| f.layout_no_wrap(name.to_string(), font_id.clone(), self.entity_node.text_color));
        
        // Position name text in title bar
        let text_start_x = title_rect.min.x + self.entity_node.padding.x;
        let text_y = title_rect.center().y - name_galley.size().y * 0.5;
        let name_pos = egui::Pos2::new(text_start_x, text_y);
        
        painter.galley(name_pos, name_galley.clone(), self.entity_node.text_color);
        
        // Draw entity ID if provided (to the right of the name)
        if let Some(entity_id) = entity_id {
            let entity_font_id = self.entity_node.subscript_font_id();
            let entity_galley = ui.fonts(|f| f.layout_no_wrap(
                format!(" ({})", entity_id),
                entity_font_id,
                Color32::from_rgba_unmultiplied(255, 255, 255, 180)
            ));
            
            let entity_pos = egui::Pos2::new(
                name_pos.x + name_galley.size().x,
                text_y + (name_galley.size().y - entity_galley.size().y) * 0.5,
            );
            
            painter.galley(entity_pos, entity_galley, Color32::from_rgba_unmultiplied(255, 255, 255, 180));
        }
        
        // Draw content area outline (for debugging/visualization)
        let content_rect = self.content_rect();
        painter.rect_stroke(
            content_rect.shrink(2.0),
            egui::CornerRadius::same(4),
            egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)),
            egui::StrokeKind::Outside,
        );
    }
    
    /// Update the container size to accommodate children
    pub fn update_size_for_children(&mut self, child_bounds: Option<Rect>) {
        if let Some(bounds) = child_bounds {
            // Calculate required size based on child bounds
            let required_width = bounds.width() + self.child_margin.x * 2.0;
            let required_height = bounds.height() + self.child_margin.y * 2.0 + self.title_bar_height;
            
            // Apply minimum size constraints
            let new_width = required_width.max(self.min_content_size.x);
            let new_height = required_height.max(self.min_content_size.y + self.title_bar_height);
            
            self.entity_node.current_size = Vec2::new(new_width, new_height);
        } else {
            // No children, use minimum size
            self.entity_node.current_size = Vec2::new(
                self.min_content_size.x,
                self.min_content_size.y + self.title_bar_height,
            );
        }
    }
}
