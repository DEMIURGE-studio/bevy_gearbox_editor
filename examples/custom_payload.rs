use bevy::prelude::*;
use bevy_gearbox::prelude::*;
use bevy_gearbox::GearboxPlugin;
use bevy::math::primitives::{Plane3d, Sphere, Cuboid};
use bevy_gearbox::transitions::{AlwaysEdge, After};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_gearbox_editor::GearboxEditorPlugin;

// This example focuses on TransitionEvent payloads: mapping a trigger event into
// typed Entry/Exit/Effect phase events that carry data (like target and damage).
// Why payloads?
// - Keep transition logic declarative: edges listen for simple triggers, while
//   payload mapping decides what should happen in each phase.
// - Decouple cross-entity effects: the shooter can carry the defender Entity in
//   the payload without hard-coding system dependencies between their machines.
// - Strong typing across phases: Entry/Exit/Effect events are structured and testable.
// Alternatives:
// - Derive SimpleTransition and do the work in observers using global state/components.
// - Fire the final effect event directly (e.g., send TakeDamage to the defender).
// - Attach components to roots and have states read them.
// Tradeoffs:
// - Direct events/components can work, but payloads localize intent to the transition
//   and make multi-phase sequencing explicit (and toolable in the editor).
// - For one-off effects, direct events are fine; for multi-phase or cross-machine flows,
//   payloads scale better and remain readable.

// --- Events ---

#[derive(Event, Clone)]
struct Attack { pub target: Entity, pub damage: f32 }

#[derive(Event, Clone)]
struct ApplyDamage { pub target: Entity, pub damage: f32 }

#[derive(Event, Clone)]
struct TakeDamage { pub amount: f32 }

#[derive(Event, Clone)]
struct DoDamage { pub amount: f32 }

#[derive(Event, Clone)]
struct Die;

// Map the trigger into a phase payload that emits ApplyDamage on Entry.
// The edge listens for Attack, but the Entry phase receives ApplyDamage with the
// original contextual data (target, damage).
impl TransitionEvent for Attack {
    type EntryEvent = ApplyDamage;

    fn to_entry_event(&self) -> Option<Self::EntryEvent> {
        Some(ApplyDamage { target: self.target, damage: self.damage })
    }
}

impl TransitionEvent for TakeDamage {
    type EntryEvent = DoDamage;
    fn to_entry_event(&self) -> Option<Self::EntryEvent> { Some(DoDamage { amount: self.amount }) }
}

impl TransitionEvent for Die {}

// --- State markers ---

#[derive(Component, Clone)]
struct Waiting;

#[derive(Component, Clone)]
struct Attacking;

#[derive(Component, Clone)]
struct TargetWaiting;

#[derive(Component, Clone)]
struct TakingDamageState;

#[derive(Component, Clone)]
struct Dead;

#[derive(Component)]
struct DummyTarget;

#[derive(Component)]
struct Shooter;

#[derive(Component)]
struct DamageAmount(pub f32);

#[derive(Component)]
struct Life(pub f32);

#[derive(Component)]
struct BounceTowards { home: Vec3, goal: Vec3, out_speed: f32, return_speed: f32, phase: BouncePhase }

#[derive(Clone, Copy, PartialEq, Eq)]
enum BouncePhase { Out, Return }

#[derive(Resource, Default)]
struct RespawnQueue(Vec<RespawnRequest>);

struct RespawnRequest { position: Vec3, delay: f32, timer: f32 }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin, GearboxEditorPlugin))
        .init_resource::<RespawnQueue>()
        .add_transition_event::<Attack>()
        .add_transition_event::<TakeDamage>()
        .add_transition_event::<Die>()
        .add_observer(print_enter_state)
        .add_observer(apply_damage_system)
        .add_observer(on_enter_taking_damage_color)
        .add_observer(on_exit_taking_damage_color)
        .add_observer(do_damage_on_entry)
        .add_systems(Startup, setup)
        .add_systems(Update, (input_attack_event, drive_bounces, process_respawn_queue))
        .add_state_component::<Waiting>()
        .add_state_component::<Attacking>()
        .add_state_component::<TargetWaiting>()
        .add_state_component::<TakingDamageState>()
        .add_state_component::<Dead>()
        .run();
}

