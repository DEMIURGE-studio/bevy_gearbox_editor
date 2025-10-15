use egui::{text::CCursorRange, Pos2, Rect, Vec2};
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
        is_selected: bool,
        is_editing: bool,
        editing_text: &mut String,
        should_focus: bool,
        first_focus: bool,
        custom_color: Option<egui::Color32>,
    ) -> NodeResponse {
        self.show_with_border_style(
            ui, text, entity_id, is_selected, is_editing, editing_text, should_focus, first_focus, custom_color, false,
        )
    }

    pub fn show_with_border_style(
        &mut self,
        ui: &mut egui::Ui,
        text: &str,
        entity_id: Option<&str>,
        is_selected: bool,
        is_editing: bool,
        editing_text: &mut String,
        should_focus: bool,
        first_focus: bool,
        custom_color: Option<egui::Color32>,
        dotted_border: bool,
    ) -> NodeResponse {
        // Determine text color based on background color using smooth interpolation
        let text_color = if let Some(bg_color) = custom_color {
            crate::editor_state::compute_text_color_for_bg(bg_color)
        } else {
            self.entity_node.text_color
        };
        
        // Calculate text dimensions
        let main_font_id = self.entity_node.main_font_id();
        let main_text_galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), main_font_id, text_color));
        
        let subscript_galley = entity_id.map(|id| {
            let subscript_font_id = self.entity_node.subscript_font_id();
            let subscript_color = if crate::editor_state::prefers_dark_text(text_color) {
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180) // Semi-transparent black
            } else {
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180) // Semi-transparent white
            };
            ui.fonts(|f| f.layout_no_wrap(id.to_string(), subscript_font_id, subscript_color))
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
        
        // Draw the leaf node (with editing support)
        self.draw_node_with_editing(
            ui, 
            rect, 
            &main_text_galley, 
            subscript_galley.as_ref().map(|v| &**v), 
            text_gap,
            is_selected,
            is_editing,
            editing_text,
            should_focus,
            first_focus,
            custom_color,
            dotted_border
        );
        
        // Add the + button for transitions (show for selected nodes, including root for global transitions)
        if is_selected {
            let button_size = 16.0;
            let button_pos = egui::Pos2::new(
                rect.max.x - button_size - 4.0,
                rect.min.y + 4.0,
            );
            let button_rect = egui::Rect::from_min_size(button_pos, egui::Vec2::splat(button_size));
            
            let button_response = ui.allocate_rect(button_rect, egui::Sense::click());
            if button_response.clicked() {
                node_response.add_transition_clicked = true;
            }
            
            // Draw the + button
            let painter = ui.painter();
            let button_color = if button_response.hovered() {
                egui::Color32::from_rgb(100, 150, 255)
            } else {
                egui::Color32::from_rgb(80, 120, 200)
            };
            
            painter.circle_filled(button_rect.center(), button_size / 2.0, button_color);
            
            // Draw the + symbol
            let line_width = 1.5;
            let cross_size = 6.0;
            painter.line_segment(
                [
                    button_rect.center() - egui::Vec2::new(cross_size / 2.0, 0.0),
                    button_rect.center() + egui::Vec2::new(cross_size / 2.0, 0.0),
                ],
                egui::Stroke::new(line_width, egui::Color32::WHITE),
            );
            painter.line_segment(
                [
                    button_rect.center() - egui::Vec2::new(0.0, cross_size / 2.0),
                    button_rect.center() + egui::Vec2::new(0.0, cross_size / 2.0),
                ],
                egui::Stroke::new(line_width, egui::Color32::WHITE),
            );
        }
        
        node_response
    }
    
    /// Draw the leaf node with editing support
    fn draw_node_with_editing(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        main_text_galley: &egui::Galley,
        subscript_galley: Option<&egui::Galley>,
        text_gap: f32,
        is_selected: bool,
        is_editing: bool,
        editing_text: &mut String,
        should_focus: bool,
        first_focus: bool,
        custom_color: Option<egui::Color32>,
        dotted_border: bool,
    ) {
        if is_editing {
            self.draw_node_editing(ui, rect, subscript_galley, text_gap, is_selected, editing_text, should_focus, first_focus, custom_color, dotted_border);
        } else {
            self.draw_node_normal(ui, rect, main_text_galley, subscript_galley, text_gap, is_selected, custom_color, dotted_border);
        }
    }

    /// Draw the leaf node with rounded rectangle background and text
    fn draw_node_normal(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        main_text_galley: &egui::Galley,
        subscript_galley: Option<&egui::Galley>,
        text_gap: f32,
        is_selected: bool,
        custom_color: Option<egui::Color32>,
        dotted_border: bool,
    ) {
        let painter = ui.painter();
        
        // Draw background
        let bg_color = custom_color.unwrap_or_else(|| self.entity_node.current_bg_color());
        painter.rect_filled(
            rect,
            egui::CornerRadius::same(10),
            bg_color,
        );
        
        // Draw border (dotted optional)
        let selected_border = egui::Color32::from_rgb(100, 150, 255);
        let border_color = if is_selected { selected_border } else { self.entity_node.border_color };
        if dotted_border {
            super::draw_dotted_rect(
                painter,
                rect,
                egui::CornerRadius::same(10),
                egui::Stroke::new(1.5, border_color),
                2.0,
                3.0,
            );
        } else {
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(10),
                egui::Stroke::new(1.5, border_color),
                egui::StrokeKind::Outside,
            );
        }
        
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
    
    /// Draw the leaf node in editing mode with text input
    fn draw_node_editing(
        &self,
        ui: &mut egui::Ui,
        rect: Rect,
        subscript_galley: Option<&egui::Galley>,
        text_gap: f32,
        is_selected: bool,
        editing_text: &mut String,
        should_focus: bool,
        first_focus: bool,
        custom_color: Option<egui::Color32>,
        dotted_border: bool,
    ) {
        // Calculate text input area (main text area only)
        let subscript_size = subscript_galley.map(|g| g.size()).unwrap_or(egui::Vec2::ZERO);
        let total_subscript_height = if subscript_galley.is_some() { subscript_size.y + text_gap } else { 0.0 };
        
        let text_input_height = rect.height() - self.entity_node.padding.y * 2.0 - total_subscript_height;
        let text_input_rect = egui::Rect::from_min_size(
            rect.min + self.entity_node.padding,
            egui::Vec2::new(rect.width() - self.entity_node.padding.x * 2.0, text_input_height),
        );
        
        // First scope: Draw background and border using painter (same as normal, no editing-specific outline)
        {
            let painter = ui.painter();
            // Background same as normal
            let bg_color = custom_color.unwrap_or_else(|| self.entity_node.current_bg_color());
            painter.rect_filled(rect, egui::CornerRadius::same(10), bg_color);
            // Border based on selection
            let selected_border = egui::Color32::from_rgb(100, 150, 255);
            let border_color = if is_selected { selected_border } else { self.entity_node.border_color };
            if dotted_border {
                super::draw_dotted_rect(
                    painter,
                    rect,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(1.5, border_color),
                    2.0,
                    3.0,
                );
            } else {
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::same(10),
                    egui::Stroke::new(1.5, border_color),
                    egui::StrokeKind::Outside,
                );
            }
        }
        
        // Second scope: Handle text input (mutable borrow of ui)
        {
            // Create text input with a unique ID
            let text_edit_id = egui::Id::new(format!("text_edit_{:?}", self.entity_node.position));
            let text_edit = egui::TextEdit::singleline(editing_text)
                .id(text_edit_id)
                .font(self.entity_node.main_font_id())
                .text_color(egui::Color32::WHITE)
                .desired_width(text_input_rect.width())
                .margin(egui::Vec2::ZERO);
            
            // Position and show the text input using a child UI
            let mut child_ui = ui.new_child(egui::UiBuilder::new()
                .max_rect(text_input_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)));
            let output = text_edit.show(&mut child_ui);
            
            if should_focus {
                output.response.request_focus();
                
                // Select all text on first focus
                if first_focus {
                    // Use the built-in select_all method
                    let select_all_range = CCursorRange::select_all(&output.galley);
                    
                    // Update the cursor state
                    let mut new_state = output.state.clone();
                    new_state.cursor.set_char_range(Some(select_all_range));
                    
                    // Store the updated state
                    new_state.store(ui.ctx(), output.response.id);
                }
            }
        }
        
        // Third scope: Draw subscript text using painter
        if let Some(subscript_galley) = subscript_galley {
            let painter = ui.painter();
            let subscript_pos = egui::Pos2::new(
                rect.center().x - subscript_size.x * 0.5,
                text_input_rect.max.y + text_gap,
            );
            painter.galley(subscript_pos, subscript_galley.clone().into(), egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180));
        }
    }
}