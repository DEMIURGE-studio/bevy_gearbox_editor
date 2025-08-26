//! Bevy Gearbox Editor
//! 
//! A visual editor for Bevy state machines with multi-window support,
//! hierarchical node editing, and real-time entity inspection.

use bevy::prelude::*;
use bevy_egui::EguiContext;
use bevy_gearbox::{StateMachine, InitialState};
use bevy_gearbox::transitions::{Target, Source, EdgeKind, AlwaysEdge};
use bevy_ecs::schedule::ScheduleLabel;
use bevy_gearbox::transitions::TransitionEventAppExt;

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
pub mod node_kind;

// Re-exports
pub use editor_state::*;

// Import new events - these are also re-exported by the glob import above
// but we need them explicitly for the observers

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
        // app.add_plugins((
        //     EguiPlugin::default(),
        //     DefaultInspectorConfigPlugin,
        // ));

        // Initialize resources
        app.init_resource::<EditorState>();
        // NodeKind index is now transient per-machine; no global resource

        // Register reflectable types for scene serialization
        app.register_type::<reflectable::ReflectableStateMachinePersistentData>()
            .register_type::<reflectable::ReflectableNode>()
            .register_type::<reflectable::ReflectableNodeType>()
            .register_type::<reflectable::ReflectableTransitionConnection>();

        // Add systems
        app.add_systems(Update, window_management::handle_editor_hotkeys)
            .add_observer(window_management::cleanup_editor_window)
            .add_systems(EditorWindowContextPass, editor_ui_system)
            .add_systems(EditorWindowContextPass, entity_inspector::entity_inspector_system)
            .add_systems(Update, (
                node_editor::update_node_types,
                hierarchy::constrain_children_to_parents,
                hierarchy::recalculate_parent_sizes,
                update_transition_pulses,
                update_node_pulses,
                reflectable::sync_reflectable_on_persistent_change,
            ).chain())
            // Derive visual transitions from ECS edges each frame (preserving offsets)
            .add_systems(Update, sync_edge_visuals_from_ecs)
            // NodeKind dogfood state machines (per selected machine)
            .add_systems(Update, node_kind::sync_node_kind_machines)
            // NodeKind event listeners
            .add_transition_event::<node_kind::AddChildClicked>()
            .add_transition_event::<node_kind::ChildAdded>()
            .add_transition_event::<node_kind::AllChildrenRemoved>()
            .add_transition_event::<node_kind::MakeParallelClicked>()
            .add_transition_event::<node_kind::MakeParentClicked>()
            .add_transition_event::<node_kind::MakeLeafClicked>()
            .add_observer(node_kind::on_enter_nodekind_state_parallel)
            .add_observer(node_kind::on_enter_nodekind_state_parent)
            .add_observer(node_kind::on_enter_nodekind_state_parent_via_make_parent)
            .add_observer(node_kind::on_enter_nodekind_state_leaf)
            .add_observer(node_kind::on_remove_state_children);

        // Handle requests to set InitialState centrally
        app.add_observer(handle_set_initial_state_request);

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
            .add_observer(handle_node_enter_pulse)
            .add_observer(handle_delete_transition)
            .add_observer(handle_delete_transition_by_edge)
            .add_observer(handle_delete_node)
            .add_observer(handle_background_context_menu_request)
            .add_observer(handle_open_machine_request)
            .add_observer(handle_close_machine_request)
            .add_observer(handle_view_related);
    }
}