fn setup(mut commands: Commands) {
    commands.queue(|world: &mut World| {
        // Camera
        world.spawn((
            Camera3d::default(),
            Transform::from_xyz(0.0, 8.0, 14.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));

        // Light
        world.spawn((
            DirectionalLight::default(),
            Transform::from_xyz(6.0, 10.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        ));

        // Ground
        {
            let ground_mesh = {
                let mut meshes = world.resource_mut::<Assets<Mesh>>();
                meshes.add(Mesh::from(Plane3d::default().mesh().size(50.0, 50.0)))
            };
            let ground_mat = {
                let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.22, 0.25),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            };

            world.spawn((
                Name::new("Ground"),
                Mesh3d(ground_mesh),
                MeshMaterial3d(ground_mat),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ));
        }

        // Shooter and Target visuals
        let shooter = {
            // Shooter assets
            let shooter_mesh = {
                let mut meshes = world.resource_mut::<Assets<Mesh>>();
                meshes.add(Mesh::from(Sphere { radius: 0.5 }))
            };
            let shooter_mat = {
                let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                materials.add(StandardMaterial { base_color: Color::from(bevy::color::palettes::css::GREEN), ..default() })
            };

            world.spawn((
                Name::new("Shooter"),
                Shooter,
                Mesh3d(shooter_mesh),
                MeshMaterial3d(shooter_mat),
                Transform::from_xyz(-4.0, 0.5, 0.0),
                DamageAmount(25.0),
            )).id()
        };

        // Spawn initial defender via template
        let _ = spawn_defender(world, Vec3::new(4.0, 0.75, 0.0));

        // Shooter state machine
        let waiting = world.spawn((
            Name::new("Waiting"),
            StateChildOf(shooter),
            StateComponent(Waiting),
        )).id();

        let attacking = world.spawn((
            Name::new("Attack"),
            StateChildOf(shooter),
            StateComponent(Attacking),
        )).id();

        // Edge: Waiting --(Attack{target,damage})--> Attack
        world.spawn((
            Name::new("Attack"),
            Source(waiting),
            Target(attacking),
            EventEdge::<Attack>::default(),
        ));

        // Edge: Attack --(Always)--> Waiting (immediate return)
        world.spawn((
            Name::new("Always"),
            Source(attacking),
            Target(waiting),
            AlwaysEdge,
        ));

        // Bounce motion: on entering Attack, add BounceTowards to shooter toward defender
        world.entity_mut(attacking).observe(|trigger: Trigger<EnterState>,
            child_of: Query<&StateChildOf>,
            transforms: Query<&Transform>,
            mut commands: Commands,
            targets: Query<&Transform, With<DummyTarget>>,
        |{
            let state = trigger.target();
            let root = child_of.root_ancestor(state);
            if let Ok(tf) = transforms.get(root) {
                // Clamp bounce distance to avoid reaching the target; anchor to starting position
                let home = tf.translation;
                // Try to get current target position; fall back to a fixed point
                let goal = targets.iter().next().map(|t| t.translation).unwrap_or(Vec3::new(4.0, 0.75, 0.0));
                let dir = (goal - home).normalize_or_zero();
                let bump = 0.8; // meters to move outward
                let goal_pos = home + dir * bump;
                commands.entity(root).insert(BounceTowards { home, goal: goal_pos, out_speed: 18.0, return_speed: 24.0, phase: BouncePhase::Out });
            }
        });

        world.entity_mut(shooter).insert(InitialState(waiting));
        world.entity_mut(shooter).insert(StateMachine::new());

        // Defender machine is created inside spawn_defender

    });
}

// Press Space to fire: send Attack(target, damage) to the shooter machine.
// Why send Attack to the shooter instead of TakeDamage to the defender?
// - The shooter owns the decision to attack. Payloads carry the defender Entity and
//   damage through the transition phases, so the edge can stay generic and the effect
//   (ApplyDamage/DoDamage) happens at the right time (Entry of target state).
fn input_attack_event(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    shooters: Query<(Entity, &DamageAmount), With<Shooter>>,
    dummy: Query<Entity, With<DummyTarget>>,
    mut commands: Commands,
) {
    if !keyboard_input.just_pressed(KeyCode::Space) { return; }
    let Ok((machine, damage)) = shooters.single() else { return; };
    let Ok(target) = dummy.single() else { return; };
    println!("\n-- Space: Attack -> target {:?}, damage {}", target, damage.0);
    commands.trigger_targets(Attack { target, damage: damage.0 }, machine);
}

fn drive_bounces(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut BounceTowards)>,
) {
    for (mut tf, mut b) in &mut q {
        match b.phase {
            BouncePhase::Out => {
                let to = b.goal - tf.translation;
                let d = to.length();
                let step = b.out_speed * time.delta().as_secs_f32();
                if d <= step { tf.translation = b.goal; b.phase = BouncePhase::Return; } else if d > 0.0 { tf.translation += to.normalize() * step; }
            }
            BouncePhase::Return => {
                let to = b.home - tf.translation;
                let d = to.length();
                let step = b.return_speed * time.delta().as_secs_f32();
                if d <= step { tf.translation = b.home; }
                else if d > 0.0 { tf.translation += to.normalize() * step; }
            }
        }
    }
}


// Entry-phase handler: we received ApplyDamage (created from Attack's payload mapping).
// We forward a typed, minimal event (TakeDamage) to the specific defender entity.
// Alternative: skip ApplyDamage and trigger TakeDamage directly from input.
// Why this is better: the transition remains declarative and testable; input systems
// don't need to know which phase should apply damage.
fn apply_damage_system(
    trigger: Trigger<ApplyDamage>,
    mut commands: Commands,
) {
    let ApplyDamage { target, damage } = trigger.event().clone();
    commands.trigger_targets(TakeDamage { amount: damage }, target);
}

