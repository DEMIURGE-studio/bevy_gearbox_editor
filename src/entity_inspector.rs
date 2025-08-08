//! Entity inspector integration with bevy-inspector-egui
//! 
//! This module handles:
//! - Rendering the entity inspector UI
//! - Integration with bevy-inspector-egui
//! - Managing inspector state

use bevy::prelude::*;
use bevy::ecs::reflect::ReflectComponent;
use bevy_egui::egui;
use bevy_inspector_egui::{
    bevy_inspector::ui_for_entity,
    bevy_egui::EguiContext,
};


use crate::editor_state::{EditorState, EditorWindow, get_entity_name_from_world};

/// Helper function to try adding components via reflection
fn try_add_component_via_reflection(world: &mut World, entity: Entity, component_type_name: &str) -> bool {
    let component_name = component_type_name.split("::").last().unwrap_or(component_type_name);
    
    // Step 1: Extract reflection info and component data
    let (reflect_component, default_component) = get_component_reflection_data(world, component_type_name);
    
    // Step 2: Insert the component using the ReflectComponent function pointer
    if let (Some(reflect_component), Some(component)) = (reflect_component, default_component) {
        insert_default_component(world, entity, reflect_component, component, component_name)
    } else {
        info!("❌ Component {} not found or missing ReflectDefault/ReflectComponent", component_name);
        false
    }
}

/// Get reflection data for a component type
fn get_component_reflection_data(
    world: &mut World, 
    component_type_name: &str
) -> (Option<ReflectComponent>, Option<Box<dyn PartialReflect>>) {
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
    component: Box<dyn PartialReflect>,
    component_name: &str,
) -> bool {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // Check if entity exists
        if world.get_entity(entity).is_err() {
            info!("❌ Entity {:?} does not exist", entity);
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
            info!("✅ Successfully added {} to entity {:?}", component_name, entity);
            true
        },
        Err(_) => {
            info!("❌ Failed to add {} - insertion failed", component_name);
            false
        }
    }
}

/// Hierarchical component organization structure
#[derive(Debug, Clone, Default)]
pub struct ComponentHierarchy {
    pub components: std::collections::BTreeMap<String, ComponentNode>,
}

#[derive(Debug, Clone)]
pub enum ComponentNode {
    Component(String), // Full type path
    Namespace(std::collections::BTreeMap<String, ComponentNode>),
}

/// Get all available component types organized hierarchically
fn get_available_components_hierarchical(world: &World) -> ComponentHierarchy {
    let type_registry = world.resource::<AppTypeRegistry>();
    let registry = type_registry.read();
    
    let mut hierarchy = ComponentHierarchy::default();
    
    for registration in registry.iter() {
        // Only include types that have both ReflectComponent and ReflectDefault
        if registration.data::<ReflectComponent>().is_some() 
            && registration.data::<ReflectDefault>().is_some() {
            let type_path = registration.type_info().type_path();
            insert_component_into_hierarchy(&mut hierarchy.components, type_path);
        }
    }
    
    hierarchy
}

/// Insert a component type path into the hierarchical structure
fn insert_component_into_hierarchy(
    map: &mut std::collections::BTreeMap<String, ComponentNode>, 
    type_path: &str
) {
    let parts: Vec<&str> = type_path.split("::").collect();
    
    if parts.len() == 1 {
        // This is a root-level component
        map.insert(parts[0].to_string(), ComponentNode::Component(type_path.to_string()));
        return;
    }
    
    // Navigate/create the namespace hierarchy
    let mut current_map = map;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // This is the final component name
            current_map.insert(part.to_string(), ComponentNode::Component(type_path.to_string()));
        } else {
            // This is a namespace
            let entry = current_map.entry(part.to_string()).or_insert_with(|| {
                ComponentNode::Namespace(std::collections::BTreeMap::new())
            });
            
            match entry {
                ComponentNode::Namespace(ref mut nested_map) => {
                    current_map = nested_map;
                }
                ComponentNode::Component(_) => {
                    // This shouldn't happen in well-formed type paths, but handle gracefully
                    return;
                }
            }
        }
    }
}

/// System to render the entity inspector UI
/// 
/// Shows the bevy-inspector-egui interface for the currently inspected entity.
/// This system takes `&mut World` as its only parameter to work with bevy-inspector-egui.
pub fn entity_inspector_system(world: &mut World) {
    // Get the editor state
    let inspected_entity = if let Some(editor_state) = world.get_resource::<EditorState>() {
        editor_state.inspected_entity
    } else {
        return;
    };

    if let Some(inspected_entity) = inspected_entity {
        // Get the entity name
        let entity_name = get_entity_name_from_world(inspected_entity, world);
        
        // Get the egui context from editor windows only
        let Ok(egui_context) = world
            .query_filtered::<&mut EguiContext, (With<EditorWindow>, Without<bevy_egui::PrimaryEguiContext>)>()
            .single(world)
        else {
            return;
        };
        let mut ctx = egui_context.clone();
        
        let mut keep_open = true;
        egui::Window::new(format!("Inspector: {}", entity_name))
            .default_width(300.0)
            .open(&mut keep_open)
            .show(ctx.get_mut(), |ui| {
                // Use bevy-inspector-egui to render the entity
                if world.entities().contains(inspected_entity) {
                    ui_for_entity(world, inspected_entity, ui);
                    
                    ui.separator();
                    
                    // Component addition UI
                    render_component_addition_ui(world, inspected_entity, ui);
                } else {
                    ui.label("Entity no longer exists");
                }
            });
        
        // If the user closed the window, clear the inspected entity
        if !keep_open {
            if let Some(mut editor_state) = world.get_resource_mut::<EditorState>() {
                editor_state.inspected_entity = None;
            }
        }
    }
}

