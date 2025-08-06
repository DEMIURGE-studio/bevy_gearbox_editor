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
        _size_cache: &NodeSizeCache, 
        pin_cache: &mut PinPositionCache, // Changed to mutable for port distribution
        connection_animations: &ConnectionAnimations,
    ) {
        // Get all connections
        let connections: Vec<Connection> = world.query::<&Connection>().iter(world).cloned().collect();
        
        // Distribute ports based on connections and create port assignments
        let port_assignments = self.distribute_ports_for_connections(&connections, pin_cache);
        
        // Get node positions and pins for connection endpoints  
        let (_node_positions, _node_pins) = self.collect_node_data(world);
        
        // Route connections with crossing avoidance using assigned ports
        let routed_connections = self.route_connections_with_crossing_avoidance(&connections, pin_cache, &port_assignments);
        
        // Draw each routed connection
        for routed_connection in routed_connections {
            self.draw_routed_connection(ui, &routed_connection, connection_animations);
        }
    }
    
    /// Enhanced greedy port assignment with lookahead to minimize crossings
    /// Returns a map of connection -> (from_pin, to_pin) assignments
    fn distribute_ports_for_connections(&self, connections: &[Connection], pin_cache: &mut PinPositionCache) -> std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)> {
        // Filter out initial state connections (they have special handling)
        let regular_connections: Vec<_> = connections.iter()
            .filter(|conn| conn.from_pin_index != usize::MAX)
            .collect();
        
        if regular_connections.is_empty() {
            return std::collections::HashMap::new();
        }
        
        // Use enhanced greedy algorithm with lookahead
        self.assign_ports_with_lookahead(&regular_connections, pin_cache)
    }
    
    /// Determine which edges should be used for a connection
    fn determine_connection_edges(&self, from_pins: &crate::resources::EdgePins, to_pins: &crate::resources::EdgePins) -> (crate::resources::EdgeSide, crate::resources::EdgeSide) {
        // Simple heuristic: use the geometric relationship between node centers
        let from_center = from_pins.rect.center();
        let to_center = to_pins.rect.center();
        
        let dx = to_center.x - from_center.x;
        let dy = to_center.y - from_center.y;
        
        if dx.abs() > dy.abs() {
            // Primarily horizontal relationship
            if dx > 0.0 {
                (crate::resources::EdgeSide::Right, crate::resources::EdgeSide::Left) // from goes right, to receives from left
            } else {
                (crate::resources::EdgeSide::Left, crate::resources::EdgeSide::Right) // from goes left, to receives from right
            }
        } else {
            // Primarily vertical relationship
            if dy > 0.0 {
                (crate::resources::EdgeSide::Bottom, crate::resources::EdgeSide::Top) // from goes down, to receives from top
            } else {
                (crate::resources::EdgeSide::Top, crate::resources::EdgeSide::Bottom) // from goes up, to receives from bottom
            }
        }
    }
    

    /// Route connections with crossing avoidance using greedy algorithm
    fn route_connections_with_crossing_avoidance(&self, connections: &[Connection], pin_cache: &PinPositionCache, port_assignments: &std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)>) -> Vec<crate::resources::RoutedConnection> {
        
        let mut routed_connections = Vec::new();
        
        // Separate initial state connections (handle them specially)
        let (initial_connections, regular_connections): (Vec<_>, Vec<_>) = connections.iter()
            .partition(|conn| conn.from_pin_index == usize::MAX);
        
        // Handle initial state connections first (they have fixed routing)
        for connection in initial_connections {
            if let Some(routed) = self.route_initial_state_connection(connection, pin_cache) {
                routed_connections.push(routed);
            }
        }
        
        // Sort regular connections by "constraint level" (most constrained first)
        let mut sorted_connections = regular_connections;
        sorted_connections.sort_by(|a, b| {
            let constraint_a = self.calculate_connection_constraint(a, pin_cache);
            let constraint_b = self.calculate_connection_constraint(b, pin_cache);
            constraint_b.partial_cmp(&constraint_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Route each connection greedily
        for connection in sorted_connections {
            if let Some(routed) = self.route_connection_with_crossing_avoidance(connection, &routed_connections, pin_cache, port_assignments) {
                routed_connections.push(routed);
            }
        }
        
        routed_connections
    }
    
    /// Route a single connection, choosing the route with fewer crossings
    fn route_connection_with_crossing_avoidance(&self, connection: &Connection, existing_routes: &[RoutedConnection], pin_cache: &PinPositionCache, port_assignments: &std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)>) -> Option<RoutedConnection> {
        // Get assigned pins for this specific connection
        let (from_pin, to_pin) = if let Some(&(from_pos, to_pos)) = port_assignments.get(&(connection.from_entity, connection.to_entity)) {
            (from_pos, to_pos)
        } else {
            // Fallback to closest pins if no assignment found
            let from_edge_pins = pin_cache.edge_pins.get(&connection.from_entity)?;
            let to_edge_pins = pin_cache.edge_pins.get(&connection.to_entity)?;
            from_edge_pins.get_closest_pins(to_edge_pins)
        };
        
        // Determine edge sides for this connection
        let from_edge_pins = pin_cache.edge_pins.get(&connection.from_entity)?;
        let to_edge_pins = pin_cache.edge_pins.get(&connection.to_entity)?;
        let (from_edge, to_edge) = self.determine_connection_edges(from_edge_pins, to_edge_pins);
        
        // Determine the best routing strategy based on geometry and constraints
        let shape = self.determine_routing_strategy(connection, existing_routes, from_pin, to_pin, from_edge, to_edge);
        
        // Calculate stagger offset based on parallel connections
        let stagger_offset = self.calculate_stagger_offset(connection, existing_routes, from_pin, to_pin, from_edge, to_edge);
        
        // Try both routing options with the determined shape
        let horizontal_route = crate::resources::RoutedConnection::new_with_edges(
            connection.clone(), 
            crate::resources::ManhattanRoute::HorizontalFirst, 
            shape,
            from_pin, 
            to_pin,
            from_edge,
            to_edge,
            stagger_offset
        );
        let vertical_route = crate::resources::RoutedConnection::new_with_edges(
            connection.clone(), 
            crate::resources::ManhattanRoute::VerticalFirst, 
            shape,
            from_pin, 
            to_pin,
            from_edge,
            to_edge,
            stagger_offset
        );
        
        // Count crossings for each route
        let horizontal_crossings = existing_routes.iter().filter(|existing| horizontal_route.crosses(existing)).count();
        let vertical_crossings = existing_routes.iter().filter(|existing| vertical_route.crosses(existing)).count();
        
        // Choose the route with fewer crossings
        if horizontal_crossings <= vertical_crossings {
            Some(horizontal_route)
        } else {
            Some(vertical_route)
        }
    }
    
    /// Determine the best routing strategy based on geometry and constraints
    fn determine_routing_strategy(
        &self,
        _connection: &Connection,
        existing_routes: &[RoutedConnection],
        from_pin: egui::Pos2,
        to_pin: egui::Pos2,
        from_edge: crate::resources::EdgeSide,
        to_edge: crate::resources::EdgeSide,
    ) -> crate::resources::ConnectionShape {
        let dx = to_pin.x - from_pin.x;
        let dy = to_pin.y - from_pin.y;
        let distance = from_pin.distance(to_pin);
        
        // Check if nodes are closely aligned (within 20px tolerance)
        let is_horizontally_aligned = dy.abs() < 20.0;
        let is_vertically_aligned = dx.abs() < 20.0;
        
        // Check if edges are directly opposite and aligned
        let is_direct_opposite = self.are_opposite_edges(from_edge, to_edge);
        
        // 1. STRAIGHT LINE: Use for closely aligned nodes with opposite edges
        if (is_horizontally_aligned || is_vertically_aligned) && is_direct_opposite {
            return crate::resources::ConnectionShape::Straight;
        }
        
        // 2. L-SHAPE: Use for simple cases with good separation
        let is_good_l_shape_candidate = distance > 80.0 && 
            !self.are_same_edge_side(from_edge, to_edge) &&
            !is_direct_opposite;
        
        // Check if L-shape would create a clean path without tight turns
        let l_shape_has_good_segments = match (from_edge, to_edge) {
            // Horizontal-first L-shape
            (crate::resources::EdgeSide::Right, crate::resources::EdgeSide::Left) |
            (crate::resources::EdgeSide::Left, crate::resources::EdgeSide::Right) => {
                dx.abs() > 40.0 && dy.abs() > 40.0
            }
            // Vertical-first L-shape  
            (crate::resources::EdgeSide::Top, crate::resources::EdgeSide::Bottom) |
            (crate::resources::EdgeSide::Bottom, crate::resources::EdgeSide::Top) => {
                dx.abs() > 40.0 && dy.abs() > 40.0
            }
            _ => dx.abs() > 40.0 && dy.abs() > 40.0
        };
        
        if is_good_l_shape_candidate && l_shape_has_good_segments {
            //return crate::resources::ConnectionShape::LShape;
        }
        
        // 3. S-SHAPE: Use for complex cases, close nodes, or same-side connections
        let needs_s_shape = 
            distance < 80.0 ||  // Close nodes need S-shape for clean routing
            self.are_same_edge_side(from_edge, to_edge) || // Same side always needs S-shape
            !l_shape_has_good_segments; // Poor L-shape geometry
        
        // Count parallel connections for staggering decision
        let parallel_count = existing_routes.iter()
            .filter(|route| {
                route.from_edge == from_edge && route.to_edge == to_edge &&
                (route.from_pin.distance(from_pin) < 50.0 || route.to_pin.distance(to_pin) < 50.0)
            })
            .count();
        
        if needs_s_shape || parallel_count > 1 {
            crate::resources::ConnectionShape::SShape
        } else {
            crate::resources::ConnectionShape::SShape
            //crate::resources::ConnectionShape::LShape
        }
    }
    
    /// Check if two edges are on the same side of their respective nodes
    fn are_same_edge_side(&self, edge1: crate::resources::EdgeSide, edge2: crate::resources::EdgeSide) -> bool {
        edge1 == edge2
    }
    
    /// Check if two edges are opposite (top/bottom or left/right)
    fn are_opposite_edges(&self, edge1: crate::resources::EdgeSide, edge2: crate::resources::EdgeSide) -> bool {
        matches!(
            (edge1, edge2),
            (crate::resources::EdgeSide::Top, crate::resources::EdgeSide::Bottom) |
            (crate::resources::EdgeSide::Bottom, crate::resources::EdgeSide::Top) |
            (crate::resources::EdgeSide::Left, crate::resources::EdgeSide::Right) |
            (crate::resources::EdgeSide::Right, crate::resources::EdgeSide::Left)
        )
    }
    
    /// Calculate stagger offset for bend points to avoid overlapping parallel connections
    fn calculate_stagger_offset(
        &self,
        _connection: &Connection,
        existing_routes: &[RoutedConnection],
        from_pin: egui::Pos2,
        to_pin: egui::Pos2,
        from_edge: crate::resources::EdgeSide,
        to_edge: crate::resources::EdgeSide,
    ) -> f32 {
        // Find how many existing routes use similar paths
        let similar_routes: Vec<_> = existing_routes.iter()
            .filter(|route| {
                route.from_edge == from_edge && route.to_edge == to_edge &&
                (route.from_pin.distance(from_pin) < 100.0 && route.to_pin.distance(to_pin) < 100.0)
            })
            .collect();
        
        // Stagger by 15px per similar route
        similar_routes.len() as f32 * 15.0
    }
    
    /// Route an initial state connection (from root entity)
    fn route_initial_state_connection(&self, connection: &Connection, pin_cache: &PinPositionCache) -> Option<RoutedConnection> {
        // Initial state connections use a fixed pin position and closest target pin
        let initial_pin_pos = egui::Pos2::new(20.0 + 6.0, 20.0 + 6.0); // Same as in draw_connection_line
        
        let to_edge_pins = pin_cache.edge_pins.get(&connection.to_entity)?;
        let all_target_pins = to_edge_pins.get_all_pins();
        
        // Find closest target pin
        let mut best_pin = all_target_pins.get(0).copied().unwrap_or_default();
        let mut min_distance = f32::INFINITY;
        
        for pin in all_target_pins {
            let distance = initial_pin_pos.distance(pin);
            if distance < min_distance {
                min_distance = distance;
                best_pin = pin;
            }
        }
        
        // Always use horizontal-first for initial state connections
        Some(crate::resources::RoutedConnection::new(connection.clone(), crate::resources::ManhattanRoute::HorizontalFirst, initial_pin_pos, best_pin))
    }
    
    /// Calculate how constrained a connection is (higher = more constrained = route first)
    fn calculate_connection_constraint(&self, connection: &Connection, pin_cache: &PinPositionCache) -> f32 {
        let from_edge_pins = pin_cache.edge_pins.get(&connection.from_entity);
        let to_edge_pins = pin_cache.edge_pins.get(&connection.to_entity);
        
        if let (Some(from_pins), Some(to_pins)) = (from_edge_pins, to_edge_pins) {
            // Use distance as a simple constraint measure (longer connections are more constrained)
            let (from_pin, to_pin) = from_pins.get_closest_pins(to_pins);
            from_pin.distance(to_pin)
        } else {
            0.0
        }
    }
    
    /// Enhanced greedy algorithm with lookahead for optimal port assignment
    fn assign_ports_with_lookahead(&self, connections: &[&Connection], pin_cache: &mut PinPositionCache) -> std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)> {
        let mut assignments = std::collections::HashMap::new();
        let mut available_ports = self.initialize_available_ports(connections, pin_cache);
        
        // Sort connections by constraint level (most constrained first)
        let mut sorted_connections = connections.to_vec();
        sorted_connections.sort_by(|a, b| {
            let constraint_a = self.calculate_port_constraint(a, &available_ports, pin_cache);
            let constraint_b = self.calculate_port_constraint(b, &available_ports, pin_cache);
            constraint_b.partial_cmp(&constraint_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Assign ports greedily with lookahead
        for (i, connection) in sorted_connections.iter().enumerate() {
            let remaining_connections = &sorted_connections[i + 1..];
            
            if let Some(best_assignment) = self.find_best_port_assignment(
                connection, 
                &available_ports, 
                &assignments, 
                remaining_connections, 
                pin_cache
            ) {
                // Make the assignment
                assignments.insert((connection.from_entity, connection.to_entity), best_assignment);
                
                // Remove used ports from available pools
                self.remove_used_ports(&mut available_ports, &best_assignment, connection, pin_cache);
            }
        }
        
        // Update the actual pin cache with the distributed ports
        self.update_pin_cache_with_assignments(&assignments, connections, pin_cache);
        
        assignments
    }
    
    /// Initialize available port pools for each entity edge
    fn initialize_available_ports(&self, connections: &[&Connection], pin_cache: &PinPositionCache) -> std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>> {
        let mut available_ports = std::collections::HashMap::new();
        let mut entity_edge_counts: std::collections::HashMap<(Entity, crate::resources::EdgeSide), usize> = std::collections::HashMap::new();
        
        // Count how many connections each edge needs to handle
        for connection in connections {
            if let (Some(from_pins), Some(to_pins)) = (
                pin_cache.edge_pins.get(&connection.from_entity),
                pin_cache.edge_pins.get(&connection.to_entity)
            ) {
                let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
                
                *entity_edge_counts.entry((connection.from_entity, from_edge)).or_insert(0) += 1;
                *entity_edge_counts.entry((connection.to_entity, to_edge)).or_insert(0) += 1;
            }
        }
        
        // Generate port positions for each edge based on connection count
        for ((entity, edge_side), count) in entity_edge_counts {
            if let Some(edge_pins) = pin_cache.edge_pins.get(&entity) {
                let ports = self.generate_ports_for_edge(&edge_pins.rect, edge_side, count);
                available_ports.insert((entity, edge_side), ports);
            }
        }
        
        available_ports
    }
    
    /// Generate evenly spaced port positions along an edge
    fn generate_ports_for_edge(&self, rect: &egui::Rect, edge_side: crate::resources::EdgeSide, count: usize) -> Vec<egui::Pos2> {
        if count == 0 {
            return Vec::new();
        }
        
        if count == 1 {
            // Single port at center of edge
            return vec![match edge_side {
                crate::resources::EdgeSide::Top => egui::Pos2::new(rect.center().x, rect.min.y),
                crate::resources::EdgeSide::Right => egui::Pos2::new(rect.max.x, rect.center().y),
                crate::resources::EdgeSide::Bottom => egui::Pos2::new(rect.center().x, rect.max.y),
                crate::resources::EdgeSide::Left => egui::Pos2::new(rect.min.x, rect.center().y),
            }];
        }
        
        // Multiple ports evenly distributed
        let mut ports = Vec::new();
        for i in 0..count {
            let t = (i + 1) as f32 / (count + 1) as f32; // Avoid corners
            let port = match edge_side {
                crate::resources::EdgeSide::Top => {
                    let x = rect.min.x + t * rect.width();
                    egui::Pos2::new(x, rect.min.y)
                }
                crate::resources::EdgeSide::Right => {
                    let y = rect.min.y + t * rect.height();
                    egui::Pos2::new(rect.max.x, y)
                }
                crate::resources::EdgeSide::Bottom => {
                    let x = rect.min.x + t * rect.width();
                    egui::Pos2::new(x, rect.max.y)
                }
                crate::resources::EdgeSide::Left => {
                    let y = rect.min.y + t * rect.height();
                    egui::Pos2::new(rect.min.x, y)
                }
            };
            ports.push(port);
        }
        ports
    }
    
    /// Calculate how constrained a connection is for port assignment
    fn calculate_port_constraint(&self, connection: &Connection, available_ports: &std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>>, pin_cache: &PinPositionCache) -> f32 {
        if let (Some(from_pins), Some(to_pins)) = (
            pin_cache.edge_pins.get(&connection.from_entity),
            pin_cache.edge_pins.get(&connection.to_entity)
        ) {
            let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
            
            let from_available = available_ports.get(&(connection.from_entity, from_edge)).map(|v| v.len()).unwrap_or(0);
            let to_available = available_ports.get(&(connection.to_entity, to_edge)).map(|v| v.len()).unwrap_or(0);
            
            // Higher constraint = fewer available options
            let constraint = 1.0 / ((from_available * to_available) as f32 + 1.0);
            
            // Add distance as secondary factor
            let distance = from_pins.rect.center().distance(to_pins.rect.center());
            constraint + distance * 0.001 // Small weight for distance
        } else {
            0.0
        }
    }
    
    /// Find the best port assignment for a connection considering future connections
    fn find_best_port_assignment(
        &self,
        connection: &Connection,
        available_ports: &std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>>,
        existing_assignments: &std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)>,
        remaining_connections: &[&Connection],
        pin_cache: &PinPositionCache
    ) -> Option<(egui::Pos2, egui::Pos2)> {
        
        if let (Some(from_pins), Some(to_pins)) = (
            pin_cache.edge_pins.get(&connection.from_entity),
            pin_cache.edge_pins.get(&connection.to_entity)
        ) {
            let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
            
            let from_ports = available_ports.get(&(connection.from_entity, from_edge))?;
            let to_ports = available_ports.get(&(connection.to_entity, to_edge))?;
            
            let mut best_assignment = None;
            let mut best_score = f32::INFINITY;
            
            // Try all combinations of available ports
            for &from_port in from_ports {
                for &to_port in to_ports {
                    let score = self.score_port_assignment(
                        (from_port, to_port),
                        connection,
                        existing_assignments,
                        remaining_connections,
                        available_ports,
                        pin_cache
                    );
                    
                    if score < best_score {
                        best_score = score;
                        best_assignment = Some((from_port, to_port));
                    }
                }
            }
            
            best_assignment
        } else {
            None
        }
    }
    
    /// Score a potential port assignment (lower is better)
    fn score_port_assignment(
        &self,
        assignment: (egui::Pos2, egui::Pos2),
        connection: &Connection,
        existing_assignments: &std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)>,
        remaining_connections: &[&Connection],
        available_ports: &std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>>,
        pin_cache: &PinPositionCache
    ) -> f32 {
        let mut score = 0.0;
        
        // Create temporary routed connections for this assignment
        let temp_route_h = crate::resources::RoutedConnection::new(
            connection.clone(), 
            crate::resources::ManhattanRoute::HorizontalFirst, 
            assignment.0, 
            assignment.1
        );
        let temp_route_v = crate::resources::RoutedConnection::new(
            connection.clone(), 
            crate::resources::ManhattanRoute::VerticalFirst, 
            assignment.0, 
            assignment.1
        );
        
        // Count crossings with existing assignments
        for ((_from, _to), &(existing_from, existing_to)) in existing_assignments {
            let existing_route_h = crate::resources::RoutedConnection::new(
                Connection {
                    from_entity: *_from,
                    to_entity: *_to,
                    connection_type: String::new(), // Placeholder
                    from_pin_index: 0,
                    to_pin_index: 0,
                },
                crate::resources::ManhattanRoute::HorizontalFirst,
                existing_from,
                existing_to
            );
            
            let existing_route_v = crate::resources::RoutedConnection::new(
                Connection {
                    from_entity: *_from,
                    to_entity: *_to,
                    connection_type: String::new(), // Placeholder
                    from_pin_index: 0,
                    to_pin_index: 0,
                },
                crate::resources::ManhattanRoute::VerticalFirst,
                existing_from,
                existing_to
            );
            
            // Count crossings for both routing options
            let h_crossings: f32 = if temp_route_h.crosses(&existing_route_h) { 1.0 } else { 0.0 } +
                                  if temp_route_h.crosses(&existing_route_v) { 1.0 } else { 0.0 };
            let v_crossings: f32 = if temp_route_v.crosses(&existing_route_h) { 1.0 } else { 0.0 } +
                                  if temp_route_v.crosses(&existing_route_v) { 1.0 } else { 0.0 };
            
            // Add the minimum crossings to score
            score += h_crossings.min(v_crossings);
        }
        
        // Lookahead: estimate impact on future connections
        let future_impact = self.estimate_future_impact(
            assignment,
            remaining_connections,
            available_ports,
            pin_cache
        );
        score += future_impact * 0.5; // Weight future impact less than immediate crossings
        
        // Add distance penalty (prefer shorter connections)
        let distance = assignment.0.distance(assignment.1);
        score += distance * 0.0001; // Very small weight for distance
        
        score
    }
    
    /// Estimate how this assignment might impact future connections
    fn estimate_future_impact(
        &self,
        assignment: (egui::Pos2, egui::Pos2),
        remaining_connections: &[&Connection],
        available_ports: &std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>>,
        pin_cache: &PinPositionCache
    ) -> f32 {
        let mut impact = 0.0;
        
        // Create routes for this assignment
        let temp_route_h = crate::resources::RoutedConnection::new(
            Connection {
                from_entity: bevy::prelude::Entity::PLACEHOLDER,
                to_entity: bevy::prelude::Entity::PLACEHOLDER,
                connection_type: String::new(),
                from_pin_index: 0,
                to_pin_index: 0,
            },
            crate::resources::ManhattanRoute::HorizontalFirst,
            assignment.0,
            assignment.1
        );
        
        // For each remaining connection, estimate how many options would cross with this assignment
        for future_conn in remaining_connections {
            if let (Some(from_pins), Some(to_pins)) = (
                pin_cache.edge_pins.get(&future_conn.from_entity),
                pin_cache.edge_pins.get(&future_conn.to_entity)
            ) {
                let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
                
                if let (Some(future_from_ports), Some(future_to_ports)) = (
                    available_ports.get(&(future_conn.from_entity, from_edge)),
                    available_ports.get(&(future_conn.to_entity, to_edge))
                ) {
                    let mut crossing_options = 0;
                    let total_options = future_from_ports.len() * future_to_ports.len();
                    
                    for &future_from in future_from_ports {
                        for &future_to in future_to_ports {
                            let future_route = crate::resources::RoutedConnection::new(
                                (*future_conn).clone(),
                                crate::resources::ManhattanRoute::HorizontalFirst,
                                future_from,
                                future_to
                            );
                            
                            if temp_route_h.crosses(&future_route) {
                                crossing_options += 1;
                            }
                        }
                    }
                    
                    // Add penalty based on how many future options we're blocking
                    if total_options > 0 {
                        impact += crossing_options as f32 / total_options as f32;
                    }
                }
            }
        }
        
        impact
    }
    
    /// Remove used ports from available pools
    fn remove_used_ports(
        &self, 
        available_ports: &mut std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>>, 
        assignment: &(egui::Pos2, egui::Pos2), 
        connection: &Connection,
        pin_cache: &PinPositionCache
    ) {
        if let (Some(from_pins), Some(to_pins)) = (
            pin_cache.edge_pins.get(&connection.from_entity),
            pin_cache.edge_pins.get(&connection.to_entity)
        ) {
            let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
            
            // Remove the used ports from available pools
            if let Some(from_ports) = available_ports.get_mut(&(connection.from_entity, from_edge)) {
                from_ports.retain(|&p| (p.x - assignment.0.x).abs() > 1.0 || (p.y - assignment.0.y).abs() > 1.0);
            }
            if let Some(to_ports) = available_ports.get_mut(&(connection.to_entity, to_edge)) {
                to_ports.retain(|&p| (p.x - assignment.1.x).abs() > 1.0 || (p.y - assignment.1.y).abs() > 1.0);
            }
        }
    }
    
    /// Update the pin cache with the final port assignments
    fn update_pin_cache_with_assignments(
        &self, 
        assignments: &std::collections::HashMap<(Entity, Entity), (egui::Pos2, egui::Pos2)>, 
        connections: &[&Connection], 
        pin_cache: &mut PinPositionCache
    ) {
        // Group assignments by entity and edge
        let mut entity_edge_ports: std::collections::HashMap<(Entity, crate::resources::EdgeSide), Vec<egui::Pos2>> = std::collections::HashMap::new();
        
        for connection in connections {
            if let Some(&(from_pos, to_pos)) = assignments.get(&(connection.from_entity, connection.to_entity)) {
                if let (Some(from_pins), Some(to_pins)) = (
                    pin_cache.edge_pins.get(&connection.from_entity),
                    pin_cache.edge_pins.get(&connection.to_entity)
                ) {
                    let (from_edge, to_edge) = self.determine_connection_edges(from_pins, to_pins);
                    
                    entity_edge_ports.entry((connection.from_entity, from_edge)).or_default().push(from_pos);
                    entity_edge_ports.entry((connection.to_entity, to_edge)).or_default().push(to_pos);
                }
            }
        }
        
        // Update the pin cache
        for ((entity, edge_side), mut ports) in entity_edge_ports {
            if let Some(edge_pins) = pin_cache.edge_pins.get_mut(&entity) {
                // Remove duplicates and sort for consistency
                ports.sort_by(|a, b| {
                    match edge_side {
                        crate::resources::EdgeSide::Top | crate::resources::EdgeSide::Bottom => a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal),
                        crate::resources::EdgeSide::Left | crate::resources::EdgeSide::Right => a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal),
                    }
                });
                ports.dedup();
                
                match edge_side {
                    crate::resources::EdgeSide::Top => edge_pins.top = ports,
                    crate::resources::EdgeSide::Right => edge_pins.right = ports,
                    crate::resources::EdgeSide::Bottom => edge_pins.bottom = ports,
                    crate::resources::EdgeSide::Left => edge_pins.left = ports,
                }
            }
        }
    }
    
    /// Draw a routed connection with its chosen route
    fn draw_routed_connection(&self, ui: &mut egui::Ui, routed_connection: &crate::resources::RoutedConnection, connection_animations: &ConnectionAnimations) {
        // Get animated color for this connection
        let connection_color = connection_animations.get_connection_color(
            routed_connection.connection.from_entity, 
            routed_connection.connection.to_entity
        );
        
        // Draw the connection using the enhanced drawing system
        self.draw_enhanced_connection(ui, routed_connection, connection_color);
    }
    
    /// Draw Manhattan connection with specified route (backward compatibility)
    #[allow(dead_code)]
    fn draw_manhattan_connection_with_route(&self, ui: &mut egui::Ui, from: egui::Pos2, to: egui::Pos2, _label: &str, color: egui::Color32, route: crate::resources::ManhattanRoute) {
        // Create a simple L-shaped connection for backward compatibility
        let connection = crate::resources::RoutedConnection::new(
            crate::components::Connection {
                from_entity: bevy::prelude::Entity::PLACEHOLDER,
                to_entity: bevy::prelude::Entity::PLACEHOLDER,
                connection_type: String::new(),
                from_pin_index: 0,
                to_pin_index: 0,
            },
            route,
            from,
            to
        );
        
        self.draw_enhanced_connection(ui, &connection, color);
    }
    
    /// Draw an enhanced connection with support for straight, L-shape and S-shape routing
    fn draw_enhanced_connection(&self, ui: &mut egui::Ui, routed_connection: &crate::resources::RoutedConnection, color: egui::Color32) {
        let stroke = egui::Stroke::new(2.0, color);
        
        // Handle straight line connections
        if routed_connection.shape == crate::resources::ConnectionShape::Straight {
            // Draw direct line from source to target
            ui.painter().line_segment([routed_connection.from_pin, routed_connection.to_pin], stroke);
            self.draw_perpendicular_arrow_head(ui, routed_connection, color);
            return;
        }
        
        // Get all points in the path: from_pin -> bend_points -> to_pin
        let mut path_points = vec![routed_connection.from_pin];
        path_points.extend(&routed_connection.bend_points);
        path_points.push(routed_connection.to_pin);
        
        // Calculate adaptive corner radius based on connection geometry
        let corner_radius = self.calculate_adaptive_corner_radius(routed_connection, &path_points);
        
        // Draw the path with rounded corners
        self.draw_multi_segment_path(ui, &path_points, corner_radius, stroke);
        
        // Draw arrow head perpendicular to the target edge
        self.draw_perpendicular_arrow_head(ui, routed_connection, color);
    }
    
    /// Calculate adaptive corner radius based on connection geometry
    fn calculate_adaptive_corner_radius(&self, routed_connection: &crate::resources::RoutedConnection, path_points: &[egui::Pos2]) -> f32 {
        let base_radius = 10.0;
        
        if path_points.len() < 3 {
            return base_radius;
        }
        
        // Find the shortest segment length
        let mut min_segment_length = f32::INFINITY;
        for i in 0..path_points.len() - 1 {
            let segment_length = path_points[i].distance(path_points[i + 1]);
            min_segment_length = min_segment_length.min(segment_length);
        }
        
        // Calculate total connection distance
        let total_distance = routed_connection.from_pin.distance(routed_connection.to_pin);
        
        // Adaptive radius based on geometry:
        // - Smaller radius for shorter segments to avoid overlapping curves
        // - Smaller radius for close nodes to handle tight turns better
        // - Minimum radius to maintain visual quality
        let segment_factor = (min_segment_length / 40.0).min(1.0); // Scale down for short segments
        let distance_factor = (total_distance / 100.0).clamp(0.3, 1.0); // Scale down for close nodes
        
        let adaptive_radius = base_radius * segment_factor * distance_factor;
        adaptive_radius.clamp(3.0, 15.0) // Keep within reasonable bounds
    }
    
    /// Draw a multi-segment path with rounded corners
    fn draw_multi_segment_path(&self, ui: &mut egui::Ui, points: &[egui::Pos2], corner_radius: f32, stroke: egui::Stroke) {
        if points.len() < 2 {
            return;
        }
        
        if points.len() == 2 {
            // Simple straight line
            ui.painter().line_segment([points[0], points[1]], stroke);
            return;
        }
        
        // Draw segments with rounded corners
        for i in 0..points.len() - 1 {
            let current = points[i];
            let next = points[i + 1];
            
            // Skip very short segments
            if current.distance(next) <= 1.0 {
                continue;
            }
            
            if i == 0 {
                // First segment - no rounding at start
                if i + 2 < points.len() {
                    // There's a next segment, so round the end
                    let after_next = points[i + 2];
                    self.draw_segment_with_end_rounding(ui, current, next, after_next, corner_radius, stroke);
                } else {
                    // Last segment, no rounding
                    ui.painter().line_segment([current, next], stroke);
                }
            } else if i == points.len() - 2 {
                // Last segment - already handled rounding at start in previous iteration
                let prev = points[i - 1];
                self.draw_segment_with_start_rounding(ui, prev, current, next, corner_radius, stroke);
            } else {
                // Middle segment - round both ends
                let prev = points[i - 1];
                let after_next = points[i + 2];
                self.draw_segment_with_both_rounding(ui, prev, current, next, after_next, corner_radius, stroke);
            }
        }
    }
    
    /// Draw a segment with rounding at the end
    fn draw_segment_with_end_rounding(&self, ui: &mut egui::Ui, start: egui::Pos2, end: egui::Pos2, _next: egui::Pos2, radius: f32, stroke: egui::Stroke) {
        let segment_len = start.distance(end);
        if segment_len <= radius * 2.0 {
            // Segment too short for rounding
            ui.painter().line_segment([start, end], stroke);
            return;
        }
        
        // Calculate where to start the curve
        let direction = (end - start).normalized();
        let curve_start = end - direction * radius;
        
        // Draw the straight part
        ui.painter().line_segment([start, curve_start], stroke);
        
        // The curve will be drawn by the next segment
    }
    
    /// Draw a segment with rounding at the start
    fn draw_segment_with_start_rounding(&self, ui: &mut egui::Ui, prev: egui::Pos2, start: egui::Pos2, end: egui::Pos2, radius: f32, stroke: egui::Stroke) {
        let segment_len = start.distance(end);
        if segment_len <= radius * 2.0 {
            // Segment too short for rounding
            ui.painter().line_segment([start, end], stroke);
            return;
        }
        
        // Calculate where to end the curve
        let direction = (end - start).normalized();
        let curve_end = start + direction * radius;
        
        // Draw the curve from previous segment
        let prev_direction = (start - prev).normalized();
        let curve_start = start - prev_direction * radius;
        self.draw_bezier_corner(ui, curve_start, curve_end, start, radius, stroke);
        
        // Draw the straight part
        ui.painter().line_segment([curve_end, end], stroke);
    }
    
    /// Draw a segment with rounding at both ends
    fn draw_segment_with_both_rounding(&self, ui: &mut egui::Ui, prev: egui::Pos2, start: egui::Pos2, end: egui::Pos2, next: egui::Pos2, radius: f32, stroke: egui::Stroke) {
        let segment_len = start.distance(end);
        if segment_len <= radius * 4.0 {
            // Segment too short for double rounding
            ui.painter().line_segment([start, end], stroke);
            return;
        }
        
        // Calculate curve points
        let start_direction = (end - start).normalized();
        let end_direction = (next - end).normalized();
        let prev_direction = (start - prev).normalized();
        
        let curve_start_begin = start - prev_direction * radius;
        let curve_start_end = start + start_direction * radius;
        let curve_end_begin = end - start_direction * radius;
        let _curve_end_end = end + end_direction * radius;
        
        // Draw start curve
        self.draw_bezier_corner(ui, curve_start_begin, curve_start_end, start, radius, stroke);
        
        // Draw straight middle part
        ui.painter().line_segment([curve_start_end, curve_end_begin], stroke);
        
        // End curve will be drawn by next segment
    }
    
    /// Draw arrow head perpendicular to the target edge
    fn draw_perpendicular_arrow_head(&self, ui: &mut egui::Ui, routed_connection: &crate::resources::RoutedConnection, color: egui::Color32) {
        let arrow_size = 8.0;
        let target_pos = routed_connection.to_pin;
        
        // Calculate arrow direction based on target edge
        // Arrow should point INTO the target edge from outside the node
        let arrow_points = match routed_connection.to_edge {
            crate::resources::EdgeSide::Top => {
                // Arrow pointing down (into top edge from above)
                vec![
                    target_pos,
                    egui::Pos2::new(target_pos.x - arrow_size/2.0, target_pos.y - arrow_size),
                    egui::Pos2::new(target_pos.x + arrow_size/2.0, target_pos.y - arrow_size),
                ]
            }
            crate::resources::EdgeSide::Right => {
                // Arrow pointing left (into right edge from the right)
                vec![
                    target_pos,
                    egui::Pos2::new(target_pos.x + arrow_size, target_pos.y - arrow_size/2.0),
                    egui::Pos2::new(target_pos.x + arrow_size, target_pos.y + arrow_size/2.0),
                ]
            }
            crate::resources::EdgeSide::Bottom => {
                // Arrow pointing up (into bottom edge from below)
                vec![
                    target_pos,
                    egui::Pos2::new(target_pos.x - arrow_size/2.0, target_pos.y + arrow_size),
                    egui::Pos2::new(target_pos.x + arrow_size/2.0, target_pos.y + arrow_size),
                ]
            }
            crate::resources::EdgeSide::Left => {
                // Arrow pointing right (into left edge from the left)
                vec![
                    target_pos,
                    egui::Pos2::new(target_pos.x - arrow_size, target_pos.y - arrow_size/2.0),
                    egui::Pos2::new(target_pos.x - arrow_size, target_pos.y + arrow_size/2.0),
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

    /// Draw a single connection line between closest edge pins (unused after crossing avoidance refactor)
    #[allow(dead_code)]
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
            
            // Find closest edge pin on target from all available pins
            let all_target_pins = to_edge_pins.get_all_pins();
            let mut min_distance = f32::INFINITY;
            let mut best_pin = all_target_pins.get(0).copied().unwrap_or_default();
            
            for pin in all_target_pins {
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
        
        // For now, use closest pins logic with the new distributed ports
        // TODO: In the future, this should use specific assigned ports based on connection index
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

    /// Draw a Manhattan-style connection between two points with rounded corners and arrow head (unused after crossing avoidance refactor)
    #[allow(dead_code)]
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