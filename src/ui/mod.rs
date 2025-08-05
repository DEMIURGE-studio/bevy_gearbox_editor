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
use widgets::EntityInspectorPanel;

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

    // Get root entity information for window title
    let (root_entity, root_name, root_initial_target) = find_root_entity(world)
        .unwrap_or((Entity::PLACEHOLDER, None, None));
    let window_title = root_name.as_deref().unwrap_or("Node Graph");
    
    // Main node graph window with dynamic title
    egui::Window::new(window_title)
        .default_size([800.0, 600.0])
        .show(egui_context.get_mut(), |ui| {
            // Create a scrollable area for the graph
            egui::ScrollArea::both()
                .show(ui, |ui| {
                    // Set minimum size to allow panning
                    ui.set_min_size([1200.0, 800.0].into());
                    
                    render_graph_content(ui, world, &mut ui_resources, root_entity, root_initial_target);
                });
            
            // Render dialogs
            ComponentDialog::render(ui, world, &mut ui_resources.dialog_state);
            TransitionDialog::render(ui, world, &mut ui_resources.transition_state);
        });
    
    // Separate inspector panel window
    egui::Window::new("Entity Inspector")
        .default_size([400.0, 600.0])
        .show(egui_context.get_mut(), |ui| {
            EntityInspectorPanel::new(ui_resources.selected_entity.entity)
                .show(ui, world, &mut ui_resources.dialog_state, &mut ui_resources.transition_state);
        });
    
    // Restore resources to the world
    ui_resources.restore_to_world(world);
}

/// Renders the main graph content using a multi-pass approach
fn render_graph_content(
    ui: &mut egui::Ui, 
    world: &mut World, 
    ui_resources: &mut UiResources,
    root_entity: Entity,
    root_initial_target: Option<Entity>,
) {
    let node_renderer = NodeRenderer::new();
    let connection_renderer = ConnectionRenderer::new();
    
    // Collect node data for multi-pass rendering
    let node_data = collect_node_data(world);
    
    // PASS 1: Handle ALL input events (no visual rendering)
    let drag_changes = node_renderer.handle_interactions(
        ui, world, &node_data, ui_resources
    );
    
    // PASS 2: Visual-only rendering of unselected nodes
    node_renderer.render_unselected_nodes(
        ui, world, &node_data, ui_resources
    );
    
    // PASS 3: Draw connections (on top of unselected nodes, under canvas pin)
    connection_renderer.render_connections(ui, world, &ui_resources.size_cache, &ui_resources.pin_cache, &ui_resources.connection_animations);
    
    // PASS 4: Render canvas initial state pin (on top of connections)  
    if root_entity != Entity::PLACEHOLDER {
        render_canvas_initial_state_pin_in_graph(ui, root_entity, root_initial_target, ui_resources);
    }
    
    // PASS 5: Visual-only rendering of selected node (on top of everything)
    node_renderer.render_selected_node(
        ui, world, &node_data, ui_resources
    );
    
    // Apply position changes back to components
    apply_node_changes(world, drag_changes, ui_resources);
}

/// Collects node data for rendering (excludes StateMachineRoot entities)
fn collect_node_data(world: &mut World) -> Vec<(Entity, Vec2, bool, Option<String>)> {
    let mut nodes_query = world.query::<(Entity, &GraphNode, &EntityNode, Option<&Name>)>();
    nodes_query.iter(world)
        .filter(|(entity, _, _, _)| {
            // Filter out entities with StateMachineRoot - they represent the canvas, not nodes
            !world.entity(*entity).contains::<bevy_gearbox::StateMachineRoot>()
        })
        .map(|(entity, graph_node, _entity_node, name)| {
            let display_name = name.map(|n| n.to_string());
            (entity, graph_node.position, graph_node.expanded, display_name)
        }).collect()
}

/// Find the root entity (with StateMachineRoot component)
fn find_root_entity(world: &mut World) -> Option<(Entity, Option<String>, Option<Entity>)> {
    let mut root_query = world.query::<(Entity, Option<&Name>, Option<&bevy_gearbox::InitialState>)>();
    for (entity, name, initial_state) in root_query.iter(world) {
        if world.entity(entity).contains::<bevy_gearbox::StateMachineRoot>() {
            let display_name = name.map(|n| n.to_string());
            let initial_target = initial_state.map(|is| is.0);
            return Some((entity, display_name, initial_target));
        }
    }
    None
}

