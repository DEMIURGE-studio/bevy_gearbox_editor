//! Bevy Gearbox Editor
//! 
//! A visual editor for Bevy state machines with multi-window support,
//! hierarchical node editing, and real-time entity inspection.

use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_gearbox::{StateMachineRoot, InitialState};
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

// Re-exports
pub use editor_state::*;

// Additional imports for transition creation
use bevy::ecs::reflect::ReflectComponent;

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

        // Register events
        app.add_event::<NodeContextMenuRequested>()
            .add_event::<NodeActionTriggered>()
            .add_event::<NodeDragged>()
            .add_event::<TransitionCreationRequested>()
            .add_event::<CreateTransition>();

        // Add systems
        app.add_systems(Update, window_management::handle_editor_hotkeys)
            .add_systems(EditorWindowContextPass, editor_ui_system)
            .add_systems(EditorWindowContextPass, entity_inspector::entity_inspector_system)
            .add_systems(Update, (
                machine_list::ensure_node_actions,
                hierarchy::ensure_initial_states,
                node_editor::update_node_types,
                hierarchy::constrain_children_to_parents,
                hierarchy::recalculate_parent_sizes,
            ).chain());

        // Add observers
        app.add_observer(context_menu::handle_context_menu_request)
            .add_observer(context_menu::handle_node_action)
            .add_observer(hierarchy::handle_parent_child_movement)
            .add_observer(handle_transition_creation_request)
            .add_observer(handle_create_transition);
    }
}

/// System to render the main editor UI
/// Only runs when an editor window exists
fn editor_ui_system(
    mut editor_context: Query<&mut EguiContext, (With<EditorWindow>, Without<bevy_egui::PrimaryEguiContext>)>,
    mut editor_state: ResMut<EditorState>,
    state_machines: Query<(Entity, Option<&Name>), With<StateMachineRoot>>,
    all_entities: Query<(Entity, Option<&Name>, Option<&InitialState>)>,
    child_of_query: Query<&ChildOf>,
    children_query: Query<&Children>,
    mut commands: Commands,
) {
    // Only run if there's an editor window
    if let Ok(mut egui_context) = editor_context.single_mut() {
        let ctx = egui_context.get_mut();
        
        if editor_state.selected_machine.is_some() {
            // Show the node editor for the selected machine
            node_editor::show_machine_editor(
                ctx,
                &mut editor_state,
                &all_entities,
                &child_of_query,
                &children_query,
                &mut commands,
            );
        } else {
            // Show the machine list
            machine_list::show_machine_list(
                ctx,
                &mut editor_state,
                &state_machines,
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
    mut editor_state: ResMut<EditorState>,
    type_registry: Res<AppTypeRegistry>,
) {
    let event = trigger.event();
    
    // Start the transition creation process
    editor_state.transition_creation.start_transition(event.source_entity);
    
    // Discover available event types for TransitionListener
    discover_transition_listener_event_types(&mut editor_state.transition_creation, &type_registry);
}

/// Observer to handle transition creation with selected event type
fn handle_create_transition(
    trigger: Trigger<CreateTransition>,
    mut editor_state: ResMut<EditorState>,
    mut commands: Commands,
) {
    let event = trigger.event();
    
    // Queue the transition creation as a command
    let source = event.source_entity;
    let target = event.target_entity;
    let event_type = event.event_type.clone();
    
    commands.queue(move |world: &mut World| {
        if let Err(e) = create_transition_listener_component(
            world,
            source,
            target,
            &event_type,
        ) {
            warn!("Failed to create transition: {}", e);
        } else {
            info!("✅ Created TransitionListener<{}> from {:?} to {:?}", 
                  event_type, source, target);
        }
    });
    
    // Complete the transition creation process
    editor_state.transition_creation.complete();
    
    // Add the visual transition to the list for immediate display
    if let (Some(source_rect), Some(target_rect)) = (
        editor_state.nodes.get(&event.source_entity).map(|n| n.current_rect()),
        editor_state.nodes.get(&event.target_entity).map(|n| n.current_rect())
    ) {
        // Position event node at midpoint between source and target initially
        let initial_event_position = egui::Pos2::new(
            (source_rect.center().x + target_rect.center().x) / 2.0,
            (source_rect.center().y + target_rect.center().y) / 2.0,
        );
        
        editor_state.visual_transitions.push(TransitionConnection {
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

/// Discover available TransitionListener event types from the type registry
fn discover_transition_listener_event_types(
    transition_state: &mut TransitionCreationState,
    type_registry: &AppTypeRegistry,
) {
    let registry = type_registry.read();
    let mut event_types = Vec::new();
    
    for registration in registry.iter() {
        let type_path = registration.type_info().type_path();
        
        // Look for TransitionListener<EventType> patterns
        if let Some(start) = type_path.find("TransitionListener<") {
            if let Some(end) = type_path[start..].find('>') {
                let event_type = &type_path[start + 19..start + end]; // 19 = len("TransitionListener<")
                
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

/// Create a TransitionListener component using reflection (adapted from your old code)
fn create_transition_listener_component(
    world: &mut World,
    source_entity: Entity,
    target_entity: Entity,
    event_type: &str,
) -> Result<(), String> {
    use bevy_gearbox::Connection;
    
    // Create the connection
    let connection = Connection {
        target: target_entity,
        guards: None,
    };
    
    // Find the full TransitionListener type path and get reflection data
    let (type_path, reflect_component) = {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let mut transition_listener_type_path = None;
        for registration in registry.iter() {
            let type_path = registration.type_info().type_path();
            if type_path.contains("TransitionListener<") && type_path.contains(event_type) {
                transition_listener_type_path = Some(type_path.to_string());
                break;
            }
        }
        
        let Some(type_path) = transition_listener_type_path else {
            return Err(format!("TransitionListener<{}> not found in type registry", event_type));
        };
        
        // Get reflection data
        let Some(registration) = registry.get_with_type_path(&type_path) else {
            return Err(format!("Type registration not found for {}", type_path));
        };
        
        let Some(reflect_component) = registration.data::<ReflectComponent>() else {
            return Err(format!("ReflectComponent not found for {}", type_path));
        };
        
        (type_path, reflect_component.clone())
    };
    
    // Create the component instance
    let dynamic_struct = {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let Some(registration) = registry.get_with_type_path(&type_path) else {
            return Err(format!("Type registration not found for {}", type_path));
        };
        
        let type_info = registration.type_info();
        if let bevy::reflect::TypeInfo::Struct(_struct_info) = type_info {
            let mut dynamic_struct = bevy::reflect::DynamicStruct::default();
            dynamic_struct.set_represented_type(Some(type_info));
            
            // Add the connection field
            dynamic_struct.insert_boxed("connection", connection.to_dynamic());
            
            dynamic_struct
        } else {
            return Err(format!("TransitionListener is not a struct type: {}", type_path));
        }
    };
    
    // Insert the component (separate scope to avoid borrowing conflicts)
    {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        let mut entity_mut = world.entity_mut(source_entity);
        
        reflect_component.insert(
            &mut entity_mut,
            dynamic_struct.as_partial_reflect(),
            &registry,
        );
    }
    
    Ok(())
}

