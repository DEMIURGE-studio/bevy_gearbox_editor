use bevy::prelude::*;
use bevy_egui::egui::{self, Widget, Response, Ui, Sense, Color32, Pos2, Vec2 as EguiVec2};
use bevy_inspector_egui::bevy_inspector;

/// Response data from node widgets containing interaction and position info
pub struct NodeWidgetResponse {
    pub response: Response,
    pub expansion_changed: Option<bool>,
    pub input_pin_pos: Option<Pos2>,
    pub output_pin_positions: Vec<((Entity, usize), Pos2)>,
}

impl NodeWidgetResponse {
    pub fn new(response: Response) -> Self {
        Self {
            response,
            expansion_changed: None,
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

/// Widget for the node header with input pin, name, and expand button
pub struct NodeHeader {
    pub entity: Entity,
    pub display_name: String,
    pub expanded: bool,
}

impl NodeHeader {
    pub fn new(entity: Entity, display_name: String, expanded: bool) -> Self {
        Self {
            entity,
            display_name,
            expanded,
        }
    }
    
    /// Show this widget and return expansion state and pin position
    pub fn show(self, ui: &mut Ui) -> (Response, Option<bool>, Option<Pos2>) {
        let mut expansion_changed = None;
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
                
                // Entity name and ID
                ui.vertical(|ui| {
                    ui.strong(&self.display_name);
                    ui.small(format!("Entity: {:?}", self.entity));
                });
                
                // Expand/collapse button on the right - use right-to-left layout to auto-align
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let expand_text = if self.expanded { "▼" } else { "▶" };
                    let button_response = ui.small_button(expand_text);
                    if button_response.clicked() {
                        expansion_changed = Some(!self.expanded);
                        println!("🔽 Widget: Toggled expansion for {:?}: {} -> {}", self.entity, self.expanded, !self.expanded);
                    }
                    button_response
                })
            }
        ).response;
        
        (response, expansion_changed, input_pin_pos)
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
    pub expanded: bool,
    pub transitions: Vec<(String, usize)>,
}

impl NodeBody {
    pub fn new(entity: Entity, display_name: String, expanded: bool, transitions: Vec<(String, usize)>) -> Self {
        Self {
            entity,
            display_name,
            expanded,
            transitions,
        }
    }
    
    /// Show this widget and return comprehensive interaction data
    pub fn show(self, ui: &mut Ui, world: &mut World) -> NodeWidgetResponse {
        let mut expansion_changed = None;
        let mut input_pin_pos = None;
        let mut output_pin_positions = Vec::new();
        
        // Strategy: Use allocate_ui to control exact width, let height grow naturally
        // First determine the minimum width needed
        let mut target_width: f32 = 200.0; // Minimum node width
        
        // If expanded, pre-measure inspector content to get required width
        if self.expanded {
            // Quick invisible measurement of inspector content only
            let inspector_width = ui.allocate_ui_with_layout(
                egui::Vec2::new(ui.available_width(), 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_invisible();
                    ui.separator();
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        bevy_inspector::ui_for_entity(world, self.entity, ui);
                    }));
                    ui.separator();
                    let _ = ui.button("+ Add Component");
                    let _ = ui.button("+ Add Transition Listener");
                }
            ).response.rect.width();
            
            target_width = target_width.max(inspector_width);
        }
        
        // Now render everything with the determined width
        let response = ui.allocate_ui_with_layout(
            egui::Vec2::new(target_width, 0.0), // Fixed width, auto height
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // 1. Header
                let (header_response, header_expansion_changed, header_input_pin_pos) = NodeHeader::new(
                    self.entity, 
                    self.display_name, 
                    self.expanded
                ).show(ui);
                
                expansion_changed = header_expansion_changed;
                input_pin_pos = header_input_pin_pos;
                
                // 2. Transition section
                if !self.transitions.is_empty() {
                    ui.label("Transitions:");
                    for (transition_name, pin_index) in self.transitions {
                        let (_, pin_pos) = TransitionWidget::new(transition_name, pin_index, self.entity).show(ui);
                        if let Some(pos) = pin_pos {
                            output_pin_positions.push(((self.entity, pin_index), pos));
                        }
                    }
                }
                
                // 3. Entity components (if expanded)
                if self.expanded {
                    ui.separator();
                    
                    // Inspector UI
                    let inspector_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        bevy_inspector::ui_for_entity(world, self.entity, ui);
                    }));
                    
                    if inspector_result.is_err() {
                        ui.label("⚠️ Inspector UI unavailable for this entity");
                    }
                    
                    ui.separator();
                    
                    // Action buttons
                    let _ = ui.button("+ Add Component");
                    let _ = ui.button("+ Add Transition Listener");
                }
                
                header_response // Return header response for drag/click handling
            }
        );
        
        let mut widget_response = NodeWidgetResponse::new(response.inner);
        widget_response.expansion_changed = expansion_changed;
        widget_response.input_pin_pos = input_pin_pos;
        widget_response.output_pin_positions = output_pin_positions;
        widget_response
    }
}

impl Widget for NodeBody {
    fn ui(self, ui: &mut Ui) -> Response {
        // For the Widget trait, we can't access world, so return a simple response
        ui.vertical(|ui| {
            ui.add(NodeHeader::new(self.entity, self.display_name, self.expanded));
            ui.add(TransitionSection::new(self.entity, self.transitions));
            ui.add(EntityComponents::new(self.entity, self.expanded));
        }).response
    }
}