use bevy::prelude::*;
use bevy_egui::egui::{self, Widget, Response, Ui, Sense, Color32, Pos2, Vec2 as EguiVec2};
use bevy_inspector_egui::bevy_inspector;

/// Response data from node widgets containing interaction and position info
pub struct NodeWidgetResponse {
    pub response: Response,
    pub input_pin_pos: Option<Pos2>,
    pub output_pin_positions: Vec<((Entity, usize), Pos2)>,
}

impl NodeWidgetResponse {
    pub fn new(response: Response) -> Self {
        Self {
            response,
            input_pin_pos: None,
            output_pin_positions: Vec::new(),
        }
    }
}

/// Widget for displaying a single transition with name and output pin
pub struct TransitionWidget {
    pub transition_name: String,
    pub pin_index: usize,
    pub entity: Entity,
}

impl TransitionWidget {
    pub fn new(transition_name: String, pin_index: usize, entity: Entity) -> Self {
        Self {
            transition_name,
            pin_index,
            entity,
        }
    }
    
    /// Show this widget and return pin position information
    pub fn show(self, ui: &mut Ui) -> (Response, Option<Pos2>) {
        let mut pin_pos = None;
        
        let response = ui.allocate_ui_with_layout(
            EguiVec2::new(ui.available_width(), 25.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // Transition name on the left
                ui.label(&self.transition_name);
                
                // Output pin on the right - use right-to-left layout to auto-align
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pin_radius = 6.0;
                    let response = ui.allocate_response(
                        EguiVec2::new(pin_radius * 2.0, pin_radius * 2.0), 
                        Sense::hover()
                    );
                    let pin_center = response.rect.center();
                    
                    ui.painter().circle_filled(
                        pin_center,
                        pin_radius,
                        Color32::from_rgb(255, 100, 100), // Red for output
                    );
                    
                    pin_pos = Some(pin_center);
                    response
                })
            }
        ).response;
        
        (response, pin_pos)
    }
}

impl Widget for TransitionWidget {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui).0
    }
}

/// Widget for the node header with input pin and name
pub struct NodeHeader {
    pub entity: Entity,
    pub display_name: String,
}

impl NodeHeader {
    pub fn new(entity: Entity, display_name: String) -> Self {
        Self {
            entity,
            display_name,
        }
    }
    
    /// Show this widget and return pin position
    pub fn show(self, ui: &mut Ui) -> (Response, Option<Pos2>) {
        let mut input_pin_pos = None;
        
        let response = ui.allocate_ui_with_layout(
            EguiVec2::new(ui.available_width(), 30.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                // Input pin on the left
                let pin_radius = 6.0;
                let pin_pos = ui.cursor().min + EguiVec2::new(6.0, 15.0); // Center vertically
                ui.painter().circle_filled(
                    pin_pos,
                    pin_radius,
                    Color32::from_rgb(100, 150, 255), // Blue for input
                );
                input_pin_pos = Some(pin_pos);
                
                ui.allocate_space(EguiVec2::new(12.0, 0.0)); // Space for pin
                
                // Entity name and ID - centered content
                ui.vertical(|ui| {
                    ui.strong(&self.display_name);
                    ui.small(format!("Entity: {:?}", self.entity));
                });
            }
        ).response;
        
        (response, input_pin_pos)
    }
}

impl Widget for NodeHeader {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui).0
    }
}

/// Widget for the transition section that contains all transitions
pub struct TransitionSection {
    pub entity: Entity,
    pub transitions: Vec<(String, usize)>, // (name, pin_index)
}

impl TransitionSection {
    pub fn new(entity: Entity, transitions: Vec<(String, usize)>) -> Self {
        Self {
            entity,
            transitions,
        }
    }
}

impl Widget for TransitionSection {
    fn ui(self, ui: &mut Ui) -> Response {
        if self.transitions.is_empty() {
            // Return a zero-size response if no transitions
            return ui.allocate_response(EguiVec2::ZERO, Sense::hover());
        }
        
        // Calculate height: label + transitions
        let section_height = 20.0 + (self.transitions.len() as f32 * 25.0);
        
        ui.allocate_ui_with_layout(
            EguiVec2::new(ui.available_width(), section_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // Section label
                ui.label("Transitions:");
                
                // Add each transition widget
                for (transition_name, pin_index) in self.transitions {
                    ui.add(TransitionWidget::new(transition_name, pin_index, self.entity));
                }
            }
        ).response
    }
}

/// Widget for entity components (inspector area when expanded)
pub struct EntityComponents {
    pub entity: Entity,
    pub expanded: bool,
}

impl EntityComponents {
    pub fn new(entity: Entity, expanded: bool) -> Self {
        Self {
            entity,
            expanded,
        }
    }
}

impl Widget for EntityComponents {
    fn ui(self, ui: &mut Ui) -> Response {
        if !self.expanded {
            // Return zero-size response if not expanded
            return ui.allocate_response(EguiVec2::ZERO, Sense::hover());
        }
        
        ui.vertical(|ui| {
            ui.separator();
            
            // Inspector UI - let it size naturally
            let inspector_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // TODO: Need world access here - will need to refactor
                // bevy_inspector::ui_for_entity(world, self.entity, ui);
            }));
            
