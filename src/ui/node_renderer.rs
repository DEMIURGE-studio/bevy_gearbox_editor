use bevy::prelude::*;
use bevy_egui::egui;
use crate::components::*;
use crate::resources::*;
use super::UiResources;
use super::widgets::{NodeBody, ParentNodeBody};
use crate::resources::ResizeEdge;

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
    ) -> Vec<(Entity, Vec2)> {
        let mut drag_changes = Vec::new();
        
        // First, handle resize interactions for parent zones (higher priority)
        self.handle_resize_interactions(ui, world, ui_resources);
        
        // Then handle regular node interactions
        for (entity, position, _expanded, _display_name) in node_data {
            if let Some(new_pos) = self.handle_node_interactions(
                ui, *entity, *position, world, ui_resources
            ) {
                if new_pos != *position {
                    drag_changes.push((*entity, new_pos));
                }
            }
        }
        
        drag_changes
    }

    /// Render unselected nodes visually (PASS 2)
    pub fn render_unselected_nodes(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        node_data: &[(Entity, Vec2, bool, Option<String>)],
        ui_resources: &mut UiResources,
    ) {
        for (entity, position, _expanded, display_name) in node_data {
            if ui_resources.selected_entity.entity != Some(*entity) {
                self.draw_node_visual_only(
                    ui, *entity, *position, display_name.clone(), 
                    world, ui_resources
                );
            }
        }
    }

    /// Render selected node visually (PASS 4)
    pub fn render_selected_node(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        node_data: &[(Entity, Vec2, bool, Option<String>)],
        ui_resources: &mut UiResources,
    ) {
        if let Some(selected_entity_id) = ui_resources.selected_entity.entity {
            if let Some((entity, position, _expanded, display_name)) = 
                node_data.iter().find(|(e, _, _, _)| *e == selected_entity_id) {
                self.draw_node_visual_only(
                    ui, *entity, *position, display_name.clone(), 
                    world, ui_resources
                );
            }
        }
    }

    /// Handle all input interactions for a single node
    fn handle_node_interactions(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) -> Option<Vec2> {
        // Prevent children from being dragged independently while their parent is being dragged
        if ui_resources.drag_drop_state.dragging_children.contains(&entity) {
            // This child is following its parent - don't process independent interactions
            return None;
        }
        
        // Prevent dragging if we're currently resizing
        if ui_resources.drag_drop_state.resizing_entity.is_some() {
            // Don't process drag interactions while resizing
            return None;
        }
        // Use the last measured size for interaction area (from previous frame)
        let measured_size = ui_resources.size_cache.sizes.get(&entity)
            .copied()
            .unwrap_or(egui::Vec2::new(200.0, 80.0)); // Fallback for first frame
        
        // Create invisible interaction area using measured size with relative positioning
        let ui_pos = ui.min_rect().min + egui::Vec2::new(position.x, position.y);
        let rect = egui::Rect::from_min_size(ui_pos, measured_size);
        let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());
        
        // Track changes
        let mut new_position = position;
        
        // Handle selection
        if response.clicked() || response.drag_started() {
            ui_resources.selected_entity.entity = Some(entity);
        }
        
        // Handle drag start
        if response.drag_started() && !ui_resources.transition_state.selecting_target {
            ui_resources.drag_drop_state.dragging_entity = Some(entity);
            
            // If this entity has children, collect them for coordinated movement
            let entity_ref = world.entity(entity);
            if entity_ref.contains::<Children>() {
                self.setup_parent_drag(entity, position, world, ui_resources);
                println!("🖱️ Started dragging parent entity {:?} with children", entity);
            } else {
                println!("🖱️ Started dragging entity {:?}", entity);
            }
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
            
            // If this is a parent being dragged, also move all its children
            if ui_resources.drag_drop_state.dragging_entity == Some(entity) 
                && !ui_resources.drag_drop_state.dragging_children.is_empty() {
                self.update_children_positions(delta, world, ui_resources);
            }
            
            // Update zone hover state during drag
            self.update_zone_hover_state(entity, new_position, world, ui_resources);
        }
        
        // Handle drag end
        if response.drag_stopped() && ui_resources.drag_drop_state.dragging_entity == Some(entity) {
            println!("🖱️ Stopped dragging entity {:?}", entity);
            
            // Apply parent-child relationship changes based on final position
            self.apply_drag_drop_changes(entity, world, ui_resources);
            
            // Clear drag state
            ui_resources.drag_drop_state.dragging_entity = None;
            ui_resources.drag_drop_state.hover_zone_entity = None;
            ui_resources.drag_drop_state.would_create_child_relationship = false;
            ui_resources.drag_drop_state.dragging_children.clear();
            ui_resources.drag_drop_state.children_initial_positions.clear();
            ui_resources.drag_drop_state.parent_initial_position = None;
            
            // Clear resize state
            ui_resources.drag_drop_state.resizing_entity = None;
            ui_resources.drag_drop_state.resize_edge = None;
            ui_resources.drag_drop_state.initial_zone_bounds = None;
            ui_resources.drag_drop_state.resize_start_mouse_pos = None;
        }
        
        // Return position change if any occurred
        if new_position != position {
            Some(new_position)
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

    /// Collect transition data from the entity for widget rendering
    fn collect_transitions(&self, entity: Entity, world: &World) -> Vec<(String, usize)> {
        let Ok(entity_ref) = world.get_entity(entity) else { return Vec::new(); };
        let Some(node_pins) = entity_ref.get::<NodePins>() else { return Vec::new(); };
        
        node_pins.pins.iter()
            .enumerate()
            .filter(|(_, pin)| pin.pin_type == PinType::Output)
            .map(|(index, pin)| (pin.label.clone(), index))
            .collect()
    }

    /// Draw the visual representation of a node using widget-based architecture
    fn draw_node_visual_only(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        display_name: Option<String>,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        let display_name = display_name
            .as_deref()
            .unwrap_or("Unnamed Entity")
            .to_string();
        
        // Check if this entity has children to determine if it's a parent node
        // Collect all the data we need from world before the closure
        let entity_ref = world.entity(entity);
        let has_children = entity_ref.contains::<Children>();
        let initial_state_target = if has_children {
            entity_ref.get::<crate::components::InitialStatePointer>()
                .and_then(|isp| isp.target_child)
                .or_else(|| {
                    // Fallback: use first child as initial state for now
                    entity_ref.get::<Children>().and_then(|children| children.first().copied())
                })
        } else {
            None
        };
        let transitions = if !has_children {
            self.collect_transitions(entity, world)
        } else {
            Vec::new()
        };
        
        // Create a custom frame with the node's background color
        let fill_color = self.get_node_fill_color(entity, &ui_resources.transition_state);
        let frame = egui::Frame::default()
            .fill(fill_color)
            .corner_radius(5.0)
            .inner_margin(8.0);
        
        if has_children {
            // Parent node: render zone background + header at top-left
            self.render_parent_node_with_zone_relative(
                ui, entity, position, display_name, initial_state_target, 
                world, ui_resources
            );
        } else {
            // Leaf node: render using relative positioning within the scroll area
            self.render_leaf_node_relative(
                ui, entity, position, display_name, transitions, frame, world, ui_resources
            );
        }
    }
    
    /// Render a leaf node using UI-relative positioning
    fn render_leaf_node_relative(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        display_name: String,
        transitions: Vec<(String, usize)>,
        frame: egui::Frame,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        // Convert position to be relative to the current UI min rect
        let ui_pos = ui.min_rect().min + egui::Vec2::new(position.x, position.y);
        
        // Allocate space at the specified position
        let node_size = egui::Vec2::new(200.0, 100.0);
        let node_rect = egui::Rect::from_min_size(ui_pos, node_size);
        
        // Create a child UI at the specified position
        let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(node_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        
        // Capture child UI min_rect before entering the closure
        let child_ui_min = child_ui.min_rect().min;
        
        // Render the frame and widget in the child UI
        let frame_response = frame.show(&mut child_ui, |ui| {
            let widget_response = NodeBody::new(
                entity,
                display_name,
                transitions,
            ).show(ui, world);
            
            // Update pin caches with widget data - convert to parent UI coordinates
            if let Some(input_pos) = widget_response.input_pin_pos {
                // Convert child UI coordinates to parent UI coordinates
                let parent_pos = ui_pos + (input_pos - child_ui_min);
                ui_resources.pin_cache.input_pins.insert(entity, parent_pos);
            }
            
            // Process output pins
            let output_positions = widget_response.output_pin_positions;
            for ((pin_entity, pin_index), pin_pos) in output_positions {
                // Convert child UI coordinates to parent UI coordinates
                let parent_pos = ui_pos + (pin_pos - child_ui_min);
                ui_resources.pin_cache.output_pins.insert((pin_entity, pin_index), parent_pos);
            }
            
            widget_response.response
        });
        
        // Store the actual measured size for interactions
        let measured_size = frame_response.response.rect.size();
        ui_resources.size_cache.sizes.insert(entity, measured_size);
    }

    /// Render a parent node with its zone background using relative positioning
    fn render_parent_node_with_zone_relative(
        &self,
        ui: &mut egui::Ui,
        entity: Entity,
        position: Vec2,
        display_name: String,
        initial_state_target: Option<Entity>,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        // Get zone bounds from ParentZone component or use defaults
        let zone_bounds = world.entity(entity).get::<crate::components::ParentZone>()
            .map(|pz| pz.bounds)
            .unwrap_or(bevy::math::Rect::new(0.0, 0.0, 400.0, 300.0)); // Default zone size
        
        // Convert position to be relative to the current UI
        let ui_min = ui.min_rect().min;
        let zone_pos = ui_min + egui::Vec2::new(position.x + zone_bounds.min.x, position.y + zone_bounds.min.y);
        let zone_size = egui::Vec2::new(zone_bounds.width(), zone_bounds.height());
        let zone_rect = egui::Rect::from_min_size(zone_pos, zone_size);
        
        // 1. Draw zone background with highlighting for drag-over state
        let (zone_fill_color, zone_stroke_color) = if ui_resources.drag_drop_state.hover_zone_entity == Some(entity) 
            && ui_resources.drag_drop_state.would_create_child_relationship {
            // Highlight zone when dragging over and would create relationship
            (
                egui::Color32::from_rgba_unmultiplied(100, 200, 100, 60), // Brighter green background
                egui::Color32::from_rgba_unmultiplied(100, 200, 100, 180), // Bright green border
            )
        } else {
            // Normal zone appearance
            (
                egui::Color32::from_rgba_unmultiplied(100, 100, 120, 30), // Very subtle background
                egui::Color32::from_rgba_unmultiplied(100, 100, 120, 100), // Subtle border
            )
        };
        
        // Draw zone background and border
        ui.painter().rect_filled(zone_rect, 5.0, zone_fill_color);
        
        // Draw border
        let border_points = vec![
            zone_rect.left_top(),
            zone_rect.right_top(),
            zone_rect.right_bottom(),
            zone_rect.left_bottom(),
            zone_rect.left_top(), // Close the loop
        ];
        ui.painter().add(egui::epaint::PathShape::line(
            border_points,
            egui::Stroke::new(2.0, zone_stroke_color)
        ));
        
        // 2. Position header at top-left of zone using child UI
        let header_pos = zone_rect.min + egui::Vec2::new(10.0, 10.0); // Small margin from edges
        let header_size = egui::Vec2::new(200.0, 80.0);
        let header_rect = egui::Rect::from_min_size(header_pos, header_size);
        
        let mut header_ui = ui.new_child(egui::UiBuilder::new().max_rect(header_rect).layout(egui::Layout::top_down(egui::Align::Min)));
        
        // Capture header UI min_rect before entering the closure
        let header_ui_min = header_ui.min_rect().min;
        
        // Use a frame for the header to make it stand out
        let header_fill_color = self.get_node_fill_color(entity, &ui_resources.transition_state);
        let header_frame = egui::Frame::default()
            .fill(header_fill_color)
            .corner_radius(5.0)
            .inner_margin(8.0);
        
        let frame_response = header_frame.show(&mut header_ui, |ui| {
            // Render the parent node widget
            let widget_response = ParentNodeBody::new(
                entity,
                display_name,
                initial_state_target,
            ).show(ui, world);
            
            // Update pin caches with widget data - convert to parent UI coordinates
            if let Some(input_pos) = widget_response.input_pin_pos {
                let parent_pos = header_pos + (input_pos - header_ui_min);
                ui_resources.pin_cache.input_pins.insert(entity, parent_pos);
            }
            
            // Process output pins
            let output_positions = widget_response.output_pin_positions;
            for ((pin_entity, pin_index), pin_pos) in output_positions {
                let parent_pos = header_pos + (pin_pos - header_ui_min);
                ui_resources.pin_cache.output_pins.insert((pin_entity, pin_index), parent_pos);
            }
            
            widget_response.response
        });
        
        // Store the header size for interactions (not the full zone)
        let measured_size = frame_response.response.rect.size();
        ui_resources.size_cache.sizes.insert(entity, measured_size);
    }



    /// Handle resize interactions for parent zones
    fn handle_resize_interactions(
        &self,
        ui: &mut egui::Ui,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        let mouse_pos = if let Some(hover_pos) = ui.ctx().pointer_hover_pos() {
            // Convert screen mouse position to UI-relative position
            let ui_relative_pos = hover_pos - ui.min_rect().min;
            Vec2::new(ui_relative_pos.x, ui_relative_pos.y)
        } else {
            return; // No mouse position available
        };
        
        // Check if we're currently resizing
        if let Some(resizing_entity) = ui_resources.drag_drop_state.resizing_entity {
            // Handle ongoing resize operation
            if ui.ctx().input(|i| i.pointer.primary_down()) {
                self.apply_zone_resize(resizing_entity, mouse_pos, world, ui_resources);
            } else {
                // Resize ended
                println!("🔄 Finished resizing entity {:?}", resizing_entity);
                ui_resources.drag_drop_state.resizing_entity = None;
                ui_resources.drag_drop_state.resize_edge = None;
                ui_resources.drag_drop_state.initial_zone_bounds = None;
                ui_resources.drag_drop_state.resize_start_mouse_pos = None;
            }
            return;
        }
        
        // Check for resize edge detection on parent zones
        let mut parent_zones_query = world.query::<(Entity, &GraphNode, &mut ParentZone)>();
        let parent_zones: Vec<_> = parent_zones_query
            .iter(world)
            .map(|(entity, graph_node, parent_zone)| (entity, graph_node.position, parent_zone.bounds))
            .collect();
        
        for (zone_entity, zone_position, zone_bounds) in parent_zones {
            if let Some(resize_edge) = self.detect_resize_edge(mouse_pos, zone_position, zone_bounds) {
                // Set cursor for resize
                match resize_edge {
                    ResizeEdge::Right => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
                    ResizeEdge::Bottom => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical),
                    ResizeEdge::Corner => ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe),
                }
                
                // Check for drag start
                if ui.ctx().input(|i| i.pointer.primary_pressed()) {
                    println!("🔄 Started resizing {:?} edge: {:?}", zone_entity, resize_edge);
                    ui_resources.drag_drop_state.resizing_entity = Some(zone_entity);
                    ui_resources.drag_drop_state.resize_edge = Some(resize_edge);
                    ui_resources.drag_drop_state.initial_zone_bounds = Some(zone_bounds);
                    ui_resources.drag_drop_state.resize_start_mouse_pos = Some(mouse_pos);
                }
                return; // Only handle one resize at a time
            }
        }
    }
    
    /// Detect which edge of a parent zone the mouse is over for resizing
    fn detect_resize_edge(&self, mouse_pos: Vec2, zone_position: Vec2, zone_bounds: bevy::math::Rect) -> Option<ResizeEdge> {
        let edge_threshold = 8.0; // Distance from edge to be considered "on edge"
        
        // Convert zone bounds to world coordinates
        let zone_world_rect = bevy::math::Rect::new(
            zone_position.x + zone_bounds.min.x,
            zone_position.y + zone_bounds.min.y,
            zone_position.x + zone_bounds.max.x,
            zone_position.y + zone_bounds.max.y,
        );
        
        let right_edge = zone_world_rect.max.x;
        let bottom_edge = zone_world_rect.max.y;
        
        let near_right = (mouse_pos.x - right_edge).abs() < edge_threshold;
        let near_bottom = (mouse_pos.y - bottom_edge).abs() < edge_threshold;
        
        // Check if mouse is within the zone area (for edge detection)
        let in_zone_x = mouse_pos.x >= zone_world_rect.min.x - edge_threshold 
                     && mouse_pos.x <= zone_world_rect.max.x + edge_threshold;
        let in_zone_y = mouse_pos.y >= zone_world_rect.min.y - edge_threshold 
                     && mouse_pos.y <= zone_world_rect.max.y + edge_threshold;
        
        if near_right && near_bottom && in_zone_x && in_zone_y {
            Some(ResizeEdge::Corner)
        } else if near_right && in_zone_y && mouse_pos.y >= zone_world_rect.min.y && mouse_pos.y <= zone_world_rect.max.y {
            Some(ResizeEdge::Right)
        } else if near_bottom && in_zone_x && mouse_pos.x >= zone_world_rect.min.x && mouse_pos.x <= zone_world_rect.max.x {
            Some(ResizeEdge::Bottom)
        } else {
            None
        }
    }
    
    /// Apply resize changes to a parent zone
    fn apply_zone_resize(
        &self,
        resizing_entity: Entity,
        current_mouse_pos: Vec2,
        world: &mut World,
        ui_resources: &UiResources,
    ) {
        let Some(resize_edge) = &ui_resources.drag_drop_state.resize_edge else { return; };
        let Some(initial_bounds) = ui_resources.drag_drop_state.initial_zone_bounds else { return; };
        let Some(start_mouse_pos) = ui_resources.drag_drop_state.resize_start_mouse_pos else { return; };
        
        // Calculate mouse delta
        let delta = current_mouse_pos - start_mouse_pos;
        
        // Get the parent zone component to update
        let Ok(mut parent_zone) = world.query::<&mut ParentZone>().get_mut(world, resizing_entity) else { return; };
        
        // Calculate new bounds based on resize edge and mouse delta
        let mut new_bounds = initial_bounds;
        
        match resize_edge {
            ResizeEdge::Right => {
                new_bounds.max.x = initial_bounds.max.x + delta.x;
            },
            ResizeEdge::Bottom => {
                new_bounds.max.y = initial_bounds.max.y + delta.y;
            },
            ResizeEdge::Corner => {
                new_bounds.max.x = initial_bounds.max.x + delta.x;
                new_bounds.max.y = initial_bounds.max.y + delta.y;
            },
        }
        
        // Ensure minimum size constraints
        let min_width = parent_zone.min_size.x;
        let min_height = parent_zone.min_size.y;
        
        if new_bounds.width() < min_width {
            new_bounds.max.x = new_bounds.min.x + min_width;
        }
        if new_bounds.height() < min_height {
            new_bounds.max.y = new_bounds.min.y + min_height;
        }
        
        // Update the parent zone bounds
        parent_zone.bounds = new_bounds;
        
        // Update resize handles (for future reference)
        let handle_size = 10.0;
        parent_zone.resize_handles = [
            // Top edge (not used currently)
            bevy::math::Rect::new(new_bounds.min.x, new_bounds.min.y - handle_size/2.0, 
                                new_bounds.max.x, new_bounds.min.y + handle_size/2.0),
            // Right edge
            bevy::math::Rect::new(new_bounds.max.x - handle_size/2.0, new_bounds.min.y, 
                                new_bounds.max.x + handle_size/2.0, new_bounds.max.y),
            // Bottom edge
            bevy::math::Rect::new(new_bounds.min.x, new_bounds.max.y - handle_size/2.0, 
                                new_bounds.max.x, new_bounds.max.y + handle_size/2.0),
            // Left edge (not used currently)
            bevy::math::Rect::new(new_bounds.min.x - handle_size/2.0, new_bounds.min.y, 
                                new_bounds.min.x + handle_size/2.0, new_bounds.max.y),
        ];
    }

    /// Update zone hover state during drag operations
    fn update_zone_hover_state(
        &self,
        dragging_entity: Entity,
        drag_position: Vec2,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        // Get all parent entities (entities with Children component)
        let mut parent_zones_query = world.query::<(Entity, &GraphNode, &ParentZone)>();
        let parent_zones: Vec<_> = parent_zones_query
            .iter(world)
            .map(|(entity, graph_node, parent_zone)| (entity, graph_node.position, parent_zone.bounds))
            .collect();
        
        let mut hover_zone_entity = None;
        let mut would_create_relationship = false;
        
        // Check if drag position is within any parent zone
        for (zone_entity, zone_position, zone_bounds) in parent_zones {
            // Skip if trying to drag into itself
            if zone_entity == dragging_entity {
                continue;
            }
            
            // Convert zone bounds to world coordinates
            let zone_world_rect = bevy::math::Rect::new(
                zone_position.x + zone_bounds.min.x,
                zone_position.y + zone_bounds.min.y,
                zone_position.x + zone_bounds.max.x,
                zone_position.y + zone_bounds.max.y,
            );
            
            // Check if drag position is within this zone
            if zone_world_rect.contains(drag_position) {
                hover_zone_entity = Some(zone_entity);
                
                // Check if this would create a valid parent-child relationship
                // (i.e., the dragging entity isn't already a child of this parent)
                let dragging_entity_ref = world.entity(dragging_entity);
                let current_parent = dragging_entity_ref.get::<ChildOf>()
                    .map(|child_of| child_of.0);
                
                would_create_relationship = current_parent != Some(zone_entity);
                break; // Take the first matching zone (could be improved with z-order)
            }
        }
        
        // Update drag drop state
        ui_resources.drag_drop_state.hover_zone_entity = hover_zone_entity;
        ui_resources.drag_drop_state.would_create_child_relationship = would_create_relationship;
    }
    
    /// Set up parent drag by collecting all descendants and their relative positions
    fn setup_parent_drag(
        &self,
        parent_entity: Entity,
        parent_position: Vec2,
        world: &mut World,
        ui_resources: &mut UiResources,
    ) {
        // Collect all descendants recursively
        let descendants = self.collect_all_descendants(parent_entity, world);
        
        println!("🔄 Setting up parent drag for {:?} with {} descendants", 
                parent_entity, descendants.len());
        
        // Store parent's initial position
        ui_resources.drag_drop_state.parent_initial_position = Some(parent_position);
        
        // Store each child's current position
        let mut children_query = world.query::<&GraphNode>();
        for child_entity in &descendants {
            if let Ok(child_graph_node) = children_query.get(world, *child_entity) {
                ui_resources.drag_drop_state.children_initial_positions
                    .insert(*child_entity, child_graph_node.position);
                println!("  📍 Child {:?} at position {:?}", child_entity, child_graph_node.position);
            }
        }
        
        // Store the list of dragging children
        ui_resources.drag_drop_state.dragging_children = descendants;
    }
    
    /// Update positions of all children following the dragged parent
    fn update_children_positions(
        &self,
        drag_delta: egui::Vec2,
        world: &mut World,
        ui_resources: &UiResources,
    ) {
        let mut nodes_query = world.query::<&mut GraphNode>();
        let bevy_delta = Vec2::new(drag_delta.x, drag_delta.y);
        
        // Apply the same delta to all dragging children
        for child_entity in &ui_resources.drag_drop_state.dragging_children {
            if let Ok(mut child_graph_node) = nodes_query.get_mut(world, *child_entity) {
                child_graph_node.position += bevy_delta;
            }
        }
        
        if !ui_resources.drag_drop_state.dragging_children.is_empty() {
            println!("🚚 Moved {} children by delta {:?}", 
                    ui_resources.drag_drop_state.dragging_children.len(), bevy_delta);
        }
    }
    
    /// Recursively collect all descendants of an entity
    fn collect_all_descendants(&self, entity: Entity, world: &World) -> Vec<Entity> {
        let mut descendants = Vec::new();
        self.collect_descendants_recursive(entity, world, &mut descendants);
        descendants
    }
    
    /// Recursive helper to collect descendants
    fn collect_descendants_recursive(&self, entity: Entity, world: &World, descendants: &mut Vec<Entity>) {
        if let Some(children) = world.entity(entity).get::<Children>() {
            for child in children.iter() {
                descendants.push(child);
                // Recursively collect grandchildren, great-grandchildren, etc.
                self.collect_descendants_recursive(child, world, descendants);
            }
        }
    }

    /// Apply parent-child relationship changes based on drag-drop result
    fn apply_drag_drop_changes(
        &self,
        dragging_entity: Entity,
        world: &mut World,
        ui_resources: &UiResources,
    ) {
        if let Some(target_parent) = ui_resources.drag_drop_state.hover_zone_entity {
            if ui_resources.drag_drop_state.would_create_child_relationship {
                // Add ChildOf component to create parent-child relationship
                world.entity_mut(dragging_entity).insert(ChildOf(target_parent));
                println!("👶 Made entity {:?} a child of {:?}", dragging_entity, target_parent);
            }
        } else {
            // Dragged outside any zone - assign to root entity instead of removing ChildOf
            // Since we can't easily query with an immutable World reference, we'll let the enforce_root_hierarchy system handle this
            // For now, just remove the ChildOf and let the system assign it to root on the next frame
            let entity_ref = world.entity(dragging_entity);
            if entity_ref.contains::<ChildOf>() {
                world.entity_mut(dragging_entity).remove::<ChildOf>();
                println!("🆓 Removed entity {:?} from parent relationship (will be assigned to root by system)", dragging_entity);
            }
        }
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

    // Note: Old rendering methods removed - now handled by widgets in widgets.rs
}