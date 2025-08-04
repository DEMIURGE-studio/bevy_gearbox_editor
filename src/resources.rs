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

/// Resource to track actual pin positions for accurate connection drawing
#[derive(Resource, Default)]
pub struct PinPositionCache {
    pub input_pins: HashMap<Entity, egui::Pos2>,
    pub output_pins: HashMap<(Entity, usize), egui::Pos2>, // (entity, pin_index) -> position
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