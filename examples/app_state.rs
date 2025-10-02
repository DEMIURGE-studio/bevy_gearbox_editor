// Why drive Bevy States with Gearbox?
// - App state visualization: Build and inspect your flow as a chart. With
//   bevy_gearbox_editor you can lay out nodes/edges and persist positions.
// - Typed transition payloads: Use strongly-typed events and map them to entry
//   events to carry data into the next state.
// - History states: Shallow/Deep history restore the last active child/leaf
//   when you re-enter a region.
// - Parallel regions: Model orthogonal UI/logic regions that update in
//   parallel under a parent state.
// - Control primitives: Guards (enable/disable edges), After timers (delay),
//   and deferred events (queue until a state exits) let you express complex
//   behavior declaratively.
// - Decoupled input/UI: Emit events toward the chart (e.g. via a root marker),
//   and keep transition logic out of UI systems.
// - Seamless bridge: The provided bridge updates Bevy's State/NextState so you
//   can still gate systems with in_state(..) while authoring logic in one place.
// - Consistency: If you're already using Gearbox to drive your character state
//   machine, you can use the same API to drive your app state machine.

use bevy::prelude::*;
use bevy_gearbox::prelude::*;
use bevy_gearbox::GearboxPlugin;
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;

#[derive(States, Component, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
enum ExampleState {
    #[default]
    Menu,
    Playing,
    Paused,
}

#[derive(EntityEvent, Clone, bevy_gearbox::SimpleTransition)]
struct Start {
    #[event_target]
    pub target: Entity,
}
#[derive(EntityEvent, Clone, bevy_gearbox::SimpleTransition)]
struct Pause {
    #[event_target]
    pub target: Entity,
}
#[derive(EntityEvent, Clone, bevy_gearbox::SimpleTransition)]
struct Resume {
    #[event_target]
    pub target: Entity,
}

#[derive(Component)]
struct ChartRoot;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin, bevy_gearbox_editor::GearboxEditorPlugin))
        .init_state::<ExampleState>()
        .add_state_bridge::<ExampleState>()
        .add_transition_event::<Start>()
        .add_transition_event::<Pause>()
        .add_transition_event::<Resume>()
        .add_systems(Startup, setup_machine)
        .add_systems(OnEnter(ExampleState::Menu), || println!("ExampleState::Menu"))
        .add_systems(OnEnter(ExampleState::Playing), || println!("ExampleState::Playing"))
        .add_systems(OnEnter(ExampleState::Paused), || println!("ExampleState::Paused"))
        .add_systems(Update, demo_input)
        .add_observer(on_enter_state)
        .run();
}

fn setup_machine(mut commands: Commands) {
    // root -> { menu, playing, paused }
    let root = commands.spawn((ChartRoot, Name::new("AppStateMachine"))).id();

    let menu = commands.spawn((StateChildOf(root), ExampleState::Menu, Name::new("Menu"))).id();
    let playing = commands.spawn((StateChildOf(root), ExampleState::Playing, Name::new("Playing"))).id();
    let paused = commands.spawn((StateChildOf(root), ExampleState::Paused, Name::new("Paused"))).id();

    // Initial state is Menu
    commands.entity(root).insert((StateMachine::new(), InitialState(menu)));

    // Edges
    commands.spawn((Name::new("Start"), Source(menu), Target(playing), EventEdge::<Start>::default()));
    commands.spawn((Name::new("Pause"), Source(playing), Target(paused), EventEdge::<Pause>::default()));
    commands.spawn((Name::new("Resume"), Source(paused), Target(playing), EventEdge::<Resume>::default()));
}

fn demo_input(
    kb: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
) {
    use bevy_gearbox::prelude::GearboxCommandsExt;
    if kb.just_pressed(KeyCode::Digit1) {
        println!("Event: Start (Menu -> Playing)");
        commands.emit_to_chart::<ChartRoot>(move |root| Start { target: root });
    }
    if kb.just_pressed(KeyCode::Digit2) {
        println!("Event: Pause (Playing -> Paused)");
        commands.emit_to_chart::<ChartRoot>(move |root| Pause { target: root });
    }
    if kb.just_pressed(KeyCode::Digit3) {
        println!("Event: Resume (Paused -> Playing)");
        commands.emit_to_chart::<ChartRoot>(move |root| Resume { target: root });
    }
}

fn on_enter_state(
    enter_state: On<EnterState>,
    q_state: Query<&ExampleState>,
) {
    let entity = enter_state.target;

    let Ok(state) = q_state.get(entity) else {
        return;
    };
    println!("Enter gearbox state: {:?}", state);
}