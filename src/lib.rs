//! Bevy Gearbox Editor
//! 
//! A visual editor for Bevy state machines with multi-window support,
//! hierarchical node editing, and real-time entity inspection.

use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_gearbox::{StateMachineRoot, InitialState};
use bevy_gearbox::transitions::{EdgeTarget, Source, TransitionKind};
use bevy_ecs::schedule::ScheduleLabel;

// Module declarations
mod editor_state;
mod hierarchy;
mod node_editor;
mod context_menu;
mod window_management;
mod entity_inspector;
mod machine_list;
pub mod components;
pub mod reflectable;

// Re-exports
pub use editor_state::*;

// Additional imports for transition creation
use bevy::ecs::reflect::ReflectComponent;
use bevy::prelude::AppTypeRegistry;

/// Schedule label for the editor window context
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EditorWindowContextPass;

/// Main plugin for the Bevy Gearbox Editor
pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        // Add required plugins
        app.add_plugins((
            EguiPlugin::default(),
            DefaultInspectorConfigPlugin,
        ));

        // Initialize resources
        app.init_resource::<EditorState>();

        // Register reflectable types for scene serialization
        app.register_type::<reflectable::ReflectableStateMachinePersistentData>()
            .register_type::<reflectable::ReflectableNode>()
            .register_type::<reflectable::ReflectableNodeType>()
            .register_type::<reflectable::ReflectableTransitionConnection>();

        // Add systems
        app.add_systems(Update, window_management::handle_editor_hotkeys)
            .add_systems(EditorWindowContextPass, editor_ui_system)
            .add_systems(EditorWindowContextPass, entity_inspector::entity_inspector_system)
            .add_systems(Update, (
                hierarchy::ensure_initial_states,
                node_editor::update_node_types,
                hierarchy::constrain_children_to_parents,
                hierarchy::recalculate_parent_sizes,
                update_transition_pulses,
            ).chain());

        // Add observers
        app.add_observer(context_menu::handle_context_menu_request)
            .add_observer(context_menu::handle_node_action)
            .add_observer(context_menu::handle_transition_context_menu_request)
            .add_observer(hierarchy::handle_parent_child_movement)
            .add_observer(handle_transition_creation_request)
            .add_observer(handle_create_transition)
            .add_observer(handle_save_state_machine)
            .add_observer(reflectable::on_add_reflectable_state_machine)
            .add_observer(handle_transition_pulse)
            .add_observer(handle_delete_transition)
            .add_observer(handle_delete_node);
    }
}

/// System to render the main editor UI
/// Only runs when an editor window exists
fn editor_ui_system(
    mut editor_context: Query<&mut EguiContext, (With<EditorWindow>, Without<bevy_egui::PrimaryEguiContext>)>,
    mut editor_state: ResMut<EditorState>,
    mut state_machines: Query<(Entity, Option<&Name>, Option<&mut StateMachinePersistentData>, Option<&mut StateMachineTransientData>), With<StateMachineRoot>>,
    machine_list_query: Query<(Entity, Option<&Name>), With<StateMachineRoot>>,
    all_entities: Query<(Entity, Option<&Name>, Option<&InitialState>)>,
    child_of_query: Query<&ChildOf>,
    children_query: Query<&Children>,
    active_query: Query<&bevy_gearbox::active::Active>,
    mut commands: Commands,
) {
    // Only run if there's an editor window
    if let Ok(mut egui_context) = editor_context.single_mut() {
        let ctx = egui_context.get_mut();
        
        if let Some(selected_machine) = editor_state.selected_machine {
            // Get the editor data for the selected machine
            if let Ok((_, _, persistent_data_opt, transient_data_opt)) = state_machines.get_mut(selected_machine) {
                // Ensure the machine has both components
                let mut persistent_data = if let Some(data) = persistent_data_opt {
                    data
                } else {
                    // Add the component if it doesn't exist
                    commands.entity(selected_machine).insert(StateMachinePersistentData::default());
                    // For this frame, create a temporary default
                    let mut temp_persistent = StateMachinePersistentData::default();
                    let mut temp_transient = StateMachineTransientData::default();
                    node_editor::show_machine_editor(
                        ctx,
                        &mut editor_state,
                        &mut temp_persistent,
                        &mut temp_transient,
                        &all_entities,
                        &child_of_query,
                        &children_query,
                        &active_query,
                        &mut commands,
                    );
                    return;
                };
                
                let mut transient_data = if let Some(data) = transient_data_opt {
                    data
                } else {
                    // Add the component if it doesn't exist
                    commands.entity(selected_machine).insert(StateMachineTransientData::default());
                    // For this frame, create a temporary default
                    let mut temp_persistent = StateMachinePersistentData::default();
                    let mut temp_transient = StateMachineTransientData::default();
                    node_editor::show_machine_editor(
                        ctx,
                        &mut editor_state,
                        &mut temp_persistent,
                        &mut temp_transient,
                        &all_entities,
                        &child_of_query,
                        &children_query,
                        &active_query,
                        &mut commands,
                    );
                    return;
                };
                
                // Show the node editor for the selected machine
                node_editor::show_machine_editor(
                    ctx,
                    &mut editor_state,
                    &mut persistent_data,
                    &mut transient_data,
                    &all_entities,
                    &child_of_query,
                    &children_query,
                    &active_query,
                    &mut commands,
                );
            }
        } else {
            // Show the machine list
            machine_list::show_machine_list(
                ctx,
                &mut editor_state,
                &machine_list_query,
                &mut commands,
            );
        }

        // Render context menu if requested
        context_menu::render_context_menu(ctx, &mut editor_state, &mut commands);
    }
}

