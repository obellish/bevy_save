# Bevy_save
[![][img_bevy]][bevy] [![][img_version]][crates] [![][img_doc]][doc] [![][img_license]][license] [![][img_tracking]][tracking] [![][img_downloads]][crates]

A framework for saving and loading game state in Bevy.

<https://user-images.githubusercontent.com/29737477/234151375-4c561c53-a8f4-4bfe-a5e7-b69af883bf65.mp4>

## Features

### Serialization and Deserialization

While Bevy's `DynamicScene` only allows you to save entities and components, `bevy_save` enables you to save everything, including resources.

- `World::serialize<S>()` and `World::deserialize<D>()` allow you to manually serialize and deserialize game state with your own serializer / deserializer.

### Save file management

`bevy_save` automatically uses your app's workspace name to create a unique, permanent save location in the correct place for [whatever platform](#platforms) it is running on.

- `World::save()` and `World::load()` uses your app's save location to save and load your game state, handling all serialization and deserialization for you.
- The `AppSaver` and `AppLoader` resources determine what save format is used.
  - By default, this is set up to use `rmp_serde` for serialization and deserialization.
  - However, is extremely easy to switch to a custom save file format, see `"examples/json.rs"` for how you can do this.
- The `AppBackend` resource determines how and where to store save files.
  - The default `FileIO` backend saves each named snapshot to an individual file on the disk.
  - Many games have different requirements like saving to multiple directories, to a database, or to WebStorage.
  - You can override the backend by modifying the `AppBackend` resource with your own `Backend` implementation.

#### Save directory location

`WORKSPACE` is the name of your project's workspace (parent folder) name.

