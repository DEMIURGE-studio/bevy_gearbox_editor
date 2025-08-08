use std::collections::HashMap;

use bevy::prelude::*;

use crate::{StateMachinePersistentData, TransitionConnection};
use crate::components::{NodeType, LeafNode, ParentNode};

#[derive(Reflect)]
pub struct ReflectableStateMachinePersistentData {
    pub nodes: HashMap<Entity, ReflectableNode>,
    pub visual_transitions: Vec<ReflectableTransitionConnection>,
}

#[derive(Reflect)]
pub struct ReflectableNode {
    pub position: Vec2,
    pub node_type: ReflectableNodeType,
}

#[derive(Reflect)]
pub enum ReflectableNodeType {
    Leaf,
    Parent,
}

#[derive(Reflect)]
pub struct ReflectableTransitionConnection {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub event_type: String,
    pub position: Vec2,
    pub offset: Vec2,
}

fn vec2_from_pos2(pos: egui::Pos2) -> Vec2 {
    Vec2::new(pos.x, pos.y)
}

fn pos2_from_vec2(vec: Vec2) -> egui::Pos2 {
    egui::Pos2::new(vec.x, vec.y)
}

fn vec2_from_egui_vec2(egui_vec: egui::Vec2) -> Vec2 {
    Vec2::new(egui_vec.x, egui_vec.y)
}

fn egui_vec2_from_vec2(vec: Vec2) -> egui::Vec2 {
    egui::Vec2::new(vec.x, vec.y)
}

impl ReflectableStateMachinePersistentData {
    /// Convert from StateMachinePersistentData to reflectable format
    pub fn from_persistent_data(
        state_machine: &StateMachinePersistentData,
        world: &World,
    ) -> Self {
        let mut nodes = HashMap::new();
        let mut visual_transitions = Vec::new();

        // Convert nodes with type information
        for (&entity, node) in &state_machine.nodes {
            let node_type = determine_node_type(entity, world);
            nodes.insert(entity, ReflectableNode {
                position: vec2_from_pos2(node.position()),
                node_type,
            });
        }

        // Convert visual transitions
        for transition in &state_machine.visual_transitions {
            visual_transitions.push(ReflectableTransitionConnection {
                source_entity: transition.source_entity,
                target_entity: transition.target_entity,
                event_type: transition.event_type.clone(),
                position: vec2_from_pos2(transition.event_node_position),
                offset: vec2_from_egui_vec2(transition.event_node_offset),
            });
        }

        Self {
            nodes,
            visual_transitions,
        }
    }

    /// Convert back to StateMachinePersistentData
    pub fn to_persistent_data(&self) -> StateMachinePersistentData {
        let mut nodes = HashMap::new();
        let mut visual_transitions = Vec::new();

        // Convert nodes back to NodeType
        for (&entity, reflectable_node) in &self.nodes {
            let position = pos2_from_vec2(reflectable_node.position);
            let node = match reflectable_node.node_type {
                ReflectableNodeType::Leaf => {
                    NodeType::Leaf(LeafNode::new(position))
                }
                ReflectableNodeType::Parent => {
                    NodeType::Parent(ParentNode::new(position))
                }
            };
            nodes.insert(entity, node);
        }

        // Convert visual transitions back
        for reflectable_transition in &self.visual_transitions {
            visual_transitions.push(TransitionConnection {
                source_entity: reflectable_transition.source_entity,
                target_entity: reflectable_transition.target_entity,
                event_type: reflectable_transition.event_type.clone(),
                source_rect: egui::Rect::NOTHING, // Will be updated when nodes are rendered
                target_rect: egui::Rect::NOTHING, // Will be updated when nodes are rendered
                event_node_position: pos2_from_vec2(reflectable_transition.position),
                is_dragging_event_node: false,
                event_node_offset: egui_vec2_from_vec2(reflectable_transition.offset),
            });
        }

        StateMachinePersistentData {
            nodes,
            visual_transitions,
        }
    }
}

/// Determine the node type based on whether the entity has children
fn determine_node_type(entity: Entity, world: &World) -> ReflectableNodeType {
    // Check if the entity has children (making it a parent node)
    if world.get::<Children>(entity).is_some() {
        ReflectableNodeType::Parent
    } else {
        ReflectableNodeType::Leaf
    }
}