//! State machine list and management
//! 
//! This module handles:
//! - Displaying the list of available state machines
//! - Creating new state machines
//! - Selecting machines for editing

use bevy::prelude::*;
use bevy_gearbox::StateMachine;
use bevy_egui::egui;

use crate::editor_state::EditorState;

/// Render the state machine list interface
pub fn show_machine_list(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    state_machines: &Query<(Entity, Option<&Name>), With<StateMachine>>,
    commands: &mut Commands,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("State Machines");
        ui.separator();
        
        // List existing state machines (hide internal NodeKind machines)
        for (entity, name_opt) in state_machines.iter() {
            if let Some(name) = name_opt {
                if name.as_str() == "NodeKind" { continue; }
            }

            let display_name = if let Some(name) = name_opt {
                format!("{} (Entity {:?})", name.as_str(), entity)
            } else {
                format!("Unnamed Machine (Entity {:?})", entity)
            };

            if ui.button(&display_name).clicked() {
                editor_state.selected_machine = Some(entity);
            }
        }
        
        ui.separator();
        
        // "Create New" option
        if ui.button("Create New State Machine").clicked() {
            // For now, create with default name - could add a dialog later
            let new_entity = commands.spawn((
                StateMachine::new(),
                Name::new("New Machine"),
            )).id();
            
            editor_state.selected_machine = Some(new_entity);
        }
    });
}
