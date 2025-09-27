use std::time::Duration;

use bevy::{
    animation::RepeatAnimation,
    prelude::*,
    pbr::CascadeShadowConfigBuilder,
    window::PrimaryWindow,
    asset::AssetPlugin,
};
use bevy_gearbox::prelude::*;
use bevy_gearbox::GearboxPlugin;
use bevy_gearbox::transitions::{EdgeKind, DeferEvent};
use bevy::app::Animation as AnimationSet;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_gearbox_editor::GearboxEditorPlugin;

/// WARNING
/// WARNING
/// WARNING
/// This example is a testing ground for me to try to bridge the gap between bevy animation and gearbox.
/// It is not a good example of how to use gearbox.
/// It is not a good example of how to use bevy animation.
/// Hopefully what I learn here will help me make bevy gearbox into a powerful driver of animation.

// Point the AssetServer to the demiurge assets so we can load models/character.glb
const DEMIURGE_ASSETS_PATH: &str = "C:/git/demiurge/assets";
const CHARACTER_GLTF: &str = "models/character.glb";

#[derive(Event, Clone, SimpleTransition)]
struct SetIdle;
#[derive(Event, Clone, SimpleTransition)]
struct SetWalk;
#[derive(Event, Clone, SimpleTransition)]
struct SetRun;
#[derive(Event, Clone, SimpleTransition)]
struct Attack;

#[derive(Event, Clone, SimpleTransition)]
struct AnimationComplete;

#[derive(Component, Clone)]
struct AnimRequest {
    node: AnimationNodeIndex,
    crossfade: Duration,
    repeat: RepeatAnimation,
}

#[derive(Component, Clone)]
struct AnimationCompleteEmitter {
    node: AnimationNodeIndex,
}

#[derive(Resource)]
struct AnimGraph {
    handle: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    punch: AnimationNodeIndex,
}

pub fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(AssetPlugin {
                file_path: DEMIURGE_ASSETS_PATH.into(),
                ..default()
            }),
        )
        .add_plugins(GearboxPlugin)
        .add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin, GearboxEditorPlugin))
        .add_transition_event::<SetIdle>()
        .add_transition_event::<SetWalk>()
        .add_transition_event::<SetRun>()
        .add_transition_event::<Attack>()
        .add_transition_event::<AnimationComplete>()
        .add_state_component::<AnimRequest>()
        .insert_resource(AmbientLight { color: Color::WHITE, brightness: 1500., ..default() })
        .add_systems(Startup, (setup_camera_light, setup_character))
        .add_systems(Update, (
            setup_player_once_loaded,
            build_machine_when_ready,
            input,
            evaluate_parameter_edges,
        ))
        .add_systems(PostUpdate, emit_animation_complete_events.after(AnimationSet))
        .add_observer(apply_anim_request_on_enter)
        // Debug observers to trace state entries/exits
        .add_observer(log_enter_state)
        .add_observer(log_exit_state)
        .run();
}

fn setup_camera_light(
    mut commands: Commands,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let aspect = if let Some(win) = windows.iter().next() {
        win.width() / win.height().max(1.0)
    } else { 16.0 / 9.0 };

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(6.0 * aspect, 3.0, 8.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    commands.spawn((
        Transform::from_xyz(5.0, 8.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        DirectionalLight { shadows_enabled: true, illuminance: 20_000., ..default() },
        CascadeShadowConfigBuilder { first_cascade_far_bound: 12.0, maximum_distance: 40.0, ..default() }.build(),
    ));
}

fn setup_character(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    // Build an animation graph matching your animations in demiurge/src/character/animation.rs
    let mut graph = AnimationGraph::new();
    let idle = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(0).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _combat_idle = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(1).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _death = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(2).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _flinch1 = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(3).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _flinch2 = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(4).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let punch = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(5).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _dodge_roll = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(6).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _kick = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(7).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let walk = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(8).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let run = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(9).from_asset(CHARACTER_GLTF)), 1.0, graph.root);
    let _jump = graph.add_clip(asset_server.load(GltfAssetLabel::Animation(10).from_asset(CHARACTER_GLTF)), 1.0, graph.root);

    let handle = graphs.add(graph);

    commands.insert_resource(AnimGraph {
        handle,
        idle,
        walk,
        run,
        punch,
    });

    // Spawn the character scene root
    commands.spawn((
        SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset(CHARACTER_GLTF))),
        Name::new("CharacterRoot"),
    ));
}

#[derive(Component)]
struct MachineBuilt;

#[derive(Component)]
struct AnimMachineRoot;

// Removed ClipTracker; completion is detected from ActiveAnimation flags post animation update

// Generic parameter plan (example-level only)
#[derive(Component, Debug, Clone, Copy, Default)]
struct Velocity(Vec3);

trait ParameterOf<T: Component> {
    fn in_range(&self, param: &T) -> bool;
}

// Edge guard marker to denote a parameter-based guard
#[derive(Component)]
struct EdgeParameter;

