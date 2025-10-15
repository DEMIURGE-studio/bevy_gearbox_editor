//! Editor-internal NodeKind state machines (Leaf | Parent | Parallel) per state entity
//! Uses bevy_gearbox to dogfood state handling for editor policies.

use bevy::prelude::*;
use bevy_gearbox::prelude::*;
use bevy_gearbox::transitions::{Source, Target, EventEdge};

use crate::editor_state::StateMachinePersistentData;
use crate::editor_state::{DeleteNode, MachineNodesPopulated};
use crate::components::{NodeType, LeafNode};
use crate::editor_state::SetInitialStateRequested;

/// Marker on NodeKind machine roots
#[derive(Component)]
pub struct NodeKindRoot;

/// Associates a NodeKind machine or its state node with the editor state entity it controls
#[derive(Component, Copy, Clone)]
pub struct NodeKindFor(pub Entity);

/// Markers for NodeKind variant states
#[derive(Component)]
pub struct NodeKindLeaf;

#[derive(Component)]
pub struct NodeKindParent;

#[derive(Component)]
pub struct NodeKindParallel;

// Events that drive NodeKind transitions (entity-targeted at the NodeKind machine root)
#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct AddChildClicked {
    #[event_target]
    pub target: Entity,
}

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct ChildAdded {
    #[event_target]
    pub target: Entity,
}

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct AllChildrenRemoved {
    #[event_target]
    pub target: Entity,
}

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeParallelClicked {
    #[event_target]
    pub target: Entity,
}

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeParentClicked {
    #[event_target]
    pub target: Entity,
}

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeLeafClicked {
    #[event_target]
    pub target: Entity,
}

impl AddChildClicked { pub fn new(entity: Entity) -> Self { Self { target: entity } } }
impl ChildAdded { pub fn new(entity: Entity) -> Self { Self { target: entity } } }
impl AllChildrenRemoved { pub fn new(entity: Entity) -> Self { Self { target: entity } } }
impl MakeParallelClicked { pub fn new(entity: Entity) -> Self { Self { target: entity } } }
impl MakeParentClicked { pub fn new(entity: Entity) -> Self { Self { target: entity } } }
impl MakeLeafClicked { pub fn new(entity: Entity) -> Self { Self { target: entity } } }