/// System to render the main editor UI
/// Only runs when an editor window exists
fn editor_ui_system(
    mut editor_context: Query<&mut EguiContext, (With<EditorWindow>, Without<bevy_egui::PrimaryEguiContext>)>,
    mut editor_state: ResMut<EditorState>,
    mut state_machines: Query<(Entity, Option<&Name>, Option<&mut StateMachinePersistentData>, Option<&mut StateMachineTransientData>), With<StateMachine>>,
    machine_list_query: Query<(Entity, Option<&Name>), With<StateMachine>>,
    all_entities: Query<(Entity, Option<&Name>, Option<&InitialState>)>,
    child_of_query: Query<&bevy_gearbox::StateChildOf>,
    children_query: Query<&bevy_gearbox::StateChildren>,
    active_query: Query<&bevy_gearbox::active::Active>,
    parallel_query: Query<&bevy_gearbox::Parallel>,
    mut commands: Commands,
) {
    // Only run if there's an editor window
    if let Ok(mut egui_context) = editor_context.single_mut() {
        let ctx = egui_context.get_mut();
        
        egui::CentralPanel::default().show(ctx, |ui| {
            // Handle background interactions first
            handle_background_interactions(ui, &mut editor_state, &mut commands);
            
            // Render each open machine directly on the canvas
            for open_machine in &editor_state.open_machines.clone() {
                if let Ok((_, _, persistent_data_opt, transient_data_opt)) = state_machines.get_mut(open_machine.entity) {
                    // Ensure the machine has both components
                    if persistent_data_opt.is_none() {
                        commands.entity(open_machine.entity).insert(StateMachinePersistentData::default());
                        continue;
                    }
                    if transient_data_opt.is_none() {
                        commands.entity(open_machine.entity).insert(StateMachineTransientData::default());
                        continue;
                    }
                    
                    let (_, _, Some(mut persistent_data), Some(mut transient_data)) = state_machines.get_mut(open_machine.entity).unwrap() else {
                        continue;
                    };
                    
                    // Apply canvas offset to all node positions before rendering
                    apply_canvas_offset_to_nodes(&mut persistent_data, open_machine.canvas_offset);
                    
                    // Show the machine editor directly on the main canvas
                    node_editor::show_single_machine_on_canvas(
                        ui,
                        &mut editor_state,
                        &mut persistent_data,
                        &mut transient_data,
                        open_machine.entity,
                        &open_machine.display_name,
                        &all_entities,
                        &child_of_query,
                        &children_query,
                        &active_query,
                        &parallel_query,
                        &mut commands,
                    );
                    
                    // Remove canvas offset after rendering to keep stored positions clean
                    remove_canvas_offset_from_nodes(&mut persistent_data, open_machine.canvas_offset);
                }
            }
            
            // Render context menus
            context_menu::render_context_menu(
                ctx,
                &mut editor_state,
                &mut commands,
                &all_entities,
                &child_of_query,
                &parallel_query,
            );
            
            // Render background context menu
            render_background_context_menu(
                ctx,
                &mut editor_state,
                &machine_list_query,
                &mut commands,
            );
        });
    }
}

/// Observer to handle transition creation requests
fn handle_transition_creation_request(
    trigger: Trigger<TransitionCreationRequested>,
    editor_state: Res<EditorState>,
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachine>>,
    type_registry: Res<AppTypeRegistry>,
) {
    let event = trigger.event();
    
    // For transition creation, we need to find which machine contains the source entity
    // For now, we'll iterate through all open machines to find the right one
    let mut selected_machine = None;
    for open_machine in &editor_state.open_machines {
        // This is a simplified check - in practice we'd need to verify the entity belongs to this machine
        selected_machine = Some(open_machine.entity);
        break; // For now, just use the first open machine
    }
    
    let Some(selected_machine) = selected_machine else {
        return;
    };
    
    let Ok(mut transient_data) = state_machines.get_mut(selected_machine) else {
        return;
    };
    
    // Start the transition creation process
    transient_data.transition_creation.start_transition(event.source_entity);
    
    // Discover available event types for EventEdge
    discover_transition_edge_listener_event_types(&mut transient_data.transition_creation, &type_registry);
}