// Example multi-purpose parameter component living on an edge.
// For now we only implement ParameterOf<Velocity>, but this can grow to include more sources.
#[derive(Component, Debug, Clone, Copy, Default)]
struct LocomotionParams {
    lower_velocity: f32,
    upper_velocity: f32,
    hysteresis_velocity: f32,
}

impl ParameterOf<Velocity> for LocomotionParams {
    fn in_range(&self, param: &Velocity) -> bool {
        let v = param.0.length();
        v + self.hysteresis_velocity >= self.lower_velocity && v - self.hysteresis_velocity <= self.upper_velocity
    }
}

fn setup_player_once_loaded(
    mut commands: Commands,
    graph: Res<AnimGraph>,
    mut q: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut q {
        // Attach the graph and start idle via AnimationTransitions so we can crossfade later
        let mut transitions = AnimationTransitions::new();
        transitions.play(&mut player, graph.idle, Duration::ZERO).repeat();
        commands.entity(entity)
            .insert(AnimationGraphHandle(graph.handle.clone()))
            .insert(transitions);
    }
}

fn build_machine_when_ready(
    mut commands: Commands,
    graph: Res<AnimGraph>,
    q_player: Query<Entity, (With<AnimationPlayer>, Without<MachineBuilt>)>,
) {
    for root in &q_player {
        // Root is the animated entity; build a small state machine under it
        // States: Grounded (History::Deep) -> Locomotion (Idle/Walk/Run), and Punch sibling
        let grounded = commands.spawn((StateChildOf(root), Name::new("Grounded"), History::Deep)).id();
        let locomotion = commands.spawn((StateChildOf(grounded), Name::new("Locomotion"), History::Deep)).id();
        // Note: Transitions list is auto-managed via relationships when edges are spawned
        let idle_state = commands.spawn((
            StateChildOf(locomotion),
            Name::new("Idle"),
            StateComponent(AnimRequest { node: graph.idle, crossfade: Duration::from_millis(200), repeat: RepeatAnimation::Forever }),
        )).id();
        let walk_state = commands.spawn((
            StateChildOf(locomotion),
            Name::new("Walk"),
            StateComponent(AnimRequest { node: graph.walk, crossfade: Duration::from_millis(200), repeat: RepeatAnimation::Forever }),
        )).id();
        let run_state = commands.spawn((
            StateChildOf(locomotion),
            Name::new("Run"),
            StateComponent(AnimRequest { node: graph.run, crossfade: Duration::from_millis(200), repeat: RepeatAnimation::Forever }),
        )).id();
        let punch_state = commands.spawn((
            StateChildOf(grounded),
            Name::new("Punch"),
            StateComponent(AnimRequest { node: graph.punch, crossfade: Duration::from_millis(120), repeat: RepeatAnimation::Count(1) }),
            DeferEvent::<Attack>::new(),
            AnimationCompleteEmitter { node: graph.punch },
        )).id();

        // Initials
        commands.entity(grounded).insert(InitialState(locomotion));
        commands.entity(locomotion).insert(InitialState(idle_state));
        commands.entity(root).insert((StateMachine::new(), InitialState(grounded)));
        // Attach example parameter source to the machine root
        commands.entity(root).insert(Velocity(Vec3::ZERO));

        // Edges on Locomotion: events select a child
        let _e_idle = commands.spawn((
            Source(locomotion),
            Target(idle_state),
            EventEdge::<SetIdle>::default(),
            EdgeKind::Internal,
            Name::new("Locomotion->Idle"),
        )).id();
        let _e_walk = commands.spawn((
            Source(locomotion),
            Target(walk_state),
            EventEdge::<SetWalk>::default(),
            EdgeKind::Internal,
            Name::new("Locomotion->Walk"),
        )).id();
        let _e_run = commands.spawn((
            Source(locomotion),
            Target(run_state),
            EventEdge::<SetRun>::default(),
            EdgeKind::Internal,
            Name::new("Locomotion->Run"),
        )).id();

        // Add Always edges with parameter guards to drive child selection
        let _p_to_idle = commands.spawn((
            Source(locomotion),
            Target(idle_state),
            AlwaysEdge,
            EdgeKind::Internal,
            LocomotionParams { lower_velocity: 0.0, upper_velocity: 0.15, hysteresis_velocity: 0.03 },
            EdgeParameter,
            Name::new("Param: speed in [0, 0.15] -> Idle"),
        )).id();
        let _p_to_walk = commands.spawn((
            Source(locomotion),
            Target(walk_state),
            AlwaysEdge,
            EdgeKind::Internal,
            LocomotionParams { lower_velocity: 0.15, upper_velocity: 1.2, hysteresis_velocity: 0.05 },
            EdgeParameter,
            Name::new("Param: speed in (0.15, 1.2] -> Walk"),
        )).id();
        let _p_to_run = commands.spawn((
            Source(locomotion),
            Target(run_state),
            AlwaysEdge,
            EdgeKind::Internal,
            LocomotionParams { lower_velocity: 1.2, upper_velocity: 999.0, hysteresis_velocity: 0.1 },
            EdgeParameter,
            Name::new("Param: speed > 1.2 -> Run"),
        )).id();

        // Edge on Grounded: Attack goes to Punch
        let _e_attack = commands.spawn((
            Source(grounded),
            Target(punch_state),
            EventEdge::<Attack>::default(),
            EdgeKind::Internal,
            Name::new("Grounded->Punch"),
        )).id();
        // After Punch, return to Grounded; History::Deep will restore previous Locomotion substate
        let _e_punch_done = commands.spawn((
            Source(punch_state),
            Target(grounded ),
            EventEdge::<AnimationComplete>::default(),
            Name::new("Punch->Locomotion(AnimationComplete)"),
        )).id();


        // Initialize the machine by inserting a marker so we don't rebuild; the plugin will auto-init on add
        commands.entity(root).insert((MachineBuilt, AnimMachineRoot));
    }
}