| Windows                                             | Linux/*BSD                       | MacOS                                           |
|-----------------------------------------------------|----------------------------------|-------------------------------------------------|
| `C:\Users\%USERNAME%\AppData\Local\WORKSPACE\saves` | `~/.local/share/WORKSPACE/saves` | `~/Library/Application Support/WORKSPACE/saves` |

### Snapshots and Rollback

`bevy_save` is not just about save files, it is about total control over game state.

This crate introduces a few snapshot types which may be used directly:

- `Snapshot` is a serializable snapshot of all saveable resources, entities, and components.
- `Rollback` is a serializable snapshot of all saveable resources, entities, and components that are included in rollbacks.

Or via the `World` extension methods:

- `World::snapshot()` captures a snapshot of the current game state, including resources. (equivalent to `Snapshot::from_world()`)
- `World::checkpoint()` captures a snapshot for later rollback / rollforward.
- `World::rollback()` rolls the game state backwards or forwards through any checkpoints you have created.

The `Rollbacks` resource also gives you fine-tuned control of the currently stored rollbacks.

### Type registration

`bevy_save` adds methods to Bevy's `App` for registering types that should be saved. 
As long as the type implements `Reflect`, it can be registered and used with `bevy_save`.
**Types that are not explicitly registered in the `SaveableRegistry` are not included in save/load**.

- `App.register_saveable::<T>()` registers a type as saveable, allowing it to be included in saves and rollbacks.
- `App.ignore_rollback::<T>()` excludes a type from rollback.
- `App.allow_rollback::<T>()` allows you to re-include a type in rollback after it has already been set to ignore rollback.

### Type filtering

While types that are not registered with `SaveableRegistry` are automatically filtered out for you,
`bevy_save` also allows you to explicitly filter types when creating a snapshot.

- `Snapshot::from_world_with_filter()`
- `Rollback::from_world_with_filter()`

### Entity mapping

As Entity ids are not intended to be used as unique identifiers, `bevy_save` supports mapping Entity ids.

First, you'll need to get a `SnapshotApplier`:

- `World::deserialize_applier()` while manually deserializing a snapshot.
- `World::load_applier()` while loading from a named save.
- `World::rollback_applier()` while rolling back / forward.

Or directly on the snapshot types:

- `Snapshot::applier()`
- `Rollback::applier()`

The `SnapshotApplier` will then allow you to configure the `EntityMap` (and other settings) before applying:

```rust,ignore
let snapshot = Snapshot::from_world(world);

snapshot
    .applier(world)

    // Your entity map - in many cases this can be omitted
    .map(EntityMap::default())

    // Despawn all entities matching (With<A>, Without<B>)
    .despawn(DespawnMode::all_with::<(With<A>, Without<B>)>())

    // Do not overwrite existing entities
    .mapping(MappingMode::Strict)

    .apply();
```

**By default, `bevy_save` snapshots do not behave like Bevy's `DynamicScene` when applying.**

If you use the methods that do not return an `Applier` (`deserialize`, `load`, `rollback`, `apply`), the default settings are used:
- `DespawnMode::Missing` - Any entities not present in the snapshot are despawned.
- `MappingMode::Simple` - Existing entities may be overridden with snapshot data.

You can change the default behavior using the `AppDespawnMode` and `AppMappingMode` resources.

It is also possible to match `DynamicScene` behavior by using `DespawnMode::None` and `MappingMode::Strict`.

#### MapEntities

`bevy_save` also supports `MapEntities` via reflection to allow you to update entity ids within components and resources.

See [Bevy's Parent Component](https://github.com/bevyengine/bevy/blob/v0.11.0/crates/bevy_hierarchy/src/components/parent.rs) for a simple example.

### Entity hooks

You are also able to add hooks when applying snapshots, similar to `bevy-scene-hook`.

This can be used for many things, like spawning the snapshot as a child of an entity:

```rust,ignore
let snapshot = Snapshot::from_world(world);

snapshot
    .applier(world)

    // This will be run for every Entity in the snapshot
    // It runs after the Entity's Components are loaded
    .hook(move |entity, cmds| {
        // You can use the hook to add, get, or remove Components
        if !entity.contains::<Parent>() {
            cmds.set_parent(parent);
        }
    })

    .apply();
```

Hooks may also despawn entities:

```rust,ignore
let snapshot = Snapshot::from_world(world);

snapshot
    .applier(world)
    
    .hook(|entity, cmds| {
        if entity.contains::<A>() {
            cmds.despawn();
        }
    })
```

### Partial Snapshots

While `bevy_save` aims to make it as easy as possible to save your entire world, some games also need to be able to save only parts of the world.

`Builder` allows you to manually create snapshots like `DynamicSceneBuilder`:

```rust,ignore
fn build_snapshot(world: &World, target: Entity, children: Query<&Children>) -> Snapshot {
    Snapshot::builder(world)
        // Extract all saveable resources
        .extract_all_resources()

        // Extract all descendants of `target`
        // This will include all saveable components
        .extract_entities(children.iter_descendants(target))

        // Entities without any saveable components will also be extracted
        // You can use `clear_empty` to remove them
        // NOTE: If applied with the default `MappingMode` this may cause your `Window` entity to be despawned
        // .clear_empty()

        // Build the `Snapshot`
        .build()
}
```

You are also able to extract resources by type name:

```rust,ignore
Snapshot::builder(world)
    // Extract the resource by the type name
    // In this case, we extract the resource from the `manual` example
    .extract_resource("manual::FancyMap")

    // Build the `Snapshot`
    // It will only contain the one resource we extracted
    .build()
```

Additionally, explicit type filtering like `Applier` is available when building snapshots:

```rust,ignore
Snapshot::builder(world)
    // Exclude `Transform` from this `Snapshot`
    .filter(|reg| reg.type_name() != "bevy_transform::components::transform::Transform")

    // Extract all matching entities and resources
    .extract_all()
    
    // Clear all extracted entities without any components
    .clear_empty()
    
    // Build the `Snapshot`
    .build()
```

## License

`bevy_save` is dual-licensed under MIT and Apache-2.0.

## Compatibility

### Bevy

NOTE: We do not track Bevy main.

| Bevy Version | Crate Version                     |
|--------------|-----------------------------------|
| `0.11`       | `0.9`                             |
| `0.10`       | `0.4`, `0.5`, `0.6`, `0.7`, `0.8` |
| `0.9`        | `0.1`, `0.2`, `0.3`               |

### Platforms

| Platform | Support              |
|----------|----------------------|
| Windows  | :heavy_check_mark:   |
| MacOS    | :heavy_check_mark:   |
| Linux    | :heavy_check_mark:   |
| WASM     | :hammer_and_wrench:† |
| Android  | :question:           |
| iOS      | :question:           |

:heavy_check_mark: = First Class Support
—
:ok: = Best Effort Support
—
:zap: = Untested, but should work
—
:question: = Untested, probably won't work
—
:hammer_and_wrench: = In progress

† Everything but `World::save` and `World::load` should work, full support is possible now via a custom backend

### Third-party Crates

`bevy_save` should work with most third-party crates, but you must register their types as saveable to be included in saves.

Registering as saveable requires the type implements `Reflect`. Components will also need to implement `ReflectComponent` and Resources will need to implement `ReflectResource`.

If a type stores `Entity` values, it must also have a `MapEntities` implementation and `ReflectMapEntities` registration to handle entity remapping properly.

Automatic registration for certain crates may be available via a feature flag. Only some types from those crates will be registered.

Registering a type again after it has already been registered will have no effect.

| Name                     | Support             | Feature Flag        | Example             | Notes                    | 
|--------------------------|---------------------|---------------------|---------------------|--------------------------|
| `bevy`                   | :heavy_check_mark:  | :white_check_mark:  | :white_check_mark:  |                          |
| `bevy_ecs_tilemap`       | :ok:                | :white_check_mark:  | :white_check_mark:  | No `MapEntities` support |
| `bevy_rapier`            | :zap:               | :hammer_and_wrench: | :hammer_and_wrench: |                          |
| `bevy_tweening`          | :question:          | :hammer_and_wrench: | :hammer_and_wrench: |                          |
| `leafwing-input-manager` | :zap:               | :hammer_and_wrench: | :hammer_and_wrench: |                          |

:heavy_check_mark: = First Class Support
—
:ok: = Best Effort Support
—
:zap: = Untested, but should work
—
:question: = Untested, probably won't work
—
:hammer_and_wrench: = In progress

[img_bevy]: https://img.shields.io/badge/Bevy-0.11-blue
[img_version]: https://img.shields.io/crates/v/bevy_save.svg
[img_doc]: https://docs.rs/bevy_save/badge.svg
[img_license]: https://img.shields.io/badge/license-MIT%2FApache-blue.svg
[img_downloads]:https://img.shields.io/crates/d/bevy_save.svg
[img_tracking]: https://img.shields.io/badge/Bevy%20tracking-released%20version-lightblue

[bevy]: https://crates.io/crates/bevy/0.11.0
[crates]: https://crates.io/crates/bevy_save
[doc]: https://docs.rs/bevy_save/
[license]: https://github.com/hankjordan/bevy_save#license
[tracking]: https://github.com/bevyengine/bevy/blob/main/docs/plugins_guidelines.md#main-branch-tracking
