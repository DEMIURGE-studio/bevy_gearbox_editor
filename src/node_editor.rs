//! Node editor UI and node management
//! 
//! This module handles:
//! - Rendering the main node editor interface
//! - Converting between node types (Leaf <-> Parent)
//! - Z-ordering and selection management
//! - Node interaction and dragging

use bevy::prelude::*;
use bevy_gearbox::{InitialState, StateMachineRoot};
use bevy_egui::egui;
use std::collections::HashSet;

use crate::editor_state::{EditorState, StateMachineEditorData, NodeDragged, NodeContextMenuRequested, RenderItem, get_entity_name, should_get_selection_boost, TransitionCreationRequested, CreateTransition, draw_arrow, draw_interactive_pill_label, closest_point_on_rect_edge};
use crate::components::{NodeType, LeafNode, ParentNode};

/// System to update node types based on entity hierarchy
/// 
/// Converts leaf nodes to parent nodes when they gain children,
/// and parent nodes back to leaf nodes when they lose all children.
pub fn update_node_types(
    editor_state: Res<EditorState>,
    mut state_machines: Query<&mut StateMachineEditorData, With<StateMachineRoot>>,
    parent_query: Query<Entity, With<InitialState>>,
    leaf_query: Query<Entity, Without<InitialState>>,
    children_query: Query<&Children>,
) {
    if let Some(selected_root) = editor_state.selected_machine {
        if let Ok(mut machine_data) = state_machines.get_mut(selected_root) {
        
        // Get all entities in the selected state machine's hierarchy
        let mut descendants: Vec<Entity> = children_query
            .iter_descendants_depth_first(selected_root)
            .collect();
        descendants.insert(0, selected_root); // Include the root
        
        // Process each entity in the hierarchy
        for &entity in &descendants {
            if parent_query.contains(entity) {
                // Entity should be a parent node
                match machine_data.nodes.get(&entity) {
                    Some(NodeType::Parent(_)) => {
                        // Already a parent node, no change needed
                    }
                    Some(NodeType::Leaf(leaf_node)) => {
                        // Convert leaf to parent
                        let parent_node = ParentNode::new(leaf_node.entity_node.position);
                        machine_data.nodes.insert(entity, NodeType::Parent(parent_node));
                    }
                    None => {
                        // Create new parent node
                        let parent_node = ParentNode::new(egui::Pos2::new(200.0, 100.0));
                        machine_data.nodes.insert(entity, NodeType::Parent(parent_node));
                    }
                }
            } else if leaf_query.contains(entity) {
                // Entity should be a leaf node
                match machine_data.nodes.get(&entity) {
                    Some(NodeType::Leaf(_)) => {
                        // Already a leaf node, no change needed
                    }
                    Some(NodeType::Parent(parent_node)) => {
                        // Convert parent to leaf
                        let leaf_node = LeafNode::new(parent_node.entity_node.position);
                        machine_data.nodes.insert(entity, NodeType::Leaf(leaf_node));
                    }
                    None => {
                        // Create new leaf node
                        let leaf_node = LeafNode::new(egui::Pos2::new(100.0, 100.0));
                        machine_data.nodes.insert(entity, NodeType::Leaf(leaf_node));
                    }
                }
            }
        }
        
        // Remove nodes that are no longer part of the active hierarchy
        let valid_entities: HashSet<Entity> = descendants.into_iter().collect();
        machine_data.nodes.retain(|entity, _| valid_entities.contains(entity));
        }
    }
}

