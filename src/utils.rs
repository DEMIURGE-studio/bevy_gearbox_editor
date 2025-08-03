use bevy::prelude::*;
use bevy_gearbox::Connection as BevyGearboxConnection;

/// Adds a component to an entity by type name using reflection  
pub fn add_component_to_entity(world: &mut World, entity: Entity, component_type_name: &str) {
    println!("🔧 Attempting to add component {} to entity {:?}", component_type_name, entity);
    
    // Try reflection-based insertion with proper error handling
    let result = try_add_component_via_reflection(world, entity, component_type_name);  
    
    if !result {
        // Fallback to hardcoded implementations for common components
        match component_type_name {
            "bevy_core::name::Name" => {
                world.entity_mut(entity).insert(Name::new("New Component"));
                println!("✅ Added Name component to entity {:?}", entity);
            },
            "bevy_transform::components::transform::Transform" => {
                world.entity_mut(entity).insert(Transform::default());
                println!("✅ Added Transform component to entity {:?}", entity);
            },
            _ => {
                println!("❌ Component {} not supported", component_type_name);  
            }
        }
    }
}

/// Creates a TransitionListener component using reflection
pub fn create_transition_listener(world: &mut World, source_entity: Entity, target_entity: Entity, event_type: &str) {
    println!("🔗 Creating TransitionListener<{}> on {:?} -> {:?}", event_type, source_entity, target_entity);
    
    // Step 1: Create the bevy_gearbox::Connection that TransitionListener needs
    let connection = BevyGearboxConnection {
        target: target_entity,
        guards: None,
    };
    
    // Step 2: Find the TransitionListener<EventType> in the type registry
    let transition_listener_type_path = find_transition_listener_type(world, event_type);
    
    // Step 3: Create and insert the component using reflection
    if let Some(type_path) = transition_listener_type_path {
        println!("🔧 Found TransitionListener type: {}", type_path);
        
        let result = create_transition_listener_via_reflection(world, source_entity, &type_path, connection);
        
        if result {
            println!("✅ Successfully created TransitionListener<{}> on {:?}", event_type, source_entity);
        } else {
            println!("❌ Failed to create TransitionListener<{}>", event_type);
        }
    } else {
        println!("❌ TransitionListener<{}> not found in type registry", event_type);
    }
}

/// Find the full type path for a TransitionListener with the given event type
fn find_transition_listener_type(world: &mut World, event_type: &str) -> Option<String> {
    let type_registry = world.resource::<AppTypeRegistry>();
    let registry = type_registry.read();
    
    for registration in registry.iter() {
        let type_path = registration.type_info().type_path();
        
        // Look for TransitionListener<EventType> that matches our event
        if type_path.contains("TransitionListener<") && type_path.contains(event_type) {
            return Some(type_path.to_string());
        }
    }
    None
}

/// Helper function to create TransitionListener components via reflection
fn create_transition_listener_via_reflection(
    world: &mut World, 
    entity: Entity, 
    component_type_name: &str, 
    connection: BevyGearboxConnection
) -> bool {
    let component_name = component_type_name.split("::").last().unwrap_or(component_type_name);
    
    // Step 1: Get reflection data for the TransitionListener type
    let reflect_component = get_reflect_component_data(world, component_type_name);
    let Some(reflect_component) = reflect_component else {
        println!("❌ {} missing ReflectComponent or not found", component_name);
        return false;
    };
    
    // Step 2: Create the TransitionListener instance
    let component_instance = create_transition_listener_instance(world, component_type_name, connection);
    let Some(component_instance) = component_instance else {
        return false;
    };
    
    // Step 3: Insert the component
    insert_component_via_reflection(world, entity, reflect_component, component_instance, component_name)
}

/// Get ReflectComponent data for a type
fn get_reflect_component_data(world: &mut World, component_type_name: &str) -> Option<ReflectComponent> {
    let type_registry = world.resource::<AppTypeRegistry>();
    let registry = type_registry.read();
    
    if let Some(registration) = registry.get_with_type_path(component_type_name) {
        registration.data::<ReflectComponent>().cloned()
    } else {
        None
    }
}

