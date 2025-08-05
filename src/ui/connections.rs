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

    /// Draw a single connection line between closest edge pins
    fn draw_connection_line(
        &self,
        ui: &mut egui::Ui,
        connection: &Connection,
        _node_positions: &HashMap<Entity, Vec2>,
        _size_cache: &NodeSizeCache,
        pin_cache: &PinPositionCache,
        connection_animations: &ConnectionAnimations,
    ) {
        // Special handling for initial state connections (from_pin_index == usize::MAX)
        if connection.from_pin_index == usize::MAX {
            // This is an initial state connection - use fixed pin position
            let initial_pin_pos = ui.min_rect().min + egui::Vec2::new(20.0 + 6.0, 20.0 + 6.0); // Same as root pin position
            
            // Get edge pins for target node
            let Some(to_edge_pins) = pin_cache.edge_pins.get(&connection.to_entity) else { return; };
            
            // Find closest edge pin on target
            let pins = [to_edge_pins.top, to_edge_pins.right, to_edge_pins.bottom, to_edge_pins.left];
            let mut min_distance = f32::INFINITY;
            let mut best_pin = to_edge_pins.top;
            
            for pin in pins {
                let distance = initial_pin_pos.distance(pin);
                if distance < min_distance {
                    min_distance = distance;
                    best_pin = pin;
                }
            }
            
            // Get animated color for this connection
            let connection_color = connection_animations.get_connection_color(connection.from_entity, connection.to_entity);
            
            // Draw Manhattan-style connection with animated color
            self.draw_manhattan_connection(ui, initial_pin_pos, best_pin, &connection.connection_type, connection_color);
            return;
        }
        
        // Regular connections between edge pins
        let Some(from_edge_pins) = pin_cache.edge_pins.get(&connection.from_entity) else { return; };
        let Some(to_edge_pins) = pin_cache.edge_pins.get(&connection.to_entity) else { return; };
        
        // Find the closest pair of edge pins
        let (from_pin_pos, to_pin_pos) = from_edge_pins.get_closest_pins(to_edge_pins);
        
        // Get animated color for this connection
        let connection_color = connection_animations.get_connection_color(connection.from_entity, connection.to_entity);
        
        // Draw Manhattan-style connection with animated color
        self.draw_manhattan_connection(ui, from_pin_pos, to_pin_pos, &connection.connection_type, connection_color);
    }

    /// Calculate the world position of an output pin based on node layout (unused after edge pin refactor)
    #[allow(dead_code)]
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

    /// Calculate the world position of an input pin in the header (unused after edge pin refactor)
    #[allow(dead_code)]
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

    /// Draw a Manhattan-style connection between two points with rounded corners and arrow head
    fn draw_manhattan_connection(&self, ui: &mut egui::Ui, from: egui::Pos2, to: egui::Pos2, _label: &str, color: egui::Color32) {
        let stroke = egui::Stroke::new(2.0, color);
        let corner_radius = 10.0;
        
        // If the points are aligned (same x or same y), just draw a straight line
        if (from.x - to.x).abs() <= 1.0 || (from.y - to.y).abs() <= 1.0 {
            ui.painter().line_segment([from, to], stroke);
            self.draw_arrow_head(ui, from, to, from, color); // Use from as corner for straight lines
            return;
        }
        
        // Calculate Manhattan routing - use horizontal-first approach
        // This creates an L-shaped path: from -> corner -> to
        let corner = egui::Pos2::new(to.x, from.y);
        
        // Calculate distances for each segment
        let horizontal_distance = (corner.x - from.x).abs();
        let vertical_distance = (to.y - corner.y).abs();
        
        // Only add rounded corners if both segments are long enough
        if horizontal_distance > corner_radius * 2.0 && vertical_distance > corner_radius * 2.0 {
            // Draw with rounded corner
            self.draw_rounded_manhattan_path(ui, from, corner, to, corner_radius, stroke);
        } else {
            // Fall back to sharp corners if segments are too short
            if horizontal_distance > 1.0 {
                ui.painter().line_segment([from, corner], stroke);
            }
            if vertical_distance > 1.0 {
                ui.painter().line_segment([corner, to], stroke);
            }
        }
        
        // Draw arrow head at the target point
        self.draw_arrow_head(ui, from, to, corner, color);
    }
    
    /// Draw a Manhattan path with rounded corners using bezier curves
    fn draw_rounded_manhattan_path(&self, ui: &mut egui::Ui, from: egui::Pos2, corner: egui::Pos2, to: egui::Pos2, radius: f32, stroke: egui::Stroke) {
        // Determine the direction of the turn
        let horizontal_dir = if corner.x > from.x { 1.0 } else { -1.0 };
        let vertical_dir = if to.y > corner.y { 1.0 } else { -1.0 };
        
        // Calculate the points where we start and end the curve
        let curve_start = egui::Pos2::new(corner.x - radius * horizontal_dir, from.y);
        let curve_end = egui::Pos2::new(corner.x, corner.y + radius * vertical_dir);
        
        // Draw the straight segments
        // Horizontal segment (from start to curve start)
        if (from.x - curve_start.x).abs() > 1.0 {
            ui.painter().line_segment([from, curve_start], stroke);
        }
        
        // Vertical segment (from curve end to target)
        if (curve_end.y - to.y).abs() > 1.0 {
            ui.painter().line_segment([curve_end, to], stroke);
        }
        
        // Draw the rounded corner using a bezier curve
        self.draw_bezier_corner(ui, curve_start, curve_end, corner, radius, stroke);
    }
    
    /// Draw a smooth bezier curve for the rounded corner
    fn draw_bezier_corner(&self, ui: &mut egui::Ui, start: egui::Pos2, end: egui::Pos2, corner: egui::Pos2, radius: f32, stroke: egui::Stroke) {
        // Create control points for a smooth quarter-circle-like bezier curve
        // Each control point should be positioned along the tangent direction from its respective endpoint
        let control_distance = radius * 0.552; // Magic number for approximating quarter circle with bezier
        
        // Determine which direction each segment is going
        // start -> corner direction (for control1)
        let start_to_corner_dir = egui::Vec2::new(corner.x - start.x, corner.y - start.y).normalized();
        
        // corner -> end direction (for control2) 
        let corner_to_end_dir = egui::Vec2::new(end.x - corner.x, end.y - corner.y).normalized();
        
        // Control point 1: extends from start point toward the corner
        let control1 = start + start_to_corner_dir * control_distance;
        
        // Control point 2: extends from end point back toward the corner
        let control2 = end - corner_to_end_dir * control_distance;
        
        // Draw the bezier curve
        ui.painter().add(egui::epaint::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
            [start, control1, control2, end],
            false, // Not closed
            egui::epaint::Color32::TRANSPARENT, // No fill
            stroke,
        )));
    }
    
    /// Draw an arrow head at the target point based on the connection direction
    fn draw_arrow_head(&self, ui: &mut egui::Ui, from: egui::Pos2, to: egui::Pos2, corner: egui::Pos2, color: egui::Color32) {
        let arrow_size = 8.0;
        
        // Determine the direction of the final segment to point the arrow correctly
        let arrow_points = if (from.x - to.x).abs() <= 1.0 {
            // Straight vertical line
            if to.y > from.y {
                // Arrow pointing down
                vec![
                    to,
                    egui::Pos2::new(to.x - arrow_size/2.0, to.y - arrow_size),
                    egui::Pos2::new(to.x + arrow_size/2.0, to.y - arrow_size),
                ]
            } else {
                // Arrow pointing up
                vec![
                    to,
                    egui::Pos2::new(to.x - arrow_size/2.0, to.y + arrow_size),
                    egui::Pos2::new(to.x + arrow_size/2.0, to.y + arrow_size),
                ]
            }
        } else if (from.y - to.y).abs() <= 1.0 {
            // Straight horizontal line
            if to.x > from.x {
                // Arrow pointing right
                vec![
                    to,
                    egui::Pos2::new(to.x - arrow_size, to.y - arrow_size/2.0),
                    egui::Pos2::new(to.x - arrow_size, to.y + arrow_size/2.0),
                ]
            } else {
                // Arrow pointing left
                vec![
                    to,
                    egui::Pos2::new(to.x + arrow_size, to.y - arrow_size/2.0),
                    egui::Pos2::new(to.x + arrow_size, to.y + arrow_size/2.0),
                ]
            }
        } else {
            // L-shaped path - arrow direction is based on the final segment (corner -> to)
            if to.y > corner.y {
                // Final segment goes down - arrow pointing down
                vec![
                    to,
                    egui::Pos2::new(to.x - arrow_size/2.0, to.y - arrow_size),
                    egui::Pos2::new(to.x + arrow_size/2.0, to.y - arrow_size),
                ]
            } else {
                // Final segment goes up - arrow pointing up
                vec![
                    to,
                    egui::Pos2::new(to.x - arrow_size/2.0, to.y + arrow_size),
                    egui::Pos2::new(to.x + arrow_size/2.0, to.y + arrow_size),
                ]
            }
        };
        
        // Draw the filled triangle arrow head
        ui.painter().add(egui::epaint::Shape::convex_polygon(
            arrow_points,
            color,
            egui::Stroke::NONE,
        ));
    }


}