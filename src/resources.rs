use bevy::prelude::*;
use bevy::reflect::Reflect;
use std::collections::{HashMap, HashSet};

// --- Resources ---

#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct GraphState {
    pub selected_nodes: HashSet<Entity>,
    pub next_position: Vec2,
    pub dragging_node: Option<Entity>,
    pub drag_offset: Vec2,
}

/// Resource to track measured sizes of expanded nodes for dynamic sizing
#[derive(Resource, Default)]
pub struct NodeSizeCache {
    pub sizes: HashMap<Entity, egui::Vec2>,
}

/// Enum for specifying which edge of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeSide {
    Top,
    Right,
    Bottom,
    Left,
}

/// Edge positions for a node - supports multiple ports per edge
#[derive(Debug, Clone)]
pub struct EdgePins {
    pub top: Vec<egui::Pos2>,
    pub right: Vec<egui::Pos2>,
    pub bottom: Vec<egui::Pos2>,
    pub left: Vec<egui::Pos2>,
    pub rect: egui::Rect, // Store the original rect for calculations
}

impl EdgePins {
    pub fn from_rect(rect: egui::Rect) -> Self {
        // Start with single center pins per edge (will be updated with port distribution)
        let center = rect.center();
        Self {
            top: vec![egui::Pos2::new(center.x, rect.min.y)],
            right: vec![egui::Pos2::new(rect.max.x, center.y)],
            bottom: vec![egui::Pos2::new(center.x, rect.max.y)],
            left: vec![egui::Pos2::new(rect.min.x, center.y)],
            rect,
        }
    }
    