            if inspector_result.is_err() {
                ui.label("⚠️ Inspector UI unavailable for this entity");
            }
            
            ui.separator();
            
            // Action buttons
            let _ = ui.button("+ Add Component");
            let _ = ui.button("+ Add Transition Listener");
        }).response
    }
}

/// Root container widget that composes all node sections
pub struct NodeBody {
    pub entity: Entity,
    pub display_name: String,
    pub transitions: Vec<(String, usize)>,
}

impl NodeBody {
    pub fn new(entity: Entity, display_name: String, transitions: Vec<(String, usize)>) -> Self {
        Self {
            entity,
            display_name,
            transitions,
        }
    }
    
    /// Show this widget and return comprehensive interaction data
    pub fn show(self, ui: &mut Ui, _world: &mut World) -> NodeWidgetResponse {
        let mut input_pin_pos = None;
        let mut output_pin_positions = Vec::new();
        
        // Simple fixed-width compact nodes
        let target_width: f32 = 200.0; // Consistent node width
        
        let response = ui.allocate_ui_with_layout(
            egui::Vec2::new(target_width, 0.0), // Fixed width, auto height
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // 1. Header
                let (header_response, header_input_pin_pos) = NodeHeader::new(
                    self.entity, 
                    self.display_name
                ).show(ui);
                
                input_pin_pos = header_input_pin_pos;
                
                // 2. Transition section (if transitions exist)
                if !self.transitions.is_empty() {
                    ui.label("Transitions:");
                    for (transition_name, pin_index) in self.transitions {
                        let (_, pin_pos) = TransitionWidget::new(transition_name, pin_index, self.entity).show(ui);
                        if let Some(pos) = pin_pos {
                            output_pin_positions.push(((self.entity, pin_index), pos));
                        }
                    }
                }
                
                header_response // Return header response for drag/click handling
            }
        );
        
        let mut widget_response = NodeWidgetResponse::new(response.inner);
        widget_response.input_pin_pos = input_pin_pos;
        widget_response.output_pin_positions = output_pin_positions;
        widget_response
    }
}

impl Widget for NodeBody {
    fn ui(self, ui: &mut Ui) -> Response {
        // For the Widget trait, we can't access world, so return a simple response
        ui.vertical(|ui| {
            ui.add(NodeHeader::new(self.entity, self.display_name));
            ui.add(TransitionSection::new(self.entity, self.transitions));
        }).response
    }
}

/// Separate inspector panel widget for showing details of the selected entity
pub struct EntityInspectorPanel {
    pub selected_entity: Option<Entity>,
}

impl EntityInspectorPanel {
    pub fn new(selected_entity: Option<Entity>) -> Self {
        Self { selected_entity }
    }

    /// Show the inspector panel with entity details
    pub fn show(self, ui: &mut egui::Ui, world: &mut World, dialog_state: &mut crate::resources::ComponentDialogState, transition_state: &mut crate::resources::TransitionCreationState) -> Response {
        ui.vertical(|ui| {
            ui.heading("Entity Inspector");
            ui.separator();
            
            match self.selected_entity {
                Some(entity) => {
                    ui.label(format!("Selected Entity: {:?}", entity));
                    ui.separator();
                    
                    // Show entity components using bevy-inspector-egui with proper scrolling
                    let available_height = ui.available_height() - 100.0; // Reserve space for buttons
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .max_height(available_height)
                        .show(ui, |ui| {
                            // Use full available width for inspector content
                            ui.set_max_width(ui.available_width());
                            
                            let inspector_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                bevy_inspector::ui_for_entity(world, entity, ui);
                            }));
                            
                            if inspector_result.is_err() {
                                ui.label("⚠️ Inspector UI unavailable for this entity");
                            }
                        });
                    
                    ui.separator();
                    
                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("+ Add Component").clicked() {
                            // Trigger the component dialog
                            dialog_state.open_for_entity = Some(entity);
                            dialog_state.selected_component = None;
                        }
                        if ui.button("+ Add Transition Listener").clicked() {
                            // Trigger the transition dialog  
                            transition_state.source_entity = Some(entity);
                            transition_state.selected_event_type = None;
                            transition_state.selecting_target = false;
                        }
                    });
                },
                None => {
                    ui.label("No entity selected");
                    ui.label("Click on a node to inspect its components");
                }
            }
        }).response
    }
}

impl Widget for EntityInspectorPanel {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.vertical(|ui| {
            ui.heading("Entity Inspector");
            ui.separator();
            
            match self.selected_entity {
                Some(entity) => {
                    ui.label(format!("Selected Entity: {:?}", entity));
                    ui.label("(Inspector requires world access - use show() method)");
                },
                None => {
                    ui.label("No entity selected");
                    ui.label("Click on a node to inspect its components");
                }
            }
        }).response
    }
}