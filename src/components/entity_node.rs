use egui::{Color32, FontId, Pos2};

/// Shared properties for all node types (leaf and parent nodes)
#[derive(Debug, Clone)]
pub struct EntityNode {
    /// Position of the node in the editor
    pub position: Pos2,
    /// Current size of the node (updated during rendering)
    pub current_size: egui::Vec2,
    /// Whether this node is currently selected
    pub selected: bool,
    /// Minimum size constraints
    pub min_size: egui::Vec2,
    /// Maximum size constraints  
    pub max_size: egui::Vec2,
    /// Internal padding
    pub padding: egui::Vec2,
    /// Font size for text
    pub font_size: f32,
    /// Background color when not selected
    pub bg_color: Color32,
    /// Background color when selected
    pub selected_bg_color: Color32,
    /// Text color
    pub text_color: Color32,
    /// Border color
    pub border_color: Color32,
    /// Whether this node is currently being dragged by the primary mouse button
    pub is_being_dragged_by_primary: bool,
}

impl EntityNode {
    /// Create a new EntityNode with default styling
    pub fn new(position: Pos2) -> Self {
        Self {
            position,
            current_size: egui::Vec2::new(80.0, 40.0),
            selected: false,
            min_size: egui::Vec2::new(80.0, 40.0),
            max_size: egui::Vec2::new(300.0, 200.0),
            padding: egui::Vec2::new(12.0, 8.0),
            font_size: 14.0,
            bg_color: Color32::from_rgb(45, 45, 55),
            selected_bg_color: Color32::from_rgb(65, 65, 85),
            text_color: Color32::WHITE,
            border_color: Color32::from_rgb(80, 80, 90),
            is_being_dragged_by_primary: false,
        }
    }
    
    /// Get the font ID for the main text
    pub fn main_font_id(&self) -> FontId {
        FontId::new(self.font_size, egui::FontFamily::Proportional)
    }
    
    /// Get the font ID for subscript text (70% of main font size)
    pub fn subscript_font_id(&self) -> FontId {
        FontId::new(self.font_size * 0.7, egui::FontFamily::Proportional)
    }
    
    /// Get the background color based on selection state
    pub fn current_bg_color(&self) -> Color32 {
        if self.selected {
            self.selected_bg_color
        } else {
            self.bg_color
        }
    }
    
    /// Get the current bounding rectangle of this node
    pub fn current_rect(&self) -> egui::Rect {
        egui::Rect::from_min_size(self.position, self.current_size)
    }
}

/// Response from node interaction
#[derive(Debug, Default)]
pub struct NodeResponse {
    pub clicked: bool,
    pub dragged: bool,
    pub drag_delta: egui::Vec2,
    pub hovered: bool,
    pub right_clicked: bool,
    pub add_transition_clicked: bool,
}

