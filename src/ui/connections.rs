use bevy::prelude::*;
use bevy_egui::egui;
use crate::components::*;
use crate::resources::*;
use std::collections::HashMap;

pub struct ConnectionRenderer;

impl ConnectionRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render all connections between nodes
    pub fn render_connections(
        &self,
        ui: &mut egui::Ui, 
        world: &mut World, 
        size_cache: &NodeSizeCache, 
        pin_cache: &PinPositionCache,
        connection_animations: &ConnectionAnimations,
    ) {
        // Get all connections
        let connections: Vec<Connection> = world.query::<&Connection>().iter(world).cloned().collect();
        
        // Get node positions and pins for connection endpoints  
        let (node_positions, _node_pins) = self.collect_node_data(world);
        
        // Draw each connection
        for connection in connections {
            self.draw_connection_line(ui, &connection, &node_positions, size_cache, pin_cache, connection_animations);
        }
    }

    /// Collect node positions and pin data for connection rendering
    fn collect_node_data(&self, world: &mut World) -> (HashMap<Entity, Vec2>, HashMap<Entity, NodePins>) {
        let mut node_positions = HashMap::new();
        let mut node_pins = HashMap::new();
        
        for (entity, graph_node, pins) in world.query::<(Entity, &GraphNode, Option<&NodePins>)>().iter(world) {
            node_positions.insert(entity, graph_node.position);
            if let Some(pins) = pins {
                node_pins.insert(entity, pins.clone());
            }
        }
        
        (node_positions, node_pins)
    }

    /// Draw a single bezier connection line between two pins
    fn draw_connection_line(
        &self,
        ui: &mut egui::Ui,
        connection: &Connection,
        node_positions: &HashMap<Entity, Vec2>,
        size_cache: &NodeSizeCache,
        pin_cache: &PinPositionCache,
        connection_animations: &ConnectionAnimations,
    ) {
        // Try to get actual pin positions from cache first
        let from_pin_pos = pin_cache.output_pins.get(&(connection.from_entity, connection.from_pin_index));
        let to_pin_pos = pin_cache.input_pins.get(&connection.to_entity);
        
        // If we have cached positions, use them; otherwise fallback to calculated positions
        let (from_pin_pos, to_pin_pos) = match (from_pin_pos, to_pin_pos) {
            (Some(&from_pos), Some(&to_pos)) => (from_pos, to_pos),
            _ => {
                // Fallback to calculated positions if cache is empty (first frame)
                let Some(from_node_pos) = node_positions.get(&connection.from_entity) else { return; };
                let Some(to_node_pos) = node_positions.get(&connection.to_entity) else { return; };
                
                let from_calc = self.calculate_output_pin_position(*from_node_pos, connection.from_pin_index, connection.from_entity, size_cache);
                let to_calc = self.calculate_input_pin_position(*to_node_pos);
                (from_calc, to_calc)
            }
        };
        
        // Get animated color for this connection
        let connection_color = connection_animations.get_connection_color(connection.from_entity, connection.to_entity);
        
        // Draw bezier curve with animated color
        self.draw_bezier_connection(ui, from_pin_pos, to_pin_pos, &connection.connection_type, connection_color);
    }

    /// Calculate the world position of an output pin based on node layout
    fn calculate_output_pin_position(
        &self,
        node_pos: Vec2, 
        pin_index: usize, 
        entity: Entity, 
        size_cache: &NodeSizeCache
    ) -> egui::Pos2 {
        let header_height = 50.0; // Approximate header height
        let pin_spacing = 25.0; // Space between output pins in body
        let body_start_y = header_height + 20.0; // After header + separator
        let transitions_label_height = 20.0; // Height for "Transitions:" label
        
        let y_offset = body_start_y + transitions_label_height + (pin_index as f32 * pin_spacing) + 12.0; // Center of pin row
        
        // Get actual node width from size cache, or use default
        let node_width = size_cache.sizes.get(&entity)
            .map(|size| size.x)
            .unwrap_or(200.0);
        
        // Output pins are on the right side of the node
        egui::Pos2::new(
            node_pos.x + node_width - 12.0, // Small offset from right edge
            node_pos.y + y_offset
        )
    }

    /// Calculate the world position of an input pin in the header
    fn calculate_input_pin_position(
        &self,
        node_pos: Vec2, 
    ) -> egui::Pos2 {
        // Input pin is in the header, left side
        egui::Pos2::new(
            node_pos.x + 12.0, // Small offset from left edge
            node_pos.y + 25.0  // Middle of header
        )
    }

    /// Draw a bezier curve connection between two points
    fn draw_bezier_connection(&self, ui: &mut egui::Ui, from: egui::Pos2, to: egui::Pos2, _label: &str, color: egui::Color32) {
        let stroke = egui::Stroke::new(2.0, color);
        
        // Create bezier control points for right-to-left connection (output to input)
        let control_distance = 80.0;
        let control1 = egui::Pos2::new(from.x + control_distance, from.y); // Control point extends right from output pin
        let control2 = egui::Pos2::new(to.x - control_distance, to.y);     // Control point extends left from input pin
        
        // Draw the bezier curve by sampling points
        let samples = 32;
        let mut points = Vec::with_capacity(samples + 1);
        
        for i in 0..=samples {
            let t = i as f32 / samples as f32;
            let point = self.sample_cubic_bezier(from, control1, control2, to, t);
            points.push(point);
        }
        
        // Draw the curve as connected line segments
        for i in 0..points.len() - 1 {
            ui.painter().line_segment([points[i], points[i + 1]], stroke);
        }
    }

    /// Sample a cubic bezier curve at parameter t
    fn sample_cubic_bezier(&self, p0: egui::Pos2, p1: egui::Pos2, p2: egui::Pos2, p3: egui::Pos2, t: f32) -> egui::Pos2 {
        let t2 = t * t;
        let t3 = t2 * t;
        let mt = 1.0 - t;
        let mt2 = mt * mt;
        let mt3 = mt2 * mt;
        
        egui::Pos2::new(
            mt3 * p0.x + 3.0 * mt2 * t * p1.x + 3.0 * mt * t2 * p2.x + t3 * p3.x,
            mt3 * p0.y + 3.0 * mt2 * t * p1.y + 3.0 * mt * t2 * p2.y + t3 * p3.y,
        )
    }
}