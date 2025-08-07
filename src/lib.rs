//! Bevy Gearbox Editor
//! 
//! A visual editor for Bevy state machines with multi-window support,
//! hierarchical node editing, and real-time entity inspection.

use bevy::prelude::*;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_gearbox::StateMachineRoot;
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
            .add_event::<NodeDragged>();

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
            .add_observer(hierarchy::handle_parent_child_movement);
    }
}

/// System to render the main editor UI
/// Only runs when an editor window exists
fn editor_ui_system(
    mut editor_context: Query<&mut EguiContext, (With<EditorWindow>, Without<bevy_egui::PrimaryEguiContext>)>,
    mut editor_state: ResMut<EditorState>,
    state_machines: Query<(Entity, Option<&Name>), With<StateMachineRoot>>,
    all_entities: Query<(Entity, Option<&Name>)>,
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