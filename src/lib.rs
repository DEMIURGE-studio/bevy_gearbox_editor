use bevy::prelude::*;
use bevy_ecs::system::SystemState;
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use egui_snarl::{
    ui::{PinInfo, SnarlPin, SnarlStyle, SnarlViewer},
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
};
use std::collections::VecDeque;

pub mod entity_ui;
pub struct GearboxEditorPlugin;

impl Plugin for GearboxEditorPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EguiPlugin::default(), DefaultInspectorConfigPlugin))
            .insert_resource(NodeGraphState::default())
            .insert_resource(EntitySpawnQueue::default())
            .add_systems(Update, process_entity_spawn_requests)
            .add_systems(EguiPrimaryContextPass, node_graph_ui_system_world);
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
    pub selected_node: Option<NodeId>,
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
            viewer: GearboxViewer {
                pending_selection: None,
                current_selection: None,
            },
            selected_node: None,
        }
    }
}

fn node_graph_ui_system_world(world: &mut World) {
    // Extract the necessary resources using a SystemState
    let mut system_state: SystemState<(
        EguiContexts,
        ResMut<NodeGraphState>,
        ResMut<EntitySpawnQueue>,
    )> = SystemState::new(world);
    
    #[allow(unused_mut)]
    let (mut contexts, mut graph_state, mut spawn_queue) = system_state.get_mut(world);

    if let Ok(ctx) = contexts.ctx_mut() {
        // Check for nodes that need entity spawning
        let nodes_to_check: Vec<(NodeId, GearboxNode)> = graph_state.snarl.node_ids().map(|(id, node)| (id, node.clone())).collect();
        for (node_id, node) in nodes_to_check {
            if let GearboxNode::Entity(entity_node) = &node {
                if entity_node.needs_spawn {
                    spawn_queue.request_spawn(node_id);
                }
            }
        }
        
        let ctx = ctx.clone();
        system_state.apply(world);
        
        // Now we can create RestrictedWorldView and use our custom UI
        egui::Window::new("Node Graph Editor")
            .default_size([800.0, 600.0])
            .show(&ctx, |ui| {
                show_node_graph_with_custom_ui(world, ui);
            });
    }
}

fn show_node_graph_with_custom_ui(world: &mut World, ui: &mut egui::Ui) {
    egui::SidePanel::right("entity_inspector")
        .default_width(300.0)
        .show_inside(ui, |ui| {
            ui.heading("Entity Inspector");
            show_entity_inspector_panel(world, ui);
        });
    
    egui::CentralPanel::default().show_inside(ui, |ui| {
        let mut graph_state = world.resource_mut::<NodeGraphState>();
        let NodeGraphState { snarl, viewer, selected_node } = &mut *graph_state;
        
        // Sync the viewer's current selection with the resource
        if viewer.current_selection != *selected_node {
            println!("🔄 Syncing viewer selection: {:?} -> {:?}", viewer.current_selection, selected_node);
        }
        viewer.current_selection = *selected_node;
        
        snarl.show(
            viewer,
            &SnarlStyle::new(),
            "gearbox_graph",
            ui,
        );
        
        // Check if there's a pending selection and update the selected node
        if let Some(pending) = viewer.pending_selection.take() {
            println!("✅ Processing pending selection: {:?} -> updating selected_node", pending);
            *selected_node = Some(pending);
            println!("📋 Selected node is now: {:?}", selected_node);
        }
        
        // Also check snarl's built-in selection system (shift+drag selection)
        let snarl_selected_nodes = Snarl::<GearboxNode>::get_selected_nodes_at("gearbox_graph", ui.id(), ui.ctx());
        if !snarl_selected_nodes.is_empty() {
            // Take the first selected entity node
            for node_id in snarl_selected_nodes.iter() {
                if let Some(node) = snarl.get_node(*node_id) {
                    if let GearboxNode::Entity(_) = node {
                        if *selected_node != Some(*node_id) {
                            println!("🔍 Snarl selection detected: {:?} (shift+drag)", node_id);
                            *selected_node = Some(*node_id);
                        }
                        break; // Only select the first entity node
                    }
                    
                }
            }
        }
    });
}

