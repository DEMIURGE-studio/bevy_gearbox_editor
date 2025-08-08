use std::collections::HashMap;
use std::path::Path;

use bevy::{
    prelude::*,
    scene::{DynamicScene, DynamicSceneBuilder, DynamicSceneRoot},
    tasks::IoTaskPool,
};
use bevy_ecs::component::{Mutable, StorageType};

use crate::{StateMachinePersistentData, TransitionConnection};
use crate::components::{NodeType, LeafNode, ParentNode};

#[derive(Reflect, Clone)]
#[reflect(Component)]
pub struct ReflectableStateMachinePersistentData {
    pub nodes: HashMap<Entity, ReflectableNode>,
    pub visual_transitions: Vec<ReflectableTransitionConnection>,
}

impl Component for ReflectableStateMachinePersistentData {
    const STORAGE_TYPE: StorageType = StorageType::Table;

    type Mutability = Mutable;

    fn map_entities<E: EntityMapper>(this: &mut Self, entity_mapper: &mut E) {
        let mut new_nodes = HashMap::new();
        for (entity, node) in this.nodes.iter() {
            new_nodes.insert(entity_mapper.get_mapped(*entity), (*node).clone());
        }
        this.nodes = new_nodes;

        let mut new_visual_transitions = Vec::new();
        for transition in this.visual_transitions.iter() {
            new_visual_transitions.push(ReflectableTransitionConnection {
                source_entity: entity_mapper.get_mapped(transition.source_entity),
                target_entity: entity_mapper.get_mapped(transition.target_entity),
                event_type: transition.event_type.clone(),
                position: transition.position.clone(),
                offset: transition.offset.clone(),
            });
        }
        this.visual_transitions = new_visual_transitions;
    }
}

#[derive(Reflect, Clone)]
pub struct ReflectableNode {
    pub position: Vec2,
    pub node_type: ReflectableNodeType,
}

#[derive(Reflect, Clone)]
pub enum ReflectableNodeType {
    Leaf,
    Parent,
}

#[derive(Reflect, Clone)]
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

    /// Save a state machine to a scene file
    pub fn save_state_machine_to_file(
        world: &mut World,
        root_entity: Entity,
        file_path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create the scene from the state machine
        let scene = Self::create_state_machine_scene(world, root_entity)?;
        
        // Serialize the scene
        let type_registry = world.resource::<AppTypeRegistry>();
        let type_registry = type_registry.read();
        let serialized_scene = scene.serialize(&type_registry)?;
        
        // Write to file asynchronously
        let file_path = file_path.as_ref().to_path_buf();
        IoTaskPool::get()
            .spawn(async move {
                std::fs::write(&file_path, serialized_scene.as_bytes())
                    .map_err(|e| format!("Failed to write scene to {:?}: {}", file_path, e))
            })
            .detach();
        
        Ok(())
    }

    /// Create a DynamicScene from a state machine hierarchy
    pub fn create_state_machine_scene(
        world: &mut World,
        root_entity: Entity,
    ) -> Result<DynamicScene, Box<dyn std::error::Error>> {
        // Find all entities in the state machine hierarchy
        let hierarchy_entities = Self::collect_state_machine_entities(world, root_entity)?;
        
        // Convert persistent data to reflectable format before creating scene
        if let Some(persistent_data) = world.get::<StateMachinePersistentData>(root_entity) {
            let reflectable_data = ReflectableStateMachinePersistentData::from_persistent_data(
                persistent_data, 
                world
            );
            // Temporarily replace the persistent data with reflectable version
            world.entity_mut(root_entity).remove::<StateMachinePersistentData>();
            world.entity_mut(root_entity).insert(reflectable_data);
        }
        
        // Create scene using DynamicSceneBuilder
        let scene_builder = DynamicSceneBuilder::from_world(world);
        let scene = scene_builder
            .extract_entities(hierarchy_entities.iter().copied())
            .allow_all() // No need to deny anything as transient data does not implement reflect
            .build();
        
        // Restore the original persistent data
        if let Some(reflectable_data) = world.get::<ReflectableStateMachinePersistentData>(root_entity) {
            let persistent_data = reflectable_data.to_persistent_data();
            world.entity_mut(root_entity).remove::<ReflectableStateMachinePersistentData>();
            world.entity_mut(root_entity).insert(persistent_data);
        }
        
        Ok(scene)
    }

    /// Load a state machine from a scene file using Bevy's asset system
    /// This spawns an entity with DynamicSceneRoot component that will load the scene
    pub fn load_state_machine_from_file(
        commands: &mut Commands,
        asset_server: &AssetServer,
        file_path: impl AsRef<Path>,
    ) -> Entity {
        // Load the scene asset and spawn an entity with DynamicSceneRoot
        // This follows the same pattern as in repeater.rs
        let scene_handle = asset_server.load(file_path.as_ref());
        
        let entity = commands.spawn((
            Name::new("State Machine (from scene)"),
            DynamicSceneRoot(scene_handle),
        )).id();
        
        info!("✅ Loading state machine from {:?} as entity {:?}", file_path.as_ref(), entity);
        entity
    }

    /// Collect all entities that belong to a state machine hierarchy
    fn collect_state_machine_entities(
        world: &World,
        root_entity: Entity,
    ) -> Result<Vec<Entity>, Box<dyn std::error::Error>> {
        let mut entities = Vec::new();
        let mut to_process = vec![root_entity];
        
        while let Some(entity) = to_process.pop() {
            if !world.entities().contains(entity) {
                continue;
            }
            
            entities.push(entity);
            
            // Add children to the processing queue
            if let Some(children) = world.get::<Children>(entity) {
                for child in children.iter() {
                    to_process.push(child);
                }
            }
        }
        
        Ok(entities)
    }

    /// Restore editor data after loading a state machine from a scene
    pub fn restore_editor_data_after_load(
        world: &mut World,
        root_entity: Entity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Convert the reflectable data back to StateMachinePersistentData
        if let Some(reflectable_data) = world.get::<ReflectableStateMachinePersistentData>(root_entity) {
            let reflectable_data = reflectable_data.clone(); // Clone to avoid borrow issues
            let persistent_data = reflectable_data.to_persistent_data();
            
            // Remove the reflectable component and add the actual persistent data
            world.entity_mut(root_entity).remove::<ReflectableStateMachinePersistentData>();
            world.entity_mut(root_entity).insert(persistent_data);
            
            // Add the transient data component (starts with default state)
            world.entity_mut(root_entity).insert(crate::StateMachineTransientData::default());
            
            info!("✅ Editor data restored for entity {:?}", root_entity);
        }
        
        Ok(())
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

pub(crate) fn on_add_reflectable_state_machine(
    trigger: Trigger<OnAdd, ReflectableStateMachinePersistentData>,
    query: Query<&ReflectableStateMachinePersistentData>,
    mut commands: Commands,
) {
    let entity = trigger.target();

    let reflectable_data = query.get(entity).unwrap();
    let persistent_data = reflectable_data.to_persistent_data();

    commands.entity(entity)
        .insert(persistent_data)
        .remove::<ReflectableStateMachinePersistentData>();
}