/// Render the node editor interface for a selected state machine
pub fn show_machine_editor(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    machine_data: &mut StateMachineEditorData,
    all_entities: &Query<(Entity, Option<&Name>, Option<&InitialState>)>,
    child_of_query: &Query<&ChildOf>,
    children_query: &Query<&Children>,
    commands: &mut Commands,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Add back button at the top
        ui.horizontal(|ui| {
            if ui.button("← Back to Machine List").clicked() {
                editor_state.selected_machine = None;
                machine_data.selected_node = None;
            }
            
            if let Some(selected_root) = editor_state.selected_machine {
                let machine_name = get_entity_name(selected_root, all_entities);
                ui.separator();
                ui.label(format!("Editing: {}", machine_name));
            }
        });
        
        ui.separator();
        
        if let Some(selected_root) = editor_state.selected_machine {
            // Build render queue with z-order based on hierarchy depth
            let mut render_queue = Vec::new();
            
            // Get all entities in depth-first order for natural z-ordering
            let mut hierarchy_entities: Vec<Entity> = children_query
                .iter_descendants_depth_first(selected_root)
                .collect();
            hierarchy_entities.insert(0, selected_root);
            
            for (hierarchy_index, entity) in hierarchy_entities.iter().enumerate() {
                if let Some(_node) = machine_data.nodes.get(entity) {
                    let base_z_order = hierarchy_index as i32 * 10;
                    let selection_boost = if should_get_selection_boost(*entity, machine_data.selected_node, child_of_query) { 
                        5 
                    } else { 
                        0 
                    };
                    
                    render_queue.push(RenderItem {
                        entity: *entity,
                        z_order: base_z_order + selection_boost,
                    });
                }
            }
            
            // Sort by z-order (lower values render first, higher values on top)
            render_queue.sort_by_key(|item| item.z_order);
            
            // Render all nodes in z-order
            for render_item in render_queue {
                let entity = render_item.entity;
                let entity_name = get_entity_name(entity, all_entities);
                
                if let Some(node) = machine_data.nodes.get_mut(&entity) {
                    let is_selected = machine_data.selected_node == Some(entity);
                    let is_root = selected_root == entity;
                    let is_editing = machine_data.text_editing.is_editing(entity);
                    let should_focus = machine_data.text_editing.should_focus;
                    
                    let first_focus = machine_data.text_editing.first_focus;
                    
                    let response = match node {
                        NodeType::Leaf(leaf_node) => {
                            leaf_node.show(ui, &entity_name, Some(&format!("{:?}", entity)), is_selected, is_root, is_editing, &mut machine_data.text_editing.current_text, should_focus, first_focus)
                        }
                        NodeType::Parent(parent_node) => {
                            parent_node.show(ui, &entity_name, Some(&format!("{:?}", entity)), is_selected, is_root, is_editing, &mut machine_data.text_editing.current_text, should_focus, first_focus)
                        }
                    };
                    
                    // Clear focus flag after first frame
                    if should_focus {
                        machine_data.text_editing.should_focus = false;
                    }
                    
                    // Clear first focus flag after it's been used
                    if first_focus {
                        machine_data.text_editing.first_focus = false;
                    }
                    
                    // Handle selection
                    if response.clicked {
                        // Check if we're in transition creation mode and waiting for target selection
                        if machine_data.transition_creation.awaiting_target_selection {
                            let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
                            machine_data.transition_creation.set_target(entity, pointer_pos);
                        } else {
                            machine_data.selected_node = Some(entity);
                        }
                    }
                    
                    // Handle + button click for transition creation (leaf nodes only)
                    if response.add_transition_clicked {
                        commands.trigger(TransitionCreationRequested {
                            source_entity: entity,
                        });
                    }
                    
                    // Handle right-click context menu
                    if response.right_clicked {
                        let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
                        commands.trigger(NodeContextMenuRequested {
                            entity,
                            position: pointer_pos,
                        });
                    }
                    
                    // Handle dragging
                    if response.dragged {
                        // Node was dragged - position is automatically updated in the component
                        // Emit event to handle parent-child movement
                        commands.trigger(NodeDragged {
                            entity,
                            drag_delta: response.drag_delta,
                        });
                    }
                }
            }
            
            // Update transition rectangles before rendering
            update_transition_rectangles(machine_data);
            
            // Render transition arrows after all nodes
            render_transition_connections(ui, machine_data);
            
            // Render initial state indicators
            render_initial_state_indicators(ui, machine_data, &all_entities, selected_root);
            
            // Handle background clicks to cancel transition creation
            if machine_data.transition_creation.awaiting_target_selection {
                // Check for clicks on background (not handled by any node)
                if ui.input(|i| i.pointer.primary_clicked()) {
                    let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
                    let clicked_on_node = machine_data.nodes.values().any(|node| {
                        node.current_rect().contains(pointer_pos)
                    });
                    
                    if !clicked_on_node {
                        machine_data.transition_creation.cancel();
                    }
                }
            }
        } else {
            ui.label("No state machine selected");
        }
        
        // Handle text editing completion
        handle_text_editing_completion(ui, machine_data, commands);
        
        // Render transition creation UI
        render_transition_creation_ui(ui, machine_data, commands);
    });
}

