use bevy::prelude::*;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;

// Re-export modules for public API
pub mod components;
pub mod resources;
pub mod systems;
pub mod ui;
pub mod utils;

// Re-export commonly used types
pub use components::*;
pub use resources::*;
pub use systems::*;

use crate::ui::render_graph_nodes_system;

pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
           .add_plugins(DefaultInspectorConfigPlugin)
           // Initialize resources
           .init_resource::<GraphState>()
           .init_resource::<NodeSizeCache>()
           .init_resource::<PinPositionCache>()
           .init_resource::<ComponentDialogState>()
           .init_resource::<TransitionCreationState>()
           .init_resource::<SelectedEntity>()
           // Register custom components for reflection (so bevy-inspector-egui can show them)
           .register_type::<GraphCanvas>()
           .register_type::<GraphNode>() 
           .register_type::<EntityNode>()
           .register_type::<GraphState>()
           // Register connection system types
           .register_type::<NodePins>()
           .register_type::<NodePin>()
           .register_type::<PinType>()
           .register_type::<Connection>()
           // Add systems
           .add_systems(Startup, setup_graph_canvas)  
           .add_systems(Update, auto_discover_connections)
           .add_systems(EguiPrimaryContextPass, render_graph_nodes_system);
    }
}