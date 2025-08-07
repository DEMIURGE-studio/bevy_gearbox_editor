//! Editor state management, events, and shared types

use bevy::prelude::*;
use egui::Pos2;
use std::collections::HashMap;

use crate::components::NodeType;

/// Resource that holds the editor's current state
#[derive(Resource, Default)]
pub struct EditorState {
    /// Currently selected state machine root entity
    pub selected_machine: Option<Entity>,
    /// Map of entity to its UI node representation
    pub nodes: HashMap<Entity, NodeType>,
    /// Currently selected node for z-ordering
    pub selected_node: Option<Entity>,
    /// Entity for which a context menu is requested
    pub context_menu_entity: Option<Entity>,
    /// Position where the context menu should appear
    pub context_menu_position: Option<Pos2>,
    /// Entity currently being inspected
    pub inspected_entity: Option<Entity>,
}

/// Component marking an entity as an editor window
#[derive(Component)]
pub struct EditorWindow;

/// Event fired when a context menu is requested for a node
#[derive(Event)]
pub struct NodeContextMenuRequested {
    pub entity: Entity,
    pub position: Pos2,
}

/// Available actions that can be performed on nodes
#[derive(Debug, Clone)]
pub enum NodeAction {
    Inspect,
    AddChild,
}

/// Event fired when a node action is triggered
#[derive(Event)]
pub struct NodeActionTriggered {
    pub entity: Entity,
    pub action: NodeAction,
}

/// Event fired when a node is dragged
#[derive(Event, Debug)]
pub struct NodeDragged {
    pub entity: Entity,
    pub drag_delta: egui::Vec2,
}

/// Item to be rendered in the node editor, with z-order information
pub struct RenderItem {
    pub entity: Entity,
    pub z_order: i32,
}

/// Get a human-readable name for an entity
pub fn get_entity_name(entity: Entity, all_entities: &Query<(Entity, Option<&Name>)>) -> String {
    if let Ok((_, name_opt)) = all_entities.get(entity) {
        if let Some(name) = name_opt {
            name.as_str().to_string()
        } else {
            format!("Entity {:?}", entity)
        }
    } else {
        format!("Unknown Entity {:?}", entity)
    }
}

/// Get a human-readable name for an entity using world access
pub fn get_entity_name_from_world(entity: Entity, world: &mut World) -> String {
    let mut query = world.query::<(Entity, Option<&Name>)>();
    if let Ok((_, name_opt)) = query.get(world, entity) {
        if let Some(name) = name_opt {
            name.as_str().to_string()
        } else {
            format!("Entity {:?}", entity)
        }
    } else {
        format!("Unknown Entity {:?}", entity)
    }
}

/// Determine if an entity should get a selection boost for z-ordering
pub fn should_get_selection_boost(
    entity: Entity,
    selected_node: Option<Entity>,
    child_of_query: &Query<&ChildOf>,
) -> bool {
    if let Some(selected) = selected_node {
        if entity == selected {
            return true;
        }
        
        // Check if this entity is an ancestor of the selected node
        let mut current = selected;
        while let Ok(child_of) = child_of_query.get(current) {
            if child_of.0 == entity {
                return true;
            }
            current = child_of.0;
        }
    }
    false
}
