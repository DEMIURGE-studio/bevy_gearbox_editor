use bevy::prelude::*;
use bevy_egui::egui;
use bevy::reflect::ReflectFromPtr;

/// Draws a read-only UI for all reflected components on an entity.
pub fn custom_ui_for_entity_readonly(world: &World, entity: Entity, ui: &mut egui::Ui) {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();

    let Ok(entity_ref) = world.get_entity(entity) else {
        ui.label("Entity does not exist.");
        return;
    };

    let mut components = Vec::new();
    for component_id in entity_ref.archetype().components() {
        if let Some(component_info) = world.components().get_info(component_id) {
            if let Some(type_id) = component_info.type_id() {
                if let Some(registration) = type_registry.get(type_id) {
                    if registration.data::<ReflectFromPtr>().is_some() {
                        let name = component_info.name().split("::").last().unwrap_or("");
                        components.push((name, component_id, type_id));
                    }
                }
            }
        }
    }
    components.sort_by_key(|(name, _, _)| *name);

    for (component_name, component_id, type_id) in components {
        let Some(component_ptr) = world.get_by_id(entity, component_id) else {
            continue;
        };

        ui.collapsing(component_name, |ui| {
            // Get the ReflectFromPtr to safely convert the component pointer to a reflection
            if let Some(registration) = type_registry.get(type_id) {
                if let Some(reflect_from_ptr) = registration.data::<ReflectFromPtr>() {
                    // SAFETY: We know the component exists and the type matches
                    let reflected_component = unsafe { reflect_from_ptr.as_reflect(component_ptr) };
                    custom_ui_for_reflect_readonly(reflected_component, ui);
                }
            }
        });
    }
}

/// Draws a read-only UI for a single reflected value (e.g., a component or a field).
pub fn custom_ui_for_reflect_readonly(value: &dyn Reflect, ui: &mut egui::Ui) {
    if value.is::<f32>() {
        let val = value.downcast_ref::<f32>().unwrap();
        ui.label(format!("{:.3}", val));
    } else if value.is::<String>() {
        let val = value.downcast_ref::<String>().unwrap();
        ui.label(val);
    } else if value.is::<bool>() {
        let val = value.downcast_ref::<bool>().unwrap();
        ui.label(if *val { "true" } else { "false" });
    } else if value.is::<Vec2>() {
        let val = value.downcast_ref::<Vec2>().unwrap();
        ui.label(format!("({:.3}, {:.3})", val.x, val.y));
    } else if value.is::<Vec3>() {
        let val = value.downcast_ref::<Vec3>().unwrap();
        ui.label(format!("({:.3}, {:.3}, {:.3})", val.x, val.y, val.z));
    } else if value.is::<Entity>() {
        let val = value.downcast_ref::<Entity>().unwrap();
        ui.label(format!("Entity({:?})", val));
    } else {
        // Fall back to complex types (like structs)
        match value.reflect_ref() {
            bevy::reflect::ReflectRef::Struct(s) => {
                egui::Grid::new(s.reflect_type_path()).show(ui, |ui| {
                    for i in 0..s.field_len() {
                        let field_name = s.name_at(i).unwrap_or("Unknown Field");
                        if field_name.starts_with('_') {
                            continue;
                        }

                        let field_value = s.field_at(i).unwrap();
                        // Convert from PartialReflect to Reflect if possible
                        if let Some(reflect_value) = field_value.try_as_reflect() {
                            ui.label(field_name);
                            custom_ui_for_reflect_readonly(reflect_value, ui);
                            ui.end_row();
                        }
                    }
                });
            }
            _ => {
                ui.label(format!(
                    "Unsupported reflect type: {}",
                    value.reflect_type_path()
                ));
            }
        }
    }
}

/// Draws an editable UI for a single reflected value (e.g., a component or a field).
pub fn custom_ui_for_reflect(value: &mut dyn Reflect, ui: &mut egui::Ui) {
    if value.is::<f32>() {
        let val = value.downcast_mut::<f32>().unwrap();
        ui.add(egui::DragValue::new(val).speed(0.1));
    } else if value.is::<String>() {
        let val = value.downcast_mut::<String>().unwrap();
        ui.text_edit_singleline(val);
    } else if value.is::<bool>() {
        let val = value.downcast_mut::<bool>().unwrap();
        ui.checkbox(val, "");
    } else if value.is::<Vec2>() {
        let val = value.downcast_mut::<Vec2>().unwrap();
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut val.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut val.y).speed(0.1));
        });
    } else if value.is::<Vec3>() {
        let val = value.downcast_mut::<Vec3>().unwrap();
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut val.x).speed(0.1));
            ui.add(egui::DragValue::new(&mut val.y).speed(0.1));
            ui.add(egui::DragValue::new(&mut val.z).speed(0.1));
        });
    } else if value.is::<Entity>() {
        let val = value.downcast_ref::<Entity>().unwrap();
        ui.label(format!("Entity({:?})", val));
    } else {
        // Fall back to complex types (like structs)
        match value.reflect_mut() {
            bevy::reflect::ReflectMut::Struct(s) => {
                egui::Grid::new(s.reflect_type_path()).show(ui, |ui| {
                    for i in 0..s.field_len() {
                        // Get field name first and own it to avoid borrowing issues
                        let field_name = s.name_at(i).unwrap_or("Unknown Field").to_owned();
                        if field_name.starts_with('_') {
                            continue;
                        }
                        
                        let field_value = s.field_at_mut(i).unwrap();
                        // Convert from PartialReflect to Reflect if possible
                        if let Some(reflect_value) = field_value.try_as_reflect_mut() {
                            ui.label(&field_name);
                            custom_ui_for_reflect(reflect_value, ui);
                            ui.end_row();
                        }
                    }
                });
            }
            _ => {
                ui.label(format!(
                    "Unsupported reflect type: {}",
                    value.reflect_type_path()
                ));
            }
        }
    }
}