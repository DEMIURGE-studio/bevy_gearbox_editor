use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Color32, ComboBox, Pos2, Stroke, Vec2},
    EguiContext, EguiContexts, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext,
};
use bevy::reflect::ReflectRef;
use bevy::prelude::ReflectComponent;
use bevy_inspector_egui::{bevy_inspector, quick::WorldInspectorPlugin};
use bevy_gearbox::prelude::Connection;
use std::collections::HashMap;


#[derive(Component)]
pub struct DisplayEgui;

// === Node Editor Structures ===

/// Unique identifier for a node in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Represents an entity as a node in the graph
#[derive(Debug, Clone)]
pub struct EntityNode {
    pub entity: Entity,
    pub position: Pos2,
    pub size: Vec2,
    pub connection_border_width: f32, // Width of the connection interface border
}

/// Connection zones on the borders of an entity
#[derive(Debug, Clone, Copy)]
pub enum ConnectionZone {
    LeftTop,
    RightTop,
}

/// Represents a connection between two pins
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeConnection {
    pub from_node: NodeId,
    pub from_pin: usize,
    pub to_node: NodeId,  
    pub to_pin: usize,
}

/// Represents an output pin on a component field that references an entity
#[derive(Debug, Clone)]
pub struct OutputPin {
    pub node_id: NodeId,
    pub component_name: String,
    pub field_path: String,
    pub target_entity: Entity,
    pub position: Pos2, // Position of the pin in screen coordinates
    pub side: PinSide, // Which side of the node this pin is on
}

#[derive(Debug, Clone, Copy)]
pub enum PinSide {
    Left,
    Right,
}

/// The main node graph that holds all entities as nodes
#[derive(Resource, Default)]
pub struct EditorNodeGraph {
    pub nodes: HashMap<NodeId, EntityNode>,
    pub connections: Vec<NodeConnection>,
    pub output_pins: Vec<OutputPin>,
    pub next_node_id: usize,
    pub entity_to_node: HashMap<Entity, NodeId>,
}

impl EditorNodeGraph {
    pub fn add_node(&mut self, entity: Entity, position: Pos2) -> NodeId {
        let node_id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        
        let node = EntityNode {
            entity,
            position,
            size: Vec2::new(200.0, 150.0), // Default size
            connection_border_width: 16.0, // Space for connection interface
        };
        
        self.nodes.insert(node_id, node);
        self.entity_to_node.insert(entity, node_id);
        node_id
    }
    
    pub fn remove_node(&mut self, node_id: NodeId) {
        if let Some(node) = self.nodes.remove(&node_id) {
            self.entity_to_node.remove(&node.entity);
            // Remove all connections involving this node
            self.connections.retain(|conn| conn.from_node != node_id && conn.to_node != node_id);
        }
    }
    
    pub fn add_connection(&mut self, connection: NodeConnection) {
        if !self.connections.contains(&connection) {
            self.connections.push(connection);
        }
    }
    
    pub fn get_node_for_entity(&self, entity: Entity) -> Option<NodeId> {
        self.entity_to_node.get(&entity).copied()
    }
}

#[derive(Default)]
pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AddComponentUi>()
            .init_resource::<EditorNodeGraph>()
            .add_plugins((
                bevy_inspector_egui::DefaultInspectorConfigPlugin,
                EguiPlugin::default(),
                WorldInspectorPlugin::new(),
            ))
            .add_systems(
                EguiPrimaryContextPass,
                (
                    editor_context_menu,
                    render_node_editor,
                    add_transition_listener_window,
                ),
            );
    }
}

#[derive(Resource, Default)]
struct AddComponentUi {
    target_entity: Option<Entity>,
    selected_listener_type: Option<usize>,
    selected_connection_target: Option<Entity>,
    add_button_clicked: bool,
}

fn editor_context_menu(
    mut commands: Commands, 
    mut contexts: EguiContexts, 
    mut node_graph: ResMut<EditorNodeGraph>
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::CentralPanel::default().show(ctx, |ui| {
        let response = ui.allocate_response(ui.available_size(), egui::Sense::click_and_drag());

        response.context_menu(|ui| {
            if ui.button("Spawn entity").clicked() {
                let entity = commands.spawn((Name::new("New Entity"), DisplayEgui)).id();
                
                // Add entity to node graph at the clicked position
                let click_pos = response.interact_pointer_pos().unwrap_or(Pos2::new(100.0, 100.0));
                node_graph.add_node(entity, click_pos);
                
                ui.close_menu();
            }
        });
    });
}

