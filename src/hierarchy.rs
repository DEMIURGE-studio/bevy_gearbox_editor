//! Hierarchy management for parent-child relationships in the node editor
//! 
//! This module handles:
//! - Recursive parent-child movement when nodes are dragged
//! - Constraining children to stay within parent bounds
//! - Auto-resizing parents to fit their children
//! - Managing InitialState components for parent entities

use bevy::prelude::*;
use bevy_gearbox::StateMachine;
use std::collections::HashMap;
use bevy::platform::collections::HashSet;

use crate::editor_state::{EditorState, NodeDragged};
use crate::components::NodeType;
use crate::StateMachinePersistentData;



/// Observer to handle parent-child movement when nodes are dragged
/// 
/// This observer recursively moves all children when a parent is dragged,
/// maintaining relative positions throughout the hierarchy.
pub fn handle_parent_child_movement(
    trigger: Trigger<NodeDragged>,
    mut q_sm: Query<&mut StateMachinePersistentData, With<StateMachine>>,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
    q_children: Query<&bevy_gearbox::StateChildren>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Find which machine contains the dragged entity
    let selected_machine = q_child_of.root_ancestor(event.entity);
    
    // Get the machine data for the selected state machine
    let Ok(mut machine_data) = q_sm.get_mut(selected_machine) else {
        return;
    };
    
    // Get all entities that have the dragged entity as their parent
    let Ok(children) = q_children.get(event.entity) else {
        return;
    };
    
    // Move all children by the same delta as the parent
    for child_entity in children.iter() {
        if let Some(child_node) = machine_data.nodes.get_mut(&child_entity) {
            match child_node {
                NodeType::Leaf(leaf_node) => {
                    leaf_node.entity_node.position += event.drag_delta;
                }
                NodeType::Parent(parent_node) => {
                    parent_node.entity_node.position += event.drag_delta;
                }
            }
            
            // 🔄 Recursively trigger NodeDragged for this child to move its children
            commands.trigger(NodeDragged {
                entity: child_entity,
                drag_delta: event.drag_delta,
            });
        } else {
            warn!("  ❌ Child entity {:?} not found in editor_state.nodes", child_entity);
        }
    }
}

// NOTE: Removed automatic ensure_initial_states.

/// System to constrain child nodes to stay within their parent's bounds
/// 
/// Children are prevented from moving left or up outside their parent,
/// but can move right and down freely (which will trigger parent expansion).
pub fn constrain_children_to_parents(
    editor_state: Res<EditorState>,
    mut q_sm: Query<&mut StateMachinePersistentData, With<StateMachine>>,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
    q_children: Query<&bevy_gearbox::StateChildren>,
) {
    // Process each open machine separately
    for open_machine in &editor_state.open_machines {
        let Ok(mut machine_data) = q_sm.get_mut(open_machine.entity) else {
            continue;
        };
    
        // Process all parent nodes in this machine
        let parent_entities: Vec<Entity> = machine_data.nodes.keys()
            .filter(|&&entity| q_children.contains(entity))
            .copied()
            .collect();
        
        for parent_entity in parent_entities {
            if let Ok(children) = q_children.get(parent_entity) {
                for child_entity in children.iter() {
                    constrain_child_to_parent(child_entity, &mut machine_data, &q_child_of);
                }
            }
        }

        // Constrain transition event node positions to the appropriate parent content area
        // To avoid conflicting borrows of machine_data.nodes, copy needed parent rects first
        let mut parent_content_rects: HashMap<Entity, egui::Rect> = HashMap::new();
        for (&entity, node) in machine_data.nodes.iter() {
            if let NodeType::Parent(parent) = node {
                parent_content_rects.insert(entity, parent.content_rect());
            }
        }

        for t in machine_data.visual_transitions.iter_mut() {
            // Determine which end is higher in the hierarchy
            let source_depth = hierarchy_depth(t.source_entity, &q_child_of);
            let target_depth = hierarchy_depth(t.target_entity, &q_child_of);
            let higher = if source_depth <= target_depth { t.source_entity } else { t.target_entity };
            let other = if higher == t.source_entity { t.target_entity } else { t.source_entity };
            // Exception: direct Parent->Child connections contain by parent; else parent of higher (or higher if root)
            let is_direct_child = match q_child_of.get(other) { Ok(rel) => rel.0 == higher, Err(_) => false };
            let parent_for_pill = if is_direct_child { higher } else if let Ok(rel) = q_child_of.get(higher) { rel.0 } else { higher };

            if let Some(content_rect) = parent_content_rects.get(&parent_for_pill).copied() {
                let margin = egui::Vec2::new(10.0, 10.0);
                // Use the same approximate size as used for sizing calculation
                let pill_half = egui::Vec2::new(45.0, 12.0);
                let min_allowed = content_rect.min + margin + pill_half;
                let max_allowed = content_rect.max - margin - pill_half;

                // Guard against inverted bounds (very small parent); skip in that case
                if min_allowed.x <= max_allowed.x && min_allowed.y <= max_allowed.y {
                    let clamped = egui::Pos2::new(
                        t.event_node_position.x.clamp(min_allowed.x, max_allowed.x),
                        t.event_node_position.y.clamp(min_allowed.y, max_allowed.y),
                    );
                    t.event_node_position = clamped;
                }
            }
        }
    }
}

