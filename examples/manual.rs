//! An example of how to use `bevy_save` to manually serialize/deserialize world state in other formats.
//! See the `JSON` example if you want to use a custom format when saving and loading.

use std::{
    collections::HashMap,
    fs::File,
};

use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_save::prelude::*;

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
pub struct FancyMap {
    map: HashMap<String, i32>,
    float: f32,
    bool: bool,
}

#[derive(Component, Deref, DerefMut, Reflect, Default)]
#[reflect(Component)]
pub struct Velocity(Vec2);

fn setup_world(mut commands: Commands) {
    commands.spawn((
        SpatialBundle::from_transform(Transform::from_xyz(0.0, 1.0, 2.0)),
        Velocity(Vec2::new(1.0, 2.0)),
    ));

    commands.spawn((
        SpatialBundle::from_transform(Transform::from_xyz(-4.0, 1.0, 2.0)),
        Velocity(Vec2::new(16.0, 2.0)),
    ));

    commands.spawn((
        SpatialBundle::from_transform(Transform::from_xyz(0.0, 4.0, 2.0)),
        Velocity(Vec2::new(1.0, -2.0)),
    ));

    commands.spawn((
        SpatialBundle::from_transform(Transform::from_xyz(0.0, 1.0, 8.0)),
        Velocity(Vec2::new(0.0, -2.0)),
    ));
}

fn apply_velocity(time: Res<Time>, mut query: Query<(&mut Transform, &Velocity)>) {
    for (mut transform, velocity) in &mut query {
        transform.translation.x += velocity.x * time.delta_seconds();
        transform.translation.y += velocity.y * time.delta_seconds();
    }
}

// Saving to the examples folder is a terrible idea in a real game.
// It is done here to display what a JSON save file generated by `bevy_save` might look like.
const PATH: &str = "examples/saves/example.json";

fn handle_save_input(world: &mut World) {
    let keys = world.resource::<Input<KeyCode>>();

    // While it is possible to use a custom format via AppSaver and AppLoader,
    // we don't do so here because we want to demonstrate
    // what manual serialization and deserialization looks like.

    if keys.just_released(KeyCode::Return) {
        let file = File::create(PATH).expect("Could not open file for serialization");

        let mut ser = serde_json::Serializer::pretty(file);

        world
            .serialize(&mut ser)
            .expect("Could not serialize World");
    } else if keys.just_released(KeyCode::Back) {
        let file = File::open(PATH).expect("Could not open file for deserialization");

        let mut de = serde_json::Deserializer::from_reader(file);

        world
            .deserialize(&mut de)
            .expect("Could not deserialize World");
    }
}

fn main() {
    let mut fancy_map = FancyMap::default();

    fancy_map.map.insert("MyKey".into(), 123);
    fancy_map.map.insert("Another".into(), 456);
    fancy_map.map.insert("More!".into(), -555);
    fancy_map.float = 42.005;
    fancy_map.bool = true;

    App::new()
        // Assets
        .add_plugins((
            DefaultPlugins.build().set(AssetPlugin {
                asset_folder: "examples/assets".to_owned(),
                ..default()
            }),
            // Inspector
            WorldInspectorPlugin::new(),
            // Bevy Save
            SavePlugins,
        ))
        // Register our types as saveable
        .register_saveable::<FancyMap>()
        .register_saveable::<Velocity>()

        // Bevy's reflection requires we register each generic instance of a type individually
        // Note that we only need to register it in the AppTypeRegistry and not in the SaveableRegistry
        .register_type::<HashMap<String, i32>>()

        // Resources
        .insert_resource(fancy_map)
        
        // Systems
        .add_systems(Startup, setup_world)
        .add_systems(Update, (apply_velocity, handle_save_input))
        .run();
}
