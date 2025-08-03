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