/// Constrain a child node's position to stay within its parent's content area
/// 
/// Only constrains left and top edges - children can move right and down freely.
fn constrain_child_to_parent(
    child_entity: Entity,
    machine_data: &mut StateMachinePersistentData,
    q_child_of: &Query<&bevy_gearbox::StateChildOf>,
) {
    if let Ok(child_of) = q_child_of.get(child_entity) {
        if let Some(parent_node) = machine_data.nodes.get(&child_of.0) {
            if let NodeType::Parent(parent) = parent_node {
                if let Some(child_node) = machine_data.nodes.get(&child_entity) {
                    let child_rect = child_node.current_rect();
                    let content_rect = parent.content_rect();
                    let margin = egui::Vec2::new(10.0, 10.0);
                    let constrained_min = content_rect.min + margin;
                    
                    // Only constrain left and top edges
                    let constrained_pos = egui::Pos2::new(
                        child_rect.min.x.max(constrained_min.x), // Only constrain left edge
                        child_rect.min.y.max(constrained_min.y), // Only constrain top edge
                    );
                    
                    // Update child position if it was constrained
                    if constrained_pos != child_rect.min {
                        if let Some(child_node) = machine_data.nodes.get_mut(&child_entity) {
                            match child_node {
                                NodeType::Leaf(leaf_node) => {
                                    leaf_node.entity_node.position = constrained_pos;
                                }
                                NodeType::Parent(parent_node) => {
                                    parent_node.entity_node.position = constrained_pos;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// System to recalculate parent sizes based on their children's positions
/// 
/// Parents automatically expand to contain all their children, with a margin.
/// This uses a bottom-up approach, processing leaf nodes first, then their parents.
pub fn recalculate_parent_sizes(
    editor_state: Res<EditorState>,
    mut q_sm: Query<&mut StateMachinePersistentData, With<StateMachine>>,
    q_children: Query<&bevy_gearbox::StateChildren>,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
) {
    // Process each open machine separately
    for open_machine in &editor_state.open_machines {
        let Ok(mut machine_data) = q_sm.get_mut(open_machine.entity) else {
            continue;
        };
        
        let mut processed_entities = HashSet::new();
        
        // Preassign transition pills to a parent based on the higher endpoint in the hierarchy
        let mut transition_rects_by_parent: HashMap<Entity, Vec<egui::Rect>> = HashMap::new();
        for t in &machine_data.visual_transitions {
            let source_depth = hierarchy_depth(t.source_entity, &q_child_of);
            let target_depth = hierarchy_depth(t.target_entity, &q_child_of);
            let higher = if source_depth <= target_depth { t.source_entity } else { t.target_entity };
            let other = if higher == t.source_entity { t.target_entity } else { t.source_entity };
            // Exception: for direct Parent->Child connections, contain by the parent itself
            let is_direct_child = match q_child_of.get(other) { Ok(rel) => rel.0 == higher, Err(_) => false };
            // Otherwise, use the higher's parent if it exists; fallback to higher (root)
            let parent_for_pill = if is_direct_child { higher } else if let Ok(rel) = q_child_of.get(higher) { rel.0 } else { higher };
            // Approximate pill rect around the event node position
            let pill_size = egui::Vec2::new(90.0, 24.0);
            let pill_rect = egui::Rect::from_center_size(t.event_node_position, pill_size);
            transition_rects_by_parent.entry(parent_for_pill).or_default().push(pill_rect);
        }
        let mut made_progress = true;
        
        // Keep iterating until we've processed all entities
        while made_progress {
            made_progress = false;
            
            // Find parent entities whose children have all been processed
            let parent_entities: Vec<Entity> = machine_data.nodes.keys().copied().collect();
            
            for parent_entity in parent_entities {
                if processed_entities.contains(&parent_entity) {
                    continue;
                }
                
                if let Ok(children) = q_children.get(parent_entity) {
                    // Check if all children have been processed (or are leaf nodes)
                    let all_children_ready = children.into_iter().all(|&child| {
                        processed_entities.contains(&child) || 
                        !q_children.contains(child) // Leaf nodes (no children)
                    });
                    
                    if all_children_ready {
                        // Collect child rectangles using an immutable borrow
                        let mut child_rects: Vec<egui::Rect> = children.into_iter()
                            .filter_map(|&child| machine_data.nodes.get(&child))
                            .map(|node| node.current_rect())
                            .collect();
                        // Include transition pill rects assigned to this parent
                        if let Some(extra_rects) = transition_rects_by_parent.get(&parent_entity) {
                            child_rects.extend(extra_rects.iter().copied());
                        }
                        
                        // Now update the parent with a mutable borrow
                        if let Some(NodeType::Parent(parent_node)) = machine_data.nodes.get_mut(&parent_entity) {
                            parent_node.calculate_size_for_children(&child_rects);
                        }
                        
                        processed_entities.insert(parent_entity);
                        made_progress = true;
                    }
                } else {
                    // No children, mark as processed
                    processed_entities.insert(parent_entity);
                    made_progress = true;
                }
            }
        }
    }
}

fn hierarchy_depth(mut entity: Entity, q_child_of: &Query<&bevy_gearbox::StateChildOf>) -> usize {
    let mut depth = 0;
    while let Ok(rel) = q_child_of.get(entity) {
        depth += 1;
        entity = rel.0;
    }
    depth
}