/// Render the transition creation dropdown UI
fn render_transition_creation_ui(
    ui: &mut egui::Ui,
    machine_data: &mut StateMachineEditorData,
    commands: &mut Commands,
) {
    // Show visual arrow from source to mouse if we're waiting for target selection
    if machine_data.transition_creation.awaiting_target_selection {
        if let Some(source) = machine_data.transition_creation.source_entity {
            // Draw arrow from source entity to mouse cursor
            if let Some(source_node) = machine_data.nodes.get(&source) {
                let mouse_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
                let source_rect = source_node.current_rect();
                
                // Draw from the edge of the source node to the mouse cursor
                let source_edge = closest_point_on_rect_edge(source_rect, mouse_pos);
                
                // Draw a dashed line from source edge to mouse (white color)
                let painter = ui.painter();
                draw_dashed_arrow(&painter, source_edge, mouse_pos, egui::Color32::WHITE);
            }
            
            // Check for cancellation via right-click, escape key, or clicking background
            if ui.input(|i| {
                i.pointer.secondary_clicked() || // Right click
                i.key_pressed(egui::Key::Escape) // Escape key
            }) {
                machine_data.transition_creation.cancel();
            }
        }
    }
    
    // Show event type dropdown
    if machine_data.transition_creation.show_event_dropdown {
        if let (Some(source), Some(target), Some(position)) = (
            machine_data.transition_creation.source_entity,
            machine_data.transition_creation.target_entity,
            machine_data.transition_creation.dropdown_position,
        ) {
            let dropdown_id = egui::Id::new("transition_event_dropdown");
            
            egui::Area::new(dropdown_id)
                .fixed_pos(position)
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .show(ui, |ui| {
                            ui.set_min_width(200.0);
                            ui.heading("Select Event Type");
                            ui.separator();
                            
                            if machine_data.transition_creation.available_event_types.is_empty() {
                                ui.label("No TransitionListener event types found.");
                                ui.label("Make sure event types are registered with the type registry.");
                            } else {
                                for event_type in &machine_data.transition_creation.available_event_types.clone() {
                                    if ui.button(event_type).clicked() {
                                        commands.trigger(CreateTransition {
                                            source_entity: source,
                                            target_entity: target,
                                            event_type: event_type.clone(),
                                        });
                                    }
                                }
                            }
                            
                            ui.separator();
                            if ui.button("Cancel").clicked() {
                                machine_data.transition_creation.cancel();
                            }
                        });
                });
            
            // Close dropdown if clicked elsewhere
            if ui.input(|i| i.pointer.any_click()) {
                let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
                let dropdown_rect = egui::Rect::from_min_size(position, egui::Vec2::new(200.0, 150.0));
                
                if !dropdown_rect.contains(pointer_pos) {
                    machine_data.transition_creation.cancel();
                }
            }
        }
    }
}

/// Render visual connections for existing transitions
fn render_transition_connections(
    ui: &mut egui::Ui,
    machine_data: &mut StateMachineEditorData,
) {
    // Extract data needed for rendering to avoid borrowing issues
    let transitions_data: Vec<_> = machine_data.visual_transitions.iter().enumerate().map(|(index, transition)| {
        (index, 
         transition.calculate_two_segment_points(),
         transition.event_node_position,
         transition.event_type.clone(),
         transition.is_dragging_event_node)
    }).collect();
    
    let painter = ui.painter();
    let mut interaction_data = Vec::new();
    
    // First pass: Draw all the arrows (using painter)
    for (_index, (source_start, source_end, target_start, target_end), _event_pos, _event_type, _is_dragging) in &transitions_data {
        // Draw the two arrow segments
        draw_arrow(&painter, *source_start, *source_end, egui::Color32::WHITE);
        draw_arrow(&painter, *target_start, *target_end, egui::Color32::WHITE);
    }
    
    // Second pass: Draw interactive event nodes (using ui mutably)
    for (index, (_source_start, _source_end, _target_start, _target_end), event_pos, event_type, is_dragging) in transitions_data {
        // Draw the interactive event node
        let font_id = egui::FontId::new(12.0, egui::FontFamily::Proportional);
        let response = draw_interactive_pill_label(
            ui, 
            event_pos, 
            &event_type, 
            font_id,
            is_dragging
        );
        
        // Store interaction data for later processing
        interaction_data.push((index, response));
    }
    
    // Process interactions after rendering
    for (index, response) in interaction_data {
        let transition = &mut machine_data.visual_transitions[index];
        
        // Handle event node dragging
        if response.drag_started() {
            transition.is_dragging_event_node = true;
        }
        
        if response.dragged() && transition.is_dragging_event_node {
            transition.event_node_position += response.drag_delta();
        }
        
        if response.drag_stopped() {
            transition.is_dragging_event_node = false;
            // Update the offset based on the new position
            transition.update_event_node_offset();
        }
    }
}

