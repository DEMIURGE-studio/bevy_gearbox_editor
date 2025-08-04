use bevy::prelude::*;
use crate::components::*;

/// Initialize the graph canvas
pub fn setup_graph_canvas(mut commands: Commands) {
    commands.spawn(GraphCanvas);
    println!("🎯 Graph canvas created");
}

/// Auto-discovers TransitionListener components and creates pins/connections
/// This mimics egui-snarl's approach but uses Bevy ECS reflection
pub fn auto_discover_connections(
    mut commands: Commands,
    // Query for entities with EntityNode that might need pins
    node_entities: Query<Entity, With<EntityNode>>,
    // Query for existing connections to avoid duplicates
    existing_connections: Query<&Connection>,
    // We need to use reflection to find TransitionListener components dynamically
    type_registry: Res<AppTypeRegistry>,
    world: &World,
) {
    let registry = type_registry.read();
    
    for entity in node_entities.iter() {
        let Ok(entity_ref) = world.get_entity(entity) else { continue; };
        
        // Get or create NodePins component
        let existing_pins = entity_ref.get::<NodePins>().cloned()
            .unwrap_or_default();
        let mut pins = existing_pins.pins;
        
        // Ensure there's always an input pin
        if !pins.iter().any(|p| p.pin_type == PinType::Input) {
            pins.push(NodePin {
                pin_type: PinType::Input,
                pin_index: 0,
                label: "Input".to_string(),
            });
        }
        
        // Check if this entity has Children (is a parent node) or is a StateMachineRoot and needs an initial state pin
        if entity_ref.contains::<Children>() || entity_ref.contains::<bevy_gearbox::StateMachineRoot>() {
            // Check if we already have an initial state output pin
            if !pins.iter().any(|p| p.pin_type == PinType::Output && p.label == "InitialState") {
                pins.push(NodePin {
                    pin_type: PinType::Output,
                    pin_index: usize::MAX, // Special index for initial state pin
                    label: "InitialState".to_string(),
                });
                
                let entity_type = if entity_ref.contains::<bevy_gearbox::StateMachineRoot>() {
                    "root"
                } else {
                    "parent"
                };
                println!("🔌 Created initial state output pin for {} entity {:?}", entity_type, entity);
            }
        }
        
        // Look for TransitionListener components using reflection
        let archetype = entity_ref.archetype();
        let mut output_pin_index = 0;
        
        for component_id in archetype.components() {
            let Some(component_info) = world.components().get_info(component_id) else { continue; };
            let Some(type_id) = component_info.type_id() else { continue; };
            let Some(type_registration) = registry.get(type_id) else { continue; };
            
            let type_path = type_registration.type_info().type_path();
            
            // Check if this is a TransitionListener component
            if type_path.contains("TransitionListener<") {
                // Extract the event type from the type path
                let event_type = extract_transition_event_type(type_path);
                
                // Check if we already have this output pin
                if !pins.iter().any(|p| p.pin_type == PinType::Output && p.label == event_type) {
                    pins.push(NodePin {
                        pin_type: PinType::Output,
                        pin_index: output_pin_index,
                        label: event_type.clone(),
                    });
                    
                    output_pin_index += 1;
                    println!("🔌 Created output pin '{}' for entity {:?}", event_type, entity);
                }
            }
        }
        
        // Update the NodePins component
        commands.entity(entity).insert(NodePins { pins: pins.clone() });
        
        // Create connections based on TransitionListener targets
        create_connections_from_transition_listeners(
            &mut commands,
            entity,
            entity_ref,
            &existing_connections,
            &registry,
            &pins,
            world
        );
        
        // Create connections for initial state pins (if this is a parent entity)
        create_initial_state_connections(
            &mut commands,
            entity,
            entity_ref,
            &existing_connections,
            world
        );
    }
}

/// Extract event type from TransitionListener type path
/// e.g., "bevy_gearbox::transition_listener::TransitionListener<repeater::OnInvoke>" -> "OnInvoke"
fn extract_transition_event_type(type_path: &str) -> String {
    if let Some(start) = type_path.find('<') {
        if let Some(end) = type_path.rfind('>') {
            let inner = &type_path[start + 1..end];
            // Extract just the event type name (after the last ::)
            if let Some(last_colon) = inner.rfind("::") {
                return inner[last_colon + 2..].to_string();
            }
            return inner.to_string();
        }
    }
    "Unknown".to_string()
}

