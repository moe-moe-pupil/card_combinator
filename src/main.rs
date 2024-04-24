#[allow(dead_code, unused_variables, unused_mut, unused_imports)]
mod game;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::game::GamePlugin;

fn main() {
    App::new()
        .insert_resource(AmbientLight {
            color: Color::WHITE,
            brightness: 0.4,
        })
        .insert_resource(ClearColor(Color::rgb(0.2, 0.2, 0.2)))
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..Default::default()
        }))
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        // .add_plugin(bevy_inspector_egui::WorldInspectorPlugin::new())
        // .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugins(GamePlugin)
        .run();
}
