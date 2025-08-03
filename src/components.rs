use bevy::prelude::*;
use bevy::reflect::Reflect;

// --- Core Components ---

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
pub struct GraphCanvas;

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct GraphNode {
    pub position: Vec2,
    pub size: Vec2,     // Last measured size from egui (for interactions)
    pub expanded: bool,
}

#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct EntityNode; // Marker component - indicates this entity should be shown as a node

// --- Connection System Components (inspired by egui-snarl) ---

/// Collection of pins on a node entity
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct NodePins {
    pub pins: Vec<NodePin>,
}

/// Represents a single pin on a node (input or output)
#[derive(Reflect, Clone, Debug)]
pub struct NodePin {
    pub pin_type: PinType,
    pub pin_index: usize,
    pub label: String, // Label for the pin (e.g., "OnInvoke", "Input")
}

/// Type of pin - input or output
#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum PinType {
    Input,
    Output,
}

/// Represents a connection between two pins
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct Connection {
    pub from_entity: Entity, // Source entity (has output pin)
    pub from_pin_index: usize,
    pub to_entity: Entity,   // Target entity (has input pin)  
    pub to_pin_index: usize,
    pub connection_type: String, // e.g., "OnInvoke", for labeling/filtering
}

// --- Parent-Child Zone System Components ---

/// Represents a parent entity's zone area for containing child entities
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct ParentZone {
    pub bounds: bevy::math::Rect,           // Current zone boundaries (in world coordinates)
    pub resize_handles: [bevy::math::Rect; 4], // [Top, Right, Bottom, Left] edge hit areas
    pub min_size: Vec2,                     // Minimum allowed zone size
}

impl Default for ParentZone {
    fn default() -> Self {
        let default_bounds = bevy::math::Rect::from_center_size(Vec2::ZERO, Vec2::new(300.0, 200.0));
        Self {
            bounds: default_bounds,
            resize_handles: [bevy::math::Rect::default(); 4], // Will be calculated during rendering
            min_size: Vec2::new(150.0, 100.0),
        }
    }
}

/// Points from a parent entity to its initial/default child state
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component)]
pub struct InitialStatePointer {
    pub target_child: Option<Entity>, // The child entity that's entered when parent is activated
}

impl Default for InitialStatePointer {
    fn default() -> Self {
        Self {
            target_child: None,
        }
    }
}