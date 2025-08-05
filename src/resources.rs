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

/// Edge positions for a node
#[derive(Debug, Clone)]
pub struct EdgePins {
    pub top: egui::Pos2,
    pub right: egui::Pos2,
    pub bottom: egui::Pos2,
    pub left: egui::Pos2,
}

impl EdgePins {
    pub fn from_rect(rect: egui::Rect) -> Self {
        let center = rect.center();
        Self {
            top: egui::Pos2::new(center.x, rect.min.y),
            right: egui::Pos2::new(rect.max.x, center.y),
            bottom: egui::Pos2::new(center.x, rect.max.y),
            left: egui::Pos2::new(rect.min.x, center.y),
        }
    }
    
    pub fn get_closest_pins(&self, other: &EdgePins) -> (egui::Pos2, egui::Pos2) {
        let pins = [
            (self.top, "top"),
            (self.right, "right"),
            (self.bottom, "bottom"),
            (self.left, "left"),
        ];
        
        let other_pins = [
            (other.top, "top"),
            (other.right, "right"),
            (other.bottom, "bottom"),
            (other.left, "left"),
        ];
        
        let mut min_distance = f32::INFINITY;
        let mut best_pair = (self.top, other.top);
        
        for (from_pin, _) in &pins {
            for (to_pin, _) in &other_pins {
                let distance = from_pin.distance(*to_pin);
                if distance < min_distance {
                    min_distance = distance;
                    best_pair = (*from_pin, *to_pin);
                }
            }
        }
        
        best_pair
    }
    
    /// Get the best source and target pins using a foolproof geometric algorithm
    pub fn get_optimal_connection_pins(&self, target: &EdgePins) -> (egui::Pos2, egui::Pos2, ManhattanRoute) {
        // Calculate centers for direction analysis
        let source_center = egui::Pos2::new(
            (self.left.x + self.right.x) / 2.0,
            (self.top.y + self.bottom.y) / 2.0,
        );
        let target_center = egui::Pos2::new(
            (target.left.x + target.right.x) / 2.0,
            (target.top.y + target.bottom.y) / 2.0,
        );
        
        // Calculate direction vector (no normalization needed)
        let dx = target_center.x - source_center.x;
        let dy = target_center.y - source_center.y;
        
        // Fallback to closest pins if nodes are at same position
        if dx.abs() < 1.0 && dy.abs() < 1.0 {
            let (source_pin, target_pin) = self.get_closest_pins(target);
            return (source_pin, target_pin, ManhattanRoute::HorizontalFirst);
        }
        
        // Stage 1: Find source pin closest to target center (distance-based)
        let source_pins = [self.top, self.right, self.bottom, self.left];
        let mut best_source_pin = self.top;
        let mut min_distance = f32::INFINITY;
        
        for source_pin in &source_pins {
            let distance = source_pin.distance(target_center);
            if distance < min_distance {
                min_distance = distance;
                best_source_pin = *source_pin;
            }
        }
        
        // Stage 2: Determine target face based on primary direction
        let (target_pin, route_type) = if dx.abs() > dy.abs() {
            // Primary direction is horizontal
            if dx > 0.0 {
                // Going RIGHT → approach target's LEFT face with HORIZONTAL-FIRST routing
                (target.left, ManhattanRoute::HorizontalFirst)
            } else {
                // Going LEFT → approach target's RIGHT face with HORIZONTAL-FIRST routing  
                (target.right, ManhattanRoute::HorizontalFirst)
            }
        } else {
            // Primary direction is vertical
            if dy > 0.0 {
                // Going DOWN → approach target's TOP face with VERTICAL-FIRST routing
                (target.top, ManhattanRoute::VerticalFirst)
            } else {
                // Going UP → approach target's BOTTOM face with VERTICAL-FIRST routing
                (target.bottom, ManhattanRoute::VerticalFirst)
            }
        };
        
        (best_source_pin, target_pin, route_type)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ManhattanRoute {
    HorizontalFirst,
    VerticalFirst,
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