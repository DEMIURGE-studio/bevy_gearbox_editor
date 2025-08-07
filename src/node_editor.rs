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

use crate::editor_state::{EditorState, NodeDragged, NodeContextMenuRequested, RenderItem, get_entity_name, should_get_selection_boost};
use crate::components::{NodeType, LeafNode, ParentNode};

/// System to update node types based on entity hierarchy
/// 
/// Converts leaf nodes to parent nodes when they gain children,
/// and parent nodes back to leaf nodes when they lose all children.
pub fn update_node_types(
    mut editor_state: ResMut<EditorState>,
    root_query: Query<Entity, With<StateMachineRoot>>,
    parent_query: Query<Entity, With<InitialState>>,
    leaf_query: Query<Entity, Without<InitialState>>,
    children_query: Query<&Children>,
) {
    if let Some(selected_root) = editor_state.selected_machine {
        if !root_query.contains(selected_root) { 
            return; 
        }
        
        // Get all entities in the selected state machine's hierarchy
        let mut descendants: Vec<Entity> = children_query
            .iter_descendants_depth_first(selected_root)
            .collect();
        descendants.insert(0, selected_root); // Include the root
        
        // Process each entity in the hierarchy
        for &entity in &descendants {
            if parent_query.contains(entity) {
                // Entity should be a parent node
                match editor_state.nodes.get(&entity) {
                    Some(NodeType::Parent(_)) => {
                        // Already a parent node, no change needed
                    }
                    Some(NodeType::Leaf(leaf_node)) => {
                        // Convert leaf to parent
                        let parent_node = ParentNode::new(leaf_node.entity_node.position);
                        editor_state.nodes.insert(entity, NodeType::Parent(parent_node));
                    }
                    None => {
                        // Create new parent node
                        let parent_node = ParentNode::new(egui::Pos2::new(200.0, 100.0));
                        editor_state.nodes.insert(entity, NodeType::Parent(parent_node));
                    }
                }
            } else if leaf_query.contains(entity) {
                // Entity should be a leaf node
                match editor_state.nodes.get(&entity) {
                    Some(NodeType::Leaf(_)) => {
                        // Already a leaf node, no change needed
                    }
                    Some(NodeType::Parent(parent_node)) => {
                        // Convert parent to leaf
                        let leaf_node = LeafNode::new(parent_node.entity_node.position);
                        editor_state.nodes.insert(entity, NodeType::Leaf(leaf_node));
                    }
                    None => {
                        // Create new leaf node
                        let leaf_node = LeafNode::new(egui::Pos2::new(100.0, 100.0));
                        editor_state.nodes.insert(entity, NodeType::Leaf(leaf_node));
                    }
                }
            }
        }
        
        // Remove nodes that are no longer part of the active hierarchy
        let valid_entities: HashSet<Entity> = descendants.into_iter().collect();
        editor_state.nodes.retain(|entity, _| valid_entities.contains(entity));
    }
}

/// Render the node editor interface for a selected state machine
pub fn show_machine_editor(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    all_entities: &Query<(Entity, Option<&Name>)>,
    child_of_query: &Query<&ChildOf>,
    children_query: &Query<&Children>,
    commands: &mut Commands,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        // Add back button at the top
        ui.horizontal(|ui| {
            if ui.button("← Back to Machine List").clicked() {
                editor_state.selected_machine = None;
                editor_state.selected_node = None;
                // Clear nodes when going back
                editor_state.nodes.clear();
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
                if let Some(_node) = editor_state.nodes.get(entity) {
                    let base_z_order = hierarchy_index as i32 * 10;
                    let selection_boost = if should_get_selection_boost(*entity, editor_state.selected_node, child_of_query) { 
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
                
                if let Some(node) = editor_state.nodes.get_mut(&entity) {
                    let response = match node {
                        NodeType::Leaf(leaf_node) => {
                            leaf_node.show(ui, &entity_name, Some(&format!("{:?}", entity)))
                        }
                        NodeType::Parent(parent_node) => {
                            parent_node.show(ui, &entity_name, Some(&format!("{:?}", entity)))
                        }
                    };
                    
                    // Handle selection
                    if response.clicked {
                        editor_state.selected_node = Some(entity);
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
        } else {
            ui.label("No state machine selected");
        }
    });
}
