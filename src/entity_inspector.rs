//! Entity inspector integration with bevy-inspector-egui
//! 
//! This module handles:
//! - Rendering the entity inspector UI
//! - Integration with bevy-inspector-egui
//! - Managing inspector state

use bevy::prelude::*;
use bevy_egui::egui;
use bevy_inspector_egui::{
    bevy_inspector::ui_for_entity,
    bevy_egui::{EguiContext, PrimaryEguiContext},
};

use crate::editor_state::{EditorState, get_entity_name_from_world};

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
        
        // Get the egui context using the same approach as bevy-inspector-egui examples
        let Ok(egui_context) = world
            .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
            .single(world)
        else {
            return;
        };
        let mut ctx = egui_context.clone();
        
        egui::Window::new(format!("Inspector: {}", entity_name))
            .default_width(300.0)
            .show(ctx.get_mut(), |ui| {
                // Use bevy-inspector-egui to render the entity
                if world.entities().contains(inspected_entity) {
                    ui_for_entity(world, inspected_entity, ui);
                } else {
                    ui.label("Entity no longer exists");
                }
            });
    }
}
