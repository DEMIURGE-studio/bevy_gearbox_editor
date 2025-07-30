#![allow(clippy::use_self)]

use eframe::{App, CreationContext};
use egui::{Color32, Id, Ui};
use egui_snarl::{
    ui::{
        get_selected_nodes, AnyPins, NodeLayout, PinInfo, PinPlacement, SnarlPin, SnarlStyle, SnarlViewer, SnarlWidget
    }, InPin, InPinId, NodeId, OutPin, Snarl
};
use serde::{Deserialize, Serialize};

use bevy::prelude::Entity;

const STRING_COLOR: Color32 = Color32::from_rgb(0x00, 0xb0, 0x00);
const NUMBER_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0x00);
const IMAGE_COLOR: Color32 = Color32::from_rgb(0xb0, 0x00, 0xb0);
const UNTYPED_COLOR: Color32 = Color32::from_rgb(0xb0, 0xb0, 0xb0);

/// Represents a node for a Bevy entity in the graph.
///
/// This struct holds a serializable representation of an entity's structure,
/// which is derived using Bevy's reflection capabilities.
#[derive(Clone, Serialize, Deserialize)]
pub struct EntityNode {
    /// The entity this node represents.
    /// For this to be serializable, Bevy must be compiled with the "serde" feature.
    pub entity: Entity,

    /// A display name for the node, possibly from the entity's `Name` component.
    pub name: String,

    /// A collection of the entity's components that are reflectable.
    pub components: Vec<ComponentInfo>,
}

/// Holds reflection data for a single component.
#[derive(Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    /// Display name of the component.
    pub name: String,

    /// The full type name of the component, used for reflection lookup.
    pub type_name: String,

    /// The fields of this component, which will be represented as pins on the node.
    pub fields: Vec<FieldInfo>,
}

/// Holds reflection data for a single field within a component.
#[derive(Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    /// The name of the field.
    pub name: String,

    /// The index of this field within the component struct.
    pub field_index: usize,

    /// The type name of the field, for UI purposes (e.g., pin color, connection validation).
    pub field_type_name: String,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum DemoNode {
    /// An entity from the Bevy world, with its reflected components.
    Entity(EntityNode),
}

impl DemoNode {
    const fn name(&self) -> &str {
        match self {
            DemoNode::Entity(entity_node) => &entity_node.name.as_str(),
        }
    }
}

struct DemoViewer;

