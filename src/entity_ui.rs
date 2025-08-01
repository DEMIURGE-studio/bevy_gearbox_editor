use std::any::TypeId;

use bevy::{ecs::world::CommandQueue, prelude::*, reflect::TypeRegistry};
use bevy_ecs::component::ComponentId;
use bevy_inspector_egui::{reflect_inspector::{Context, InspectorUi}, restricted_world_view::{ReflectBorrow, RestrictedWorldView}};

/// Display the components of the given entity
pub(crate) fn ui_for_entity_components(
    world: &mut RestrictedWorldView<'_>,
    mut queue: Option<&mut CommandQueue>,
    entity: Entity,
    ui: &mut egui::Ui,
    id: egui::Id,
    type_registry: &TypeRegistry,
) {
    let Ok(components) = components_of_entity(world, entity) else {
        return;
    };

    for (name, component_id, component_type_id, size) in components {
        let id = id.with(component_id);

        let header = egui::CollapsingHeader::new(&name).id_salt(id);

        let Some(component_type_id) = component_type_id else {
            continue;
        };

        if size == 0 {
            continue;
        }

        // create a context with access to the world except for the currently viewed component
        let (mut component_view, world) = world.split_off_component((entity, component_type_id));
        let mut cx = Context {
            world: Some(world),
            #[allow(clippy::needless_option_as_deref)]
            queue: queue.as_deref_mut(),
        };

        let value = match component_view.get_entity_component_reflect(
            entity,
            component_type_id,
            type_registry,
        ) {
            Ok(value) => value,
            Err(_e) => {
                continue;
            }
        };

        let _response = header.show(ui, |ui| {
            ui.reset_style();

            let mut env = InspectorUi::for_bevy(type_registry, &mut cx);
            let id = id.with(component_id);
            let options = &();

            match value {
                ReflectBorrow::Mutable(mut value) => {
                    let changed = env.ui_for_reflect_with_options(
                        value.bypass_change_detection().as_partial_reflect_mut(),
                        ui,
                        id,
                        options,
                    );

                    if changed {
                        value.set_changed();
                    }
                }
                ReflectBorrow::Immutable(value) => env.ui_for_reflect_readonly_with_options(
                    value.as_partial_reflect(),
                    ui,
                    id,
                    options,
                ),
            };
        });

        ui.reset_style();
    }
}

fn components_of_entity(
    world: &mut RestrictedWorldView<'_>,
    entity: Entity,
) -> Result<Vec<(String, ComponentId, Option<TypeId>, usize)>> {
    let entity_ref = world.world().get_entity(entity)?;

    let archetype = entity_ref.archetype();
    let mut components: Vec<_> = archetype
        .components()
        .map(|component_id| {
            let info = world.world().components().get_info(component_id).unwrap();
            let name = pretty_type_name_str(info.name());

            (name, component_id, info.type_id(), info.layout().size())
        })
        .collect();
    components.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));
    Ok(components)
}

pub fn pretty_type_name_str(val: &str) -> String {
    format!("{:?}", disqualified::ShortName(val))
}