/// Observer to handle transition creation with selected event type
fn handle_create_transition(
    trigger: Trigger<CreateTransition>,
    editor_state: Res<EditorState>,
    mut state_machines: Query<(&mut StateMachineTransientData, &mut StateMachinePersistentData), With<StateMachine>>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Find which machine contains the source entity
    let mut selected_machine = None;
    for open_machine in &editor_state.open_machines {
        selected_machine = Some(open_machine.entity);
        break; // For now, just use the first open machine
    }
    
    let Some(selected_machine) = selected_machine else {
        return;
    };
    
    let Ok((mut transient_data, mut persistent_data)) = state_machines.get_mut(selected_machine) else {
        return;
    };
    
    // Queue the transition creation as a command
    let source = event.source_entity;
    let target = event.target_entity;
    let event_type = event.event_type.clone();

    let edge_entity = commands.spawn_empty().id();
    
    commands.queue(move |world: &mut World| {
        match create_transition_edge_entity(world, edge_entity, source, target, &event_type) {
            Ok(edge) => {
                info!("✅ Created transition edge {:?} for {:?} -> {:?} ({})", edge, source, target, event_type);
            }
            Err(e) => {
                warn!("Failed to create transition: {}", e);
            }
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
            edge_entity: edge_entity,
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

/// Discover available EventEdge event types from the type registry
fn discover_transition_edge_listener_event_types(
    transition_state: &mut TransitionCreationState,
    type_registry: &AppTypeRegistry,
) {
    let registry = type_registry.read();
    let mut event_types = Vec::new();
    
    for registration in registry.iter() {
        let type_path = registration.type_info().type_path();
        
        // Look for EventEdge<EventType> patterns
        if let Some(start) = type_path.find("EventEdge<") {
            if let Some(end) = type_path[start..].find('>') {
                let event_type = &type_path[start + 10..start + end]; // 10 = len("EventEdge<")
                
                // Skip generic parameters and extract just the event type name
                if let Some(last_part) = event_type.split("::").last() {
                    if !event_types.contains(&last_part.to_string()) {
                        event_types.push(last_part.to_string());
                    }
                }
            }
        }
    }
    
    // Sort for consistent ordering and prepend a default "Always" option
    event_types.sort();
    if !event_types.iter().any(|e| e == "Always") {
        event_types.insert(0, "Always".to_string());
    }
    transition_state.available_event_types = event_types;
}

/// Create a transition edge entity using reflection (marker component on the edge)
fn create_transition_edge_entity(
    world: &mut World,
    edge_entity: Entity,
    source_entity: Entity,
    target_entity: Entity,
    event_type: &str,
) -> Result<(), String> {
    // Special-case: create an Always transition without a listener
    if event_type == "Always" {
        world.entity_mut(edge_entity).insert((Source(source_entity), Target(target_entity), EdgeKind::External, AlwaysEdge, Name::new("Always")));
        return Ok(());
    }
    // Find the full EventEdge type path and get reflection data
    let (type_path, reflect_component) = {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let mut transition_listener_type_path = None;
        for registration in registry.iter() {
            let type_path = registration.type_info().type_path();
            if type_path.contains("EventEdge<") && type_path.contains(event_type) {
                transition_listener_type_path = Some(type_path.to_string());
                break;
            }
        }
        
        let Some(type_path) = transition_listener_type_path else {
            return Err(format!("EventEdge<{}> not found in type registry", event_type));
        };
        
        // Get reflection data
        let Some(registration) = registry.get_with_type_path(&type_path) else { return Err(format!("Type registration not found for {}", type_path)); };
        let Some(reflect_component) = registration.data::<ReflectComponent>() else { return Err(format!("ReflectComponent not found for {}", type_path)); };
        (type_path, reflect_component.clone())
    };
    // Use the provided edge entity; insert core components
    let edge = edge_entity;
    world
        .entity_mut(edge)
        .insert((Source(source_entity), Target(target_entity), EdgeKind::External));

    // Attach the event-specific listener via reflection to the edge entity (empty struct)
    {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let Some(registration) = registry.get_with_type_path(&type_path) else { return Err(format!("Type registration not found for {}", type_path)); };
        let type_info = registration.type_info();
        let mut dynamic_struct = bevy::reflect::DynamicStruct::default();
        if let bevy::reflect::TypeInfo::Struct(_) = type_info { dynamic_struct.set_represented_type(Some(type_info)); } else { return Err(format!("EventEdge is not a struct type: {}", type_path)); }
        let mut entity_mut = world.entity_mut(edge);
        reflect_component.insert(&mut entity_mut, dynamic_struct.as_partial_reflect(), &registry);
    }

    // Give the edge a human-readable name matching the selected event type
    world.entity_mut(edge).insert(Name::new(event_type.to_string()));

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
    mut state_machines: Query<&mut StateMachinePersistentData, With<StateMachine>>,
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
        // Find the EventEdge<Event> type registration
        let mut reflect_listener: Option<bevy::ecs::reflect::ReflectComponent> = None;
        for registration in registry.iter() {
            let type_info = registration.type_info();
            let type_name = type_info.type_path_table().short_path();
            if type_name.starts_with("EventEdge<") && type_name.contains(&event_type) {
                reflect_listener = registration.data::<ReflectComponent>().cloned();
                break;
            }
        }
        if reflect_listener.is_none() {
            warn!("Could not resolve EventEdge<{}> for deletion", event_type);
            return;
        }
        let reflect_listener = reflect_listener.unwrap();

        // Search for an edge with Source, Target, and that listener
        let mut to_remove: Option<Entity> = None;
        let mut q = world.query::<(Entity, &Source, &Target)>();
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
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachine>>,
    edge_target_query: Query<&Target>,
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
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachine>>,
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

/// Observer to track EnterState events and create node pulses
fn handle_node_enter_pulse(
    trigger: Trigger<bevy_gearbox::EnterState>,
    child_of_query: Query<&bevy_gearbox::StateChildOf>,
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachine>>,
) {
    let state = trigger.target();
    let root = child_of_query.root_ancestor(state);
    if let Ok(mut transient) = state_machines.get_mut(root) {
        transient.node_pulses.push(NodePulse::new(state));
    }
}

/// System to update node pulse timers and remove expired pulses
fn update_node_pulses(
    mut state_machines: Query<&mut StateMachineTransientData, With<StateMachine>>,
    time: Res<Time>,
) {
    for mut transient in state_machines.iter_mut() {
        for pulse in transient.node_pulses.iter_mut() {
            pulse.timer.tick(time.delta());
        }
        transient.node_pulses.retain(|p| !p.timer.finished());
    }
}

/// Observer to handle node deletion with all edge cases
fn handle_delete_node(
    trigger: Trigger<DeleteNode>,
    mut state_machines: Query<&mut StateMachinePersistentData, With<StateMachine>>,
    state_child_of_query: Query<&bevy_gearbox::StateChildOf>,
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

    // Only remove transitions that TARGET the selected node
    let incoming_to_deleted: Vec<_> = persistent_data
        .visual_transitions
        .iter()
        .filter(|t| t.target_entity == entity_to_delete)
        .cloned()
        .collect();

    for t in incoming_to_deleted {
        commands.trigger(DeleteTransition {
            source_entity: t.source_entity,
            target_entity: t.target_entity,
            event_type: t.event_type.clone(),
        });
    }

    // Remove the visual node for the deleted entity only
    persistent_data.nodes.remove(&entity_to_delete);

    // Despawn only the selected entity. Children and source transitions will be cleaned up by relationships.
    commands.entity(entity_to_delete).despawn();
}

/// Derive visual transitions each frame from ECS edges while preserving user offsets
fn sync_edge_visuals_from_ecs(
    editor_state: Res<EditorState>,
    mut machines: Query<&mut StateMachinePersistentData, With<StateMachine>>,
    edges_q: Query<(Entity, &Source, &Target)>,
    names_q: Query<&Name>,
    child_of_q: Query<&bevy_gearbox::StateChildOf>,
) {
    // Sync edges for all open machines
    for open_machine in &editor_state.open_machines {
        let selected_root = open_machine.entity;
        let Ok(mut persistent) = machines.get_mut(selected_root) else { continue; };

    // Build a set of current edges under this root
    let mut seen_edges: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    // Snapshot node rects to avoid borrow conflicts
    let mut node_rects: std::collections::HashMap<Entity, egui::Rect> = std::collections::HashMap::new();
    for (entity, node) in &persistent.nodes {
        node_rects.insert(*entity, node.current_rect());
    }

    // Ensure each ECS edge has a visual entry; update rects and label
    for (edge, source, target) in &edges_q {
        if child_of_q.root_ancestor(source.0) != selected_root { continue; }
        seen_edges.insert(edge);

        // Compute rects if available
        let (Some(source_rect), Some(target_rect)) = (
            node_rects.get(&source.0).copied(),
            node_rects.get(&target.0).copied(),
        ) else { continue; };

        // Derive display label from Name or fallback to ID
        let label = if let Ok(n) = names_q.get(edge) { n.as_str().to_string() } else { format!("{:?}", edge) };

        // Find existing visual or create a new one
        if let Some(vt) = persistent.visual_transitions.iter_mut().find(|t| t.edge_entity == edge) {
            vt.source_entity = source.0;
            vt.target_entity = target.0;
            vt.source_rect = source_rect;
            vt.target_rect = target_rect;
            vt.event_type = label;
            if !vt.is_dragging_event_node {
                vt.update_event_node_position();
            }
        } else {
            let midpoint = egui::Pos2::new(
                (source_rect.center().x + target_rect.center().x) / 2.0,
                (source_rect.center().y + target_rect.center().y) / 2.0,
            );
            persistent.visual_transitions.push(TransitionConnection {
                source_entity: source.0,
                edge_entity: edge,
                target_entity: target.0,
                event_type: label,
                source_rect,
                target_rect,
                event_node_position: midpoint,
                is_dragging_event_node: false,
                event_node_offset: egui::Vec2::ZERO,
            });
        }
    }

        // Remove visuals whose edges no longer exist
        persistent.visual_transitions.retain(|t| seen_edges.contains(&t.edge_entity));
    }
}

/// Observer to handle transition deletion by edge entity
fn handle_delete_transition_by_edge(
    trigger: Trigger<DeleteTransitionByEdge>,
    mut commands: Commands,
) {
    let event = trigger.event();
    let edge = event.edge_entity;
    commands.queue(move |world: &mut World| {
        if world.entities().contains(edge) {
            world.entity_mut(edge).despawn();
            info!("✅ Removed edge {:?}", edge);
        } else {
            warn!("⚠️ DeleteTransitionByEdge: edge {:?} does not exist", edge);
        }
    });
}

/// Observer to handle SetInitialStateRequested requests
fn handle_set_initial_state_request(
    trigger: Trigger<SetInitialStateRequested>,
    mut commands: Commands,
) {
    let req = trigger.event();
    let child = req.child_entity;
    commands.queue(move |world: &mut World| {
        if let Some(child_of) = world.entity(child).get::<bevy_gearbox::StateChildOf>() {
            let parent = child_of.0;
            world.entity_mut(parent).insert(InitialState(child));
            info!("✅ Set InitialState({:?}) on parent {:?}", child, parent);
        } else {
            warn!("⚠️ SetInitialStateRequested: entity {:?} has no StateChildOf parent", child);
        }
    });
}

/// Handle background interactions for the canvas
fn handle_background_interactions(
    ui: &mut egui::Ui,
    editor_state: &mut EditorState,
    commands: &mut Commands,
) {
    // Check for right-click on background
    if ui.input(|i| i.pointer.secondary_clicked()) {
        let pointer_pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
        
        // Check if the click was on empty space (not on any machine)
        let clicked_on_machine = editor_state.open_machines.iter().any(|machine| {
            let machine_rect = calculate_machine_rect(machine);
            machine_rect.contains(pointer_pos)
        });
        
        if !clicked_on_machine {
            commands.trigger(BackgroundContextMenuRequested {
                position: pointer_pos,
            });
        }
    }
}

/// Calculate the rectangle for a machine on the canvas (for interaction detection)
fn calculate_machine_rect(open_machine: &OpenMachine) -> egui::Rect {
    let size = egui::Vec2::new(500.0, 350.0); // Default machine size
    egui::Rect::from_min_size(
        egui::Pos2::new(open_machine.canvas_offset.x, open_machine.canvas_offset.y),
        size,
    )
}

/// Apply canvas offset to all nodes in a state machine (for rendering)
fn apply_canvas_offset_to_nodes(persistent_data: &mut StateMachinePersistentData, offset: egui::Vec2) {
    for node in persistent_data.nodes.values_mut() {
        match node {
            crate::components::NodeType::Leaf(leaf_node) => {
                leaf_node.entity_node.position += offset;
            }
            crate::components::NodeType::Parent(parent_node) => {
                parent_node.entity_node.position += offset;
            }
        }
    }
    
    // Also offset transition event node positions
    for transition in persistent_data.visual_transitions.iter_mut() {
        transition.event_node_position += offset;
    }
}

/// Remove canvas offset from all nodes in a state machine (after rendering)
fn remove_canvas_offset_from_nodes(persistent_data: &mut StateMachinePersistentData, offset: egui::Vec2) {
    for node in persistent_data.nodes.values_mut() {
        match node {
            crate::components::NodeType::Leaf(leaf_node) => {
                leaf_node.entity_node.position -= offset;
            }
            crate::components::NodeType::Parent(parent_node) => {
                parent_node.entity_node.position -= offset;
            }
        }
    }
    
    // Also remove offset from transition event node positions
    for transition in persistent_data.visual_transitions.iter_mut() {
        transition.event_node_position -= offset;
    }
}

/// Render the background context menu
fn render_background_context_menu(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    machine_list_query: &Query<(Entity, Option<&Name>), With<StateMachine>>,
    commands: &mut Commands,
) {
    if let Some(position) = editor_state.background_context_menu_position {
        let menu_id = egui::Id::new("background_context_menu");
        
        egui::Area::new(menu_id)
            .fixed_pos(position)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(200.0);
                    ui.heading("Canvas");
                    ui.separator();
                    
                    if ui.button("Open State Machine").clicked() {
                        editor_state.show_machine_selection_menu = true;
                    }
                    
                    if ui.button("Create New Machine").clicked() {
                        // Create a new state machine
                        let new_entity = commands.spawn((
                            StateMachine::new(),
                            Name::new("New Machine"),
                        )).id();
                        
                        commands.trigger(OpenMachineRequested { entity: new_entity });
                        editor_state.background_context_menu_position = None;
                    }
                });
            });
        
        // Close menu if clicked elsewhere (but not if machine selection menu is open)
        if !editor_state.show_machine_selection_menu && ctx.input(|i| i.pointer.any_click()) {
            let pointer_pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
            let menu_rect = egui::Rect::from_min_size(position, egui::Vec2::new(200.0, 150.0));
            
            if !menu_rect.contains(pointer_pos) {
                editor_state.background_context_menu_position = None;
            }
        }
    }
    
    // Render machine selection submenu
    if editor_state.show_machine_selection_menu {
        if let Some(base_position) = editor_state.background_context_menu_position {
            let submenu_position = egui::Pos2::new(base_position.x + 210.0, base_position.y);
            let submenu_id = egui::Id::new("machine_selection_submenu");
            
            egui::Area::new(submenu_id)
                .fixed_pos(submenu_position)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(200.0);
                        ui.heading("Select Machine");
                        ui.separator();
                        
                        let mut found_machines = false;
                        for (entity, name_opt) in machine_list_query.iter() {
                            // Skip machines that are already open
                            if editor_state.is_machine_open(entity) {
                                continue;
                            }
                            
                            // Skip internal NodeKind machines
                            if let Some(name) = name_opt {
                                if name.as_str() == "NodeKind" {
                                    continue;
                                }
                            }
                            
                            found_machines = true;
                            let display_name = if let Some(name) = name_opt {
                                name.as_str().to_string()
                            } else {
                                format!("Unnamed Machine")
                            };
                            
                            if ui.button(&display_name).clicked() {
                                commands.trigger(OpenMachineRequested { entity });
                                editor_state.background_context_menu_position = None;
                                editor_state.show_machine_selection_menu = false;
                            }
                        }
                        
                        if !found_machines {
                            ui.label("No available machines");
                        }
                        
                        ui.separator();
                        if ui.button("Cancel").clicked() {
                            editor_state.show_machine_selection_menu = false;
                        }
                    });
                });
            
            // Close submenu if clicked elsewhere
            if ctx.input(|i| i.pointer.any_click()) {
                let pointer_pos = ctx.input(|i| i.pointer.hover_pos().unwrap_or_default());
                let main_menu_rect = egui::Rect::from_min_size(base_position, egui::Vec2::new(200.0, 150.0));
                let submenu_rect = egui::Rect::from_min_size(submenu_position, egui::Vec2::new(200.0, 150.0));
                
                if !main_menu_rect.contains(pointer_pos) && !submenu_rect.contains(pointer_pos) {
                    editor_state.background_context_menu_position = None;
                    editor_state.show_machine_selection_menu = false;
                }
            }
        }
    }
}

