//! Editor-internal NodeKind state machines (Leaf | Parent | Parallel) per state entity
//! Uses bevy_gearbox to dogfood state handling for editor policies.

use bevy::prelude::*;
use bevy_gearbox::prelude::*;
use bevy_gearbox::transitions::{Source, Target, EventEdge};

use crate::editor_state::{EditorState, StateMachinePersistentData};
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
pub struct AddChildClicked(Entity);

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct ChildAdded(Entity);

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct AllChildrenRemoved(Entity);

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeParallelClicked(Entity);

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeParentClicked(Entity);

#[derive(SimpleTransition, EntityEvent, Clone)]
pub struct MakeLeafClicked(Entity);

impl AddChildClicked { pub fn new(entity: Entity) -> Self { Self(entity) } }
impl ChildAdded { pub fn new(entity: Entity) -> Self { Self(entity) } }
impl AllChildrenRemoved { pub fn new(entity: Entity) -> Self { Self(entity) } }
impl MakeParallelClicked { pub fn new(entity: Entity) -> Self { Self(entity) } }
impl MakeParentClicked { pub fn new(entity: Entity) -> Self { Self(entity) } }
impl MakeLeafClicked { pub fn new(entity: Entity) -> Self { Self(entity) } }

/// Ensure there is a NodeKind machine for every editor node under the selected machine
pub fn sync_node_kind_machines(
    editor_state: Res<EditorState>,
    mut commands: Commands,
    mut q_sm: Query<(&StateMachinePersistentData, &mut crate::editor_state::StateMachineTransientData), With<StateMachine>>,    
) {
    // Find which machine to use (simplified approach)
    let root = if let Some(open_machine) = editor_state.open_machines.first() {
        open_machine.entity
    } else {
        return;
    };
    let Ok((persistent, mut transient)) = q_sm.get_mut(root) else { return; };

    for (&state_entity, _node) in persistent.nodes.iter() {
        if transient.node_kind_roots.contains_key(&state_entity) { continue; }

        // Build a tiny machine: Root -> {Leaf, Parent, Parallel}
        let leaf = commands.spawn((Name::new("NodeKind::Leaf"), NodeKindLeaf)).id();
        let parent = commands.spawn((Name::new("NodeKind::Parent"), NodeKindParent)).id();
        let parallel = commands.spawn((Name::new("NodeKind::Parallel"), NodeKindParallel)).id();

        let root_entity = commands
            .spawn((
                Name::new("NodeKind"),
                NodeKindRoot,
                NodeKindFor(state_entity),
                InitialState(leaf),
                StateMachine::new(),
            ))
            .id();

        // Attach state nodes under this machine root
        commands.entity(leaf).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity)));
        commands.entity(parent).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity)));
        commands.entity(parallel).insert((NodeKindFor(state_entity), bevy_gearbox::StateChildOf(root_entity)));

        // Transitions
        // Leaf --(AddChildClicked|ChildAdded)--> Parent
        commands.spawn((Source(leaf), Target(parent), EventEdge::<AddChildClicked>::default(), NodeKindFor(state_entity)));
        commands.spawn((Source(leaf), Target(parent), EventEdge::<ChildAdded>::default(), NodeKindFor(state_entity)));
        // Leaf/Parent --(MakeParallelClicked)--> Parallel
        commands.spawn((Source(parent), Target(parallel), EventEdge::<MakeParallelClicked>::default(), NodeKindFor(state_entity)));
        commands.spawn((Source(leaf), Target(parallel), EventEdge::<MakeParallelClicked>::default(), NodeKindFor(state_entity)));
        // Leaf/Parallel --(MakeParentClicked)--> Parent
        commands.spawn((Source(leaf), Target(parent), EventEdge::<MakeParentClicked>::default(), NodeKindFor(state_entity)));
        commands.spawn((Source(parallel), Target(parent), EventEdge::<MakeParentClicked>::default(), NodeKindFor(state_entity)));
        // Parent/Parallel --(AllChildrenRemoved)--> Leaf
        commands.spawn((Source(parent), Target(leaf), EventEdge::<AllChildrenRemoved>::default(), NodeKindFor(state_entity)));
        commands.spawn((Source(parallel), Target(leaf), EventEdge::<AllChildrenRemoved>::default(), NodeKindFor(state_entity)));
        // Parent/Parallel --(MakeLeafClicked)--> Leaf
        commands.spawn((Source(parent), Target(leaf), EventEdge::<MakeLeafClicked>::default(), NodeKindFor(state_entity)));
        commands.spawn((Source(parallel), Target(leaf), EventEdge::<MakeLeafClicked>::default(), NodeKindFor(state_entity)));

        transient.node_kind_roots.insert(state_entity, root_entity);
    }
}

/// On entering Parallel state: ensure editor state has Parallel marker and no InitialState
pub fn on_enter_nodekind_state_parallel(
    trigger: Trigger<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParallel>>,
    mut commands: Commands,
    editor_state: Res<EditorState>,
) {
    let nk_state = trigger.target();
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).insert(bevy_gearbox::Parallel);
    commands.entity(state).remove::<bevy_gearbox::InitialState>();

    // Ensure at least one child exists; if none, create one and add a visual node
    // Find which machine to use (simplified approach)
    let root = if let Some(open_machine) = editor_state.open_machines.first() {
        open_machine.entity
    } else {
        return;
    };
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
    trigger: Trigger<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParent>>,
    mut commands: Commands,
    editor_state: Res<EditorState>,
) {
    let nk_state = trigger.target();
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).remove::<bevy_gearbox::Parallel>();

    // Find which machine to use (simplified approach)
    let root = if let Some(open_machine) = editor_state.open_machines.first() {
        open_machine.entity
    } else {
        return;
    };
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
    trigger: Trigger<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindLeaf>>,
    mut commands: Commands,
) {
    let nk_state = trigger.target();
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    commands.entity(state).remove::<bevy_gearbox::Parallel>();
    commands.entity(state).remove::<bevy_gearbox::InitialState>();
    commands.entity(state).remove::<bevy_gearbox::StateChildren>();
}

/// On entering Parent via MakeParentClicked, ensure child and set InitialState
pub fn on_enter_nodekind_state_parent_via_make_parent(
    trigger: Trigger<EnterState>,
    q_nk_for: Query<&NodeKindFor, With<NodeKindParent>>,
    mut commands: Commands,
    editor_state: Res<EditorState>,
) {
    let nk_state = trigger.target();
    let Ok(NodeKindFor(target_state_entity)) = q_nk_for.get(nk_state) else { return; };
    let state = *target_state_entity;
    // Find which machine to use (simplified approach)
    let root = if let Some(open_machine) = editor_state.open_machines.first() {
        open_machine.entity
    } else {
        return;
    };
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
    trigger: On<Remove, bevy_gearbox::StateChildren>,
    editor_state: Res<EditorState>,
    mut q: Query<&mut crate::editor_state::StateMachineTransientData, With<StateMachine>>,
    mut commands: Commands,
) {
    let parent = trigger.event().entity;
    // Find which machine to use (simplified approach)
    let root = if let Some(open_machine) = editor_state.open_machines.first() {
        open_machine.entity
    } else {
        return;
    };
    let Ok(transient) = q.get_mut(root) else { return; };
    let Some(&nk_root) = transient.node_kind_roots.get(&parent) else { return; };
    commands.trigger(AllChildrenRemoved::new(nk_root));
}