/// Observer to handle transition creation requests
fn handle_transition_creation_request(
    trigger: Trigger<TransitionCreationRequested>,
    editor_state: Res<EditorState>,
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachineRoot>>,
    type_registry: Res<AppTypeRegistry>,
) {
    let event = trigger.event();
    
    // Get the currently selected state machine
    let Some(selected_machine) = editor_state.selected_machine else {
        return;
    };
    
    let Ok(mut transient_data) = state_machines.get_mut(selected_machine) else {
        return;
    };
    
    // Start the transition creation process
    transient_data.transition_creation.start_transition(event.source_entity);
    
    // Discover available event types for TransitionEdgeListener
    discover_transition_edge_listener_event_types(&mut transient_data.transition_creation, &type_registry);
}

/// Observer to handle transition creation with selected event type
fn handle_create_transition(
    trigger: Trigger<CreateTransition>,
    editor_state: Res<EditorState>,
    mut state_machines: Query<(&mut StateMachineTransientData, &mut StateMachinePersistentData), With<StateMachineRoot>>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Get the currently selected state machine
    let Some(selected_machine) = editor_state.selected_machine else {
        return;
    };
    
    let Ok((mut transient_data, mut persistent_data)) = state_machines.get_mut(selected_machine) else {
        return;
    };
    
    // Queue the transition creation as a command
    let source = event.source_entity;
    let target = event.target_entity;
    let event_type = event.event_type.clone();
    
    commands.queue(move |world: &mut World| {
        if let Err(e) = create_transition_edge_entity(world, source, target, &event_type) {
            warn!("Failed to create transition: {}", e);
        }
    });
    
    // Complete the transition creation process
    transient_data.transition_creation.complete();
    
    // Add the visual transition to the list for immediate display
    if let (Some(source_rect), Some(target_rect)) = (
        persistent_data.nodes.get(&event.source_entity).map(|n| n.current_rect()),
        persistent_data.nodes.get(&event.target_entity).map(|n| n.current_rect())
    ) {
        // Position event node at midpoint between source and target initially
        let initial_event_position = egui::Pos2::new(
            (source_rect.center().x + target_rect.center().x) / 2.0,
            (source_rect.center().y + target_rect.center().y) / 2.0,
        );
        
        persistent_data.visual_transitions.push(TransitionConnection {
            source_entity: event.source_entity,
            target_entity: event.target_entity,
            event_type: event.event_type.clone(),
            source_rect,
            target_rect,
            event_node_position: initial_event_position,
            is_dragging_event_node: false,
            event_node_offset: egui::Vec2::ZERO, // Initially at midpoint
        });
    }
}