/// Ensure there is a NodeKind machine for every editor node under the selected machine
/// Observer: when a machine is opened on the canvas, ensure NodeKind machines exist for its nodes
pub fn on_machine_nodes_populated_sync_node_kind(
    populated: On<MachineNodesPopulated>,
    mut commands: Commands,
    mut q_sm: Query<(&StateMachinePersistentData, &mut crate::editor_state::StateMachineTransientData), With<StateMachine>>,    
) {
    let root = populated.root;
    let Ok((persistent, mut transient)) = q_sm.get_mut(root) else { return; };

    for (&state_entity, _node) in persistent.nodes.iter() {
        if transient.node_kind_roots.contains_key(&state_entity) {
            continue;
        }

        // Build a tiny machine: Root -> {Leaf, Parent, Parallel}
        let leaf = commands.spawn((Name::new("NodeKind::Leaf"), NodeKindLeaf, ChildOf(root))).id();
        let parent = commands.spawn((Name::new("NodeKind::Parent"), NodeKindParent, ChildOf(root))).id();
        let parallel = commands.spawn((Name::new("NodeKind::Parallel"), NodeKindParallel, ChildOf(root))).id();

        let root_entity = commands
            .spawn((
                Name::new("NodeKind"),
                NodeKindRoot,
                NodeKindFor(state_entity),
                InitialState(leaf),
                StateMachine::new(),
                ChildOf(root),
            ))
            .id();

        // Attach state nodes under this machine root
        commands.entity(leaf).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity), ChildOf(root_entity)));
        commands.entity(parent).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity), ChildOf(root_entity)));
        commands.entity(parallel).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity), ChildOf(root_entity)));

        // Transitions
        // Leaf --(AddChildClicked|ChildAdded)--> Parent
        commands.spawn((Source(leaf), Target(parent), EventEdge::<AddChildClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        commands.spawn((Source(leaf), Target(parent), EventEdge::<ChildAdded>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        // Leaf/Parent --(MakeParallelClicked)--> Parallel
        commands.spawn((Source(parent), Target(parallel), EventEdge::<MakeParallelClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        commands.spawn((Source(leaf), Target(parallel), EventEdge::<MakeParallelClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        // Leaf/Parallel --(MakeParentClicked)--> Parent
        commands.spawn((Source(leaf), Target(parent), EventEdge::<MakeParentClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        commands.spawn((Source(parallel), Target(parent), EventEdge::<MakeParentClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        // Parent/Parallel --(AllChildrenRemoved)--> Leaf
        commands.spawn((Source(parent), Target(leaf), EventEdge::<AllChildrenRemoved>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        commands.spawn((Source(parallel), Target(leaf), EventEdge::<AllChildrenRemoved>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        // Parent/Parallel --(MakeLeafClicked)--> Leaf
        commands.spawn((Source(parent), Target(leaf), EventEdge::<MakeLeafClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));
        commands.spawn((Source(parallel), Target(leaf), EventEdge::<MakeLeafClicked>::default(), NodeKindFor(state_entity), ChildOf(root_entity)));

        transient.node_kind_roots.insert(state_entity, root_entity);
    }
}

/// Observer: clean up NodeKind machine when a node is deleted
pub fn on_delete_node_cleanup_node_kind(
    delete_node: On<DeleteNode>,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
    mut q: Query<&mut crate::editor_state::StateMachineTransientData, With<StateMachine>>,
    mut commands: Commands,
) {
    let entity_to_delete = delete_node.entity;
    let root = q_child_of.root_ancestor(entity_to_delete);
    let Ok(mut transient) = q.get_mut(root) else { return; };
    if let Some(nk_root) = transient.node_kind_roots.remove(&entity_to_delete) {
        // Despawn NK machine root and its direct state children
        commands.queue(move |world: &mut World| {
            // Collect direct children of nk_root
            let mut to_despawn: Vec<Entity> = Vec::new();
            {
                let mut q_children = world.query::<(Entity, &bevy_gearbox::StateChildOf)>();
                for (e, rel) in q_children.iter(world) {
                    if rel.0 == nk_root { to_despawn.push(e); }
                }
            }
            // Despawn children first
            for e in to_despawn { world.entity_mut(e).despawn(); }
            // Despawn root
            if world.entities().contains(nk_root) {
                world.entity_mut(nk_root).despawn();
            }
        });
    }
}

/// On entering Parallel state: ensure editor state has Parallel marker and no InitialState
pub fn on_enter_nodekind_state_parallel(
    enter_state: On<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParallel>>,
    mut commands: Commands,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
) {
    let nk_state = enter_state.target;
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).insert(bevy_gearbox::Parallel);
    commands.entity(state).remove::<bevy_gearbox::InitialState>();

    // Ensure at least one child exists; if none, create one and add a visual node
    // Resolve the owning machine for this state via relationships
    let root = q_child_of.root_ancestor(state);
    commands.queue(move |world: &mut World| {
        let has_child = world
            .get::<bevy_gearbox::StateChildren>(state)
            .map(|c| c.into_iter().next().is_some())
            .unwrap_or(false);
        if has_child { return; }

        let child = world.spawn((bevy_gearbox::StateChildOf(state), Name::new("New State"))).id();
        if let Some(mut persistent) = world.get_mut::<StateMachinePersistentData>(root) {
            if let Some(parent_node) = persistent.nodes.get(&state) {
                let parent_pos = match parent_node {
                    NodeType::Leaf(leaf) => leaf.entity_node.position,
                    NodeType::Parent(parent) => parent.entity_node.position,
                };
                let pos = parent_pos + egui::Vec2::new(50.0, 50.0);
                persistent.nodes.insert(child, NodeType::Leaf(LeafNode::new(pos)));
            }
        }
    });
}

/// On entering Parent state: ensure Parallel marker is removed
pub fn on_enter_nodekind_state_parent(
    enter_state: On<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParent>>,
    mut commands: Commands,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
) {
    let nk_state = enter_state.target;
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).remove::<bevy_gearbox::Parallel>();

    // Resolve the owning machine for this state via relationships
    let root = q_child_of.root_ancestor(state);
    commands.queue(move |world: &mut World| {
        // Ensure at least one child
        let first_child: Option<Entity> = world
            .get::<bevy_gearbox::StateChildren>(state)
            .and_then(|children| children.into_iter().next().copied())
            .or_else(|| {
                let child = world.spawn((bevy_gearbox::StateChildOf(state), Name::new("New State"))).id();
                let Some(mut persistent) = world.get_mut::<StateMachinePersistentData>(root) else { return None; };
                let Some(parent_node) = persistent.nodes.get(&state) else { return None; };
                let parent_pos = match parent_node {
                    NodeType::Leaf(leaf) => leaf.entity_node.position,
                    NodeType::Parent(parent) => parent.entity_node.position,
                };
                let pos = parent_pos + egui::Vec2::new(50.0, 50.0);
                persistent.nodes.insert(child, NodeType::Leaf(LeafNode::new(pos)));
                Some(child)
            });

        let Some(init) = first_child else { return; };
        world.trigger(SetInitialStateRequested { child_entity: init });
    });
}

/// On entering Leaf state: remove Parallel and InitialState
pub fn on_enter_nodekind_state_leaf(
    enter_state: On<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindLeaf>>,
    mut commands: Commands,
) {
    let nk_state = enter_state.target;
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).remove::<bevy_gearbox::Parallel>();
    commands.entity(state).remove::<bevy_gearbox::InitialState>();
    commands.entity(state).remove::<bevy_gearbox::StateChildren>();
}

/// On entering Parent via MakeParentClicked, ensure child and set InitialState
pub fn on_enter_nodekind_state_parent_via_make_parent(
    enter_state: On<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParent>>,
    mut commands: Commands,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
) {
    let nk_state = enter_state.target;
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    // Resolve the owning machine for this state via relationships
    let root = q_child_of.root_ancestor(state);
    commands.queue(move |world: &mut World| {
        let mut first_child: Option<Entity> = world
            .get::<bevy_gearbox::StateChildren>(state)
            .and_then(|children| children.into_iter().next().copied());
        if first_child.is_none() {
            let child = world.spawn((bevy_gearbox::StateChildOf(state), Name::new("New State"))).id();
            first_child = Some(child);
            let Some(mut persistent) = world.get_mut::<StateMachinePersistentData>(root) else { return; };
            let Some(parent_node) = persistent.nodes.get(&state) else { return; };
            let parent_pos = match parent_node {
                NodeType::Leaf(leaf) => leaf.entity_node.position,
                NodeType::Parent(parent) => parent.entity_node.position,
            };
            let pos = parent_pos + egui::Vec2::new(50.0, 50.0);
            persistent.nodes.insert(child, NodeType::Leaf(LeafNode::new(pos)));
        }
        let Some(init) = first_child else { return; };
        world.trigger(SetInitialStateRequested { child_entity: init });
    });
}

/// When a state loses its StateChildren component (no more children), demote to Leaf
pub fn on_remove_state_children(
    remove: On<Remove, bevy_gearbox::StateChildren>,
    q_child_of: Query<&bevy_gearbox::StateChildOf>,
    mut q: Query<&mut crate::editor_state::StateMachineTransientData, With<StateMachine>>,
    mut commands: Commands,
) {
    let parent = remove.entity;
    // Resolve the owning machine for this state via relationships
    let root = q_child_of.root_ancestor(parent);
    let Ok(transient) = q.get_mut(root) else { return; };
    let Some(&nk_root) = transient.node_kind_roots.get(&parent) else { return; };
    commands.trigger(AllChildrenRemoved::new(nk_root));
}