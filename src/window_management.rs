//! Multi-window support for the editor
//! 
//! This module handles:
//! - Creating new editor windows via hotkeys
//! - Managing window entities and cameras
//! - Setting up Egui contexts for multiple windows

use bevy::prelude::*;
use bevy::render::camera::RenderTarget;
use bevy::window::{PrimaryWindow, WindowRef, WindowResolution};
use bevy_egui::EguiMultipassSchedule;

use crate::editor_state::EditorWindow;
use crate::EditorWindowContextPass;

/// System to handle hotkeys for opening editor windows
/// 
/// Listens for Ctrl+O to spawn new editor windows that go directly to the canvas.
/// Only creates a new window if one doesn't already exist.
pub fn handle_editor_hotkeys(
    input: Res<ButtonInput<KeyCode>>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    existing_editor_windows: Query<Entity, With<EditorWindow>>,
    mut commands: Commands,
) {
    if input.pressed(KeyCode::ControlLeft) && input.just_pressed(KeyCode::KeyO) {
        if let Ok(_primary_entity) = primary_window.single() {
            // Only spawn a new editor window if one doesn't already exist
            if existing_editor_windows.is_empty() {
                spawn_editor_window(&mut commands);
            } else {
                info!("🪟 Editor window already exists, ignoring Ctrl+O");
            }
        }
    }
}

/// Spawn a new editor window
/// 
/// Creates a new window entity with its own camera and Egui context.
fn spawn_editor_window(commands: &mut Commands) {
    // Spawn the window
    let window_entity = commands.spawn((
        Window {
            title: "Gearbox Editor".to_string(),
            resolution: WindowResolution::new(1200.0, 800.0),
            ..default()
        },
        EditorWindow,
    )).id();
    
    // Spawn a camera for this window with the editor multipass schedule
    commands.spawn((
        Camera3d::default(),
        Camera {
            target: RenderTarget::Window(WindowRef::Entity(window_entity)),
            ..default()
        },
        EguiMultipassSchedule::new(EditorWindowContextPass),
        EditorWindow, // Mark this camera as belonging to the editor
    ));
    
    info!("🪟 Spawned new editor window");
}

/// Clean up editor camera when its window is closed/despawned, to avoid reusing
/// the same Egui multipass schedule with a lingering context.
pub fn cleanup_editor_window(
    trigger: Trigger<OnRemove, Window>,
    cameras: Query<(Entity, &Camera), With<EditorWindow>>,
    mut editor_state: ResMut<crate::editor_state::EditorState>,
    mut commands: Commands,
) {
    let removed_window = trigger.target();
    for (cam_entity, camera) in cameras.iter() {
        if let RenderTarget::Window(WindowRef::Entity(win_entity)) = camera.target {
            if win_entity == removed_window {
                commands.entity(cam_entity).despawn();
            }
        }
    }
    // Clear all open machines when window closes
    editor_state.open_machines.clear();
}
