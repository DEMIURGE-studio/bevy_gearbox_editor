use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_gearbox::prelude::*;
use bevy_gearbox::GearboxPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (input_system, repeater_system))
        .add_observer(transition_listener::<CastAbility>)
        .add_observer(transition_listener::<OnComplete>)
        .add_observer(print_enter_state_messages)
        .add_observer(reset_repeater_on_cast)
        .register_type::<AbilityMachine>()
        .register_type::<Repeater>()
        .register_type::<TransitionListener<CastAbility>>()
        .register_type::<TransitionListener<OnComplete>>()
        .add_plugins(bevy_gearbox_editor::GearboxEditorPlugin)
        .run();
}

// --- State Machine Definition ---

/// The root of our ability's state machine.
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct AbilityMachine;

/// A component to manage the repeater's state.
#[derive(Component, Reflect)]
#[reflect(Component, Default)]
struct Repeater {
    timer: Timer,
    remaining: u32,
}

impl Default for Repeater {
    fn default() -> Self {
        Self {
            timer: Timer::new(std::time::Duration::from_secs(1), TimerMode::Repeating),
            remaining: 5,
        }
    }
}

// --- Event to trigger state transitions ---
#[derive(Event, Clone, Reflect, Default)]
struct CastAbility;

/// An event fired by a state when its internal logic has completed.
#[derive(Event, Clone, Reflect, Default)]
struct OnComplete;

/// Creates the ability state machine hierarchy.
fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Spawn a camera - required for bevy-inspector-egui to render
    commands.spawn(Camera2d);
    
    // In the future, this will spawn a scene asset created by the editor.
    // For now, we spawn a placeholder entity that will eventually
    // be the root of the state machine loaded from a scene.
    commands.spawn((
        Name::new("StateMachineRoot (from scene)"),
        // This component will eventually be replaced by the SceneSpawner
        // logic that loads our asset.
        DynamicSceneRoot(asset_server.load("repeatertest.scn.ron")),
    ));
}

/// Listens for keyboard input and sends events to trigger state transitions.
fn input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    query: Query<Entity, With<AbilityMachine>>,
    mut commands: Commands
) {
    let Ok(machine) = query.single() else {
        return;
    };

    // Press 'C' to cast or reset the ability.
    if keyboard_input.just_pressed(KeyCode::KeyC) {
        println!("\n--- 'C' Pressed: Sending CastAbility event! ---");
        commands.trigger_targets(CastAbility, machine);
    }
}

/// The core logic for the repeater. Ticks the timer and fires "projectiles".
fn repeater_system(
    mut repeater_query: Query<(Entity, &mut Repeater), With<Active>>,
    child_of_query: Query<&bevy_gearbox::StateChildOf>,
    root_query: Query<&StateMachineRoot>,
    time: Res<Time>,
    mut commands: Commands,
) {
    // This system only runs when the machine is in the `Repeating` state.
    for (entity, mut repeater) in repeater_query.iter_mut() {
        repeater.timer.tick(time.delta());
        if repeater.timer.just_finished() {
            if repeater.remaining > 0 {
                println!("   => PEW! ({} remaining)", repeater.remaining - 1);
                repeater.remaining -= 1;
            }

            let root_entity = child_of_query.iter_ancestors(entity).find(|parent| root_query.contains(*parent)).unwrap_or(entity);

            if repeater.remaining == 0 {
                // The repeater is done. Fire the `OnComplete` event on the `Repeating`
                // state entity. The `TransitionListener` on that entity will handle
                // transitioning back to the `Ready` state.
                commands.trigger_targets(OnComplete, root_entity);
            }
        }
    }
}

/// When we re-enter the 'Ready' state, reset the repeater's values.
fn reset_repeater_on_cast(
    trigger: Trigger<ExitState>,
    mut repeater_query: Query<&mut Repeater>,
) {
    let target = trigger.target();
    if let Ok(mut repeater) = repeater_query.get_mut(target) {
        repeater.remaining = 5;
        repeater.timer.reset();
    }
}

/// A debug system to print a message every time any state is entered.
fn print_enter_state_messages(trigger: Trigger<EnterState>, query: Query<&Name>) {
    if let Ok(name) = query.get(trigger.target()) {
        println!("[STATE ENTERED]: {}", name);
    }
}