/// Create a TransitionListener instance using reflection
fn create_transition_listener_instance(
    world: &mut World, 
    component_type_name: &str, 
    connection: BevyGearboxConnection
) -> Option<Box<dyn bevy::reflect::PartialReflect>> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        // Find the TransitionListener type registration
        if let Some(registration) = registry.get_with_type_path(component_type_name) {
            let type_info = registration.type_info();
            
            // Create a dynamic struct with the same structure as TransitionListener
            if let bevy::reflect::TypeInfo::Struct(_struct_info) = type_info {
                let mut dynamic_struct = bevy::reflect::DynamicStruct::default();
                dynamic_struct.set_represented_type(Some(type_info));
                
                // Add the connection field
                dynamic_struct.insert_boxed("connection", connection.to_dynamic());
                
                // The _marker field is #[reflect(ignore)] so we don't need to set it
                
                println!("🔧 Successfully created TransitionListener structure");
                Some(Box::new(dynamic_struct) as Box<dyn bevy::reflect::PartialReflect>)
            } else {
                println!("❌ TransitionListener is not a struct type");
                None
            }
        } else {
            println!("❌ Could not find type registration");
            None
        }
    }));
    
    match result {
        Ok(instance) => instance,
        Err(_) => {
            println!("❌ Failed to create TransitionListener - component creation failed");
            None
        }
    }
}

/// Insert a component using reflection
fn insert_component_via_reflection(
    world: &mut World,
    entity: Entity,
    reflect_component: ReflectComponent,
    component_instance: Box<dyn bevy::reflect::PartialReflect>,
    component_name: &str,
) -> bool {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        
        // Insert the component into the entity
        let mut entity_mut = world.entity_mut(entity);
        reflect_component.insert(&mut entity_mut, component_instance.as_partial_reflect(), &registry);
    }));
    
    match result {
        Ok(_) => {
            println!("✅ Successfully inserted {} to entity {:?}", component_name, entity);
            true
        },
        Err(_) => {
            println!("❌ Failed to insert {} - insertion failed", component_name);
            false
        }
    }
}

/// Helper function to try adding components via reflection
fn try_add_component_via_reflection(world: &mut World, entity: Entity, component_type_name: &str) -> bool {
    let component_name = component_type_name.split("::").last().unwrap_or(component_type_name);
    
    // Step 1: Extract reflection info and component data
    let (reflect_component, default_component) = get_component_reflection_data(world, component_type_name);
    
    // Step 2: Insert the component using the ReflectComponent function pointer
    if let (Some(reflect_component), Some(component)) = (reflect_component, default_component) {
        insert_default_component(world, entity, reflect_component, component, component_name)
    } else {
        println!("❌ Component {} not found or missing ReflectDefault/ReflectComponent", component_name);
        false
    }
}

/// Get reflection data for a component type
fn get_component_reflection_data(
    world: &mut World, 
    component_type_name: &str
) -> (Option<ReflectComponent>, Option<Box<dyn bevy::reflect::PartialReflect>>) {
    let type_registry = world.resource::<AppTypeRegistry>();
    let registry = type_registry.read();
    
    if let Some(registration) = registry.get_with_type_path(component_type_name) {
        if let Some(reflect_default) = registration.data::<ReflectDefault>() {
            if let Some(reflect_component) = registration.data::<ReflectComponent>() {
                let default_component = reflect_default.default();
                let reflect_component_clone = reflect_component.clone();
                return (Some(reflect_component_clone), Some(default_component));
            }
        }
    }
    (None, None)
}

/// Insert a default component instance using reflection
fn insert_default_component(
    world: &mut World,
    entity: Entity,
    reflect_component: ReflectComponent,
    component: Box<dyn bevy::reflect::PartialReflect>,
    component_name: &str,
) -> bool {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Check if entity exists
        if world.get_entity(entity).is_err() {
            println!("❌ Entity {:?} does not exist", entity);
            return;
        }
        
        // Get the type registry directly from world
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        let registry = type_registry.read();
        
        // Get entity mutably (no borrow conflicts)
        let mut entity_mut = world.entity_mut(entity);
        
        // Use the ReflectComponent::insert method directly
        reflect_component.insert(&mut entity_mut, component.as_partial_reflect(), &registry);
    }));
    
    match result {
        Ok(_) => {
            println!("✅ Successfully added {} to entity {:?}", component_name, entity);
            true
        },
        Err(_) => {
            println!("❌ Failed to add {} - insertion failed", component_name);
            false
        }
    }
}