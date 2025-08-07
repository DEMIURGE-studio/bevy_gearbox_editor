//! Hierarchy management for parent-child relationships in the node editor
//! 
//! This module handles:
//! - Recursive parent-child movement when nodes are dragged
//! - Constraining children to stay within parent bounds
//! - Auto-resizing parents to fit their children
//! - Managing InitialState components for parent entities

use bevy::prelude::*;
use bevy_gearbox::InitialState;
use std::collections::HashSet;

use crate::editor_state::{EditorState, NodeDragged};
use crate::components::NodeType;

/// Observer to handle parent-child movement when nodes are dragged
/// 
/// This observer recursively moves all children when a parent is dragged,
/// maintaining relative positions throughout the hierarchy.
pub fn handle_parent_child_movement(
    trigger: Trigger<NodeDragged>,
    mut editor_state: ResMut<EditorState>,
    child_of_query: Query<(Entity, &ChildOf)>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Find all entities that are children of the dragged entity
    let mut children_to_move = Vec::new();
    
    for (child_entity, child_of) in child_of_query.iter() {
        if child_of.0 == event.entity {
            children_to_move.push(child_entity);
        }
    }
    
    // Move all children by the same delta as the parent
    for child_entity in children_to_move {
        if let Some(child_node) = editor_state.nodes.get_mut(&child_entity) {
            let old_position = match child_node {
                NodeType::Leaf(leaf_node) => leaf_node.entity_node.position,
                NodeType::Parent(parent_node) => parent_node.entity_node.position,
            };
            
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

/// System to ensure all parent entities have InitialState components
/// 
/// Any entity with children but no InitialState will get one pointing to its first child.
pub fn ensure_initial_states(
    mut commands: Commands,
    parents_without_initial: Query<(Entity, &Children), Without<InitialState>>,
) {
    for (parent_entity, children) in parents_without_initial.iter() {
        if let Some(&first_child) = children.first() {
            commands.entity(parent_entity).insert(InitialState(first_child));
        }
    }
}

/// System to constrain child nodes to stay within their parent's bounds
/// 
/// Children are prevented from moving left or up outside their parent,
/// but can move right and down freely (which will trigger parent expansion).
pub fn constrain_children_to_parents(
    mut editor_state: ResMut<EditorState>,
    child_of_query: Query<(Entity, &ChildOf)>,
) {
    let child_entities: Vec<Entity> = child_of_query.iter()
        .filter_map(|(entity, _)| {
            if editor_state.nodes.contains_key(&entity) { Some(entity) } else { None }
        })
        .collect();
    
    for child_entity in child_entities {
        constrain_child_to_parent(child_entity, &mut editor_state, &child_of_query);
    }
}

/// Constrain a child node's position to stay within its parent's content area
/// 
/// Only constrains left and top edges - children can move right and down freely.
fn constrain_child_to_parent(
    child_entity: Entity,
    editor_state: &mut EditorState,
    child_of_query: &Query<(Entity, &ChildOf)>,
) {
    if let Ok((_, child_of)) = child_of_query.get(child_entity) {
        if let Some(parent_node) = editor_state.nodes.get(&child_of.0) {
            if let NodeType::Parent(parent) = parent_node {
                if let Some(child_node) = editor_state.nodes.get(&child_entity) {
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
                        if let Some(child_node) = editor_state.nodes.get_mut(&child_entity) {
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
    mut editor_state: ResMut<EditorState>,
    children_query: Query<&Children>,
) {
    let mut processed_entities = HashSet::new();
    let mut made_progress = true;
    
    // Keep iterating until we've processed all entities
    while made_progress {
        made_progress = false;
        
        // Find parent entities whose children have all been processed
        let parent_entities: Vec<Entity> = editor_state.nodes.keys().copied().collect();
        
        for parent_entity in parent_entities {
            if processed_entities.contains(&parent_entity) {
                continue;
            }
            
            if let Ok(children) = children_query.get(parent_entity) {
                // Check if all children have been processed (or are leaf nodes)
                let all_children_ready = children.iter().all(|child| {
                    processed_entities.contains(&child) || 
                    !children_query.contains(child) // Leaf nodes (no children)
                });
                
                if all_children_ready {
                    // Collect child rectangles using an immutable borrow
                    let child_rects: Vec<egui::Rect> = children.iter()
                        .filter_map(|child| editor_state.nodes.get(&child))
                        .map(|node| node.current_rect())
                        .collect();
                    
                    // Now update the parent with a mutable borrow
                    if let Some(NodeType::Parent(parent_node)) = editor_state.nodes.get_mut(&parent_entity) {
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