    /// Distribute ports along an edge based on the number of connections needed
    pub fn distribute_ports_on_edge(&mut self, edge: EdgeSide, count: usize) {
        if count == 0 {
            return;
        }
        
        let ports = match edge {
            EdgeSide::Top => {
                let y = self.rect.min.y;
                if count == 1 {
                    vec![egui::Pos2::new(self.rect.center().x, y)]
                } else {
                    (0..count).map(|i| {
                        let t = (i + 1) as f32 / (count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        egui::Pos2::new(x, y)
                    }).collect()
                }
            }
            EdgeSide::Right => {
                let x = self.rect.max.x;
                if count == 1 {
                    vec![egui::Pos2::new(x, self.rect.center().y)]
                } else {
                    (0..count).map(|i| {
                        let t = (i + 1) as f32 / (count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        egui::Pos2::new(x, y)
                    }).collect()
                }
            }
            EdgeSide::Bottom => {
                let y = self.rect.max.y;
                if count == 1 {
                    vec![egui::Pos2::new(self.rect.center().x, y)]
                } else {
                    (0..count).map(|i| {
                        let t = (i + 1) as f32 / (count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        egui::Pos2::new(x, y)
                    }).collect()
                }
            }
            EdgeSide::Left => {
                let x = self.rect.min.x;
                if count == 1 {
                    vec![egui::Pos2::new(x, self.rect.center().y)]
                } else {
                    (0..count).map(|i| {
                        let t = (i + 1) as f32 / (count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        egui::Pos2::new(x, y)
                    }).collect()
                }
            }
        };
        
        match edge {
            EdgeSide::Top => self.top = ports,
            EdgeSide::Right => self.right = ports,
            EdgeSide::Bottom => self.bottom = ports,
            EdgeSide::Left => self.left = ports,
        }
    }
    
    /// Distribute ports along an edge, separating incoming and outgoing connections
    pub fn distribute_ports_on_edge_with_direction(&mut self, edge: EdgeSide, outgoing: &[Entity], incoming: &[Entity]) {
        let outgoing_count = outgoing.len();
        let incoming_count = incoming.len();
        let total_count = outgoing_count + incoming_count;
        
        if total_count == 0 {
            return;
        }
        
        let ports = match edge {
            EdgeSide::Top => {
                let y = self.rect.min.y;
                if total_count == 1 {
                    vec![egui::Pos2::new(self.rect.center().x, y)]
                } else {
                    let mut all_ports = Vec::new();
                    
                    // Place outgoing connections first, then incoming
                    for i in 0..outgoing_count {
                        let t = (i + 1) as f32 / (total_count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    for i in 0..incoming_count {
                        let t = (outgoing_count + i + 1) as f32 / (total_count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    all_ports
                }
            }
            EdgeSide::Right => {
                let x = self.rect.max.x;
                if total_count == 1 {
                    vec![egui::Pos2::new(x, self.rect.center().y)]
                } else {
                    let mut all_ports = Vec::new();
                    
                    // Place outgoing connections first, then incoming
                    for i in 0..outgoing_count {
                        let t = (i + 1) as f32 / (total_count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    for i in 0..incoming_count {
                        let t = (outgoing_count + i + 1) as f32 / (total_count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    all_ports
                }
            }
            EdgeSide::Bottom => {
                let y = self.rect.max.y;
                if total_count == 1 {
                    vec![egui::Pos2::new(self.rect.center().x, y)]
                } else {
                    let mut all_ports = Vec::new();
                    
                    // Place outgoing connections first, then incoming
                    for i in 0..outgoing_count {
                        let t = (i + 1) as f32 / (total_count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    for i in 0..incoming_count {
                        let t = (outgoing_count + i + 1) as f32 / (total_count + 1) as f32;
                        let x = self.rect.min.x + t * self.rect.width();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    all_ports
                }
            }
            EdgeSide::Left => {
                let x = self.rect.min.x;
                if total_count == 1 {
                    vec![egui::Pos2::new(x, self.rect.center().y)]
                } else {
                    let mut all_ports = Vec::new();
                    
                    // Place outgoing connections first, then incoming
                    for i in 0..outgoing_count {
                        let t = (i + 1) as f32 / (total_count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    for i in 0..incoming_count {
                        let t = (outgoing_count + i + 1) as f32 / (total_count + 1) as f32;
                        let y = self.rect.min.y + t * self.rect.height();
                        all_ports.push(egui::Pos2::new(x, y));
                    }
                    
                    all_ports
                }
            }
        };
        
        match edge {
            EdgeSide::Top => self.top = ports,
            EdgeSide::Right => self.right = ports,
            EdgeSide::Bottom => self.bottom = ports,
            EdgeSide::Left => self.left = ports,
        }
    }
    
    pub fn get_closest_pins(&self, other: &EdgePins) -> (egui::Pos2, egui::Pos2) {
        let mut min_distance = f32::INFINITY;
        let mut best_pair = (self.top[0], other.top[0]); // Default fallback
        
        // Check all combinations of pins from all edges
        for edge_pins in [&self.top, &self.right, &self.bottom, &self.left] {
            for from_pin in edge_pins {
                for other_edge_pins in [&other.top, &other.right, &other.bottom, &other.left] {
                    for to_pin in other_edge_pins {
                        let distance = from_pin.distance(*to_pin);
                        if distance < min_distance {
                            min_distance = distance;
                            best_pair = (*from_pin, *to_pin);
                        }
                    }
                }
            }
        }
        
        best_pair
    }
    
    /// Get all pins from all edges (for backward compatibility)
    pub fn get_all_pins(&self) -> Vec<egui::Pos2> {
        let mut all_pins = Vec::new();
        all_pins.extend(&self.top);
        all_pins.extend(&self.right);
        all_pins.extend(&self.bottom);
        all_pins.extend(&self.left);
        all_pins
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManhattanRoute {
    HorizontalFirst,
    VerticalFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionShape {
    /// Straight line: direct connection between pins
    Straight,
    /// Simple L-shape: from -> corner -> to
    LShape,
    /// S-shape: from -> corner1 -> corner2 -> to (for parallel connections or complex routing)
    SShape,
}

/// A routed connection with its chosen path
#[derive(Debug, Clone)]
pub struct RoutedConnection {
    pub connection: crate::components::Connection,
    pub route: ManhattanRoute,
    pub shape: ConnectionShape,
    pub from_pin: egui::Pos2,
    pub to_pin: egui::Pos2,
    pub from_edge: EdgeSide, // Which edge the connection starts from
    pub to_edge: EdgeSide,   // Which edge the connection ends at
    pub bend_points: Vec<egui::Pos2>, // All bend points (1 for L-shape, 2+ for S-shape)
    pub stagger_offset: f32, // Offset for bend staggering to avoid overlaps
}

impl RoutedConnection {
    /// Create a simple L-shaped connection (backward compatibility)
    pub fn new(connection: crate::components::Connection, route: ManhattanRoute, from_pin: egui::Pos2, to_pin: egui::Pos2) -> Self {
        let corner = match route {
            ManhattanRoute::HorizontalFirst => egui::Pos2::new(to_pin.x, from_pin.y),
            ManhattanRoute::VerticalFirst => egui::Pos2::new(from_pin.x, to_pin.y),
        };
        
        Self {
            connection,
            route,
            shape: ConnectionShape::LShape,
            from_pin,
            to_pin,
            from_edge: EdgeSide::Right, // Default values - should be set properly
            to_edge: EdgeSide::Left,
            bend_points: vec![corner],
            stagger_offset: 0.0,
        }
    }
    
    /// Create a new routed connection with full control over shape and edges
    pub fn new_with_edges(
        connection: crate::components::Connection,
        route: ManhattanRoute,
        shape: ConnectionShape,
        from_pin: egui::Pos2,
        to_pin: egui::Pos2,
        from_edge: EdgeSide,
        to_edge: EdgeSide,
        stagger_offset: f32,
    ) -> Self {
        let bend_points = Self::calculate_bend_points(
            from_pin, to_pin, from_edge, to_edge, route, shape, stagger_offset
        );
        
        Self {
            connection,
            route,
            shape,
            from_pin,
            to_pin,
            from_edge,
            to_edge,
            bend_points,
            stagger_offset,
        }
    }
    
    /// Calculate bend points based on routing parameters
    fn calculate_bend_points(
        from_pin: egui::Pos2,
        to_pin: egui::Pos2,
        from_edge: EdgeSide,
        to_edge: EdgeSide,
        route: ManhattanRoute,
        shape: ConnectionShape,
        stagger_offset: f32,
    ) -> Vec<egui::Pos2> {
        match shape {
            ConnectionShape::Straight => {
                // No bend points - direct line from source to target
                vec![]
            }
            ConnectionShape::LShape => {
                // Simple L-shape with one bend point
                let corner = match route {
                    ManhattanRoute::HorizontalFirst => egui::Pos2::new(to_pin.x, from_pin.y),
                    ManhattanRoute::VerticalFirst => egui::Pos2::new(from_pin.x, to_pin.y),
                };
                vec![corner]
            }
            ConnectionShape::SShape => {
                // S-shape with two bend points for perpendicular entry/exit
                Self::calculate_s_shape_points(from_pin, to_pin, from_edge, to_edge, route, stagger_offset)
            }
        }
    }
    
    /// Calculate S-shape bend points for perpendicular connections
    fn calculate_s_shape_points(
        from_pin: egui::Pos2,
        to_pin: egui::Pos2,
        from_edge: EdgeSide,
        to_edge: EdgeSide,
        _route: ManhattanRoute,
        stagger_offset: f32,
    ) -> Vec<egui::Pos2> {
        // Calculate the distance between source and target
        let dx = to_pin.x - from_pin.x;
        let dy = to_pin.y - from_pin.y;
        
        // Step 1: Extend perpendicular from source edge by 60% of the distance to target
        let perpendicular_distance = match from_edge {
            EdgeSide::Top | EdgeSide::Bottom => dy.abs() * 0.6,
            EdgeSide::Left | EdgeSide::Right => dx.abs() * 0.6,
        };
        
        // Add stagger offset for parallel connection separation
        let total_perpendicular_distance = perpendicular_distance + stagger_offset;
        
        // Extend perpendicular from source
        let from_extended = Self::extend_perpendicular(from_pin, from_edge, total_perpendicular_distance);
        
        // Step 2: Determine turn direction based on target position
        // Turn toward the target to get closer
        let (turn_direction, connecting_distance) = match from_edge {
            EdgeSide::Top | EdgeSide::Bottom => {
                // We extended vertically, now we need to turn horizontally toward target
                if dx > 0.0 {
                    // Target is to the right, turn right
                    (1.0, dx.abs())
                } else {
                    // Target is to the left, turn left  
                    (-1.0, dx.abs())
                }
            }
            EdgeSide::Left | EdgeSide::Right => {
                // We extended horizontally, now we need to turn vertically toward target
                if dy > 0.0 {
                    // Target is below, turn down
                    (1.0, dy.abs())
                } else {
                    // Target is above, turn up
                    (-1.0, dy.abs())
                }
            }
        };
        
        // Step 3: Calculate the connecting segment endpoint
        let connecting_end = match from_edge {
            EdgeSide::Top | EdgeSide::Bottom => {
                // Turn horizontally from vertical extension
                egui::Pos2::new(from_extended.x + (connecting_distance * turn_direction), from_extended.y)
            }
            EdgeSide::Left | EdgeSide::Right => {
                // Turn vertically from horizontal extension  
                egui::Pos2::new(from_extended.x, from_extended.y + (connecting_distance * turn_direction))
            }
        };
        
        // Step 4: Calculate final approach to target (perpendicular to target edge)
        let to_approach = Self::extend_perpendicular(to_pin, to_edge, 0.0);
        
        // Return the bend points for the S-shape path:
        // from_pin -> from_extended -> connecting_end -> to_approach -> to_pin
        vec![
            from_extended,   // End of perpendicular extension from source
            connecting_end,  // End of connecting segment (turned toward target)
            to_approach,     // Start of final approach to target
        ]
    }
    
    /// Extend a point perpendicular to an edge by a given distance
    fn extend_perpendicular(point: egui::Pos2, edge: EdgeSide, distance: f32) -> egui::Pos2 {
        match edge {
            EdgeSide::Top => egui::Pos2::new(point.x, point.y - distance),
            EdgeSide::Right => egui::Pos2::new(point.x + distance, point.y),
            EdgeSide::Bottom => egui::Pos2::new(point.x, point.y + distance),
            EdgeSide::Left => egui::Pos2::new(point.x - distance, point.y),
        }
    }
    
    /// Check if this route crosses another routed connection
    pub fn crosses(&self, other: &RoutedConnection) -> bool {
        // Get the line segments for both connections
        let self_segments = self.get_line_segments();
        let other_segments = other.get_line_segments();
        
        // Check if any segment from self crosses any segment from other
        for self_seg in &self_segments {
            for other_seg in &other_segments {
                if segments_intersect(*self_seg, *other_seg) {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Get the line segments that make up this route
    fn get_line_segments(&self) -> Vec<(egui::Pos2, egui::Pos2)> {
        let mut segments = Vec::new();
        
        // Create segments from from_pin through all bend points to to_pin
        let mut current_point = self.from_pin;
        
        for &bend_point in &self.bend_points {
            if current_point.distance(bend_point) > 1.0 {
                segments.push((current_point, bend_point));
            }
            current_point = bend_point;
        }
        
        // Final segment to target
        if current_point.distance(self.to_pin) > 1.0 {
            segments.push((current_point, self.to_pin));
        }
        
        // If no segments were created, add a direct connection
        if segments.is_empty() {
            segments.push((self.from_pin, self.to_pin));
        }
        
        segments
    }
}

/// Check if two line segments intersect
fn segments_intersect(seg1: (egui::Pos2, egui::Pos2), seg2: (egui::Pos2, egui::Pos2)) -> bool {
    let (p1, p2) = seg1;
    let (p3, p4) = seg2;
    
    // Check if segments share endpoints (not a crossing)
    if p1 == p3 || p1 == p4 || p2 == p3 || p2 == p4 {
        return false;
    }
    
    // Use the orientation method to check intersection
    fn orientation(p: egui::Pos2, q: egui::Pos2, r: egui::Pos2) -> i32 {
        let val = (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);
        if val.abs() < 1e-6 { 0 } // Collinear
        else if val > 0.0 { 1 } // Clockwise
        else { 2 } // Counterclockwise
    }
    
    fn on_segment(p: egui::Pos2, q: egui::Pos2, r: egui::Pos2) -> bool {
        q.x <= p.x.max(r.x) && q.x >= p.x.min(r.x) &&
        q.y <= p.y.max(r.y) && q.y >= p.y.min(r.y)
    }
    
    let o1 = orientation(p1, p2, p3);
    let o2 = orientation(p1, p2, p4);
    let o3 = orientation(p3, p4, p1);
    let o4 = orientation(p3, p4, p2);
    
    // General case
    if o1 != o2 && o3 != o4 {
        return true;
    }
    
    // Special cases for collinear points
    if o1 == 0 && on_segment(p1, p3, p2) { return true; }
    if o2 == 0 && on_segment(p1, p4, p2) { return true; }
    if o3 == 0 && on_segment(p3, p1, p4) { return true; }
    if o4 == 0 && on_segment(p3, p2, p4) { return true; }
    
    false
}

/// Resource to track actual pin positions for accurate connection drawing
#[derive(Resource, Default)]
pub struct PinPositionCache {
    pub edge_pins: HashMap<Entity, EdgePins>,
}

/// Resource to track component addition dialog state
#[derive(Resource, Default)]
pub struct ComponentDialogState {
    pub open_for_entity: Option<Entity>,
    pub selected_component: Option<String>,
}

/// Resource to track transition creation workflow state
#[derive(Resource, Default)]
pub struct TransitionCreationState {
    pub source_entity: Option<Entity>,
    pub selected_event_type: Option<String>,
    pub selecting_target: bool,
}

/// Resource to track the currently selected entity for render ordering
#[derive(Resource, Default)]
pub struct SelectedEntity {
    pub entity: Option<Entity>,
}

/// Resource to track drag-and-drop state for parent-child zone interactions
#[derive(Resource, Default)]
pub struct DragDropState {
    /// The entity currently being dragged (if any)
    pub dragging_entity: Option<Entity>,
    /// The parent zone currently being hovered over during drag (if any)
    pub hover_zone_entity: Option<Entity>,
    /// Whether the current drag would result in a valid parent-child relationship
    pub would_create_child_relationship: bool,
    /// All children entities that should follow the dragged parent
    pub dragging_children: Vec<Entity>,
    /// Initial positions of children relative to their dragged parent
    pub children_initial_positions: HashMap<Entity, Vec2>,
    /// Initial position of the dragged parent (for calculating deltas)
    pub parent_initial_position: Option<Vec2>,
    /// The entity currently being resized (if any)
    pub resizing_entity: Option<Entity>,
    /// Which edge is being resized (Right, Bottom, or Corner for both)
    pub resize_edge: Option<ResizeEdge>,
    /// Initial bounds of the zone being resized
    pub initial_zone_bounds: Option<bevy::math::Rect>,
    /// Initial mouse position when resize started
    pub resize_start_mouse_pos: Option<Vec2>,
}

/// Represents which edge of a parent zone is being resized
#[derive(Debug, Clone, PartialEq)]
pub enum ResizeEdge {
    Right,
    Bottom,
    Corner, // Both right and bottom
}

/// Tracks animations for connection visual feedback when transitions fire
#[derive(Resource, Default)]
pub struct ConnectionAnimations {
    /// Map of (source_entity, target_entity) -> remaining animation time in seconds
    pub active_animations: std::collections::HashMap<(Entity, Entity), f32>,
}

impl ConnectionAnimations {
    /// Start a new animation for a connection (0.25 seconds)
    pub fn start_animation(&mut self, source: Entity, target: Entity) {
        self.active_animations.insert((source, target), 0.25);
        println!("🟡 Started transition animation: {:?} -> {:?}", source, target);
    }
    
    /// Update all animations with delta time, returns list of completed animations
    pub fn update(&mut self, delta_time: f32) -> Vec<(Entity, Entity)> {
        let mut completed = Vec::new();
        
        // Update existing animations
        self.active_animations.retain(|(source, target), timer| {
            *timer -= delta_time;
            if *timer <= 0.0 {
                completed.push((*source, *target));
                false // Remove completed animation
            } else {
                true // Keep active animation
            }
        });
        
        completed
    }
    
    /// Get the animation progress for a connection (0.0 = just started gold, 1.0 = back to normal)
    pub fn get_animation_progress(&self, source: Entity, target: Entity) -> Option<f32> {
        self.active_animations.get(&(source, target))
            .map(|remaining_time| {
                // Convert remaining time to progress (0.0 = gold, 1.0 = normal)
                1.0 - (remaining_time / 0.25)
            })
    }
    
    /// Check if a connection is currently animated
    pub fn is_animated(&self, source: Entity, target: Entity) -> bool {
        self.active_animations.contains_key(&(source, target))
    }
    
    /// Get the interpolated color for a connection based on animation progress
    pub fn get_connection_color(&self, source: Entity, target: Entity) -> egui::Color32 {
        if let Some(progress) = self.get_animation_progress(source, target) {
            // Lerp from gold (255, 215, 0) to normal blue (150, 150, 255)
            let gold = egui::Color32::from_rgb(255, 215, 0);
            let normal = egui::Color32::from_rgb(150, 150, 255);
            
            let r = (gold.r() as f32 * (1.0 - progress) + normal.r() as f32 * progress) as u8;
            let g = (gold.g() as f32 * (1.0 - progress) + normal.g() as f32 * progress) as u8;
            let b = (gold.b() as f32 * (1.0 - progress) + normal.b() as f32 * progress) as u8;
            
            egui::Color32::from_rgb(r, g, b)
        } else {
            // Default connection color - nice blue
            egui::Color32::from_rgb(150, 150, 255)
        }
    }
}