/// Discover available TransitionEdgeListener event types from the type registry
fn discover_transition_edge_listener_event_types(
    transition_state: &mut TransitionCreationState,
    type_registry: &AppTypeRegistry,
) {
    let registry = type_registry.read();
    let mut event_types = Vec::new();
    
    for registration in registry.iter() {
        let type_path = registration.type_info().type_path();
        
        // Look for TransitionEdgeListener<EventType> patterns
        if let Some(start) = type_path.find("TransitionEdgeListener<") {
            if let Some(end) = type_path[start..].find('>') {
                let event_type = &type_path[start + 24..start + end]; // 24 = len("TransitionEdgeListener<")
                
                // Skip generic parameters and extract just the event type name
                if let Some(last_part) = event_type.split("::").last() {
                    if !event_types.contains(&last_part.to_string()) {
                        event_types.push(last_part.to_string());
                    }
                }
            }
        }
    }
    
    // Sort for consistent ordering
    event_types.sort();
    transition_state.available_event_types = event_types;
}

/// Create a transition edge entity using reflection (marker component on the edge)
fn create_transition_edge_entity(
    world: &mut World,
    source_entity: Entity,
    target_entity: Entity,
    event_type: &str,
) -> Result<(), String> {
    // Find the full TransitionListener type path and get reflection data
    let (type_path, reflect_component) = {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let mut transition_listener_type_path = None;
        for registration in registry.iter() {
            let type_path = registration.type_info().type_path();
            if type_path.contains("TransitionEdgeListener<") && type_path.contains(event_type) {
                transition_listener_type_path = Some(type_path.to_string());
                break;
            }
        }
        
        let Some(type_path) = transition_listener_type_path else {
            return Err(format!("TransitionEdgeListener<{}> not found in type registry", event_type));
        };
        
        // Get reflection data
        let Some(registration) = registry.get_with_type_path(&type_path) else { return Err(format!("Type registration not found for {}", type_path)); };
        let Some(reflect_component) = registration.data::<ReflectComponent>() else { return Err(format!("ReflectComponent not found for {}", type_path)); };
        (type_path, reflect_component.clone())
    };
    // Spawn edge entity
    let edge = world.spawn((Source(source_entity), EdgeTarget(target_entity), TransitionKind::External)).id();

    // Attach the event-specific listener via reflection to the edge entity (empty struct)
    {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let Some(registration) = registry.get_with_type_path(&type_path) else { return Err(format!("Type registration not found for {}", type_path)); };
        let type_info = registration.type_info();
        let mut dynamic_struct = bevy::reflect::DynamicStruct::default();
        if let bevy::reflect::TypeInfo::Struct(_) = type_info { dynamic_struct.set_represented_type(Some(type_info)); } else { return Err(format!("TransitionEdgeListener is not a struct type: {}", type_path)); }
        let mut entity_mut = world.entity_mut(edge);
        reflect_component.insert(&mut entity_mut, dynamic_struct.as_partial_reflect(), &registry);
    }

    Ok(())
}

/// Observer to handle save state machine requests
fn handle_save_state_machine(
    trigger: Trigger<SaveStateMachine>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Queue the save operation as a command to access the world
    let entity = event.entity;
    commands.queue(move |world: &mut World| {
        // Generate a filename based on the entity name
        let entity_name = if let Some(name) = world.get::<Name>(entity) {
            name.as_str().to_string()
        } else {
            format!("state_machine_{:?}", entity)
        };
        
        let filename = format!("assets/{}.scn.ron", entity_name.replace(" ", "_").to_lowercase());
        
        // Save the state machine
        match crate::reflectable::ReflectableStateMachinePersistentData::save_state_machine_to_file(
            world, 
            entity, 
            &filename
        ) {
            Ok(_) => {
                info!("✅ State machine '{}' saved to {}", entity_name, filename);
            }
            Err(e) => {
                error!("❌ Failed to save state machine '{}': {}", entity_name, e);
            }
        }
    });
}