/// Update the rectangles in visual transitions to match current node positions
fn update_transition_rectangles(machine_data: &mut StateMachineEditorData) {
    for transition in &mut machine_data.visual_transitions {
        if let Some(source_node) = machine_data.nodes.get(&transition.source_entity) {
            transition.source_rect = source_node.current_rect();
        }
        if let Some(target_node) = machine_data.nodes.get(&transition.target_entity) {
            transition.target_rect = target_node.current_rect();
        }
        
        // Update event node position based on new source/target positions (unless being dragged)
        if !transition.is_dragging_event_node {
            transition.update_event_node_position();
        }
    }
}

/// Handle text editing completion (Enter key or click outside)
fn handle_text_editing_completion(
    ui: &mut egui::Ui,
    machine_data: &mut StateMachineEditorData,
    commands: &mut Commands,
) {
    if machine_data.text_editing.editing_entity.is_some() {
        let should_complete = ui.input(|i| {
            // Complete on Enter key
            i.key_pressed(egui::Key::Enter) ||
            // Complete on Escape key (cancel)
            i.key_pressed(egui::Key::Escape) ||
            // Complete when clicking outside
            i.pointer.any_click()
        });
        
        let is_escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        
        if should_complete {
            if is_escape {
                info!("❌ Cancelled text editing");
                machine_data.text_editing.cancel_editing();
            } else if let Some((entity, new_name)) = machine_data.text_editing.stop_editing() {
                info!("💾 Completing text edit for entity {:?} with name '{}'", entity, new_name);
                // Update the entity's name if it's not empty
                let trimmed_name = new_name.trim();
                if !trimmed_name.is_empty() {
                    commands.entity(entity).insert(Name::new(trimmed_name.to_string()));
                    info!("✅ Updated entity {:?} name to '{}'", entity, trimmed_name);
                } else {
                    info!("⚠️ Ignoring empty name for entity {:?}", entity);
                }
            }
        }
    }
}

/// Render initial state indicators (circle + arrow) for entities marked as InitialState
fn render_initial_state_indicators(
    ui: &mut egui::Ui,
    machine_data: &StateMachineEditorData,
    all_entities: &Query<(Entity, Option<&Name>, Option<&InitialState>)>,
    selected_root: Entity,
) {
    let painter = ui.painter();
    
    // Find all entities with InitialState component that belong to the current state machine
    for (parent_entity, _name, initial_state_opt) in all_entities.iter() {
        if let Some(initial_state) = initial_state_opt {
            let target_entity = initial_state.0;
            
            // Only render if both parent and target are in our editor nodes and belong to current state machine
            if let (Some(_parent_node), Some(target_node)) = (
                machine_data.nodes.get(&parent_entity),
                machine_data.nodes.get(&target_entity)
            ) {
                // Check if this belongs to the currently selected state machine
                // (We can do this by checking if the parent entity is a child of selected_root or is selected_root)
                let belongs_to_current_machine = parent_entity == selected_root || 
                    all_entities.iter().any(|(entity, _, _)| {
                        entity == selected_root && 
                        // This is a simplified check - in a real implementation you'd traverse the hierarchy
                        true // For now, assume all nodes in machine_data.nodes belong to current machine
                    });
                
                if belongs_to_current_machine {
                    render_initial_state_indicator(
                        &painter,
                        target_node.current_rect(),
                    );
                }
            }
        }
    }
}

