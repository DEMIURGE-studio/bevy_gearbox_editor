pub mod node_renderer;
pub mod dialogs;
pub mod connections;
pub mod widgets;

use bevy::prelude::*;
use bevy_egui::{egui, EguiContext, PrimaryEguiContext};
use crate::components::*;
use crate::resources::*;

use node_renderer::NodeRenderer;
use dialogs::{ComponentDialog, TransitionDialog};
use connections::ConnectionRenderer;

/// Main UI rendering system - coordinates all UI elements
pub fn render_graph_nodes_system(world: &mut World) {
    // Get egui context and clone it (following bevy-inspector-egui pattern)
    let egui_context = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single(world);

    let Ok(egui_context) = egui_context else {
        return;
    };
    let mut egui_context = egui_context.clone();
    
    // Extract resources for UI rendering
    let mut ui_resources = UiResources::extract_from_world(world);

    egui::Window::new("Node Graph")
        .default_size([800.0, 600.0])
        .show(egui_context.get_mut(), |ui| {
            // Create a scrollable area for the graph
            egui::ScrollArea::both()
                .show(ui, |ui| {
                    // Set minimum size to allow panning
                    ui.set_min_size([1200.0, 800.0].into());
                    
                    render_graph_content(ui, world, &mut ui_resources);
                });
            
            // Render dialogs
            ComponentDialog::render(ui, world, &mut ui_resources.dialog_state);
            TransitionDialog::render(ui, world, &mut ui_resources.transition_state);
        });
    
    // Restore resources to the world
    ui_resources.restore_to_world(world);
}

/// Renders the main graph content using a multi-pass approach
fn render_graph_content(
    ui: &mut egui::Ui, 
    world: &mut World, 
    ui_resources: &mut UiResources
) {
    let node_renderer = NodeRenderer::new();
    let connection_renderer = ConnectionRenderer::new();
    
    // Collect node data for multi-pass rendering
    let node_data = collect_node_data(world);
    
    // PASS 1: Handle ALL input events (no visual rendering)
    let (drag_changes, expansion_changes) = node_renderer.handle_interactions(
        ui, world, &node_data, ui_resources
    );
    
    // PASS 2: Visual-only rendering of unselected nodes
    let additional_expansions = node_renderer.render_unselected_nodes(
        ui, world, &node_data, ui_resources
    );
    
    // PASS 3: Draw connections (on top of unselected nodes)
    connection_renderer.render_connections(ui, world, &ui_resources.size_cache, &ui_resources.pin_cache);
    
    // PASS 4: Visual-only rendering of selected node (on top of connections)
    let selected_expansions = node_renderer.render_selected_node(
        ui, world, &node_data, ui_resources
    );
    
    // Apply all changes back to components
    apply_node_changes(world, drag_changes, expansion_changes, additional_expansions, selected_expansions, ui_resources);
}

/// Collects node data for rendering
fn collect_node_data(world: &mut World) -> Vec<(Entity, Vec2, bool, Option<String>)> {
    let mut nodes_query = world.query::<(Entity, &GraphNode, &EntityNode, Option<&Name>)>();
    nodes_query.iter(world).map(|(entity, graph_node, _entity_node, name)| {
        let display_name = name.map(|n| n.to_string());
        (entity, graph_node.position, graph_node.expanded, display_name)
    }).collect()
}

/// Applies position, expansion, and size changes back to components
fn apply_node_changes(
    world: &mut World,
    drag_changes: Vec<(Entity, Vec2)>,
    expansion_changes: Vec<(Entity, bool)>,
    additional_expansions: Vec<(Entity, bool)>,
    selected_expansions: Vec<(Entity, bool)>,
    ui_resources: &UiResources,
) {
    let mut nodes_query = world.query::<&mut GraphNode>();
    
    // Apply position changes
    for (entity, new_pos) in drag_changes {
        if let Ok(mut graph_node) = nodes_query.get_mut(world, entity) {
            graph_node.position = new_pos;
        }
    }
    
    // Apply expansion changes from all sources
    let all_expansion_changes = expansion_changes
        .into_iter()
        .chain(additional_expansions)
        .chain(selected_expansions);
        
    for (entity, new_expanded) in all_expansion_changes {
        if let Ok(mut graph_node) = nodes_query.get_mut(world, entity) {
            if graph_node.expanded != new_expanded {
                println!("🔽 Updating expansion for {:?}: {} -> {}", entity, graph_node.expanded, new_expanded);
                graph_node.expanded = new_expanded;
            }
        }
    }
    
    // Apply measured sizes from egui back to GraphNode components
    for (entity, measured_size) in &ui_resources.size_cache.sizes {
        if let Ok(mut graph_node) = nodes_query.get_mut(world, *entity) {
            // Convert egui::Vec2 to bevy::Vec2
            let bevy_size = Vec2::new(measured_size.x, measured_size.y);
            
            // Only update if size actually changed (to avoid unnecessary updates)
            if graph_node.size != bevy_size {
                graph_node.size = bevy_size;
            }
        }
    }
}

/// Container for UI resources to avoid repeated extract/restore cycles
pub struct UiResources {
    pub size_cache: NodeSizeCache,
    pub pin_cache: PinPositionCache,
    pub dialog_state: ComponentDialogState,
    pub transition_state: TransitionCreationState,
    pub selected_entity: SelectedEntity,
}

impl UiResources {
    /// Extract all UI resources from the world
    pub fn extract_from_world(world: &mut World) -> Self {
        Self {
            size_cache: world.remove_resource::<NodeSizeCache>().unwrap_or_default(),
            pin_cache: world.remove_resource::<PinPositionCache>().unwrap_or_default(),
            dialog_state: world.remove_resource::<ComponentDialogState>().unwrap_or_default(),
            transition_state: world.remove_resource::<TransitionCreationState>().unwrap_or_default(),
            selected_entity: world.remove_resource::<SelectedEntity>().unwrap_or_default(),
        }
    }
    
    /// Restore all UI resources back to the world
    pub fn restore_to_world(self, world: &mut World) {
        world.insert_resource(self.size_cache);
        world.insert_resource(self.pin_cache);
        world.insert_resource(self.dialog_state);
        world.insert_resource(self.transition_state);
        world.insert_resource(self.selected_entity);
    }
}



