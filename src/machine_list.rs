//! State machine list and management
//! 
//! This module handles:
//! - Displaying the list of available state machines
//! - Creating new state machines
//! - Selecting machines for editing

use bevy::prelude::*;
use bevy_gearbox::StateMachineRoot;
use bevy_egui::egui;

use crate::editor_state::{EditorState, StateMachineEditorData};
use crate::components::{NodeType, ParentNode};

/// System to ensure all state machine entities have StateMachineEditorData components
pub fn ensure_node_actions(
    mut commands: Commands,
    state_machines: Query<Entity, (With<StateMachineRoot>, Without<StateMachineEditorData>)>,
) {
    for entity in state_machines.iter() {
        // Add the StateMachineEditorData component with a root node
        let mut editor_data = StateMachineEditorData::default();
        let parent_node = ParentNode::new(egui::Pos2::new(200.0, 100.0));
        editor_data.nodes.insert(entity, NodeType::Parent(parent_node));
        commands.entity(entity).insert(editor_data);
    }
}

/// Render the state machine list interface
pub fn show_machine_list(
    ctx: &egui::Context,
    editor_state: &mut EditorState,
    state_machines: &Query<(Entity, Option<&Name>), With<StateMachineRoot>>,
    commands: &mut Commands,
) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("State Machines");
        ui.separator();
        
        // List existing state machines
        for (entity, name_opt) in state_machines.iter() {
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
                StateMachineRoot,
                Name::new("New Machine"),
            )).id();
            
            editor_state.selected_machine = Some(new_entity);
        }
    });
}