/// Observer to handle transition deletion requests
fn handle_delete_transition(
    trigger: Trigger<DeleteTransition>,
    mut state_machines: Query<&mut StateMachinePersistentData, With<StateMachineRoot>>,
    child_of_query: Query<&bevy_gearbox::StateChildOf>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Find the state machine root that contains the source entity
    let root = child_of_query.root_ancestor(event.source_entity);
    
    
    // Remove the visual transition from persistent data
    if let Ok(mut persistent_data) = state_machines.get_mut(root) {
        let initial_count = persistent_data.visual_transitions.len();
        persistent_data.visual_transitions.retain(|transition| {
            !(transition.source_entity == event.source_entity &&
                transition.target_entity == event.target_entity &&
                transition.event_type == event.event_type)
        });
        let final_count = persistent_data.visual_transitions.len();
        
        if initial_count > final_count {
            info!("✅ Removed visual transition from {:?} to {:?} ({}) - {} transitions remaining", 
                    event.source_entity, event.target_entity, event.event_type, final_count);
        } else {
            warn!("⚠️ No matching visual transition found to remove: {:?} -> {:?} ({})", 
                    event.source_entity, event.target_entity, event.event_type);
        }
    } else {
        warn!("⚠️ Could not find state machine persistent data for root {:?}", root);
    }
    
    // Remove the corresponding edge entity and update Transitions on the source
    let source_entity = event.source_entity;
    let target_entity = event.target_entity;
    let event_type = event.event_type.clone();
    commands.queue(move |world: &mut World| {
        let registry = world.resource::<AppTypeRegistry>().clone();
        let registry = registry.read();
        // Find the TransitionEdgeListener<Event> type registration
        let mut reflect_listener: Option<bevy::ecs::reflect::ReflectComponent> = None;
        for registration in registry.iter() {
            let type_info = registration.type_info();
            let type_name = type_info.type_path_table().short_path();
            if type_name.starts_with("TransitionEdgeListener<") && type_name.contains(&event_type) {
                reflect_listener = registration.data::<ReflectComponent>().cloned();
                break;
            }
        }
        if reflect_listener.is_none() {
            warn!("Could not resolve TransitionEdgeListener<{}> for deletion", event_type);
            return;
        }
        let reflect_listener = reflect_listener.unwrap();

        // Search for an edge with Source, EdgeTarget, and that listener
        let mut to_remove: Option<Entity> = None;
        let mut q = world.query::<(Entity, &Source, &EdgeTarget)>();
        for (edge, src, tgt) in q.iter(world) {
            if src.0 == source_entity && tgt.0 == target_entity {
                if reflect_listener.reflect(world.entity(edge)).is_some() {
                    to_remove = Some(edge);
                    break;
                }
            }
        }
        if let Some(edge) = to_remove {
            world.entity_mut(edge).despawn();
            info!("✅ Removed edge {:?} for {:?} -> {:?} ({})", edge, source_entity, target_entity, event_type);
        } else {
            warn!("⚠️ No matching edge found to remove: {:?} -> {:?} ({})", source_entity, target_entity, event_type);
        }
    });
}

/// Observer to handle transition events and create pulse animations
fn handle_transition_pulse(
    trigger: Trigger<bevy_gearbox::Transition>,
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachineRoot>>,
    edge_target_query: Query<&EdgeTarget>,
) {
    let event = trigger.event();
    let target_entity = trigger.target(); // This is the state machine root
    
    // Add pulse to the state machine's transient data
    if let Ok(mut transient_data) = state_machines.get_mut(target_entity) {
        if let Ok(edge_target) = edge_target_query.get(event.edge) {
            transient_data.transition_pulses.push(TransitionPulse::new(event.source, edge_target.0));
        }
    }
}

/// System to update transition pulse timers and remove expired pulses
fn update_transition_pulses(
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachineRoot>>,
    time: Res<Time>,
) {
    for mut transient_data in state_machines.iter_mut() {
        // Update all pulse timers
        for pulse in transient_data.transition_pulses.iter_mut() {
            pulse.timer.tick(time.delta());
        }
        
        // Remove finished pulses
        transient_data.transition_pulses.retain(|pulse| !pulse.timer.finished());
    }
}

