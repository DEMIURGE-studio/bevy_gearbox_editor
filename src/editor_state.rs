//! Editor state management, events, and shared types

use bevy::prelude::*;
use bevy_gearbox::InitialState;
use egui::Pos2;
use std::collections::HashMap;

use crate::components::NodeType;

/// State for managing text editing (renaming nodes)
#[derive(Default)]
pub struct TextEditingState {
    /// Entity currently being renamed
    pub editing_entity: Option<Entity>,
    /// Current text being edited
    pub current_text: String,
    /// Whether the text field should be focused
    pub should_focus: bool,
    /// Whether this is the first focus (to trigger select all)
    pub first_focus: bool,
}

impl TextEditingState {
    /// Start editing an entity's name
    pub fn start_editing(&mut self, entity: Entity, current_name: &str) {
        self.editing_entity = Some(entity);
        self.current_text = current_name.to_string();
        self.should_focus = true;
        self.first_focus = true;
    }
    
    /// Stop editing and return the final text if editing was active
    pub fn stop_editing(&mut self) -> Option<(Entity, String)> {
        if let Some(entity) = self.editing_entity {
            let text = self.current_text.clone();
            self.editing_entity = None;
            self.current_text.clear();
            self.should_focus = false;
            self.first_focus = false;
            Some((entity, text))
        } else {
            None
        }
    }
    
    /// Cancel editing without saving
    pub fn cancel_editing(&mut self) {
        self.editing_entity = None;
        self.current_text.clear();
        self.should_focus = false;
        self.first_focus = false;
    }
    
    /// Check if currently editing a specific entity
    pub fn is_editing(&self, entity: Entity) -> bool {
        self.editing_entity == Some(entity)
    }
}

/// State for managing transition creation workflow
#[derive(Default)]
pub struct TransitionCreationState {
    /// Source entity for the transition being created
    pub source_entity: Option<Entity>,
    /// Whether we're waiting for target selection
    pub awaiting_target_selection: bool,
    /// Position where the event type dropdown should appear
    pub dropdown_position: Option<Pos2>,
    /// Target entity that was selected
    pub target_entity: Option<Entity>,
    /// Whether the event type dropdown is open
    pub show_event_dropdown: bool,
    /// Available event types for TransitionListener
    pub available_event_types: Vec<String>,
}

impl TransitionConnection {
    /// Calculate connection points for the two-segment approach
    /// Returns (source_to_event_start, source_to_event_end, event_to_target_start, event_to_target_end)
    pub fn calculate_two_segment_points(&self) -> (egui::Pos2, egui::Pos2, egui::Pos2, egui::Pos2) {
        // Source to event node
        let source_to_event_start = closest_point_on_rect_edge(self.source_rect, self.event_node_position);
        let source_to_event_end = self.event_node_position;
        
        // Event node to target
        let event_to_target_start = self.event_node_position;
        let event_to_target_end = closest_point_on_rect_edge(self.target_rect, self.event_node_position);
        
        (source_to_event_start, source_to_event_end, event_to_target_start, event_to_target_end)
    }
    
    /// Get the event node position
    pub fn get_event_node_position(&self) -> egui::Pos2 {
        self.event_node_position
    }
    
    /// Get the pill-shaped event node rectangle for interaction
    pub fn get_event_node_rect(&self, text_size: egui::Vec2) -> egui::Rect {
        let padding = egui::Vec2::new(12.0, 6.0);
        let pill_size = text_size + padding * 2.0;
        egui::Rect::from_center_size(self.event_node_position, pill_size)
    }
    
    /// Update the event node position based on current source/target positions and stored offset
    pub fn update_event_node_position(&mut self) {
        let midpoint = egui::Pos2::new(
            (self.source_rect.center().x + self.target_rect.center().x) / 2.0,
            (self.source_rect.center().y + self.target_rect.center().y) / 2.0,
        );
        self.event_node_position = midpoint + self.event_node_offset;
    }
    
    /// Update the offset based on current event node position relative to source/target midpoint
    pub fn update_event_node_offset(&mut self) {
        let midpoint = egui::Pos2::new(
            (self.source_rect.center().x + self.target_rect.center().x) / 2.0,
            (self.source_rect.center().y + self.target_rect.center().y) / 2.0,
        );
        self.event_node_offset = self.event_node_position - midpoint;
    }
}

impl TransitionCreationState {
    /// Start creating a transition from the given source entity
    pub fn start_transition(&mut self, source: Entity) {
        self.source_entity = Some(source);
        self.awaiting_target_selection = true;
        self.target_entity = None;
        self.show_event_dropdown = false;
        self.dropdown_position = None;
    }
    
    /// Set the target entity and prepare for event type selection
    pub fn set_target(&mut self, target: Entity, dropdown_pos: Pos2) {
        self.target_entity = Some(target);
        self.awaiting_target_selection = false;
        self.show_event_dropdown = true;
        self.dropdown_position = Some(dropdown_pos);
    }
    
    /// Cancel the current transition creation
    pub fn cancel(&mut self) {
        *self = Default::default();
    }
    