/// Render the component addition UI
fn render_component_addition_ui(world: &mut World, entity: Entity, ui: &mut egui::Ui) {
    ui.heading("Add Component");
    
    // We need to temporarily extract the editor state to avoid borrowing issues
    let mut editor_state = world.remove_resource::<EditorState>().unwrap_or_default();
    
    // Update component hierarchy if needed
    if editor_state.component_addition.component_hierarchy.is_none() {
        let hierarchy = get_available_components_hierarchical(world);
        editor_state.component_addition.update_hierarchy(hierarchy);
    }
    
    // Search text input
    ui.text_edit_singleline(&mut editor_state.component_addition.search_text);
    
    // Dropdown button
    let dropdown_response = ui.button("Select Component ▼");
    if dropdown_response.clicked() {
        editor_state.component_addition.dropdown_open = !editor_state.component_addition.dropdown_open;
    }
    
    // Component dropdown list
    if editor_state.component_addition.dropdown_open {
        ui.separator();
        
        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                // Extract the hierarchy and search text to avoid borrowing conflicts
                let hierarchy_clone = editor_state.component_addition.component_hierarchy.clone();
                let search_text = editor_state.component_addition.search_text.clone();
                
                if let Some(hierarchy) = hierarchy_clone {
                    if search_text.is_empty() {
                        // Show hierarchical view when not searching
                        render_component_hierarchy(
                            ui, 
                            &hierarchy.components, 
                            String::new(), 
                            &mut editor_state.component_addition,
                            world,
                            entity
                        );
                    } else {
                        // Show flat filtered list when searching
                        render_filtered_components(
                            ui,
                            &hierarchy.components,
                            &search_text,
                            world,
                            entity,
                            &mut editor_state.component_addition
                        );
                    }
                }
            });
    }
    
    // Put the editor state back
    world.insert_resource(editor_state);
}

/// Render the hierarchical component tree
fn render_component_hierarchy(
    ui: &mut egui::Ui,
    components: &std::collections::BTreeMap<String, ComponentNode>,
    namespace_path: String,
    state: &mut crate::editor_state::ComponentAdditionState,
    world: &mut World,
    entity: Entity,
) {
    for (name, node) in components {
        let current_path = if namespace_path.is_empty() {
            name.clone()
        } else {
            format!("{}::{}", namespace_path, name)
        };
        
        match node {
            ComponentNode::Component(full_type_path) => {
                if ui.button(name).clicked() {
                    try_add_component_via_reflection(world, entity, full_type_path);
                    state.dropdown_open = false;
                }
            }
            ComponentNode::Namespace(nested_components) => {
                let is_expanded = state.is_namespace_expanded(&current_path);
                let expand_symbol = if is_expanded { "▼" } else { "▶" };
                
                if ui.button(format!("{} {}", expand_symbol, name)).clicked() {
                    state.toggle_namespace(&current_path);
                }
                
                if is_expanded {
                    ui.indent(format!("indent_{}", current_path), |ui| {
                        render_component_hierarchy(ui, nested_components, current_path, state, world, entity);
                    });
                }
            }
        }
    }
}

/// Render filtered components when searching
fn render_filtered_components(
    ui: &mut egui::Ui,
    components: &std::collections::BTreeMap<String, ComponentNode>,
    search_text: &str,
    world: &mut World,
    entity: Entity,
    state: &mut crate::editor_state::ComponentAdditionState,
) {
    let search_lower = search_text.to_lowercase();
    let mut found_any = false;
    
    collect_matching_components(components, &search_lower, ui, world, entity, state, &mut found_any);
    
    if !found_any {
        ui.label("No matching components found");
    }
}

/// Recursively collect and render components that match the search
fn collect_matching_components(
    components: &std::collections::BTreeMap<String, ComponentNode>,
    search_lower: &str,
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    state: &mut crate::editor_state::ComponentAdditionState,
    found_any: &mut bool,
) {
    for (name, node) in components {
        match node {
            ComponentNode::Component(full_type_path) => {
                if name.to_lowercase().contains(search_lower) {
                    *found_any = true;
                    if ui.button(format!("{} ({})", name, full_type_path)).clicked() {
                        try_add_component_via_reflection(world, entity, full_type_path);
                        state.dropdown_open = false;
                        state.search_text.clear();
                    }
                }
            }
            ComponentNode::Namespace(nested_components) => {
                collect_matching_components(nested_components, search_lower, ui, world, entity, state, found_any);
            }
        }
    }
}
