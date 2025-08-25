//! Editor state management, events, and shared types

use bevy::prelude::*;
use bevy_gearbox::InitialState;
use bevy_gearbox::active::Active;
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
    /// Available event types for EventEdge
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
    /// Active transition pulses for visual feedback
    pub transition_pulses: Vec<TransitionPulse>,
    /// Active node pulses for visual feedback (recently entered states)
    pub node_pulses: Vec<NodePulse>,
    /// Mapping from editor state entity -> NodeKind machine root entity (editor-internal)
    pub node_kind_roots: std::collections::HashMap<Entity, Entity>,
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
    /// Transition for which a context menu is requested
    pub transition_context_menu: Option<(Entity, Entity, String, Entity)>, // (source, target, event_type, edge)
    /// Position where the transition context menu should appear
    pub transition_context_menu_position: Option<Pos2>,
    /// Entity currently being inspected
    pub inspected_entity: Option<Entity>,
    /// Current inspector tab
    pub inspector_tab: InspectorTab,
    /// Component addition UI state
    pub component_addition: ComponentAdditionState,
}

/// Inspector tabs
#[derive(Debug, Clone, PartialEq)]
pub enum InspectorTab {
    Inspect,
    Remove,
    Add,
}

impl Default for InspectorTab {
    fn default() -> Self {
        Self::Inspect
    }
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

/// Event fired when a context menu is requested for a transition
#[derive(Event)]
pub struct TransitionContextMenuRequested {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub event_type: String,
    pub edge_entity: Entity,
    pub position: Pos2,
}

/// Available actions that can be performed on nodes
#[derive(Debug, Clone)]
pub enum NodeAction {
    Inspect,
    AddChild,
    Rename,
    SetAsInitialState,
    MakeParallel,
    MakeParent,
    MakeLeaf,
    Delete,
    ResetRegion,
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

/// Event fired when a transition should be deleted
#[derive(Event)]
pub struct DeleteTransition {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub event_type: String,
}

/// Event fired when a transition should be deleted by its edge entity
#[derive(Event)]
pub struct DeleteTransitionByEdge {
    pub edge_entity: Entity,
}

/// Event fired when a node should be deleted
#[derive(Event)]
pub struct DeleteNode {
    pub entity: Entity,
}

/// Event: request to set a child's parent InitialState to this child
#[derive(Event)]
pub struct SetInitialStateRequested {
    pub child_entity: Entity,
}

/// Data to track transition pulse animation
#[derive(Clone)]
pub struct TransitionPulse {
    pub source_entity: Entity,
    pub target_entity: Entity,
    pub timer: Timer,
}

impl TransitionPulse {
    pub fn new(source_entity: Entity, target_entity: Entity) -> Self {
        Self {
            source_entity,
            target_entity,
            timer: Timer::from_seconds(0.4, TimerMode::Once),
        }
    }
    
    /// Get the current pulse intensity (1.0 at start, 0.0 at end)
    pub fn intensity(&self) -> f32 {
        1.0 - self.timer.fraction()
    }
}

/// Colors for visual feedback
pub const ACTIVE_STATE_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 215, 0); // Gold
pub const BRIGHT_ACTIVE_STATE_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 245, 120); // Brighter gold
pub const NORMAL_NODE_COLOR: egui::Color32 = egui::Color32::from_rgb(60, 60, 60); // Dark grey
pub const TRANSITION_COLOR: egui::Color32 = egui::Color32::WHITE;

/// Calculate the color for a node based on its state
pub fn get_node_color(entity: Entity, active_query: &Query<&Active>) -> egui::Color32 {
    if active_query.contains(entity) {
        ACTIVE_STATE_COLOR
    } else {
        NORMAL_NODE_COLOR
    }
}

/// A short-lived pulse for an entered node (state), used to lerp gold->grey
#[derive(Clone)]
pub struct NodePulse {
    pub entity: Entity,
    pub timer: Timer,
}

impl NodePulse {
    pub fn new(entity: Entity) -> Self {
        Self { entity, timer: Timer::from_seconds(0.6, TimerMode::Once) }
    }
    pub fn intensity(&self) -> f32 { 1.0 - self.timer.fraction() }
}