fn apply_anim_request_on_enter(
    trigger: Trigger<EnterState>,
    q_state_component: Query<&StateComponent<AnimRequest>>,
    q_child_of: Query<&StateChildOf>,
    names: Query<&Name>,
    mut q_player: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let state = trigger.target();
    let Ok(req) = q_state_component.get(state) else { return; };
    let root = q_child_of.root_ancestor(state);
    if let Ok((mut player, mut transitions)) = q_player.get_mut(root) {
        if let Ok(name) = names.get(state) {
            println!(
                "[AnimEnter] state={} node={:?} crossfade={:?} repeat={:?}",
                name.as_str(), req.0.node, req.0.crossfade, req.0.repeat
            );
        }
        let play = transitions.play(&mut player, req.0.node, req.0.crossfade);
        match req.0.repeat {
            RepeatAnimation::Forever => {
                play.repeat();
            }
            _ => {
                if let Some(anim) = player.animation_mut(req.0.node) { anim.set_repeat(req.0.repeat).replay(); }
                println!("[AnimTrack] root={:?} node={:?} non-looping", root, req.0.node);
            }
        }
    }
}

// Simple demo: adjust locomotion speed parameter with keys and print value
// Update Velocity from arrows just to demo parameter edges
fn input(
    input: Res<ButtonInput<KeyCode>>,
    mut q: Query<(Entity, &mut Velocity), With<AnimMachineRoot>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (root, mut v) in &mut q {
        if input.just_pressed(KeyCode::Digit1) {
            println!("[Input] 1 pressed -> Attack");
            commands.trigger_targets(Attack, root);
        }

        let mut mag_delta = 0.0;
        if input.pressed(KeyCode::ArrowUp) { mag_delta += 2.0; }
        if input.pressed(KeyCode::ArrowDown) { mag_delta -= 2.0; }
        if mag_delta != 0.0 {
            let cur_speed = v.0.length();
            let new_speed = (cur_speed + mag_delta * time.delta_secs()).clamp(0.0, 10.0);
            let dir = if cur_speed <= f32::EPSILON { Vec3::Z } else { v.0.normalize() };
            v.0 = dir * new_speed;
            println!("[Velocity] |v| = {:.2}", new_speed);
        }
    }
}

// Evaluate parameter-guarded Always edges and trigger child selection events
fn evaluate_parameter_edges(
    q_roots: Query<(Entity, &Velocity), With<AnimMachineRoot>>,
    q_edges: Query<(Entity, &Source, &Target, Option<&LocomotionParams>), With<EdgeParameter>>,
    names: Query<&Name>,
    mut commands: Commands,
) {
    for (root, vel) in &q_roots {
        for (_edge, _source, target, vparam) in &q_edges {
            // Only consider edges that originate from a region under this root
            // (simple example: just evaluate all; a real impl would scope by ancestry)
            if let Some(lp) = vparam {
                if lp.in_range(vel) {
                    // Drive via existing Set* events by target name in this example
                    if let Ok(name) = names.get(target.0) {
                        match name.as_str() {
                            "Idle" => commands.trigger_targets(SetIdle, root),
                            "Walk" => commands.trigger_targets(SetWalk, root),
                            "Run" => commands.trigger_targets(SetRun, root),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

// Debug: log state entries/exits (uses Name added to states)
fn log_enter_state(trigger: Trigger<EnterState>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.target()) {
        println!("[State] Enter: {}", name.as_str());
    }
}

fn log_exit_state(trigger: Trigger<ExitState>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.target()) {
        println!("[State] Exit: {}", name.as_str());
    }
}

fn emit_animation_complete_events(
    mut commands: Commands,
    q_states: Query<(Entity, &AnimationCompleteEmitter), With<Active>>,
    q_child_of: Query<&StateChildOf>,
    q_player: Query<&AnimationPlayer>,
) {
    for (state, emitter) in &q_states {
        let root = q_child_of.root_ancestor(state);
        if let Ok(player) = q_player.get(root) {
            if let Some(active) = player.animation(emitter.node) {
                if active.is_finished() {
                    println!("[AnimComplete] state={:?} node={:?}", state, emitter.node);
                    commands.trigger_targets(AnimationComplete, root);
                }
            }
        }
    }
}
