use bevy::prelude::*;
use bevy_gearbox::prelude::*;
use bevy_gearbox::transitions::{Source, After, DeferEvent};
use bevy_gearbox::GearboxPlugin;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use std::time::Duration;
use bevy_gearbox::StateChildOf;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_plugins(EguiPlugin::default())
        .add_plugins(WorldInspectorPlugin::new())
        .add_systems(Startup, setup)
        .add_systems(Update, input_system)
        .add_transition_event::<RequestOpen>()
        .add_transition_event::<RequestClose>()
        .add_observer(print_enter_state_messages)
        .add_observer(print_exit_state_messages)
        .add_observer(replay_deferred_event::<RequestClose>)
        .add_state_component::<DoorClosed>()
        .add_state_component::<DoorOpening>()
        .add_state_component::<DoorOpen>()
        .add_state_component::<DoorClosing>()
        .run();
}

// --- State Machine Definition ---

/// The root of our door's state machine.
#[derive(Component)]
struct DoorMachine;

// --- State Marker Components ---

/// Marker component for when the door is closed
#[derive(Component, Clone)]
struct DoorClosed;

/// Marker component for when the door is opening
#[derive(Component, Clone)]
struct DoorOpening;

/// Marker component for when the door is open
#[derive(Component, Clone)]
struct DoorOpen;

/// Marker component for when the door is closing
#[derive(Component, Clone)]
struct DoorClosing;

// --- Events ---

/// Event triggered when requesting the door to open (W key)
#[derive(SimpleTransition, Event, Clone)]
struct RequestOpen;

/// Event triggered when requesting the door to close (E key)
#[derive(SimpleTransition, Event, Clone)]
struct RequestClose;

/// Creates the door state machine hierarchy.
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands.queue(move |world: &mut World| {
        // Create state entities - we need intermediate states to defer events
        let machine_entity = world.spawn(()).id();
        let closed = world.spawn(()).id();
        let opening = world.spawn(()).id();
        let open = world.spawn(()).id();
        let closing = world.spawn(()).id();

        // Create transition entities
        let closed_to_opening = world.spawn(()).id();
        let opening_to_open = world.spawn(()).id();
        let open_to_closing = world.spawn(()).id();
        let closing_to_closed = world.spawn(()).id();
        let closing_to_opening = world.spawn(()).id();

        // Set up the machine root
        world.entity_mut(machine_entity).insert((
            Name::new("DoorStateMachine"),
            DoorMachine,
            StateMachine::new(),
            InitialState(closed),
        ));

        // Set up states with marker components
        world.entity_mut(closed).insert((
            Name::new("Closed"),
            StateChildOf(machine_entity),
            StateComponent(DoorClosed),
        ));

        world.entity_mut(opening).insert((
            Name::new("Opening"),
            StateChildOf(machine_entity),
            StateComponent(DoorOpening),
            DeferEvent::<RequestClose>::new(), // Defer RequestClose while opening
        ));

        world.entity_mut(open).insert((
            Name::new("Open"),
            StateChildOf(machine_entity),
            StateComponent(DoorOpen),
        ));

        world.entity_mut(closing).insert((
            Name::new("Closing"),
            StateChildOf(machine_entity),
            StateComponent(DoorClosing),
        ));

        // Set up transitions - immediate event-driven transitions, then After delays
        world.entity_mut(closed_to_opening).insert((
            Name::new("Closed -> Opening (RequestOpen)"),
            Target(opening),
            EventEdge::<RequestOpen>::default(),
            EdgeKind::External,
            Source(closed),
        ));

        world.entity_mut(opening_to_open).insert((
            Name::new("Opening -> Open (After 1s)"),
            Target(open),
            Source(opening),
            After { duration: Duration::from_secs(1) }, // 1 second opening delay
        ));

        world.entity_mut(open_to_closing).insert((
            Name::new("Open -> Closing (RequestClose)"),
            Target(closing),
            EventEdge::<RequestClose>::default(),
            EdgeKind::External,
            Source(open),
        ));

        world.entity_mut(closing_to_closed).insert((
            Name::new("Closing -> Closed (After 1s)"),
            Target(closed),
            Source(closing),
            After { duration: Duration::from_secs(1) }, // 1 second closing delay
        ));

        world.entity_mut(closing_to_opening).insert((
            Name::new("Closing -> Opening (RequestOpen)"),
            Target(opening),
            EventEdge::<RequestOpen>::default(),
            EdgeKind::External,
            Source(closing),
        ));
    });
}

/// Handles keyboard input for door control events.
fn input_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    query: Query<Entity, With<DoorMachine>>,
    mut commands: Commands
) {
    let Ok(machine) = query.single() else { return };
    
    // Press 'W' to request door open
    if keyboard_input.just_pressed(KeyCode::KeyW) {
        println!("\n--- 'W' Pressed: Request door open (RequestOpen event) ---");
        commands.trigger_targets(RequestOpen, machine);
    }
    
    // Press 'E' to request door close
    if keyboard_input.just_pressed(KeyCode::KeyE) {
        println!("\n--- 'E' Pressed: Request door close (RequestClose event) ---");
        commands.trigger_targets(RequestClose, machine);
    }
}



/// Debug system to print messages when states are entered.
fn print_enter_state_messages(trigger: Trigger<EnterState>, query: Query<&Name>) {
    if let Ok(name) = query.get(trigger.target()) {
        println!("[STATE ENTERED]: {}", name);
    }
}

/// Debug system to print messages when states are exited.
fn print_exit_state_messages(trigger: Trigger<ExitState>, query: Query<&Name>) {
    if let Ok(name) = query.get(trigger.target()) {
        println!("[STATE EXITED]: {}", name);
    }
}