/// Render the canvas-level initial state pin inside the graph area (ScrollArea coordinate space)
fn render_canvas_initial_state_pin_in_graph(
    ui: &mut egui::Ui,
    root_entity: Entity,
    initial_target: Option<Entity>,
    ui_resources: &mut UiResources,
) {
    // Position the pin at a fixed location in the top-left of the canvas
    let pin_pos = ui.min_rect().min + egui::Vec2::new(20.0, 20.0);
    let pin_radius = 6.0; // Consistent with widget pins
    
    // Create a clickable area for the entire title bar
    let title_bar_size = egui::Vec2::new(300.0, 30.0); // Generous clickable area
    let title_bar_rect = egui::Rect::from_min_size(pin_pos - egui::Vec2::new(5.0, 5.0), title_bar_size);
    
    // Create the title bar response for the entire area
    let title_bar_response = ui.allocate_rect(title_bar_rect, egui::Sense::click());
    
    // Draw a subtle background for the title bar when selected
    if ui_resources.selected_entity.entity == Some(root_entity) {
        ui.painter().rect_filled(
            title_bar_rect, 
            3.0, 
            egui::Color32::from_rgba_unmultiplied(255, 255, 100, 40) // Yellow highlight
        );
    }
    
    // Draw the red initial state pin
    let pin_center = pin_pos + egui::Vec2::new(pin_radius, pin_radius);
    ui.painter().circle_filled(
        pin_center,
        pin_radius,
        egui::Color32::from_rgb(255, 100, 100), // Red for initial state
    );
    
    // Cache the special initial state pin position (for connections from root to children)
    // We'll handle this specially in the connections system
    
    // Also create edge pins for the root entity (treat the entire canvas as the root's bounds)
    let canvas_rect = ui.available_rect_before_wrap();
    let root_edge_pins = crate::resources::EdgePins::from_rect(canvas_rect);
    ui_resources.pin_cache.edge_pins.insert(root_entity, root_edge_pins);
    
    // Draw the label next to the pin
    let label_pos = pin_pos + egui::Vec2::new(pin_radius * 2.0 + 10.0, 0.0);
    let label_text = if let Some(target) = initial_target {
        format!("Initial State → {:?}", target)
    } else {
        "Initial State → None".to_string()
    };
    
    ui.painter().text(
        label_pos,
        egui::Align2::LEFT_TOP,
        &label_text,
        egui::FontId::default(),
        egui::Color32::WHITE,
    );
    
    // Show selection indicator
    if ui_resources.selected_entity.entity == Some(root_entity) {
        let selected_label_pos = label_pos + egui::Vec2::new(label_text.len() as f32 * 7.0 + 10.0, 0.0);
        ui.painter().text(
            selected_label_pos,
            egui::Align2::LEFT_TOP,
            "(selected)",
            egui::FontId::default(),
            egui::Color32::YELLOW,
        );
    }
    
    // Handle clicking anywhere on the title bar to select root entity
    if title_bar_response.clicked() {
        ui_resources.selected_entity.entity = Some(root_entity);
        println!("🎯 Selected root entity: {:?}", root_entity);
    }
}

/// Applies position and size changes back to components
fn apply_node_changes(
    world: &mut World,
    drag_changes: Vec<(Entity, Vec2)>,
    ui_resources: &UiResources,
) {
    let mut nodes_query = world.query::<&mut GraphNode>();
    
    // Apply position changes
    for (entity, new_pos) in drag_changes {
        if let Ok(mut graph_node) = nodes_query.get_mut(world, entity) {
            graph_node.position = new_pos;
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
    pub drag_drop_state: DragDropState,
    pub connection_animations: ConnectionAnimations,
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
            drag_drop_state: world.remove_resource::<DragDropState>().unwrap_or_default(),
            connection_animations: world.remove_resource::<ConnectionAnimations>().unwrap_or_default(),
        }
    }
    
    /// Restore all UI resources back to the world
    pub fn restore_to_world(self, world: &mut World) {
        world.insert_resource(self.size_cache);
        world.insert_resource(self.pin_cache);
        world.insert_resource(self.dialog_state);
        world.insert_resource(self.transition_state);
        world.insert_resource(self.selected_entity);
        world.insert_resource(self.drag_drop_state);
        world.insert_resource(self.connection_animations);
    }
}