/// Observer to handle background context menu requests
fn handle_background_context_menu_request(
    trigger: Trigger<BackgroundContextMenuRequested>,
    mut editor_state: ResMut<EditorState>,
) {
    let event = trigger.event();
    editor_state.background_context_menu_position = Some(event.position);
}

/// Observer to handle open machine requests
fn handle_open_machine_request(
    trigger: Trigger<OpenMachineRequested>,
    mut editor_state: ResMut<EditorState>,
    name_query: Query<&Name>,
) {
    let event = trigger.event();
    
    // Don't open if already open
    if editor_state.is_machine_open(event.entity) {
        return;
    }
    
    let display_name = if let Ok(name) = name_query.get(event.entity) {
        name.as_str().to_string()
    } else {
        format!("Machine {:?}", event.entity)
    };
    
    editor_state.add_machine(event.entity, display_name);
    info!("✅ Opened machine {:?} on canvas", event.entity);
}

/// Observer to handle close machine requests
fn handle_close_machine_request(
    trigger: Trigger<CloseMachineRequested>,
    mut editor_state: ResMut<EditorState>,
) {
    let event = trigger.event();
    editor_state.remove_machine(event.entity);
    info!("✅ Closed machine {:?} from canvas", event.entity);
}

/// Observer to handle ViewRelated events
/// If the origin entity is currently being viewed in the editor, automatically loads the target entity
fn handle_view_related(
    trigger: Trigger<ViewRelated>,
    mut editor_state: ResMut<EditorState>,
    name_query: Query<&Name>,
    state_machine_query: Query<Entity, With<StateMachine>>,
) {
    let event = trigger.event();
    
    // Check if the origin entity is currently being viewed
    if !editor_state.is_machine_open(event.origin) {
        // Origin is not being viewed, so don't load the target
        return;
    }
    
    // Verify that the target entity has a state machine
    if state_machine_query.get(event.target).is_err() {
        warn!("ViewRelated target entity {:?} does not have a StateMachine component", event.target);
        return;
    }
    
    // Don't add if already open
    if editor_state.is_machine_open(event.target) {
        return;
    }
    
    // Get display name for the target
    let display_name = if let Ok(name) = name_query.get(event.target) {
        name.as_str().to_string()
    } else {
        format!("Related {:?}", event.target)
    };
    
    // Position the related entity near its origin
    let origin_offset = editor_state.open_machines.iter()
        .find(|m| m.entity == event.origin)
        .map(|m| m.canvas_offset)
        .unwrap_or(egui::Vec2::ZERO);
    
    // Offset the related entity slightly to the right and down from the origin
    let related_offset = origin_offset + egui::Vec2::new(300.0, 100.0);
    
    // Add the related machine with the calculated offset
    editor_state.add_machine_with_offset(event.target, display_name, related_offset);
    
    // Track the relationship for cleanup purposes
    editor_state.related_entities
        .entry(event.origin)
        .or_insert_with(Vec::new)
        .push(event.target);
    
    info!("🔗 Auto-loaded related machine {:?} because origin {:?} is being viewed", 
          event.target, event.origin);
}