/// Create connections based on TransitionListener components
fn create_connections_from_transition_listeners(
    commands: &mut Commands,
    entity: Entity,
    entity_ref: bevy::ecs::world::EntityRef,
    existing_connections: &Query<&Connection>,
    registry: &bevy::reflect::TypeRegistry,
    pins: &[NodePin],
    world: &World,
) {
    let entity_archetype = entity_ref.archetype();
    
    for component_id in entity_archetype.components() {
        let Some(component_info) = world.components().get_info(component_id) else { continue; };
        let Some(type_id) = component_info.type_id() else { continue; };
        let Some(type_registration) = registry.get(type_id) else { continue; };
        
        let type_path = type_registration.type_info().type_path();
        
        // Check if this is a TransitionListener component
        if type_path.contains("TransitionListener<") {
            let event_type = extract_transition_event_type(type_path);
            
            // Try to get the component value using reflection to extract target entity
            if let Some(reflect_component) = type_registration.data::<ReflectComponent>() {
                if let Some(component_reflect) = reflect_component.reflect(entity_ref) {
                    // Navigate to connection.target field using reflection path
                    if let Some(target_reflect) = component_reflect.reflect_path("connection.target").ok() {
                        // Try to extract the Entity value
                        if let Some(target_entity) = target_reflect.try_downcast_ref::<Entity>() {
                            // Check if connection already exists
                            let connection_exists = existing_connections.iter().any(|conn| {
                                conn.from_entity == entity && 
                                conn.to_entity == *target_entity &&
                                conn.connection_type == event_type
                            });
                            
                            if !connection_exists {
                                // Find the correct output pin index for this event type
                                let output_pin_index = pins.iter()
                                    .enumerate()
                                    .find(|(_, pin)| pin.pin_type == PinType::Output && pin.label == event_type)
                                    .map(|(index, _)| index)
                                    .unwrap_or(0);
                                
                                // Create connection entity
                                commands.spawn(Connection {
                                    from_entity: entity,
                                    from_pin_index: output_pin_index,
                                    to_entity: *target_entity,
                                    to_pin_index: 0, // Input pin is always index 0 for now
                                    connection_type: event_type.clone(),
                                });
                                
                                println!("🔗 Created connection: {:?} --{}-> {:?}", 
                                        entity, event_type, target_entity);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Create connections for initial state pins from parent entities and root entities to their initial state targets
fn create_initial_state_connections(
    commands: &mut Commands,
    entity: Entity,
    entity_ref: bevy::ecs::world::EntityRef,
    existing_connections: &Query<&Connection>,
    _world: &World,
) {
    // Only process entities with Children (parent entities) or StateMachineRoot (root entities)
    if !entity_ref.contains::<Children>() && !entity_ref.contains::<bevy_gearbox::StateMachineRoot>() {
        return;
    }

    // Try to get the InitialState component directly from the entity reference
    let Some(initial_state) = entity_ref.get::<bevy_gearbox::InitialState>() else {
        return; // No InitialState component, nothing to connect
    };

    let target_entity = initial_state.0; // InitialState is a tuple struct with Entity at index 0
    
    // Check if connection already exists
    let connection_exists = existing_connections.iter().any(|conn| {
        conn.from_entity == entity && 
        conn.to_entity == target_entity &&
        conn.connection_type == "InitialState"
    });
    
    if !connection_exists {
        // Create the initial state connection
        let connection = Connection {
            from_entity: entity,
            to_entity: target_entity,
            from_pin_index: usize::MAX, // Special index for initial state pin
            to_pin_index: 0, // Input pin is always index 0
            connection_type: "InitialState".to_string(),
        };
        
        commands.spawn(connection);
        println!("🔗 Created initial state connection from {:?} to {:?}", entity, target_entity);
    }
}

/// Automatically add ParentZone components to entities with Children but no ParentZone
pub fn manage_parent_zones(
    mut commands: Commands,
    // Entities with Children but no ParentZone
    parent_entities: Query<(Entity, &GraphNode), (With<Children>, Without<ParentZone>)>,
    // Entities with ParentZone but no Children (orphaned zones)
    orphaned_zones: Query<Entity, (With<ParentZone>, Without<Children>)>,
) {
    // Add ParentZone to entities that have Children but no zone
    for (entity, _graph_node) in parent_entities.iter() {
        let default_zone = ParentZone {
            bounds: bevy::math::Rect::new(0.0, 0.0, 400.0, 300.0), // Default zone size
            resize_handles: [bevy::math::Rect::default(); 4],
            min_size: Vec2::new(200.0, 150.0),
        };
        
        commands.entity(entity).insert(default_zone);
        println!("🏗️ Added ParentZone to entity {:?} (has Children)", entity);
    }
    
    // Remove ParentZone from entities that no longer have Children
    for entity in orphaned_zones.iter() {
        commands.entity(entity).remove::<ParentZone>();
        println!("🗑️ Removed ParentZone from entity {:?} (no longer has Children)", entity);
    }
}

/// Ensures all state entities are properly parented to maintain hierarchy
/// Any EntityNode without a valid ChildOf relationship becomes a child of the root
pub fn enforce_root_hierarchy(
    mut commands: Commands,
    root_query: Query<Entity, With<bevy_gearbox::StateMachineRoot>>,
    entity_nodes: Query<Entity, (With<EntityNode>, With<GraphNode>)>,
    world: &World,
) {
    // Find the root entity
    let Some(root_entity) = root_query.iter().next() else {
        return; // No root entity found
    };
    
    for entity in entity_nodes.iter() {
        // Skip the root entity itself
        if entity == root_entity {
            continue;
        }
        
        let entity_ref = world.entity(entity);
        let needs_root_parent = if let Some(child_of) = entity_ref.get::<ChildOf>() {
            let parent_entity = child_of.0;
            
            // Check if the parent still exists and has EntityNode (is a valid state parent)
            if let Ok(parent_ref) = world.get_entity(parent_entity) {
                // If parent is root, that's fine
                if parent_entity == root_entity {
                    false
                } else if parent_ref.contains::<EntityNode>() && parent_ref.contains::<GraphNode>() {
                    // Parent is a valid state entity
                    false
                } else {
                    // Parent exists but is not a valid state entity - reassign to root
                    true
                }
            } else {
                // Parent entity no longer exists - reassign to root
                true
            }
        } else {
            // No ChildOf component - needs to be child of root
            true
        };
        
        if needs_root_parent {
            commands.entity(entity).insert(ChildOf(root_entity));
            println!("🌳 Assigned orphaned entity {:?} to root {:?}", entity, root_entity);
        }
    }
}