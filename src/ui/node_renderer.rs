use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::bevy_inspector;
use crate::components::*;
use crate::resources::*;
use super::UiResources;

pub struct NodeRenderer;

impl NodeRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Handle all input interactions for nodes (PASS 1)
    pub fn handle_interactions(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        node_data: &[(Entity, Vec2, bool, Option<String>)],
        ui_resources: &mut UiResources,
    ) -> (Vec<(Entity, Vec2)>, Vec<(Entity, bool)>) {
        let mut drag_changes = Vec::new();
        let mut expansion_changes = Vec::new();
        
        for (entity, position, expanded, _display_name) in node_data {
            if let Some((new_pos, new_expanded)) = self.handle_node_interactions(
                ui, *entity, *position, *expanded, world, ui_resources
            ) {
                if new_pos != *position {
                    drag_changes.push((*entity, new_pos));
                }
                if new_expanded != *expanded {
                    expansion_changes.push((*entity, new_expanded));
                }
            }
        }
        
        (drag_changes, expansion_changes)
    }

    /// Render unselected nodes visually (PASS 2)
    pub fn render_unselected_nodes(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        node_data: &[(Entity, Vec2, bool, Option<String>)],
        ui_resources: &mut UiResources,
    ) -> Vec<(Entity, bool)> {
        let mut expansion_changes = Vec::new();
        
        for (entity, position, expanded, display_name) in node_data {
            if ui_resources.selected_entity.entity != Some(*entity) {
                if let Some(new_expanded) = self.draw_node_visual_only(
                    ui, *entity, *position, *expanded, display_name.clone(), 
                    world, ui_resources
                ) {
                    if new_expanded != *expanded {
                        expansion_changes.push((*entity, new_expanded));
                    }
                }
            }
        }
        
        expansion_changes
    }

    /// Render selected node visually (PASS 4)
    pub fn render_selected_node(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        node_data: &[(Entity, Vec2, bool, Option<String>)],
        ui_resources: &mut UiResources,
    ) -> Vec<(Entity, bool)> {
        let mut expansion_changes = Vec::new();
        
        if let Some(selected_entity_id) = ui_resources.selected_entity.entity {
            if let Some((entity, position, expanded, display_name)) = 
                node_data.iter().find(|(e, _, _, _)| *e == selected_entity_id) {
                if let Some(new_expanded) = self.draw_node_visual_only(
                    ui, *entity, *position, *expanded, display_name.clone(), 
                    world, ui_resources
                ) {
                    if new_expanded != *expanded {
                        expansion_changes.push((*entity, new_expanded));
                    }
                }
            }
        }
        
        expansion_changes
    }

    /// Handle all input interactions for a single node
    fn handle_node_interactions(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        expanded: bool,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) -> Option<(Vec2, bool)> {
        // Use the last measured size for interaction area (from previous frame)
        let measured_size = ui_resources.size_cache.sizes.get(&entity)
            .copied()
            .unwrap_or(egui::Vec2::new(200.0, 80.0)); // Fallback for first frame
        
        // Create invisible interaction area using measured size
        let pos = egui::Pos2::new(position.x, position.y);
        let rect = egui::Rect::from_min_size(pos, measured_size);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        
        // Track changes
        let mut new_position = position;
        let new_expanded = expanded;
        
        // Handle selection
        if response.clicked() || response.drag_started() {
            ui_resources.selected_entity.entity = Some(entity);
        }
        
        // Handle transition target selection
        if ui_resources.transition_state.selecting_target && response.clicked() {
            self.handle_transition_target_selection(entity, &mut ui_resources.transition_state, world);
        }
        
        // Handle dragging
        if !ui_resources.transition_state.selecting_target && response.dragged() {
            let delta = response.drag_delta();
            new_position.x += delta.x;
            new_position.y += delta.y;
        }
        
        // Note: Button interactions now handled directly in render_node_content
        
        // Return changes if any occurred
        if new_position != position || new_expanded != expanded {
            Some((new_position, new_expanded))
        } else {
            None
        }
    }

    // Note: calculate_node_size() removed - egui now handles sizing naturally!

    /// Handle transition target selection logic
    fn handle_transition_target_selection(&self, entity: Entity, transition_state: &mut TransitionCreationState, world: &mut World) {
        println!("🎯 Node clicked while selecting target: {:?}", entity);
        
        if let (Some(source_entity), Some(event_type)) = (transition_state.source_entity, &transition_state.selected_event_type) {
            if source_entity != entity {
                println!("🔗 Completing transition: {:?} --{}-> {:?}", source_entity, event_type, entity);
                crate::utils::create_transition_listener(world, source_entity, entity, event_type);
                transition_state.source_entity = None;
                transition_state.selected_event_type = None;
                transition_state.selecting_target = false;
            } else {
                println!("❌ Cannot connect node to itself");
            }
        }
    }

    // Note: handle_button_interactions() removed - buttons now handle clicks directly in render_node_content

    /// Draw the visual representation of a node using natural egui sizing
    fn draw_node_visual_only(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        expanded: bool,
        display_name: Option<String>,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) -> Option<bool> {
        let display_name = display_name
            .as_deref()
            .unwrap_or("Unnamed Entity");
        
        // Position for the node
        let pos = egui::Pos2::new(position.x, position.y);
        let mut expansion_changed = None;
        
        // Create a custom frame with the node's background color
        let fill_color = self.get_node_fill_color(entity, &ui_resources.transition_state);
        let frame = egui::Frame::default()
            .fill(fill_color)
            .corner_radius(5.0)
            .inner_margin(8.0);
        
        // Use allocate_new_ui with a reasonable starting size, allowing natural growth
        let max_rect = egui::Rect::from_min_size(pos, egui::Vec2::new(200.0, 400.0)); // Generous space
        let ui_response = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(max_rect), |ui| {
            // Use the frame to provide background and let it size automatically to content
            let frame_response = frame.show(ui, |ui| {
                // NO width constraints - let content determine natural size!
                self.render_node_content(
                    ui, entity, expanded, display_name, world, 
                    &mut ui_resources.pin_cache, &mut expansion_changed
                );
                
                expansion_changed
            });
            
            // Store the actual measured size for interactions
            let measured_size = frame_response.response.rect.size();
            ui_resources.size_cache.sizes.insert(entity, measured_size);
            
            frame_response.inner
        });
        
        ui_response.inner
    }

    /// Get the appropriate fill color for a node based on state
    fn get_node_fill_color(&self, entity: Entity, transition_state: &TransitionCreationState) -> egui::Color32 {
        if transition_state.selecting_target {
            if transition_state.source_entity == Some(entity) {
                egui::Color32::from_rgb(80, 60, 60) // Source node in red tint
            } else {
                egui::Color32::from_rgb(50, 70, 50) // Other targets in dim green
            }
        } else {
            egui::Color32::from_rgb(60, 60, 80) // Normal color
        }
    }

    /// Render the actual content of a node
    fn render_node_content(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        expanded: bool,
        display_name: &str,
        world: &mut World,
        pin_cache: &mut PinPositionCache,
        expansion_changed: &mut Option<bool>,
    ) {
        // NO width constraints at all - let content naturally size itself!
        
        // === HEADER (always visible) ===
        self.render_header(ui, entity, display_name, expanded, pin_cache, expansion_changed);
        
        ui.separator();
        
        // === BODY ===
        self.render_output_pins(ui, entity, world, pin_cache);
        
        if expanded {
            ui.separator();
            self.render_inspector(ui, world, entity);
            ui.separator();
            self.render_action_buttons(ui, entity, expansion_changed);
        }
    }

    /// Render the node header with input pin and expand button
    fn render_header(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        display_name: &str,
        expanded: bool,
        pin_cache: &mut PinPositionCache,
        expansion_changed: &mut Option<bool>,
    ) {
        // Make header fill the full available width
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), 0.0), 
            egui::Layout::left_to_right(egui::Align::Center), 
            |ui| {
                // Draw input pin in header
                let pin_radius = 6.0;
                let pin_pos = ui.cursor().min + egui::Vec2::new(6.0, 12.0);
                ui.painter().circle_filled(
                    pin_pos,
                    pin_radius,
                    egui::Color32::from_rgb(100, 150, 255), // Blue for input
                );
                pin_cache.input_pins.insert(entity, pin_pos);
                
                ui.allocate_space(egui::Vec2::new(12.0, 12.0)); // Space for input pin
                
                // Entity name and ID in the center
                ui.vertical(|ui| {
                    ui.strong(display_name);
                    ui.small(format!("Entity: {:?}", entity));
                });
                
                // Push expand toggle to the right - now will work correctly!
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let expand_text = if expanded { "▼" } else { "▶" };
                    let expand_response = ui.small_button(expand_text);
                    if expand_response.clicked() {
                        println!("🔽 Visual pass: Toggled expansion for {:?}: {} -> {}", entity, expanded, !expanded);
                        *expansion_changed = Some(!expanded);
                    }
                });
            }
        );
    }

    /// Render output pins section
    fn render_output_pins(&self, ui: &mut egui::Ui, entity: Entity, world: &World, pin_cache: &mut PinPositionCache) {
        let Ok(entity_ref) = world.get_entity(entity) else { return; };
        let Some(node_pins) = entity_ref.get::<NodePins>() else { return; };
        
        let pin_radius = 6.0;
        
        // Draw output pins vertically
        let output_pins: Vec<_> = node_pins.pins.iter()
            .enumerate()
            .filter(|(_, pin)| pin.pin_type == PinType::Output)
            .collect();
        
        if !output_pins.is_empty() {
            ui.label("Transitions:");
            
            for (original_pin_index, pin) in output_pins.iter() {
                // Make each pin row fill the full available width
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(ui.available_width(), 0.0), 
                    egui::Layout::left_to_right(egui::Align::Center), 
                    |ui| {
                        // Draw pin label first
                        ui.label(&pin.label);
                        
                        // Push the pin to the right side - now will work correctly!
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Draw red output pin circle on the right
                            let response = ui.allocate_response(egui::Vec2::new(pin_radius * 2.0, pin_radius * 2.0), egui::Sense::hover());
                            let pin_center = response.rect.center();
                            
                            ui.painter().circle_filled(
                                pin_center,
                                pin_radius,
                                egui::Color32::from_rgb(255, 100, 100), // Red for output
                            );
                            
                            // Store the actual pin position using the original index from the full pins array
                            pin_cache.output_pins.insert((entity, *original_pin_index), pin_center);
                        });
                    }
                );
            }
        }
    }

    /// Render the inspector UI for expanded nodes
    fn render_inspector(&self, ui: &mut egui::Ui, world: &mut World, entity: Entity) {
        let inspector_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            bevy_inspector::ui_for_entity(world, entity, ui);
        }));
        
        if inspector_result.is_err() {
            ui.label("⚠️ Inspector UI unavailable for this entity");
        }
    }

    /// Render action buttons (currently visual only - will be enhanced later)
    fn render_action_buttons(&self, ui: &mut egui::Ui, _entity: Entity, _expansion_changed: &mut Option<bool>) {
        // TODO: Connect these buttons to dialog states in a future iteration
        let _ = ui.button("+ Add Component");
        let _ = ui.button("+ Add Transition Listener");
    }
}