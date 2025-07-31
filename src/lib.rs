use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use egui_snarl::{
    ui::{PinInfo, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};

pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .insert_resource(NodeGraphState::default())
            .add_systems(EguiPrimaryContextPass, node_graph_ui_system);
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
            GearboxNode::Entity(EntityNode::new(Entity::PLACEHOLDER)),
        );
        let entity_node2 = snarl.insert_node(
            egui::pos2(300.0, 200.0),
            GearboxNode::Entity(EntityNode::new(Entity::PLACEHOLDER)),
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

fn node_graph_ui_system(mut contexts: EguiContexts, mut graph_state: ResMut<NodeGraphState>) {
    if let Ok(ctx) = contexts.ctx_mut() {
        egui::Window::new("Node Graph Editor")
            .default_size([800.0, 600.0])
            .show(ctx, |ui| {
                let NodeGraphState { snarl, viewer } = &mut *graph_state;
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
    ) -> PinInfo {
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
    ) -> PinInfo {
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
                ui.label(format!("Entity: {:?}", entity_node.entity));
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
            let node = GearboxNode::Entity(EntityNode::new(Entity::PLACEHOLDER));
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