    /// Complete the transition creation
    pub fn complete(&mut self) {
        *self = Default::default();
    }
    
    /// Check if we're currently creating a transition
    pub fn is_active(&self) -> bool {
        self.source_entity.is_some()
    }
}

/// Component that holds persistent state machine editor data
/// This lives on the root state machine entity and should be saved/loaded
#[derive(Component, Default)]
pub struct StateMachinePersistentData {
    /// Map of entity to its UI node representation (positions, sizes, etc.)
    pub nodes: HashMap<Entity, NodeType>,
    /// Visual transitions with custom layouts (draggable event nodes)
    pub visual_transitions: Vec<TransitionConnection>,
}

/// Component that holds transient state machine editor data
/// This is temporary UI state that should not be persisted
#[derive(Component, Default)]
pub struct StateMachineTransientData {
    /// Currently selected node for z-ordering
    pub selected_node: Option<Entity>,
    /// Transition creation state
    pub transition_creation: TransitionCreationState,
    /// Text editing state for renaming nodes
    pub text_editing: TextEditingState,
}

/// Resource that holds the editor's UI/window state
/// This manages which state machine is being edited in each window
#[derive(Resource, Default)]
pub struct EditorState {
    /// Currently selected state machine root entity being edited
    pub selected_machine: Option<Entity>,
    /// Entity for which a context menu is requested
    pub context_menu_entity: Option<Entity>,
    /// Position where the context menu should appear
    pub context_menu_position: Option<Pos2>,
    /// Entity currently being inspected
    pub inspected_entity: Option<Entity>,
    /// Component addition UI state
    pub component_addition: ComponentAdditionState,
}

/// State for the component addition UI
#[derive(Debug, Default)]
pub struct ComponentAdditionState {
    /// Search text for filtering components
    pub search_text: String,
    /// Whether the dropdown is open
    pub dropdown_open: bool,
    /// Hierarchical component organization (cached)
    pub component_hierarchy: Option<crate::entity_inspector::ComponentHierarchy>,
    /// Expanded state for each namespace
    pub expanded_namespaces: std::collections::HashSet<String>,
}

impl ComponentAdditionState {
    /// Update the component hierarchy
    pub fn update_hierarchy(&mut self, hierarchy: crate::entity_inspector::ComponentHierarchy) {
        self.component_hierarchy = Some(hierarchy);
    }

    /// Toggle expansion state for a namespace path
    pub fn toggle_namespace(&mut self, namespace_path: &str) {
        if self.expanded_namespaces.contains(namespace_path) {
            self.expanded_namespaces.remove(namespace_path);
        } else {
            self.expanded_namespaces.insert(namespace_path.to_string());
        }
    }

    /// Check if a namespace is expanded
    pub fn is_namespace_expanded(&self, namespace_path: &str) -> bool {
        self.expanded_namespaces.contains(namespace_path)
    }
}

impl EditorState {
    /// Get the currently selected state machine entity
    pub fn current_machine(&self) -> Option<Entity> {
        self.selected_machine
    }
    
    /// Set the currently selected state machine
    pub fn set_current_machine(&mut self, entity: Option<Entity>) {
        self.selected_machine = entity;
    }
}

/// Component marking an entity as an editor window
#[derive(Component)]
pub struct EditorWindow;

/// Event fired when a context menu is requested for a node
#[derive(Event)]
pub struct NodeContextMenuRequested {
    pub entity: Entity,
    pub position: Pos2,
}

/// Available actions that can be performed on nodes
#[derive(Debug, Clone)]
pub enum NodeAction {
    Inspect,
    AddChild,
    Rename,
    SetAsInitialState,
}

/// Event fired when a node action is triggered
#[derive(Event)]
pub struct NodeActionTriggered {
    pub entity: Entity,
    pub action: NodeAction,
}

/// Event fired when a node is dragged
#[derive(Event, Debug)]
pub struct NodeDragged {
    pub entity: Entity,
    pub drag_delta: egui::Vec2,
}

/// Event fired when a transition creation is requested (+ button clicked)
#[derive(Event)]
pub struct TransitionCreationRequested {
    pub source_entity: Entity,
}

/// Event fired when a transition should be created with the selected event type
#[derive(Event)]
pub struct CreateTransition {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub event_type: String,
}

/// Event fired when a state machine should be saved
#[derive(Event)]
pub struct SaveStateMachine {
    pub entity: Entity,
}

/// Item to be rendered in the node editor, with z-order information
pub struct RenderItem {
    pub entity: Entity,
    pub z_order: i32,
}

/// Visual representation of a transition connection
#[derive(Debug, Clone)]
pub struct TransitionConnection {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub event_type: String,
    pub source_rect: egui::Rect,
    pub target_rect: egui::Rect,
    pub event_node_position: egui::Pos2,
    pub is_dragging_event_node: bool,
    /// Offset from the midpoint between source and target nodes
    pub event_node_offset: egui::Vec2,
}