impl SnarlViewer<DemoNode> for DemoViewer {
    #[inline]
    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<DemoNode>) {
        // When a user connects an output pin (an entity field) of `Node A` to an
        // input pin of `Node B`, it signifies that we want to set the entity field
        // on `Entity A` to hold the ID of `Entity B`.

        // In a real implementation, this is where you would send a command to
        // the Bevy `World` to modify the component. Since `SnarlViewer` does not
        // have access to the `World`, we can only record the connection in the
        // snarl graph for now. The change would then need to be applied to the
        // `World` elsewhere.

        // An input pin can only have one connection in this model.
        // So, we disconnect any existing connections to the target pin.
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }

        snarl.connect(from.id, to.id);
    }

    fn title(&mut self, node: &DemoNode) -> String {
        node.name().to_owned()
    }

    fn inputs(&mut self, node: &DemoNode) -> usize {
        // Each entity node has a single input pin, representing a connection
        // *to* this entity.
        match node {
            DemoNode::Entity(_) => 1,
        }
    }

    fn outputs(&mut self, node: &DemoNode) -> usize {
        // The output pins of an entity node represent its component fields that
        // hold an `Entity`. These are the "pointers" to other entities.
        match node {
            DemoNode::Entity(entity_node) => entity_node
                .components
                .iter()
                .flat_map(|component| &component.fields)
                .filter(|field| {
                    // We identify entity fields by their type name. This assumes
                    // that the `field_type_name` is the full, unambiguous type
                    // path. For Bevy's `Entity`, this would be
                    // `bevy_ecs::entity::Entity`.
                    field.field_type_name == "bevy_ecs::entity::Entity"
                })
                .count(),
        }
    }

    fn show_input(&mut self, _pin: &InPin, ui: &mut Ui, _snarl: &mut Snarl<DemoNode>) -> impl SnarlPin + 'static {
        // We don't need to show anything for the input pin, as it's just a
        // target for connections.
        ui.label("");
        PinInfo::circle().with_fill(UNTYPED_COLOR)
    }

    fn show_output(&mut self, pin: &OutPin, ui: &mut Ui, snarl: &mut Snarl<DemoNode>) -> impl SnarlPin + 'static {
        // The output pins correspond to the entity's component fields that
        // hold an `Entity`. We'll display the name of the field as the label for
        // the pin.
        let node = &snarl[pin.id.node];
        match node {
            DemoNode::Entity(entity_node) => {
                let mut field_iter = entity_node
                    .components
                    .iter()
                    .flat_map(|component| &component.fields)
                    .filter(|field| field.field_type_name == "bevy_ecs::entity::Entity");

                if let Some(field) = field_iter.nth(pin.id.output) {
                    ui.label(&field.name);
                }

                PinInfo::circle().with_fill(UNTYPED_COLOR)
            }
        }
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<DemoNode>) -> bool {
        true
    }

    fn show_graph_menu(&mut self, pos: egui::Pos2, ui: &mut Ui, snarl: &mut Snarl<DemoNode>) {
        ui.label("Add node");
        if ui.button("Add Entity").clicked() {
            let entity_node = EntityNode {
                // In a real application, you would get the entity from the
                // Bevy `World`. Here, we're just creating a dummy entity.
                entity: Entity::from_raw(snarl.nodes().count() as u32),
                name: format!("Entity {}", snarl.nodes().count()),
                components: Vec::new(),
            };
            snarl.insert_node(pos, DemoNode::Entity(entity_node));
            ui.close();
        }
    }

    fn has_dropped_wire_menu(&mut self, _src_pins: AnyPins, _snarl: &mut Snarl<DemoNode>) -> bool {
        true
    }

    fn show_dropped_wire_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut Ui,
        src_pins: AnyPins,
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Add node");
        if ui.button("Add Entity").clicked() {
            let entity_node = EntityNode {
                entity: Entity::from_raw(snarl.nodes().count() as u32),
                name: format!("Entity {}", snarl.nodes().count()),
                components: Vec::new(),
            };
            let new_node_id = snarl.insert_node(pos, DemoNode::Entity(entity_node));

            match src_pins {
                AnyPins::Out(out_pin) => {
                    // There is only one input pin on an entity node.
                    let in_pin = InPinId {
                        node: new_node_id,
                        input: 0,
                    };
                    snarl.connect(out_pin[0], in_pin);
                }
                AnyPins::In(_) => {
                    // For this simple case, we don't handle dragging from an
                    // input pin.
                }
            }
            ui.close();
        }
    }

    fn has_node_menu(&mut self, _node: &DemoNode) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<DemoNode>,
    ) {
        ui.label("Node menu");
        if ui.button("Remove").clicked() {
            snarl.remove_node(node);
            ui.close();
        }
    }

    fn has_on_hover_popup(&mut self, _: &DemoNode) -> bool {
        true
    }

    fn show_on_hover_popup(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut Ui,
        snarl: &mut Snarl<DemoNode>,
    ) {
        let node = &snarl[node];
        if let DemoNode::Entity(entity_node) = node {
            ui.label(format!("Entity ID: {:?}", entity_node.entity));
            ui.label(format!(
                "{} components",
                entity_node.components.len()
            ));
        }
    }

    fn header_frame(
        &mut self,
        frame: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<DemoNode>,
    ) -> egui::Frame {
        let node = &snarl[node];
        match node {
            DemoNode::Entity(_) => frame.fill(Color32::from_rgb(50, 50, 80)),
        }
    }
}