/// Observer to handle node deletion with all edge cases
fn handle_delete_node(
    trigger: Trigger<DeleteNode>,
    mut state_machines: Query<&mut StateMachinePersistentData, With<StateMachineRoot>>,
    child_of_query: Query<&ChildOf>,
    state_child_of_query: Query<&bevy_gearbox::StateChildOf>,
    children_query: Query<&Children>,
    initial_state_query: Query<&InitialState>,
    mut commands: Commands,
) {
    let event = trigger.event();
    let entity_to_delete = event.entity;
    
    // Find the state machine root that contains this entity
    let root = state_child_of_query.root_ancestor(entity_to_delete);
    
    // Don't allow deleting the root state machine itself
    if entity_to_delete == root {
        warn!("⚠️ Cannot delete the root state machine entity {:?}", entity_to_delete);
        return;
    }
    
    let Ok(mut persistent_data) = state_machines.get_mut(root) else {
        warn!("⚠️ Could not find persistent data for state machine root {:?}", root);
        return;
    };
    
    // Collect all entities to be deleted (the entity and all its descendants)
    let mut entities_to_delete = Vec::new();
    collect_entities_recursively(entity_to_delete, &children_query, &mut entities_to_delete);
    
    // Step 1: Remove all transitions involving any of the entities to be deleted
    remove_transitions_for_entities(&entities_to_delete, &mut persistent_data, &mut commands);
    
    // Step 2: Handle parent state changes if needed
    if let Some(child_of) = child_of_query.get(entity_to_delete).ok() {
        let parent_entity = child_of.0;
        handle_parent_state_changes(parent_entity, entity_to_delete, &entities_to_delete, 
                                   &children_query, &initial_state_query, &mut commands);
    }
    
    // Step 3: Remove visual data for all deleted entities
    for entity in &entities_to_delete {
        persistent_data.nodes.remove(entity);
    }
    
    // Step 4: Actually delete the entities
    for entity in entities_to_delete {
        commands.entity(entity).despawn();
    }
}

/// Recursively collect all entities in a hierarchy
fn collect_entities_recursively(
    entity: Entity,
    children_query: &Query<&Children>,
    entities: &mut Vec<Entity>,
) {
    entities.push(entity);
    
    if let Ok(children) = children_query.get(entity) {
        for &child in children {
            collect_entities_recursively(child, children_query, entities);
        }
    }
}

/// Remove all transitions (both incoming and outgoing) for the given entities
fn remove_transitions_for_entities(
    entities_to_delete: &[Entity],
    persistent_data: &mut StateMachinePersistentData,
    commands: &mut Commands,
) {
    // Collect transitions that need to be deleted (involving any of the entities to be deleted)
    let transitions_to_delete: Vec<_> = persistent_data.visual_transitions
        .iter()
        .filter(|transition| {
            entities_to_delete.contains(&transition.source_entity) || 
            entities_to_delete.contains(&transition.target_entity)
        })
        .cloned()
        .collect();
    
    // Use our existing DeleteTransition event system to handle component removal
    for transition in transitions_to_delete {
        // Only remove the component if the source entity is not being deleted
        // (if it's being deleted, the component will be removed automatically)
        if !entities_to_delete.contains(&transition.source_entity) {
            commands.trigger(DeleteTransition {
                source_entity: transition.source_entity,
                target_entity: transition.target_entity,
                event_type: transition.event_type.clone(),
            });
        }
    }
    
    // Remove visual transitions involving deleted entities
    persistent_data.visual_transitions.retain(|transition| {
        let should_keep = !entities_to_delete.contains(&transition.source_entity) && 
                         !entities_to_delete.contains(&transition.target_entity);
        should_keep
    });
}



/// Handle changes to parent states when children are deleted
fn handle_parent_state_changes(
    parent_entity: Entity,
    _deleted_entity: Entity,
    all_deleted_entities: &[Entity],
    children_query: &Query<&Children>,
    initial_state_query: &Query<&InitialState>,
    commands: &mut Commands,
) {
    // Get remaining children after deletion
    let remaining_children: Vec<Entity> = if let Ok(children) = children_query.get(parent_entity) {
        let mut remaining = Vec::new();
        for &child in children {
            if !all_deleted_entities.contains(&child) {
                remaining.push(child);
            }
        }
        remaining
    } else {
        Vec::new()
    };
    
    // Case 1: No children left - convert parent to leaf by removing InitialState
    if remaining_children.is_empty() {
        commands.entity(parent_entity).remove::<InitialState>();
        return;
    }
    
    // Case 2: Check if we deleted the initial state
    let current_initial_state = initial_state_query.get(parent_entity).ok().map(|is| is.0);
    
    if let Some(initial_state_entity) = current_initial_state {
        if all_deleted_entities.contains(&initial_state_entity) {
            // The initial state was deleted, assign a new one
            let new_initial_state = remaining_children[0]; // Pick the first remaining child
            commands.entity(parent_entity).insert(InitialState(new_initial_state));
        }
    }
}