/// Calculate the display color for a node, blending recent activity pulses
pub fn get_node_display_color(
    entity: Entity,
    active_query: &Query<&Active>,
    pulses: &[NodePulse],
) -> egui::Color32 {
    let is_active = active_query.contains(entity);
    if let Some(pulse) = pulses.iter().find(|p| p.entity == entity) {
        let t = pulse.intensity(); // 1.0 at enter, down to 0.0
        if is_active {
            // Recently activated and still active: lerp from bright gold to gold
            return lerp_color(ACTIVE_STATE_COLOR, BRIGHT_ACTIVE_STATE_COLOR, t);
        } else {
            // Entered then became inactive quickly: flash bright then fade to grey
            return lerp_color(BRIGHT_ACTIVE_STATE_COLOR, NORMAL_NODE_COLOR, 1.0 - t);
        }
    }
    if is_active { ACTIVE_STATE_COLOR } else { NORMAL_NODE_COLOR }
}

/// Calculate the color for a transition line/pill based on pulse state
pub fn get_transition_color(source: Entity, target: Entity, pulses: &[TransitionPulse]) -> egui::Color32 {
    // Base grey color for transitions (same as normal nodes)
    let base_transition_color = NORMAL_NODE_COLOR;
    
    // Find if there's an active pulse for this transition
    if let Some(pulse) = pulses.iter().find(|p| p.source_entity == source && p.target_entity == target) {
        let intensity = pulse.intensity();
        // Lerp between normal grey and gold based on pulse intensity
        lerp_color(base_transition_color, ACTIVE_STATE_COLOR, intensity)
    } else {
        base_transition_color
    }
}

/// Linear interpolation between two colors
fn lerp_color(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgb(
        ((from.r() as f32) * (1.0 - t) + (to.r() as f32) * t) as u8,
        ((from.g() as f32) * (1.0 - t) + (to.g() as f32) * t) as u8,
        ((from.b() as f32) * (1.0 - t) + (to.b() as f32) * t) as u8,
    )
}

/// Perceived luminance of a color (0.0-255.0 scale)
pub fn color_luminance(c: egui::Color32) -> f32 {
    // Rec. 709 luma approximation
    0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32
}

/// Compute a smoothly interpolated text color for a given background.
/// As background brightens from NORMAL_NODE_COLOR to BRIGHT_ACTIVE_STATE_COLOR,
/// text lerps from white to black.
pub fn compute_text_color_for_bg(bg: egui::Color32) -> egui::Color32 {
    let l_bg = color_luminance(bg);
    let l_min = color_luminance(NORMAL_NODE_COLOR);
    let l_max = color_luminance(BRIGHT_ACTIVE_STATE_COLOR);
    let denom = (l_max - l_min).abs().max(1.0); // avoid div by zero
    let mut t = (l_bg - l_min) / denom;
    t = t.clamp(0.0, 1.0);
    lerp_color(egui::Color32::WHITE, egui::Color32::BLACK, t)
}

/// Helper to decide if text color is closer to black (for subscript contrast)
pub fn prefers_dark_text(text: egui::Color32) -> bool {
    color_luminance(text) < 128.0
}

/// Draw an interactive pill-shaped label for transition events
pub fn draw_interactive_pill_label(
    ui: &mut egui::Ui,
    position: egui::Pos2,
    text: &str,
    font_id: egui::FontId,
    is_dragging: bool,
    color: egui::Color32,
) -> egui::Response {
    // Calculate text dimensions
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), font_id, egui::Color32::WHITE));
    let text_size = galley.size();
    
    // Calculate pill size with padding
    let padding = egui::Vec2::new(8.0, 4.0);
    let pill_size = text_size + padding * 2.0;
    
    // Create the pill rectangle centered on the position
    let pill_rect = egui::Rect::from_center_size(position, pill_size);
    
    // Handle interaction (including right-click for context menu)
    let response = ui.allocate_rect(pill_rect, egui::Sense::click_and_drag());
    
    // Draw the pill
    let painter = ui.painter();
    
    // Use the provided color, with slight modification for dragging state
    let bg_color = if is_dragging {
        // Lighten the color when dragging
        egui::Color32::from_rgb(
            (color.r() as f32 * 1.2).min(255.0) as u8,
            (color.g() as f32 * 1.2).min(255.0) as u8,
            (color.b() as f32 * 1.2).min(255.0) as u8,
        )
    } else {
        color
    };
    
    painter.rect_filled(
        pill_rect,
        egui::CornerRadius::same((pill_size.y / 2.0) as u8),
        bg_color,
    );
    
    // Draw border
    painter.rect_stroke(
        pill_rect,
        egui::CornerRadius::same((pill_size.y / 2.0) as u8),
        egui::Stroke::new(1.0, egui::Color32::WHITE),
        egui::StrokeKind::Outside,
    );
    
    // Draw text
    let text_pos = pill_rect.center() - text_size * 0.5;
    painter.galley(text_pos, galley, egui::Color32::WHITE);
    
    response
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
    pub edge_entity: Entity,
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
    child_of_query: &Query<&bevy_gearbox::StateChildOf>,
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