pub struct DemoApp {
    snarl: Snarl<DemoNode>,
    style: SnarlStyle,
}

const fn default_style() -> SnarlStyle {
    SnarlStyle {
        node_layout: Some(NodeLayout::coil()),
        pin_placement: Some(PinPlacement::Edge),
        pin_size: Some(7.0),
        node_frame: Some(egui::Frame {
            inner_margin: egui::Margin::same(8),
            outer_margin: egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 4,
            },
            corner_radius: egui::CornerRadius::same(8),
            fill: egui::Color32::from_gray(30),
            stroke: egui::Stroke::NONE,
            shadow: egui::Shadow::NONE,
        }),
        bg_frame: Some(egui::Frame {
            inner_margin: egui::Margin::ZERO,
            outer_margin: egui::Margin::same(2),
            corner_radius: egui::CornerRadius::ZERO,
            fill: egui::Color32::from_gray(40),
            stroke: egui::Stroke::NONE,
            shadow: egui::Shadow::NONE,
        }),
        ..SnarlStyle::new()
    }
}

impl DemoApp {
    pub fn new(cx: &CreationContext) -> Self {
        egui_extras::install_image_loaders(&cx.egui_ctx);

        cx.egui_ctx.style_mut(|style| style.animation_time *= 10.0);

        let snarl = cx.storage.map_or_else(Snarl::new, |storage| {
            storage
                .get_string("snarl")
                .and_then(|snarl| serde_json::from_str(&snarl).ok())
                .unwrap_or_default()
        });
        // let snarl = Snarl::new();

        let style = cx.storage.map_or_else(default_style, |storage| {
            storage
                .get_string("style")
                .and_then(|style| serde_json::from_str(&style).ok())
                .unwrap_or_else(default_style)
        });
        // let style = SnarlStyle::new();

        DemoApp { snarl, style }
    }
}

impl App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_switch(ui);

                if ui.button("Clear All").clicked() {
                    self.snarl = Snarl::default();
                }
            });
        });

        egui::SidePanel::left("style").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui_probe::Probe::new(&mut self.style).show(ui);
            });
        });

        egui::SidePanel::right("selected-list").show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.strong("Selected nodes");

                let selected = get_selected_nodes(Id::new("snarl-demo"), ui.ctx());

                let mut selected = selected
                    .into_iter()
                    .map(|id| (id, &self.snarl[id]))
                    .collect::<Vec<_>>();

                selected.sort_by_key(|(id, _)| *id);

                let mut remove = None;

                for (id, node) in selected {
                    ui.horizontal(|ui| {
                        ui.label(format!("{id:?}"));
                        ui.label(node.name());
                        ui.add_space(ui.spacing().item_spacing.x);
                        if ui.button("Remove").clicked() {
                            remove = Some(id);
                        }
                    });
                }

                if let Some(id) = remove {
                    self.snarl.remove_node(id);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            SnarlWidget::new()
                .id(Id::new("snarl-demo"))
                .style(self.style)
                .show(&mut self.snarl, &mut DemoViewer, ui);
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let snarl = serde_json::to_string(&self.snarl).unwrap();
        storage.set_string("snarl", snarl);

        let style = serde_json::to_string(&self.style).unwrap();
        storage.set_string("style", style);
    }
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "egui-snarl demo",
        native_options,
        Box::new(|cx| Ok(Box::new(DemoApp::new(cx)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn get_canvas_element() -> Option<web_sys::HtmlCanvasElement> {
    use eframe::wasm_bindgen::JsCast;

    let document = web_sys::window()?.document()?;
    let canvas = document.get_element_by_id("egui_snarl_demo")?;
    canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok()
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    let canvas = get_canvas_element().expect("Failed to find canvas with id 'egui_snarl_demo'");

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cx| Ok(Box::new(DemoApp::new(cx)))),
            )
            .await
            .expect("failed to start eframe");
    });
}