fn _draw_entity_windows(world: &mut World) {
    let mut egui_context = {
        let Ok(egui_context) = world
            .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
            .single_mut(world)
        else {
            return;
        };
        egui_context.clone()
    };
    let ctx = egui_context.get_mut();

    let entities_with_names: Vec<(Entity, String)> = world
        .query_filtered::<(Entity, Option<&Name>), With<DisplayEgui>>()
        .iter(world)
        .map(|(entity, name)| {
            let name = name
                .map(|name| name.as_str().to_string())
                .unwrap_or_else(|| format!("Entity {:?}", entity));
            (entity, name)
        })
        .collect();

    for (entity, name) in entities_with_names {
        egui::Window::new(name)
            .id(egui::Id::new(entity))
            .show(ctx, |ui| {
                bevy_inspector::ui_for_entity(world, entity, ui);
                add_component_buttons(world, entity, ui);
            });
    }
}

fn add_component_buttons(world: &mut World, entity: Entity, ui: &mut egui::Ui) {
    add_component_button(world, entity, ui);
    if ui.button("Add Transition Listener").clicked() {
        let mut add_component_ui = world.resource_mut::<AddComponentUi>();
        add_component_ui.target_entity = Some(entity);
    }
}

fn add_transition_listener_window(world: &mut World) {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();

    let listeners: Vec<_> = type_registry
        .iter()
        .filter(|reg| {
            reg.type_info()
                .type_path()
                .starts_with("bevy_gearbox::transition_listener::TransitionListener")
                && reg.data::<ReflectComponent>().is_some()
        })
        .map(|reg| reg.clone())
        .collect();

    let named_entities: Vec<(Entity, String)> = { world.query::<(Entity, &Name)>().iter(world).map(|(entity, name)| (entity, name.as_str().to_string())).collect() };

    let mut egui_context = {
        let Ok(egui_context) = world
            .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
            .single_mut(world)
        else {
            return;
        };
        egui_context.clone()
    };
    let ctx = egui_context.get_mut();

    // Check if we need to add a component and extract the required data
    let add_component_data = {
        let add_component_ui = world.resource::<AddComponentUi>();
        if add_component_ui.add_button_clicked {
            Some((
                add_component_ui.target_entity.unwrap(),
                add_component_ui.selected_listener_type.unwrap(),
                add_component_ui.selected_connection_target.unwrap(),
            ))
        } else {
            None
        }
    };

    let mut add_component_ui = world.resource_mut::<AddComponentUi>();

    if add_component_ui.target_entity.is_some() {
        let mut is_open = true;
        egui::Window::new("Add Transition Listener")
            .open(&mut is_open)
            .show(ctx, |ui| {
                let selected_listener_text = add_component_ui
                    .selected_listener_type
                    .map(|idx| listeners[idx].type_info().type_path())
                    .unwrap_or("Select Listener");

                ComboBox::new("listener_selection", "Listener Type")
                    .selected_text(selected_listener_text)
                    .show_ui(ui, |ui| {
                        for (i, reg) in listeners.iter().enumerate() {
                            if ui
                                .selectable_label(
                                    add_component_ui.selected_listener_type.map_or(false, |idx| idx == i),
                                    reg.type_info().type_path(),
                                )
                                .clicked()
                            {
                                add_component_ui.selected_listener_type = Some(i);
                            }
                        }
                    });

                let selected_target_text = add_component_ui
                    .selected_connection_target
                    .and_then(|entity| named_entities.iter().find(|(e, _)| e == &entity).map(|(_, n)| n.as_str()))
                    .unwrap_or("Select Target");

                ComboBox::new("target_selection", "Connection Target")
                    .selected_text(selected_target_text)
                    .show_ui(ui, |ui| {
                        for (entity, name) in &named_entities {
                            if ui.selectable_label(add_component_ui.selected_connection_target == Some(*entity), name.as_str()).clicked() {
                                add_component_ui.selected_connection_target = Some(*entity);
                            }
                        }
                    });

                ui.separator();
                let add_button_enabled = add_component_ui.selected_listener_type.is_some() && add_component_ui.selected_connection_target.is_some();
                if ui.add_enabled(add_button_enabled, egui::Button::new("Add")).clicked() {
                    add_component_ui.add_button_clicked = true;
                }
            });

        if !is_open {
            *add_component_ui = AddComponentUi::default();
        }
    }

    // Drop the mutable borrow of add_component_ui before accessing other resources
    drop(add_component_ui);

    if let Some((target_entity, listener_idx, connection_target)) = add_component_data {
        let registration = &listeners[listener_idx];
        let reflect_component = registration.data::<ReflectComponent>().unwrap();

        let connection = Connection {
            target: connection_target,
            guards: None,
        };

        // Clone what we need from the type registry to avoid borrowing conflicts
        let type_registry = world.resource::<AppTypeRegistry>().clone();
        
        {
            let type_registry_read = type_registry.read();
            
            // Create a dynamic struct representing the TransitionListener
            let mut dynamic_struct = bevy::reflect::DynamicStruct::default();
            dynamic_struct.set_represented_type(Some(registration.type_info()));
            dynamic_struct.insert("connection", connection);

            let mut entity_mut = world.get_entity_mut(target_entity).unwrap();
            reflect_component.insert(&mut entity_mut, &dynamic_struct, &type_registry_read);
        }

        // Reset the UI state
        let mut add_component_ui = world.resource_mut::<AddComponentUi>();
        *add_component_ui = AddComponentUi::default();
    }
}

