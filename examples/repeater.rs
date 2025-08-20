use bevy::prelude::*;
use bevy::reflect::Reflect;
use bevy_gearbox::prelude::*;
use bevy_gearbox::GearboxPlugin;
use bevy_gearbox::transitions::TransitionListener;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, input_system)
        .add_observer(transition_listener::<CastAbility>)
        .add_observer(transition_listener::<OnRepeat>)
        .add_observer(transition_listener::<OnComplete>)
        .add_observer(on_enter_repeating_emit_events)
        .add_observer(reset_repeater)
        .add_observer(print_enter_state_messages)
        .add_observer(print_onrepeat)
        .add_observer(print_oncomplete)
        .register_type::<AbilityMachine>()
        .register_type::<Repeater>()
        // ResetEdge/ResetScope are provided by core
        .register_type::<TransitionListener<CastAbility>>()
        .register_type::<TransitionListener<OnRepeat>>()
        .register_type::<TransitionListener<OnComplete>>()
        .add_plugins(bevy_gearbox_editor::GearboxEditorPlugin)
        .run();
}

// --- Events reflected so they can be referenced in the scene file ---

#[derive(Event, Clone, Reflect, Default)]
struct CastAbility;

#[derive(Event, Clone, Reflect, Default)]
struct OnRepeat;

#[derive(Event, Clone, Reflect, Default)]
struct OnComplete;

#[derive(Component, Reflect)]
#[reflect(Component)]
struct AbilityMachine;

// Component to attach to the Repeat state
#[derive(Component, Reflect)]
#[reflect(Component, Default)]
struct Repeater { remaining: u32, initial: u32 }

impl Default for Repeater { 
    fn default() -> Self { 
        Self { remaining: 5, initial: 5 } 
    } 
}

// Edge marker used by the edge action reset
// No longer needed: use ResetEdge(ResetScope) built into core

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);

    // Load the scene built to mirror examples/repeater2.rs
    commands.spawn((
        Name::new("State machine (from scene)"),
        DynamicSceneRoot(asset_server.load("repeater.scn.ron")),
    ));
}

fn input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    machines: Query<Entity, With<AbilityMachine>>,
    mut commands: Commands,
) {
    let Ok(machine) = machines.single() else { println!("No machine found"); return; };
    if keyboard_input.just_pressed(KeyCode::KeyC) {
        println!("\n--- 'C' Pressed: Sending CastAbility event! ---");
        commands.trigger_targets(CastAbility, machine);
    }
}

// Emits OnRepeat/OnComplete when entering a state with Repeater
fn on_enter_repeating_emit_events(
    trigger: Trigger<EnterState>,
    mut repeater_q: Query<&mut Repeater>,
    child_of_q: Query<&StateChildOf>,
    mut commands: Commands,
) {
    let state = trigger.target();
    let Ok(mut repeater) = repeater_q.get_mut(state) else { return; };
    let root = child_of_q.root_ancestor(state);
    repeater.remaining -= 1;
    if repeater.remaining > 0 {
        commands.trigger_targets(OnRepeat, root);
    } else {
        commands.trigger_targets(OnComplete, root);
    }
}

fn reset_repeater(
    trigger: Trigger<Reset>,
    mut repeater_q: Query<&mut Repeater>,
) {
    let state = trigger.target();

    println!("Resetting repeater for state: {:?}", state);

    let Ok(mut repeater) = repeater_q.get_mut(state) else { return; };
    repeater.remaining = repeater.initial;
}

// Debug helpers
fn print_enter_state_messages(trigger: Trigger<EnterState>, names: Query<&Name>) {
    if let Ok(name) = names.get(trigger.target()) {
        println!("[STATE ENTERED]: {}", name);
    }
}

fn print_onrepeat(_t: Trigger<OnRepeat>) {
    println!("OnRepeat event emitted");
}

fn print_oncomplete(_t: Trigger<OnComplete>) {
    println!("OnComplete event emitted");
}