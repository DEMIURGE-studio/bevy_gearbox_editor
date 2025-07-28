# Bevy Gearbox Visual Editor

A **Stately-inspired visual statechart editor** for [bevy_gearbox](../bevy_gearbox) built natively in Bevy using [egui-snarl](https://github.com/zakarumych/egui-snarl) and `bevy_reflect`.

## 🎯 Vision

NOTE: All if this is subject to change. This is the vision going in, but if better patterns emerge then we will follow this.

Create a comprehensive visual editor for bevy_gearbox statecharts that combines the best of Stately's user experience with Rust/Bevy-specific features. The editor provides visual design, code generation, real-time debugging, and seamless integration with Bevy projects.

## 🏗️ Architecture

### Technology Stack

```rust
egui-snarl        // Visual node graph editor foundation
bevy_reflect      // Automatic serialization and introspection  
bevy + egui       // Native Bevy application framework
RON               // Human-readable asset serialization format
bevy_gearbox      // State machine runtime (existing)
```

### Key Design Principles

- **Native Bevy Integration**: Runs as a Bevy plugin with full ECS access
- **Component-Driven**: States and behaviors defined through components
- **Type-Safe**: Leverages Rust's type system for compile-time validation
- **Human-Readable Assets**: Uses RON format for version control friendly files
- **Real-Time Debugging**: Live connection to running Bevy applications

## 🎨 Features

### Core Visual Editor
- **Drag-and-drop state creation** using egui-snarl's node system
- **Visual state hierarchy** for nested and compound states
- **Transition arrows** with event labels and guards
- **Parallel state regions** (orthogonal states)
- **History state indicators** (shallow/deep)
- **Component visualization** showing state-specific components

### bevy_gearbox Integration
- **Entity-based states** matching bevy_gearbox architecture
- **Event system visualization** for Bevy observer events
- **Guard condition editor** with visual badges
- **Active state marking** during runtime debugging

### Code Generation
```rust
// Generated setup function example
fn setup_ability_machine(mut commands: Commands) {
    let ready = commands.spawn(Name::new("Ready")).id();
    let casting = commands.spawn(Name::new("Casting")).id();
    
    let machine = commands.spawn((
        AbilityMachine,
        InitialState(ready),
        CurrentState(HashSet::new()),
        Name::new("Ability State Machine"),
    )).id();
    
    // Generated transition logic...
}
```

### Runtime Debugging
- **Live state visualization** showing currently active states
- **Event tracing** visualizing event flow and transitions
- **State history timeline** of state changes
- **WebSocket connection** to running Bevy applications

## 📦 Asset Format

State machines are saved as `.gearbox.ron` files using Bevy's reflection system:

```ron
StateMachine(
  id: "ability_system",
  name: "Ability State Machine",
  states: {
    "ready": StateNode(
      name: "Ready",
      position: (100.0, 200.0),
      node_type: Simple,
      components: [
        "bevy_gearbox::Active",
      ],
      transitions: [
        Transition(
          event: "CastSpell",
          target: "casting",
          guards: [],
        ),
      ],
    ),
    "casting": StateNode(
      name: "Casting",
      position: (300.0, 200.0),
      node_type: Simple,
      components: [
        "MyGame::CastingTimer",
        "MyGame::MovementBlocked",
      ],
      transitions: [
        Transition(
          event: "CastComplete",
          target: "ready",
          guards: ["has_mana"],
        ),
      ],
    ),
  },
  initial_state: "ready",
  events: [
    EventDef(name: "CastSpell", data_type: None),
    EventDef(name: "CastComplete", data_type: None),
  ],
  guards: [
    GuardDef(name: "has_mana", condition: "mana > 10"),
  ],
)
```

## 🚀 Implementation Plan

### Phase 1: Foundation (Weeks 1-2)
- [ ] Basic egui-snarl integration
- [ ] Simple state node rendering  
- [ ] RON serialization/deserialization
- [ ] File open/save functionality
- [ ] Basic editor plugin structure

### Phase 2: Core Features (Weeks 3-4)
- [ ] Transition creation and editing
- [ ] Property panels for nodes
- [ ] Event and guard nodes
- [ ] Basic code generation
- [ ] Component management system

### Phase 3: Advanced Features (Weeks 5-6)
- [ ] Hierarchical state support
- [ ] Parallel state regions
- [ ] History states (shallow/deep)
- [ ] Enhanced code generation
- [ ] Asset loading integration

### Phase 4: Integration & Polish (Weeks 7-8)
- [ ] Runtime debugging connection
- [ ] Live state visualization
- [ ] Hot-reload support
- [ ] Templates library
- [ ] Documentation and examples

## 🛠️ Core Data Structures

```rust
#[derive(Reflect, Serialize, Deserialize, Asset)]
pub struct StateMachine {
    pub id: String,
    pub name: String,
    pub states: HashMap<String, StateNode>,
    pub initial_state: String,
    pub events: Vec<EventDefinition>,
    pub guards: Vec<GuardDefinition>,
}

#[derive(Reflect, Serialize, Deserialize)]
pub struct StateNode {
    pub name: String,
    pub position: Vec2,
    pub node_type: StateNodeType,
    pub components: Vec<String>,      // Component type names
    pub transitions: Vec<Transition>,
    pub on_enter: Vec<String>,        // System names
    pub on_exit: Vec<String>,         // System names
}

#[derive(Reflect, Serialize, Deserialize)]
pub enum StateNodeType {
    Simple,
    Compound { initial: String },
    Parallel,
    History { depth: HistoryDepth },
}
```

## 🎮 Usage Example

1. **Design** your state machine in the visual editor
2. **Save** as `ability_machine.gearbox.ron`
3. **Generate** Rust code for your Bevy project
4. **Integrate** into your app:

```rust
use bevy::prelude::*;
use bevy_gearbox::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GearboxPlugin)
        .add_systems(Startup, setup_ability_machine) // Generated function
        .run();
}
```

5. **Debug** live with real-time state visualization

## 🎯 Advantages

### 🚀 Rapid Development
- **egui-snarl** handles complex graph editing logic
- **bevy_reflect** provides automatic serialization
- **Native Bevy integration** eliminates context switching

### 🔧 Perfect Integration
- **Same ECS principles** as target applications
- **RON format** is human-readable and git-friendly
- **Component-driven design** matches Bevy patterns

### 💡 Extensibility
- **Custom node types** for game-specific behaviors
- **Plugin architecture** for new features
- **Runtime debugging** provides immediate feedback

## 📋 Dependencies

```toml
[dependencies]
bevy = "0.16"
bevy_gearbox = { path = "../bevy_gearbox" }
egui-snarl = "0.8"
serde = { version = "1.0", features = ["derive"] }
ron = "0.8"
```

## 🤝 Contributing

This project is in active development. Contributions are welcome!

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests where appropriate
5. Submit a pull request

## 📄 License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