fn render_node_editor(world: &mut World) {
    // First, ensure all entities with DisplayEgui are in the node graph
    let entities_to_add: Vec<Entity> = {
        let all_entities: Vec<Entity> = world
            .query_filtered::<Entity, With<DisplayEgui>>()
            .iter(world)
            .collect();
        
        let node_graph = world.resource::<EditorNodeGraph>();
        all_entities
            .into_iter()
            .filter(|entity| !node_graph.entity_to_node.contains_key(entity))
            .collect()
    };

    // Add missing entities to node graph
    if !entities_to_add.is_empty() {
        let mut node_graph = world.resource_mut::<EditorNodeGraph>();
        for (i, entity) in entities_to_add.iter().enumerate() {
            let position = Pos2::new(100.0 + (i as f32) * 250.0, 100.0 + (i as f32) * 200.0);
            node_graph.add_node(*entity, position);
        }
    }

    // Clear existing output pins before rendering
    {
        let mut node_graph = world.resource_mut::<EditorNodeGraph>();
        node_graph.output_pins.clear();
    }

    // Get the node graph data
    let (nodes, connections) = {
        let node_graph = world.resource::<EditorNodeGraph>();
        let nodes: Vec<_> = node_graph.nodes.iter().map(|(id, node)| (*id, node.clone())).collect();
        let connections = node_graph.connections.clone();
        (nodes, connections)
    };

    // Get entity data
    let entities_with_names: Vec<(Entity, String)> = world
        .query_filtered::<(Entity, Option<&Name>), With<DisplayEgui>>()
        .iter(world)
        .map(|(entity, name)| {
            let name = name
                .map(|name| name.as_str().to_string())
                .unwrap_or_else(|| format!("Entity {:?}", entity));
            (entity, name)
        })
        .collect();

    let mut egui_context = {
        let Ok(egui_context) = world
            .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
            .single_mut(world)
        else {
            return;
        };
        egui_context.clone()
    };
    let ctx = egui_context.get_mut();

    egui::CentralPanel::default().show(ctx, |ui| {
        let available_rect = ui.available_rect_before_wrap();
        let mut allocated_ui = ui.new_child(egui::UiBuilder::new().max_rect(available_rect));
        render_nodes(world, &mut allocated_ui, &nodes, &entities_with_names);
        
        render_connections(&allocated_ui, &connections, &nodes);
    });
    
    // Draw output pin connections on top of everything else
    egui::Area::new(egui::Id::new("connections"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground) // Ensure it's drawn on top
        .show(ctx, |ui| {
            // Get output pins after rendering
            let output_pins = {
                let node_graph = world.resource::<EditorNodeGraph>();
                node_graph.output_pins.clone()
            };
            
            render_output_pin_connections(ui, &output_pins, &nodes);
            
            // Draw connection dots at zones that have connections (after connections so they're on top)
            render_connection_dots(ui, &output_pins, &nodes);
        });
}

fn render_nodes(
    world: &mut World,
    ui: &mut egui::Ui,
    nodes: &[(NodeId, EntityNode)],
    entities_with_names: &[(Entity, String)],
) {
    let mut position_updates = Vec::new();
    
    for (node_id, node) in nodes {
        let entity_name = entities_with_names
            .iter()
            .find(|(e, _)| *e == node.entity)
            .map(|(_, name)| name.as_str())
            .unwrap_or("Unknown Entity");

        // Create a proper egui window for each node
        let window_id = egui::Id::new(("entity_node", node.entity));
        
        let window_response = egui::Window::new(entity_name)
            .id(window_id)
            .default_pos(node.position)  // Use default_pos instead of fixed_pos to allow dragging
            .default_width(node.size.x)
            .resizable(true)  // Allow resizing for better UX
            .collapsible(false)
            .show(ui.ctx(), |ui| {
                // Render entity inspector content
                bevy_inspector::ui_for_entity(world, node.entity, ui);
                
                // Add output pins for entity reference fields
                render_output_pins_for_entity(world, node.entity, *node_id, node, &nodes, ui);
                
                // Add component buttons
                add_component_buttons(world, node.entity, ui);
            });

        // Capture the new window position if it was moved
        if let Some(response) = window_response {
            let window_rect = response.response.rect;
            let new_position = window_rect.min;
            // Only update if position actually changed to avoid unnecessary updates
            if (new_position - node.position).length() > 1.0 {
                position_updates.push((*node_id, new_position));
            }
        }
    }
    
    // Update node positions in the graph
    if !position_updates.is_empty() {
        let mut node_graph = world.resource_mut::<EditorNodeGraph>();
        for (node_id, new_position) in position_updates {
            if let Some(node) = node_graph.nodes.get_mut(&node_id) {
                node.position = new_position;
            }
        }
    }
}

fn render_connections(
    ui: &egui::Ui,
    connections: &[NodeConnection],
    nodes: &[(NodeId, EntityNode)],
) {
    let painter = ui.painter();
    
    for connection in connections {
        // Find the nodes involved in this connection
        let from_node = nodes.iter().find(|(id, _)| *id == connection.from_node);
        let to_node = nodes.iter().find(|(id, _)| *id == connection.to_node);
        
        if let (Some((_, from_node)), Some((_, to_node))) = (from_node, to_node) {
            // Calculate connection points
            let from_pos = from_node.position + Vec2::new(8.0, 8.0); // Entity dot
            let to_pos = to_node.position + Vec2::new(8.0, 8.0); // Entity dot
            
            // Draw bezier curve connection
            draw_connection_curve(painter, from_pos, to_pos);
        }
    }
}

/// Get the position of a connection zone on an entity
fn get_connection_zone_position(node: &EntityNode, zone: ConnectionZone) -> Pos2 {
    let border_width = node.connection_border_width;
    let half_border = border_width * 0.5;
    
    match zone {
        ConnectionZone::LeftTop => Pos2::new(
            node.position.x - half_border,
            node.position.y + node.size.y * 0.25
        ),
        ConnectionZone::RightTop => Pos2::new(
            node.position.x + node.size.x + half_border,
            node.position.y + node.size.y * 0.25
        ),
    }
}

/// Find the closest connection zone to a source position
fn get_closest_connection_zone(source_pos: Pos2, target_node: &EntityNode) -> (ConnectionZone, Pos2) {
    let zones = [
        ConnectionZone::LeftTop,
        ConnectionZone::RightTop,
    ];
    
    let mut closest_zone = zones[0];
    let mut closest_pos = get_connection_zone_position(target_node, zones[0]);
    let mut min_distance_sq = (source_pos - closest_pos).length_sq();
    
    for &zone in &zones[1..] {
        let zone_pos = get_connection_zone_position(target_node, zone);
        let distance_sq = (source_pos - zone_pos).length_sq();
        if distance_sq < min_distance_sq {
            min_distance_sq = distance_sq;
            closest_zone = zone;
            closest_pos = zone_pos;
        }
    }
    
    (closest_zone, closest_pos)
}

/// Calculate the closest attachment point on a target entity to a source position
fn get_closest_attachment_point(source_pos: Pos2, target_node: &EntityNode) -> Pos2 {
    let (_, closest_pos) = get_closest_connection_zone(source_pos, target_node);
    closest_pos
}

/// Calculate which side (left or right) an output pin should be on based on target position
fn calculate_output_pin_side(source_node: &EntityNode, target_node: &EntityNode) -> PinSide {
    let source_center = source_node.position + source_node.size * 0.5;
    let target_center = target_node.position + target_node.size * 0.5;
    
    if target_center.x > source_center.x {
        PinSide::Right
    } else {
        PinSide::Left
    }
}

fn draw_connection_curve(painter: &egui::Painter, from: Pos2, to: Pos2) {
    let control_scale = 0.5;
    let dx = to.x - from.x;
    let control1 = Pos2::new(from.x + dx * control_scale, from.y);
    let control2 = Pos2::new(to.x - dx * control_scale, to.y);
    
    // Draw the bezier curve as multiple line segments
    let segments = 20;
    let mut points = Vec::new();
    
    for i in 0..=segments {
        let t = i as f32 / segments as f32;
        let point = bezier_point(from, control1, control2, to, t);
        points.push(point);
    }
    
    for i in 0..points.len() - 1 {
        painter.line_segment(
            [points[i], points[i + 1]],
            Stroke::new(3.0, Color32::from_rgb(100, 150, 255)), // Nice blue connection
        );
    }
}

fn bezier_point(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    
    let x = uuu * p0.x + 3.0 * uu * t * p1.x + 3.0 * u * tt * p2.x + ttt * p3.x;
    let y = uuu * p0.y + 3.0 * uu * t * p1.y + 3.0 * u * tt * p2.y + ttt * p3.y;
    
    Pos2::new(x, y)
}

/// Draw connection dots only at zones that have active connections
fn render_connection_dots(
    ui: &egui::Ui,
    output_pins: &[OutputPin],
    nodes: &[(NodeId, EntityNode)]
) {
    let painter = ui.painter();
    
    // Draw connection dots for input zones (where connections end)
    for output_pin in output_pins {
        // Find the target entity's node
        let target_node = nodes.iter().find(|(_, node)| node.entity == output_pin.target_entity);
        
        if let Some((_, target_node)) = target_node {
            let (_, zone_pos) = get_closest_connection_zone(output_pin.position, target_node);
            painter.circle_filled(zone_pos, 8.0, Color32::from_rgb(100, 150, 255)); // Slightly larger for visibility
        }
        
        // Draw connection dots for output zones (where connections start) - make them very visible
        painter.circle_filled(output_pin.position, 8.0, Color32::from_rgb(255, 100, 100)); // Bright red and larger
        
        // Debug: Draw a larger background circle to ensure it's visible
        painter.circle_stroke(output_pin.position, 12.0, Stroke::new(2.0, Color32::WHITE));
    }
}

/// Render connections from output pins to target entity input pins
fn render_output_pin_connections(
    ui: &egui::Ui,
    output_pins: &[OutputPin],
    nodes: &[(NodeId, EntityNode)]
) {
    let painter = ui.painter();
    
    for output_pin in output_pins {
        // Find the target entity's node
        let target_node = nodes.iter().find(|(_, node)| node.entity == output_pin.target_entity);
        
        if let Some((_, target_node)) = target_node {
            // Calculate smart attachment points
            let from_pos = output_pin.position; // Output pin position
            let to_pos = get_closest_attachment_point(from_pos, target_node); // Smart target attachment
            
            // Draw bezier curve connection  
            draw_connection_curve(painter, from_pos, to_pos);
        }
    }
}

/// Simple function to add output pins for known entity reference fields
fn render_output_pins_for_entity(
    world: &mut World, 
    entity: Entity, 
    node_id: NodeId,
    source_node: &EntityNode,
    nodes: &[(NodeId, EntityNode)],
    ui: &mut egui::Ui
) {
    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();
    
    // Collect output pin data first to avoid borrow conflicts
    let mut output_pins_to_add = Vec::new();
    
    // Check if this entity has any TransitionListener components
    if let Ok(entity_ref) = world.get_entity(entity) {
        let archetype = entity_ref.archetype();
        
        for component_id in archetype.components() {
            let component_info = world.components().get_info(component_id).unwrap();
            let component_name = component_info.name();
            
            // Check if this is a TransitionListener component
            if component_name.contains("TransitionListener") {
                if let Some(registration) = type_registry.get(component_info.type_id().unwrap()) {
                    if let Some(reflect_component) = registration.data::<ReflectComponent>() {
                        if let Some(component_data) = reflect_component.reflect(entity_ref) {
                            // Try to access the connection.target field
                            if let ReflectRef::Struct(struct_ref) = component_data.reflect_ref() {
                                if let Some(connection_field) = struct_ref.field("connection") {
                                    if let ReflectRef::Struct(connection_struct) = connection_field.reflect_ref() {
                                        if let Some(target_field) = connection_struct.field("target") {
                                            if let Some(target_entity) = target_field.try_downcast_ref::<Entity>() {
                                                // Find the target node to determine pin placement
                                                let target_node = nodes.iter().find(|(_, node)| node.entity == *target_entity);
                                                
                                                if let Some((_, target_node)) = target_node {
                                                    // Calculate which side the pin should be on
                                                    let pin_side = calculate_output_pin_side(source_node, target_node);
                                                    
                                                    // Get target entity name for display
                                                    let target_name = world.get_entity(*target_entity)
                                                        .ok()
                                                        .and_then(|e| e.get::<Name>())
                                                        .map(|name| name.as_str().to_string())
                                                        .unwrap_or_else(|| "Unknown Entity".to_string());
                                                    
                                                    // Show connection info inside the entity
                                                    ui.horizontal(|ui| {
                                                        ui.label("Target Connection:");
                                                        ui.label(&target_name);
                                                    });
                                                    
                                                    // Calculate output pin position in the connection border
                                                    let current_y = ui.next_widget_position().y;
                                                    let pin_pos = match pin_side {
                                                        PinSide::Left => {
                                                            // Position on left border, clearly outside the window
                                                            Pos2::new(
                                                                source_node.position.x - 20.0, // Further left for visibility
                                                                current_y + 8.0 // Adjust for UI element height
                                                            )
                                                        }
                                                        PinSide::Right => {
                                                            // Position on right border, clearly outside the window
                                                            Pos2::new(
                                                                source_node.position.x + source_node.size.x + 20.0, // Further right for visibility
                                                                current_y + 8.0 // Adjust for UI element height
                                                            )
                                                        }
                                                    };
                                                    
                                                    // Store output pin data for later addition
                                                    output_pins_to_add.push(OutputPin {
                                                        node_id,
                                                        component_name: component_name.to_string(),
                                                        field_path: "connection.target".to_string(),
                                                        target_entity: *target_entity,
                                                        position: pin_pos,
                                                        side: pin_side,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Add output pins to node graph after rendering
    if !output_pins_to_add.is_empty() {
        let mut node_graph = world.resource_mut::<EditorNodeGraph>();
        node_graph.output_pins.extend(output_pins_to_add);
    }
}


fn add_component_button(world: &mut World, entity: Entity, ui: &mut egui::Ui) {
    let id = ui.make_persistent_id(entity).with("add_component_button");
    let mut add_component_state: AddComponentState =
        ui.memory_mut(|mem| mem.data.get_persisted_mut_or_default::<AddComponentState>(id).clone());

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let type_registry = type_registry.read();

    let mut selected_component_index = add_component_state.selected;
    let mut components: Vec<_> = type_registry
        .iter()
        .filter(|reg| {
            reg.data::<ReflectComponent>().is_some() && reg.data::<ReflectDefault>().is_some()
        })
        .map(|reg| reg.clone())
        .collect();

    components.sort_by(|a, b| a.type_info().type_path().cmp(b.type_info().type_path()));

    let selected_text = selected_component_index
        .map(|idx| components[idx].type_info().type_path())
        .unwrap_or("Select component");

    ui.separator();

    ComboBox::new(id.with("combo_box"), "")
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for (i, reg) in components.iter().enumerate() {
                if ui
                    .selectable_label(
                        selected_component_index.map_or(false, |idx| idx == i),
                        reg.type_info().type_path(),
                    )
                    .clicked()
                {
                    selected_component_index = Some(i);
                }
            }
        });

    add_component_state.selected = selected_component_index;

    let add_button_enabled = add_component_state.selected.is_some();

    if ui
        .add_enabled(add_button_enabled, egui::Button::new("Add Component"))
        .clicked()
    {
        if let Some(idx) = selected_component_index {
            let registration: &bevy::reflect::TypeRegistration = &components[idx];
            let reflect_component = registration.data::<ReflectComponent>().unwrap();
            let reflect_default = registration.data::<ReflectDefault>().unwrap();

            let mut entity_mut = world.get_entity_mut(entity).unwrap();
            let new_component = reflect_default.default();

            reflect_component.insert(
                &mut entity_mut,
                new_component.as_partial_reflect(),
                &type_registry,
            );
        }
    }

    ui.memory_mut(|mem| mem.data.insert_persisted(id, add_component_state));
}

#[derive(Clone, Default)]
struct AddComponentState {
    selected: Option<usize>,
}
