use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use egui_snarl::{
    ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use std::collections::VecDeque;

pub mod entity_ui;

pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .insert_resource(NodeGraphState::default())
            .insert_resource(EntitySpawnQueue::default())
            .add_systems(Update, process_entity_spawn_requests)
            .add_systems(EguiPrimaryContextPass, node_graph_ui_system);
    }
}

/// Resource to track entity spawn requests
#[derive(Resource, Default)]
pub struct EntitySpawnQueue {
    pub requests: VecDeque<NodeId>,
}

impl EntitySpawnQueue {
    pub fn request_spawn(&mut self, node_id: NodeId) {
        self.requests.push_back(node_id);
    }
}

/// System that processes entity spawn requests and updates nodes
fn process_entity_spawn_requests(
    mut commands: Commands,
    mut spawn_queue: ResMut<EntitySpawnQueue>,
    mut graph_state: ResMut<NodeGraphState>,
) {
    while let Some(node_id) = spawn_queue.requests.pop_front() {
        // Spawn a new entity in the world
        let entity = commands.spawn(Name::new("Gearbox Entity")).id();
        
        // Update the node with the spawned entity
        if let Some(node) = graph_state.snarl.get_node_mut(node_id) {
            if let GearboxNode::Entity(entity_node) = node {
                entity_node.entity = entity;
                entity_node.name = format!("Entity {:?}", entity);
                entity_node.needs_spawn = false;
            }
        }
    }
}

#[derive(Resource)]
pub struct NodeGraphState {
    pub snarl: Snarl<GearboxNode>,
    pub viewer: GearboxViewer,
}

impl Default for NodeGraphState {
    fn default() -> Self {
        let mut snarl = Snarl::new();

        let entity_node1 = snarl.insert_node(
            egui::pos2(100.0, 100.0),
            GearboxNode::Entity(EntityNode::new_pending_spawn()),
        );
        let entity_node2 = snarl.insert_node(
            egui::pos2(300.0, 200.0),
            GearboxNode::Entity(EntityNode::new_pending_spawn()),
        );

        snarl.connect(
            OutPinId {
                node: entity_node1,
                output: 0,
            },
            InPinId {
                node: entity_node2,
                input: 0,
            },
        );

        Self {
            snarl,
            viewer: GearboxViewer,
        }
    }
}

fn node_graph_ui_system(
    mut contexts: EguiContexts,
    mut graph_state: ResMut<NodeGraphState>,
    mut spawn_queue: ResMut<EntitySpawnQueue>,
) {
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::Window::new("Node Graph Editor")
            .default_size([800.0, 600.0])
            .show(ctx, |ui| {
                let NodeGraphState { snarl, viewer } = &mut *graph_state;
                
                // Check for nodes that need entity spawning
                let nodes_to_check: Vec<(NodeId, GearboxNode)> = snarl.node_ids().map(|(id, node)| (id, node.clone())).collect();
                for (node_id, node) in nodes_to_check {
                    if let GearboxNode::Entity(entity_node) = &node {
                        if entity_node.needs_spawn {
                            spawn_queue.request_spawn(node_id);
                        }
                    }
                }
                
                snarl.show(
                    viewer,
                    &SnarlStyle::new(),
                    "gearbox_graph",
                    ui,
                );
            });
    }
}

#[derive(Clone, Debug)]
pub enum GearboxNode {
    Entity(EntityNode),
    StateTransition(StateTransitionNode),
    EventTrigger(EventTriggerNode),
}

#[derive(Clone, Debug)]
pub struct EntityNode {
    pub entity: Entity,
    pub name: String,
    pub needs_spawn: bool,
}

#[derive(Clone, Debug)]
pub struct StateTransitionNode {
    pub from_state: String,
    pub to_state: String,
}

#[derive(Clone, Debug)]
pub struct EventTriggerNode {
    pub event_name: String,
}

