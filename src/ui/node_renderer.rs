use bevy::prelude::*;
use bevy_egui::egui;
use crate::components::*;
use crate::resources::*;
use super::UiResources;
use super::widgets::{NodeBody, ParentNodeBody};

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
        
        // Position for the node (converted to egui::Pos2 when needed)
        
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
            self.render_parent_node_with_zone(
                ui, entity, position, display_name, initial_state_target, 
                world, ui_resources
            );
        } else {
            // Leaf node: render as compact node
            let max_rect = egui::Rect::from_min_size(egui::Pos2::new(position.x, position.y), egui::Vec2::new(200.0, 100.0));
            let _ui_response = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(max_rect), |ui| {
                // Use the frame to provide background and let it size automatically to content
                let frame_response = frame.show(ui, |ui| {
                    let widget_response = NodeBody::new(
                        entity,
                        display_name,
                        transitions,
                    ).show(ui, world);
                    
                    // Update pin caches with widget data
                    if let Some(input_pos) = widget_response.input_pin_pos {
                        ui_resources.pin_cache.input_pins.insert(entity, input_pos);
                    }
                    
                    // Move the output_pin_positions before the loop to avoid borrow issues
                    let output_positions = widget_response.output_pin_positions;
                    for ((pin_entity, pin_index), pin_pos) in output_positions {
                        ui_resources.pin_cache.output_pins.insert((pin_entity, pin_index), pin_pos);
                    }
                    
                    // Return the response part of the widget_response
                    widget_response.response
                });
                
                // Store the actual measured size for interactions
                let measured_size = frame_response.response.rect.size();
                ui_resources.size_cache.sizes.insert(entity, measured_size);
            });
        }
    }

    /// Render a parent node with its zone background and header positioned at top-left
    fn render_parent_node_with_zone(
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
        
        // Convert bevy Rect to egui Rect for zone background
        let zone_rect = egui::Rect::from_min_size(
            egui::Pos2::new(position.x + zone_bounds.min.x, position.y + zone_bounds.min.y),
            egui::Vec2::new(zone_bounds.width(), zone_bounds.height())
        );
        
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
        
        // Draw zone background and border separately
        ui.painter().rect_filled(zone_rect, 5.0, zone_fill_color);
        
        // Draw border - let's try a simpler approach
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
        
        // 2. Position header at top-left of zone
        let header_pos = egui::Pos2::new(
            zone_rect.min.x + 10.0, // Small margin from left edge
            zone_rect.min.y + 10.0  // Small margin from top edge
        );
        
        let header_max_rect = egui::Rect::from_min_size(header_pos, egui::Vec2::new(200.0, 80.0));
        let _header_ui = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(header_max_rect), |ui| {
            // Use a frame for the header to make it stand out
            let header_fill_color = self.get_node_fill_color(entity, &ui_resources.transition_state);
            let header_frame = egui::Frame::default()
                .fill(header_fill_color)
                .corner_radius(5.0)
                .inner_margin(8.0);
            
            let frame_response = header_frame.show(ui, |ui| {
                // Render the parent node widget
                let widget_response = ParentNodeBody::new(
                    entity,
                    display_name,
                    initial_state_target,
                ).show(ui, world);
                
                // Update pin caches with widget data
                if let Some(input_pos) = widget_response.input_pin_pos {
                    ui_resources.pin_cache.input_pins.insert(entity, input_pos);
                }
                
                // Move the output_pin_positions before the loop to avoid borrow issues
                let output_positions = widget_response.output_pin_positions;
                for ((pin_entity, pin_index), pin_pos) in output_positions {
                    ui_resources.pin_cache.output_pins.insert((pin_entity, pin_index), pin_pos);
                }
                
                widget_response.response
            });
            
            // Store the header size for interactions (not the full zone)
            let measured_size = frame_response.response.rect.size();
            ui_resources.size_cache.sizes.insert(entity, measured_size);
        });
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
            // Dragged outside any zone - remove ChildOf if it exists
            let entity_ref = world.entity(dragging_entity);
            if entity_ref.contains::<ChildOf>() {
                world.entity_mut(dragging_entity).remove::<ChildOf>();
                println!("🆓 Removed entity {:?} from parent relationship", dragging_entity);
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