/// Render a single initial state indicator (circle + curved arrow)
fn render_initial_state_indicator(
    painter: &egui::Painter,
    target_rect: egui::Rect,
) {
    // Circle position: to the left and lower relative to target node (moved 6px right)
    let circle_offset = egui::Vec2::new(-13.0, 1.0);
    let circle_center = target_rect.left_top() + circle_offset;
    let circle_radius = 3.0;
    
    // Draw the circle (white)
    painter.circle_filled(
        circle_center,
        circle_radius,
        egui::Color32::WHITE,
    );
    
    // Draw circle border (slightly darker white/light gray)
    painter.circle_stroke(
        circle_center,
        circle_radius,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(200, 200, 200)),
    );
    
    // Calculate curved arrow that hits the left side at 16px from top
    let arrow_start = circle_center + egui::Vec2::new(0.0, circle_radius); // Bottom of circle
    let arrow_end = egui::Pos2::new(target_rect.left(), target_rect.top() + 16.0); // 16px from top
    
    // Create a curved path using quadratic bezier
    // Control point creates the curve - positioned to make arrow go down then right
    let control_point = egui::Pos2::new(
        arrow_start.x, // Keep at same horizontal position as start
        arrow_start.y + (arrow_end.y - arrow_start.y) * 0.7, // 70% of the way vertically
    );
    
    // Draw curved arrow using multiple line segments to approximate bezier curve
    let segments = 12;
    let mut prev_point = arrow_start;
    
    for i in 1..=segments {
        let t = i as f32 / segments as f32;
        let current_point = quadratic_bezier(arrow_start, control_point, arrow_end, t);
        
        painter.line_segment(
            [prev_point, current_point],
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );
        
        prev_point = current_point;
    }
    
    // Draw arrowhead pointing horizontally (perpendicular to left side)
    let arrowhead_size = 4.0;
    let arrowhead_direction = egui::Vec2::new(1.0, 0.0); // Point right (into the node)
    let perpendicular = egui::Vec2::new(0.0, 1.0); // Vertical perpendicular
    
    let arrowhead_point1 = arrow_end - arrowhead_direction * arrowhead_size + perpendicular * (arrowhead_size * 0.5);
    let arrowhead_point2 = arrow_end - arrowhead_direction * arrowhead_size - perpendicular * (arrowhead_size * 0.5);
    
    painter.line_segment(
        [arrow_end, arrowhead_point1],
        egui::Stroke::new(2.0, egui::Color32::WHITE),
    );
    painter.line_segment(
        [arrow_end, arrowhead_point2],
        egui::Stroke::new(2.0, egui::Color32::WHITE),
    );
}

/// Calculate a point on a quadratic bezier curve
fn quadratic_bezier(start: egui::Pos2, control: egui::Pos2, end: egui::Pos2, t: f32) -> egui::Pos2 {
    let one_minus_t = 1.0 - t;
    let one_minus_t_sq = one_minus_t * one_minus_t;
    let t_sq = t * t;
    
    egui::Pos2::new(
        one_minus_t_sq * start.x + 2.0 * one_minus_t * t * control.x + t_sq * end.x,
        one_minus_t_sq * start.y + 2.0 * one_minus_t * t * control.y + t_sq * end.y,
    )
}

/// Draw a dashed arrow from start to end position
fn draw_dashed_arrow(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: egui::Color32) {
    let direction = end - start;
    let distance = direction.length();
    
    if distance < 1.0 {
        return; // Too short to draw
    }
    
    let normalized_direction = direction / distance;
    let dash_length = 8.0;
    let gap_length = 4.0;
    let dash_and_gap = dash_length + gap_length;
    
    // Draw dashed line
    let mut current_distance = 0.0;
    while current_distance < distance {
        let dash_start = start + normalized_direction * current_distance;
        let dash_end_distance = (current_distance + dash_length).min(distance);
        let dash_end = start + normalized_direction * dash_end_distance;
        
        painter.line_segment(
            [dash_start, dash_end],
            egui::Stroke::new(2.0, color),
        );
        
        current_distance += dash_and_gap;
    }
    
    // Draw arrowhead at the end
    let arrowhead_size = 6.0;
    let perpendicular = egui::Vec2::new(-normalized_direction.y, normalized_direction.x);
    
    let arrowhead_point1 = end - normalized_direction * arrowhead_size + perpendicular * (arrowhead_size * 0.5);
    let arrowhead_point2 = end - normalized_direction * arrowhead_size - perpendicular * (arrowhead_size * 0.5);
    
    painter.line_segment(
        [end, arrowhead_point1],
        egui::Stroke::new(2.0, color),
    );
    painter.line_segment(
        [end, arrowhead_point2],
        egui::Stroke::new(2.0, color),
    );
}