impl EntityNode {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            name: format!("Entity {:?}", entity),
            needs_spawn: false,
        }
    }
    
    /// Creates a new EntityNode that needs an entity spawned
    pub fn new_pending_spawn() -> Self {
        Self {
            entity: Entity::PLACEHOLDER,
            name: "Entity (Spawning...)".to_string(),
            needs_spawn: true,
        }
    }
}

pub struct GearboxViewer;

impl SnarlViewer<GearboxNode> for GearboxViewer {
    fn title(&mut self, node: &GearboxNode) -> String {
        match node {
            GearboxNode::Entity(entity_node) => entity_node.name.clone(),
            GearboxNode::StateTransition(transition) => {
                format!("{} → {}", transition.from_state, transition.to_state)
            }
            GearboxNode::EventTrigger(event) => format!("Trigger: {}", event.event_name),
        }
    }

    fn inputs(&mut self, node: &GearboxNode) -> usize {
        match node {
            GearboxNode::Entity(_) => 1,
            GearboxNode::StateTransition(_) => 1,
            GearboxNode::EventTrigger(_) => 0,
        }
    }

    fn outputs(&mut self, node: &GearboxNode) -> usize {
        match node {
            GearboxNode::Entity(_) => 1,
            GearboxNode::StateTransition(_) => 1,
            GearboxNode::EventTrigger(_) => 1,
        }
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<GearboxNode>,
    ) -> impl SnarlPin + 'static {
        let node = &snarl[pin.id.node];

        match node {
            GearboxNode::Entity(_) => {
                ui.label("Input");
                PinInfo::circle().with_fill(egui::Color32::BLUE)
            }
            GearboxNode::StateTransition(_) => {
                ui.label("Trigger");
                PinInfo::triangle().with_fill(egui::Color32::GREEN)
            }
            GearboxNode::EventTrigger(_) => PinInfo::circle().with_fill(egui::Color32::RED),
        }
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<GearboxNode>,
    ) -> impl SnarlPin + 'static {
        let node = &snarl[pin.id.node];

        match node {
            GearboxNode::Entity(_) => {
                ui.label("Output");
                PinInfo::circle().with_fill(egui::Color32::RED)
            }
            GearboxNode::StateTransition(_) => {
                ui.label("Next");
                PinInfo::triangle().with_fill(egui::Color32::YELLOW)
            }
            GearboxNode::EventTrigger(_) => {
                ui.label("Event");
                PinInfo::square().with_fill(egui::Color32::PURPLE)
            }
        }
    }

    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<GearboxNode>,
    ) {
        match &mut snarl[node] {
            GearboxNode::Entity(entity_node) => {
                ui.text_edit_singleline(&mut entity_node.name);
                if entity_node.needs_spawn {
                    ui.label("Entity: (Spawning...)");
                } else {
                    ui.label(format!("Entity: {:?}", entity_node.entity));
                }
            }
            GearboxNode::StateTransition(transition) => {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut transition.from_state);
                    ui.label("→");
                    ui.text_edit_singleline(&mut transition.to_state);
                });
            }
            GearboxNode::EventTrigger(event) => {
                ui.text_edit_singleline(&mut event.event_name);
            }
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<GearboxNode>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<GearboxNode>,
    ) {
        ui.label("Add Node:");

        if ui.button("Entity Node").clicked() {
            // Create a new entity node that will automatically spawn an entity
            let node = GearboxNode::Entity(EntityNode::new_pending_spawn());
            snarl.insert_node(pos, node);
            ui.close_menu();
        }

        if ui.button("State Transition").clicked() {
            let node = GearboxNode::StateTransition(StateTransitionNode {
                from_state: "Idle".to_string(),
                to_state: "Active".to_string(),
            });
            snarl.insert_node(pos, node);
            ui.close_menu();
        }

        if ui.button("Event Trigger").clicked() {
            let node = GearboxNode::EventTrigger(EventTriggerNode {
                event_name: "OnComplete".to_string(),
            });
            snarl.insert_node(pos, node);
            ui.close_menu();
        }
    }
}