//! Context menu system for node interactions
//! 
//! This module handles:
//! - Right-click context menu rendering and interaction
//! - Node action processing (Inspect, Add Child)
//! - Entity creation and hierarchy management

use bevy::prelude::*;
use bevy_gearbox::{StateMachineRoot};
use bevy_egui::egui;

use crate::editor_state::{EditorState, NodeAction, NodeActionTriggered, NodeContextMenuRequested, TransitionContextMenuRequested, DeleteTransition, DeleteNode, SetInitialStateRequested};
use crate::components::{NodeType, LeafNode};
use crate::{StateMachinePersistentData, StateMachineTransientData};
use crate::node_kind::{AddChildClicked, MakeParallelClicked, MakeParentClicked, MakeLeafClicked};

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

/// Observer to handle transition context menu requests
pub fn handle_transition_context_menu_request(
    trigger: Trigger<TransitionContextMenuRequested>,
    mut editor_state: ResMut<EditorState>,
) {
    let event = trigger.event();
    
    // Store the transition context menu request in editor state for rendering
    editor_state.transition_context_menu = Some((event.source_entity, event.target_entity, event.event_type.clone()));
    editor_state.transition_context_menu_position = Some(event.position);
}

/// Observer to handle node actions triggered from context menus
/// 
/// Processes actions like Inspect and Add Child, performing the necessary
/// entity creation and component management.
pub fn handle_node_action(
    trigger: Trigger<NodeActionTriggered>,
    mut commands: Commands,
    mut editor_state: ResMut<EditorState>,
    mut state_machines: Query<(&mut StateMachinePersistentData, &mut StateMachineTransientData), With<StateMachineRoot>>,
    name_query: Query<&Name>,
) {
    let event = trigger.event();
    
    // Get the currently selected state machine
    let Some(selected_machine) = editor_state.selected_machine else {
        return;
    };
    
    let Ok((mut persistent_data, mut transient_data)) = state_machines.get_mut(selected_machine) else {
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
                bevy_gearbox::StateChildOf(event.entity),
                Name::new("New State"),
            )).id();
        
            // Add the child as a leaf node in the editor at an offset position
            if let Some(parent_node) = persistent_data.nodes.get(&event.entity) {
                let parent_pos = match parent_node {
                    NodeType::Leaf(leaf_node) => leaf_node.entity_node.position,
                    NodeType::Parent(parent_node) => parent_node.entity_node.position,
                };
            
                // Position the child at an offset from the parent
                let child_pos = parent_pos + egui::Vec2::new(50.0, 50.0);
                let leaf_node = LeafNode::new(child_pos);
                persistent_data.nodes.insert(child_entity, NodeType::Leaf(leaf_node));
            }

            // Notify NodeKind machine for this parent
            let parent_entity = event.entity;
            if let Ok(transient) = state_machines.get_mut(selected_machine).map(|(_, t)| t) {
                if let Some(&nk_root) = transient.node_kind_roots.get(&parent_entity) {
                    commands.trigger_targets(AddChildClicked, nk_root);
                    commands.trigger_targets(crate::node_kind::ChildAdded, nk_root);
                }
            }
        }
        NodeAction::Rename => {
            let entity_name = name_query.get(event.entity).unwrap().to_string();
            transient_data.text_editing.start_editing(event.entity, &entity_name);
        }
        NodeAction::MakeParallel => {
            // Notify NodeKind machine to handle Parallel transition
            let state_entity = event.entity;
            if let Ok(transient) = state_machines.get_mut(selected_machine).map(|(_, t)| t) {
                if let Some(&nk_root) = transient.node_kind_roots.get(&state_entity) {
                    commands.trigger_targets(MakeParallelClicked, nk_root);
                }
            }
        }
        NodeAction::MakeParent => {
            // Ask NK to become Parent from any current kind
            let state_entity = event.entity;
            if let Ok(transient) = state_machines.get_mut(selected_machine).map(|(_, t)| t) {
                if let Some(&nk_root) = transient.node_kind_roots.get(&state_entity) {
                    commands.trigger_targets(MakeParentClicked, nk_root);
                }
            }
        }
        NodeAction::MakeLeaf => {
            // Ask NK to become Leaf from any current kind
            let state_entity = event.entity;
            if let Ok(transient) = state_machines.get_mut(selected_machine).map(|(_, t)| t) {
                if let Some(&nk_root) = transient.node_kind_roots.get(&state_entity) {
                    commands.trigger_targets(MakeLeafClicked, nk_root);
                }
            }
        }
        NodeAction::SetAsInitialState => {
            // Request parent InitialState update via event; handled centrally
            let child_entity = event.entity;
            commands.trigger(SetInitialStateRequested { child_entity });
        }
        NodeAction::Delete => {
            // Trigger the delete node event
            commands.trigger(DeleteNode {
                entity: event.entity,
            });
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
    all_entities: &Query<(Entity, Option<&Name>, Option<&bevy_gearbox::InitialState>)>,
    child_of_query: &Query<&bevy_gearbox::StateChildOf>,
    parallel_query: &Query<&bevy_gearbox::Parallel>,
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

                        if ui.button("Rename").clicked() {
                            commands.trigger(NodeActionTriggered {
                                entity,
                                action: NodeAction::Rename,
                            });
                            editor_state.context_menu_entity = None;
                            editor_state.context_menu_position = None;
                            ui.close_menu();
                        }

                        // Determine type of node (Leaf/Parent/Parallel)
                        let is_parent = all_entities.get(entity).ok().and_then(|(_,_,init)| init.map(|_|())).is_some();
                        let is_parallel = parallel_query.get(entity).is_ok();
                        let is_leaf = !is_parent && !is_parallel;

                        // Common options already added: Inspect, Rename

                        // Leaf-specific options: Make Parallel, Make Parent
                        if is_leaf {
                            if ui.button("Make Parallel").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeParallel });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                            if ui.button("Make Parent").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeParent });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                        }

                        // Parent-specific: Make Parallel, Make Leaf, Add child
                        if is_parent {
                            if ui.button("Make Parallel").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeParallel });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                            if ui.button("Make Leaf").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeLeaf });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                            if ui.button("Add child").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::AddChild });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                        }

                        // Parallel-specific: Make Leaf, Make Parent, Add child
                        if is_parallel {
                            if ui.button("Make Leaf").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeLeaf });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                            if ui.button("Make Parent").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::MakeParent });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                            if ui.button("Add child").clicked() {
                                commands.trigger(NodeActionTriggered { entity, action: NodeAction::AddChild });
                                editor_state.context_menu_entity = None;
                                editor_state.context_menu_position = None;
                                ui.close_menu();
                            }
                        }

                        // Child of a parent: Set as Initial State
                        if let Ok(child_of) = child_of_query.get(entity) {
                            let parent_has_initial = all_entities
                                .get(child_of.0)
                                .ok()
                                .and_then(|(_,_,init)| init.map(|_| ()))
                                .is_some();
                            if parent_has_initial {
                                if ui.button("Set as Initial State").clicked() {
                                    commands.trigger(NodeActionTriggered { entity, action: NodeAction::SetAsInitialState });
                                    editor_state.context_menu_entity = None;
                                    editor_state.context_menu_position = None;
                                    ui.close_menu();
                                }
                            }
                        }
                        
                        if ui.button("🗑 Delete Node").clicked() {
                            commands.trigger(NodeActionTriggered {
                                entity,
                                action: NodeAction::Delete,
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
    
    // Render transition context menu if requested
    if let (Some((source, target, event_type)), Some(position)) = (
        editor_state.transition_context_menu.clone(),
        editor_state.transition_context_menu_position
    ) {
        let menu_id = egui::Id::new("transition_context_menu").with((source, target));
        
        egui::Area::new(menu_id)
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .show(ui, |ui| {
                        ui.set_min_width(120.0);
                        
                        if ui.button("Inspect").clicked() {
                            // Resolve using the stored edge_entity in the visual model
                            let event_type_clone = event_type.clone();
                            commands.queue(move |world: &mut World| {
                                info!(
                                    "Inspect requested (direct): source={:?} target={:?} event_type={}",
                                    source, target, event_type_clone
                                );
                                let Some(editor_state) = world.get_resource::<EditorState>() else {
                                    warn!("Inspect: EditorState missing");
                                    return;
                                };
                                let Some(root) = editor_state.selected_machine else {
                                    warn!("Inspect: no selected machine");
                                    return;
                                };
                                let Some(persistent) = world.get::<StateMachinePersistentData>(root) else {
                                    warn!("Inspect: missing StateMachinePersistentData on root {:?}", root);
                                    return;
                                };
                                if let Some(conn) = persistent.visual_transitions.iter().find(|t| t.source_entity == source && t.target_entity == target && t.event_type == event_type_clone) {
                                    let e = conn.edge_entity;
                                    if world.entities().contains(e) {
                                        if let Some(mut es) = world.get_resource_mut::<EditorState>() {
                                            es.inspected_entity = Some(e);
                                        }
                                        info!("Inspect: set inspected_entity to edge {:?}", e);
                                    } else {
                                        warn!("Inspect: stored edge_entity {:?} no longer exists", e);
                                    }
                                } else {
                                    warn!("Inspect: no matching TransitionConnection found in visual_transitions");
                                }
                            });
                            editor_state.transition_context_menu = None;
                            editor_state.transition_context_menu_position = None;
                            ui.close_menu();
                        }
                        
                        if ui.button("🗑 Delete Transition").clicked() {
                            commands.trigger(DeleteTransition {
                                source_entity: source,
                                target_entity: target,
                                event_type: event_type.clone(),
                            });
                            editor_state.transition_context_menu = None;
                            editor_state.transition_context_menu_position = None;
                            ui.close_menu();
                        }
                    });
            });
        
        // Close transition context menu if clicked elsewhere
        if ctx.input(|i| i.pointer.any_click()) {
            let pointer_pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
            let menu_rect = egui::Rect::from_min_size(position, egui::Vec2::new(120.0, 40.0));
            
            if !menu_rect.contains(pointer_pos) {
                editor_state.transition_context_menu = None;
                editor_state.transition_context_menu_position = None;
            }
        }
    }
}