/// Get a human-readable name for an entity
pub fn get_entity_name(entity: Entity, all_entities: &Query<(Entity, Option<&Name>, Option<&InitialState>)>) -> String {
    if let Ok((_, name_opt, _)) = all_entities.get(entity) {
        if let Some(name) = name_opt {
            name.as_str().to_string()
        } else {
            format!("Entity {:?}", entity)
        }
    } else {
        format!("Unknown Entity {:?}", entity)
    }
}

/// Get a human-readable name for an entity using world access
pub fn get_entity_name_from_world(entity: Entity, world: &mut World) -> String {
    let mut query = world.query::<(Entity, Option<&Name>)>();
    if let Ok((_, name_opt)) = query.get(world, entity) {
        if let Some(name) = name_opt {
            name.as_str().to_string()
        } else {
            format!("Entity {:?}", entity)
        }
    } else {
        format!("Unknown Entity {:?}", entity)
    }
}

/// Determine if an entity should get a selection boost for z-ordering
pub fn should_get_selection_boost(
    entity: Entity,
    selected_node: Option<Entity>,
    child_of_query: &Query<&ChildOf>,
) -> bool {
    if let Some(selected) = selected_node {
        if entity == selected {
            return true;
        }
        
        // Check if this entity is an ancestor of the selected node
        let mut current = selected;
        while let Ok(child_of) = child_of_query.get(current) {
            if child_of.0 == entity {
                return true;
            }
            current = child_of.0;
        }
    }
    false
}

/// Find the closest point on a rectangle's edge to a given point
pub fn closest_point_on_rect_edge(rect: egui::Rect, point: egui::Pos2) -> egui::Pos2 {
    let center = rect.center();
    let direction = point - center;
    
    // Calculate intersection with rectangle edges
    let t_left = if direction.x != 0.0 { (rect.min.x - center.x) / direction.x } else { f32::INFINITY };
    let t_right = if direction.x != 0.0 { (rect.max.x - center.x) / direction.x } else { f32::INFINITY };
    let t_top = if direction.y != 0.0 { (rect.min.y - center.y) / direction.y } else { f32::INFINITY };
    let t_bottom = if direction.y != 0.0 { (rect.max.y - center.y) / direction.y } else { f32::INFINITY };
    
    // Find the smallest positive t value
    let mut min_t = f32::INFINITY;
    if t_left > 0.0 { min_t = min_t.min(t_left); }
    if t_right > 0.0 { min_t = min_t.min(t_right); }
    if t_top > 0.0 { min_t = min_t.min(t_top); }
    if t_bottom > 0.0 { min_t = min_t.min(t_bottom); }
    
    if min_t == f32::INFINITY {
        // Fallback to center if no intersection found
        center
    } else {
        center + direction * min_t
    }
}

/// Draw an arrow from start to end point
pub fn draw_arrow(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: egui::Color32) {
    let stroke = egui::Stroke::new(2.0, color);
    
    // Draw the main line
    painter.line_segment([start, end], stroke);
    
    // Calculate arrow head
    let direction = (end - start).normalized();
    let arrow_length = 8.0;
    let arrow_angle = std::f32::consts::PI / 6.0; // 30 degrees
    
    let arrow_point1 = end - direction * arrow_length;
    let perpendicular = egui::Vec2::new(-direction.y, direction.x);
    
    let arrow_head1 = arrow_point1 + perpendicular * arrow_length * arrow_angle.sin();
    let arrow_head2 = arrow_point1 - perpendicular * arrow_length * arrow_angle.sin();
    
    // Draw arrow head
    painter.line_segment([end, arrow_head1], stroke);
    painter.line_segment([end, arrow_head2], stroke);
}

/// Draw a pill-shaped label with text and return interaction response
pub fn draw_interactive_pill_label(
    ui: &mut egui::Ui, 
    position: egui::Pos2, 
    text: &str, 
    font_id: egui::FontId,
    is_being_dragged: bool
) -> egui::Response {
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, egui::Color32::WHITE));
    let text_size = galley.size();
    
    let padding = egui::Vec2::new(12.0, 6.0);
    let pill_size = text_size + padding * 2.0;
    let pill_rect = egui::Rect::from_center_size(position, pill_size);
    
    // Allocate the rectangle for interaction
    let response = ui.allocate_rect(pill_rect, egui::Sense::click_and_drag());
    
    let painter = ui.painter();
    
    // Choose colors based on interaction state
    let (bg_color, border_color) = if response.hovered() || is_being_dragged {
        (egui::Color32::from_rgb(65, 65, 85), egui::Color32::from_rgb(100, 100, 110))
    } else {
        (egui::Color32::from_rgb(45, 45, 55), egui::Color32::from_rgb(80, 80, 90))
    };
    
    // Draw pill background (same as node background)
    painter.rect_filled(
        pill_rect,
        10.0, // Fully rounded ends
        bg_color,
    );
    
    // Draw pill border (same as node border)
    painter.rect_stroke(
        pill_rect,
        egui::CornerRadius::same((pill_size.y / 2.0) as u8),
        egui::Stroke::new(1.0, border_color),
        egui::StrokeKind::Outside,
    );
    
    // Draw text
    let text_pos = position - text_size / 2.0;
    painter.galley(text_pos, galley, egui::Color32::WHITE);
    
    response
}