fn show_entity_inspector_panel(world: &mut World, ui: &mut egui::Ui) {
    use bevy_inspector_egui::restricted_world_view::RestrictedWorldView;
    use bevy_ecs::world::CommandQueue;
    
    // Get the selected entity from the selected node
    let selected_entity = {
        let graph_state = world.resource::<NodeGraphState>();
        static mut LAST_SELECTION: Option<NodeId> = None;
        let should_debug = unsafe { LAST_SELECTION != graph_state.selected_node };
        
        if should_debug {
            println!("🔍 Inspector selection changed: {:?}", graph_state.selected_node);
            unsafe { LAST_SELECTION = graph_state.selected_node; }
        }
        
        match graph_state.selected_node {
            Some(node_id) => {
                if should_debug {
                    println!("🔍 Found selected node_id: {:?}, looking up node...", node_id);
                }
                if let Some(GearboxNode::Entity(entity_node)) = graph_state.snarl.get_node(node_id) {
                    if should_debug {
                        println!("🔍 Found entity node: {:?}, needs_spawn: {}", entity_node.entity, entity_node.needs_spawn);
                    }
                    if !entity_node.needs_spawn {
                        if should_debug {
                            println!("✅ Entity ready for inspection: {:?}", entity_node.entity);
                        }
                        Some(entity_node.entity)
                    } else {
                        if should_debug {
                            println!("⏳ Entity still spawning, can't inspect yet");
                        }
                        None
                    }
                } else {
                    if should_debug {
                        println!("❌ Selected node is not an entity node");
                    }
                    None
                }
            }
            None => {
                if should_debug {
                    println!("❌ No node selected");
                }
                None
            }
        }
    };
    
    match selected_entity {
        Some(entity) => {
            ui.heading(format!("Entity {:?}", entity));
            ui.separator();
            
            // Show the entity with your custom UI
            let type_registry = world.resource::<AppTypeRegistry>().0.clone();
            let type_registry = type_registry.read();
            let mut queue = CommandQueue::default();
            
            let mut world_view = RestrictedWorldView::new(world);
            crate::entity_ui::ui_for_entity_components(
                &mut world_view,
                Some(&mut queue),
                entity,
                ui,
                egui::Id::new(entity),
                &type_registry,
            );
            
            queue.apply(world);
        }
        None => {
            ui.vertical_centered(|ui| {
                ui.add_space(50.0);
                ui.label("Select an entity node to inspect its components");
                ui.add_space(20.0);
                ui.weak("Click on an entity node in the graph to see its details here.");
            });
        }
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

pub struct GearboxViewer {
    pub pending_selection: Option<NodeId>,
    pub current_selection: Option<NodeId>,
}

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
        // Check if this node is selected and add visual feedback
        let is_selected = self.current_selection == Some(node);
        
        if is_selected {
            println!("🎨 Node {:?} is selected, applying visual feedback", node);
            // Add a simple colored background for selected nodes
            let style = ui.style_mut();
            style.visuals.panel_fill = egui::Color32::from_rgba_premultiplied(100, 150, 255, 50);
        }
        
        // Debug available size (only for first few frames to avoid spam)
        static mut DEBUG_COUNTER: u32 = 0;
        unsafe {
            if DEBUG_COUNTER < 5 {
                let available_size = ui.available_size();
                println!("🔧 Node {:?} available size: {:?}", node, available_size);
                DEBUG_COUNTER += 1;
            }
        }
        
        match &mut snarl[node] {
            GearboxNode::Entity(entity_node) => {
                ui.text_edit_singleline(&mut entity_node.name);
                
                // Create a clickable label instead of just a regular label
                let entity_label = if entity_node.needs_spawn {
                    "Entity: (Spawning...)".to_string()
                } else {
                    format!("Entity: {:?}", entity_node.entity)
                };
                
                let label_response = ui.selectable_label(is_selected, entity_label);
                if label_response.clicked() {
                    println!("🖱️  Node {:?} entity label clicked! Setting pending selection.", node);
                    self.pending_selection = Some(node);
                }
                
                // Add a small indicator for entity nodes
                if is_selected {
                    ui.horizontal(|ui| {
                        ui.small("📋");
                        ui.small("Selected for inspection");
                    });
                }
            }
            GearboxNode::StateTransition(transition) => {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut transition.from_state);
                    ui.label("→");
                    ui.text_edit_singleline(&mut transition.to_state);
                });
                
                // Add a clickable area for non-entity nodes too
                let response = ui.allocate_response([100.0, 20.0].into(), egui::Sense::click());
                if response.clicked() {
                    println!("🖱️  Node {:?} (StateTransition) clicked! Setting pending selection.", node);
                    self.pending_selection = Some(node);
                }
            }
            GearboxNode::EventTrigger(event) => {
                ui.text_edit_singleline(&mut event.event_name);
                
                // Add a clickable area for non-entity nodes too
                let response = ui.allocate_response([100.0, 20.0].into(), egui::Sense::click());
                if response.clicked() {
                    println!("🖱️  Node {:?} (EventTrigger) clicked! Setting pending selection.", node);
                    self.pending_selection = Some(node);
                }
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