// Apply damage to Life on Entry of TakingDamage via payload event (DoDamage).
// This demonstrates consuming the Entry payload on the state's root entity.
fn do_damage_on_entry(
    trigger: Trigger<DoDamage>,
    child_of: Query<&StateChildOf>,
    mut life_q: Query<&mut Life>,
    transforms: Query<&Transform>,
    mut commands: Commands,
    mut respawns: ResMut<RespawnQueue>,
) {
    let amount = trigger.event().amount;
    let taking_state = trigger.target();
    let root = child_of.root_ancestor(taking_state);
    if let Ok(mut life) = life_q.get_mut(root) {
        life.0 -= amount;
        println!("[Damage] Applied {amount}, Life now {:.1}", life.0);
        if life.0 <= 0.0 {
            // Capture current position for respawn, enqueue, then despawn
            let mut pos = Vec3::ZERO;
            if let Ok(tf_ro) = transforms.get(root) { pos = tf_ro.translation; }
            respawns.0.push(RespawnRequest { position: pos, delay: 1.0, timer: 0.0 });
            commands.trigger_targets(Die, root);
            commands.entity(root).despawn();
        }
    }
}

// Visual feedback: turn red during TakingDamage, restore to gray on exit
fn on_enter_taking_damage_color(
    trigger: Trigger<EnterState>,
    names: Query<&Name>,
    child_of: Query<&StateChildOf>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_handles: Query<&mut MeshMaterial3d<StandardMaterial>>, 
) {
    let state = trigger.target();
    if let Ok(name) = names.get(state) {
        if name.as_str() != "TakingDamage" { return; }
    } else { return; }
    let root = child_of.root_ancestor(state);
    if let Ok(mat_handle) = material_handles.get_mut(root) {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = Color::from(bevy::color::palettes::css::RED);
        }
    }
}

fn on_exit_taking_damage_color(
    trigger: Trigger<ExitState>,
    names: Query<&Name>,
    child_of: Query<&StateChildOf>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut material_handles: Query<&mut MeshMaterial3d<StandardMaterial>>, 
) {
    let state = trigger.target();
    if let Ok(name) = names.get(state) {
        if name.as_str() != "TakingDamage" { return; }
    } else { return; }
    let root = child_of.root_ancestor(state);
    if let Ok(mat_handle) = material_handles.get_mut(root) {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = Color::from(bevy::color::palettes::css::GRAY);
        }
    }
}

fn process_respawn_queue(
    time: Res<Time>,
    mut respawns: ResMut<RespawnQueue>,
    mut commands: Commands,
) {
    // Tick timers and spawn new defenders when due
    let mut spawn_positions: Vec<Vec3> = Vec::new();
    for req in respawns.0.iter_mut() {
        req.timer += time.delta().as_secs_f32();
        if req.timer >= req.delay { spawn_positions.push(req.position); }
    }
    respawns.0.retain(|r| r.timer < r.delay);
    if spawn_positions.is_empty() { return; }
    commands.queue(move |world: &mut World| {
        for pos in spawn_positions {
            spawn_defender(world, pos);
        }
    });
}

fn spawn_defender(world: &mut World, position: Vec3) -> Entity {
    // Target assets then spawn
    let target_mesh = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        meshes.add(Mesh::from(Cuboid::new(1.0, 1.5, 1.0)))
    };
    let target_mat = {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        materials.add(StandardMaterial { base_color: Color::from(bevy::color::palettes::css::GRAY), ..default() })
    };

    let defender = world.spawn((
        Name::new("DummyTargetEntity"),
        DummyTarget,
        Mesh3d(target_mesh),
        MeshMaterial3d(target_mat),
        Transform::from_translation(position),
        Life(60.0),
    )).id();

    // Defender state machine (root = defender)
    let target_waiting = world.spawn((
        Name::new("TargetWaiting"),
        StateChildOf(defender),
        StateComponent(TargetWaiting),
    )).id();

    let taking_damage = world.spawn((
        Name::new("TakingDamage"),
        StateChildOf(defender),
        StateComponent(TakingDamageState),
    )).id();

    let dead = world.spawn((
        Name::new("Dead"),
        StateChildOf(defender),
        StateComponent(Dead),
    )).id();

    // Edge: TargetWaiting --(TakeDamage)--> TakingDamage
    world.spawn((
        Name::new("TakeDamage"),
        Source(target_waiting),
        Target(taking_damage),
        EventEdge::<TakeDamage>::default(),
    ));

    // Edge: Defender root --(Die)--> Dead
    world.spawn((
        Name::new("Die"),
        Source(defender),
        Target(dead),
        EventEdge::<Die>::default(),
    ));

    // Edge: TakingDamage --(Always, After 0.2s)--> TargetWaiting
    world.spawn((
        Name::new("Always"),
        Source(taking_damage),
        Target(target_waiting),
        AlwaysEdge,
        After { duration: std::time::Duration::from_millis(200) },
    ));

    world.entity_mut(defender).insert(InitialState(target_waiting));
    world.entity_mut(defender).insert(StateMachine::new());

    defender
}

fn print_enter_state(trigger: Trigger<EnterState>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.target()) {
        println!("[EnterState] {}", name);
    }
}