//! Context menu system for node interactions
//! 
//! This module handles:
//! - Right-click context menu rendering and interaction
//! - Node action processing (Inspect, Add Child)
//! - Entity creation and hierarchy management

use bevy::prelude::*;
use bevy_gearbox::StateMachineRoot;
use bevy_egui::egui;

use crate::editor_state::{EditorState, StateMachineEditorData, NodeAction, NodeActionTriggered, NodeContextMenuRequested};
use crate::components::{NodeType, LeafNode};

/// Observer to handle context menu requests
/// 
/// Renders a context menu at the requested position with available actions.
pub fn handle_context_menu_request(
    trigger: Trigger<NodeContextMenuRequested>,
    mut editor_state: ResMut<EditorState>,
) {
    let event = trigger.event();
    
    // Store the context menu request in editor state for rendering
    editor_state.context_menu_entity = Some(event.entity);
    editor_state.context_menu_position = Some(event.position);
}

/// Observer to handle node actions triggered from context menus
/// 
/// Processes actions like Inspect and Add Child, performing the necessary
/// entity creation and component management.
pub fn handle_node_action(
    trigger: Trigger<NodeActionTriggered>,
    mut commands: Commands,
    mut editor_state: ResMut<EditorState>,
    mut state_machines: Query<&mut StateMachineEditorData, With<StateMachineRoot>>,
    name_query: Query<&Name>,
) {
    let event = trigger.event();
    
    // Get the currently selected state machine
    let Some(selected_machine) = editor_state.selected_machine else {
        return;
    };
    
    let Ok(mut machine_data) = state_machines.get_mut(selected_machine) else {
        return;
    };
    
    match event.action {
        NodeAction::Inspect => {
            // Set the entity to be inspected
            editor_state.inspected_entity = Some(event.entity);
        }
        NodeAction::AddChild => {
            // Create a new child entity
            let child_entity = commands.spawn((
                ChildOf(event.entity),
                Name::new("New State"),
            )).id();
        
            // Add the child as a leaf node in the editor at an offset position
            if let Some(parent_node) = machine_data.nodes.get(&event.entity) {
                let parent_pos = match parent_node {
                    NodeType::Leaf(leaf_node) => leaf_node.entity_node.position,
                    NodeType::Parent(parent_node) => parent_node.entity_node.position,
                };
            
                // Position the child at an offset from the parent
                let child_pos = parent_pos + egui::Vec2::new(50.0, 50.0);
                let leaf_node = LeafNode::new(child_pos);
                machine_data.nodes.insert(child_entity, NodeType::Leaf(leaf_node));
            }
        }
        NodeAction::Rename => {
            let entity_name = name_query.get(event.entity).unwrap().to_string();
            machine_data.text_editing.start_editing(event.entity, &entity_name);
        }
    }
}

/// Render context menu UI if one is requested
/// 
/// This function should be called during UI rendering to display context menus.
pub fn render_context_menu(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    commands: &mut Commands,
) {
    if let (Some(entity), Some(position)) = (editor_state.context_menu_entity, editor_state.context_menu_position) {
        let menu_id = egui::Id::new("context_menu").with(entity);
        
        egui::Area::new(menu_id)
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_min_width(120.0);
                        
                        if ui.button("Inspect").clicked() {
                            commands.trigger(NodeActionTriggered {
                                entity,
                                action: NodeAction::Inspect,
                            });
                            editor_state.context_menu_entity = None;
                            editor_state.context_menu_position = None;
                            ui.close_menu();
                        }
                        
                        if ui.button("Add child").clicked() {
                            commands.trigger(NodeActionTriggered {
                                entity,
                                action: NodeAction::AddChild,
                            });
                            editor_state.context_menu_entity = None;
                            editor_state.context_menu_position = None;
                            ui.close_menu();
                        }
                        
                        if ui.button("Rename").clicked() {
                            commands.trigger(NodeActionTriggered {
                                entity,
                                action: NodeAction::Rename,
                            });
                            editor_state.context_menu_entity = None;
                            editor_state.context_menu_position = None;
                            ui.close_menu();
                        }
                    });
            });
        
        // Close context menu if clicked elsewhere
        if ctx.input(|i| i.pointer.any_click()) {
            let pointer_pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
            let menu_rect = egui::Rect::from_min_size(position, egui::Vec2::new(120.0, 60.0));
            
            if !menu_rect.contains(pointer_pos) {
                editor_state.context_menu_entity = None;
                editor_state.context_menu_position = None;
            }
        }
    }
}

