use bevy::prelude::*;
use bevy_egui::egui;
use crate::resources::*;

pub struct ComponentDialog;

impl ComponentDialog {
    /// Render the component addition dialog
    pub fn render(ui: &mut egui::Ui, world: &mut World, dialog_state: &mut ComponentDialogState) {
        let Some(entity) = dialog_state.open_for_entity else { return; };
        
        // Collect component names first (separate scope to release registry lock)
        let component_names = Self::collect_available_components(world);
        
        // Show modal dialog
        egui::Window::new("Add Component")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Add component to entity {:?}", entity));
                ui.separator();
                
                Self::render_component_selection(ui, dialog_state, &component_names);
                
                ui.separator();
                
                Self::render_action_buttons(ui, world, entity, dialog_state);
            });
    }

    /// Collect available components that can be added
    fn collect_available_components(world: &mut World) -> Vec<String> {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let mut names: Vec<String> = Vec::new();
        for registration in registry.iter() {
            // Only include types that have ReflectComponent AND ReflectDefault
            if registration.data::<ReflectComponent>().is_some() 
                && registration.data::<ReflectDefault>().is_some() {
                let type_info = registration.type_info();
                names.push(type_info.type_path().to_string());
            }
        }
        names.sort();
        names
    }

    /// Render the component selection dropdown
    fn render_component_selection(
        ui: &mut egui::Ui, 
        dialog_state: &mut ComponentDialogState, 
        component_names: &[String]
    ) {
        ui.horizontal(|ui| {
            ui.label("Component:");
            
            let selected_text = dialog_state.selected_component
                .as_deref()
                .unwrap_or("Select a component...");
            
            egui::ComboBox::from_label("")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for component_name in component_names {
                        ui.selectable_value(
                            &mut dialog_state.selected_component,
                            Some(component_name.clone()),
                            component_name
                        );
                    }
                });
        });
    }

    /// Render the action buttons (Add/Cancel)
    fn render_action_buttons(
        ui: &mut egui::Ui, 
        world: &mut World, 
        entity: Entity, 
        dialog_state: &mut ComponentDialogState
    ) {
        ui.horizontal(|ui| {
            if ui.button("Add").clicked() {
                if let Some(component_name) = &dialog_state.selected_component {
                    crate::utils::add_component_to_entity(world, entity, component_name);
                    Self::close_dialog(dialog_state);
                }
            }
            
            if ui.button("Cancel").clicked() {
                Self::close_dialog(dialog_state);
            }
        });
    }

    /// Close the dialog and reset state
    fn close_dialog(dialog_state: &mut ComponentDialogState) {
        dialog_state.open_for_entity = None;
        dialog_state.selected_component = None;
    }
}

pub struct TransitionDialog;

impl TransitionDialog {
    /// Render the transition creation dialog
    pub fn render(ui: &mut egui::Ui, world: &mut World, transition_state: &mut TransitionCreationState) {
        let Some(source_entity) = transition_state.source_entity else { return; };
        
        if !transition_state.selecting_target {
            Self::render_event_selection_dialog(ui, world, source_entity, transition_state);
        } else {
            Self::render_target_selection_dialog(ui, source_entity, transition_state);
        }
    }

    /// Render the event type selection dialog
    fn render_event_selection_dialog(
        ui: &mut egui::Ui, 
        world: &mut World, 
        source_entity: Entity, 
        transition_state: &mut TransitionCreationState
    ) {
        let event_types = Self::collect_available_event_types(world);
        
        egui::Window::new("Add Transition Listener")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Add transition listener to entity {:?}", source_entity));
                ui.separator();
                
                Self::render_event_type_selection(ui, transition_state, &event_types);
                
                ui.separator();
                
                Self::render_event_selection_buttons(ui, transition_state);
            });
    }

    /// Render the target selection instructions dialog
    fn render_target_selection_dialog(
        ui: &mut egui::Ui, 
        source_entity: Entity, 
        transition_state: &mut TransitionCreationState
    ) {
        egui::Window::new("Select Target Entity")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.label("Click on the target entity node to complete the transition.");
                ui.separator();
                
                if let Some(event_type) = &transition_state.selected_event_type {
                    ui.label(format!("Event: {}", event_type));
                    ui.label(format!("Source: {:?}", source_entity));
                }
                
                ui.separator();
                
                if ui.button("Cancel").clicked() {
                    Self::close_dialog(transition_state);
                }
            });
    }

    /// Collect available event types from registered TransitionListener types
    fn collect_available_event_types(world: &mut World) -> Vec<String> {
        let type_registry = world.resource::<AppTypeRegistry>();
        let registry = type_registry.read();
        
        let mut types: Vec<String> = Vec::new();
        for registration in registry.iter() {
            let type_path = registration.type_info().type_path();
            // Look for TransitionListener<EventType> patterns
            if type_path.contains("TransitionListener<") {
                if let Some(start) = type_path.find('<') {
                    if let Some(end) = type_path.rfind('>') {
                        let event_type = &type_path[start + 1..end];
                        // Extract just the event name (after the last ::)
                        let event_name = if let Some(last_colon) = event_type.rfind("::") {
                            &event_type[last_colon + 2..]
                        } else {
                            event_type
                        };
                        if !types.contains(&event_name.to_string()) {
                            types.push(event_name.to_string());
                        }
                    }
                }
            }
        }
        types.sort();
        types
    }

    /// Render the event type selection dropdown
    fn render_event_type_selection(
        ui: &mut egui::Ui, 
        transition_state: &mut TransitionCreationState, 
        event_types: &[String]
    ) {
        ui.horizontal(|ui| {
            ui.label("Event Type:");
            
            let selected_text = transition_state.selected_event_type
                .as_deref()
                .unwrap_or("Select an event type...");
            
            egui::ComboBox::from_label("")
                .selected_text(selected_text)
                .show_ui(ui, |ui| {
                    for event_type in event_types {
                        ui.selectable_value(
                            &mut transition_state.selected_event_type,
                            Some(event_type.clone()),
                            event_type
                        );
                    }
                });
        });
    }

    /// Render the event selection action buttons
    fn render_event_selection_buttons(ui: &mut egui::Ui, transition_state: &mut TransitionCreationState) {
        ui.horizontal(|ui| {
            if ui.button("Next: Select Target").clicked() {
                if transition_state.selected_event_type.is_some() {
                    transition_state.selecting_target = true;
                }
            }
            
            if ui.button("Cancel").clicked() {
                Self::close_dialog(transition_state);
            }
        });
    }

    /// Close the dialog and reset state
    fn close_dialog(transition_state: &mut TransitionCreationState) {
        transition_state.source_entity = None;
        transition_state.selected_event_type = None;
        transition_state.selecting_target = false;
    }
}