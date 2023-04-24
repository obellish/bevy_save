use std::{
    collections::HashSet,
    marker::PhantomData,
};

use bevy::{
    ecs::{
        entity::EntityMap,
        query::ReadOnlyWorldQuery,
        reflect::ReflectMapEntities,
    },
    prelude::*,
    reflect::TypeRegistration,
};

use crate::{
    entity::SaveableEntity,
    prelude::*,
};

/// A [`ReadOnlyWorldQuery`] filter.
pub struct Filter<F = ()> {
    _marker: PhantomData<F>,
}

impl Filter {
    /// Create a new filter with the given [`ReadOnlyWorldQuery`].
    pub const fn new<F>() -> Filter<F> {
        Filter {
            _marker: PhantomData,
        }
    }
}

/// Determines what the snapshot will do to existing entities when applied.
pub enum DespawnMode<F = ()> {
    /// Despawn entities missing from the save
    ///
    /// `bevy_save` default
    Missing,

    /// Despawn entities missing from the save matching filter
    MissingWith(Filter<F>),

    /// Despawn unmapped entities
    Unmapped,

    /// Despawn unmapped entities matching filter
    UnmappedWith(Filter<F>),

    /// Despawn all entities matching filter
    AllWith(Filter<F>),

    /// Keep all entities
    ///
    /// `bevy_scene` default
    None,
}

impl Default for DespawnMode {
    fn default() -> Self {
        Self::Missing
    }
}

impl DespawnMode {
    /// Create a new instance of [`DespawnMode::UnmappedWith`] with the given filter.
    pub const fn unmapped_with<F>() -> DespawnMode<F> {
        DespawnMode::UnmappedWith(Filter::new::<F>())
    }

    /// Create a new instance of [`DespawnMode::AllWith`] with the given filter.
    pub const fn all_with<F>() -> DespawnMode<F> {
        DespawnMode::UnmappedWith(Filter::new::<F>())
    }
}

/// Determines how the snapshot will map entities when applied.
#[derive(Default)]
pub enum MappingMode {
    /// If unmapped, attempt a one-to-one mapping. If that fails, spawn a new entity.
    ///
    /// `bevy_save` default
    #[default]
    Simple,

    /// If unmapped, spawn a new entity.
    ///
    /// `bevy_scene` default
    Strict,
}

/// [`Applier`] lets you configure how a snapshot will be applied to the [`World`].
pub struct Applier<'a, S, F = ()> {
    world: &'a mut World,
    snapshot: S,
    map: EntityMap,
    despawn: DespawnMode<F>,
    mapping: MappingMode,
}

impl<'a, S> Applier<'a, S> {
    /// Create a new [`Applier`] with default settings from the world and snapshot.
    pub fn new(world: &'a mut World, snapshot: S) -> Self {
        Self {
            world,
            snapshot,
            map: EntityMap::default(),
            despawn: DespawnMode::default(),
            mapping: MappingMode::default(),
        }
    }

    /// Map entities to new ids with the [`EntityMap`].
    pub fn map(mut self, map: EntityMap) -> Self {
        self.map = map;
        self
    }

    /// Change how the snapshot maps entities when applying.
    pub fn mapping(self, mode: MappingMode) -> Self {
        Applier {
            world: self.world,
            snapshot: self.snapshot,
            map: self.map,
            despawn: self.despawn,
            mapping: mode,
        }
    }

    /// Change how the snapshot affects entities when applying.
    pub fn despawn<F>(self, mode: DespawnMode<F>) -> Applier<'a, S, F> {
        Applier {
            world: self.world,
            snapshot: self.snapshot,
            map: self.map,
            despawn: mode,
            mapping: self.mapping,
        }
    }
}

pub(crate) struct RawSnapshot {
    pub(crate) resources: Vec<Box<dyn Reflect>>,
    pub(crate) entities: Vec<SaveableEntity>,
}

impl RawSnapshot {
    fn from_world_with_filter<F>(world: &World, filter: F) -> Self
    where
        F: Fn(&&TypeRegistration) -> bool,
    {
        let registry_arc = world.resource::<AppTypeRegistry>();
        let registry = registry_arc.read();

        let saveables = world.resource::<SaveableRegistry>();

        // Resources

        let resources = saveables
            .types()
            .filter_map(|name| registry.get_with_name(name))
            .filter(&filter)
            .filter_map(|reg| reg.data::<ReflectResource>())
            .filter_map(|res| res.reflect(world))
            .map(|reflect| reflect.clone_value())
            .collect::<Vec<_>>();

        // Entities

        let mut entities = Vec::new();

        for entity in world.iter_entities().map(|entity| entity.id()) {
            let mut entry = SaveableEntity {
                entity: entity.index(),
                components: Vec::new(),
            };

            let entity = world.entity(entity);

            for component_id in entity.archetype().components() {
                let reflect = world
                    .components()
                    .get_info(component_id)
                    .filter(|info| saveables.contains(info.name()))
                    .and_then(|info| info.type_id())
                    .and_then(|id| registry.get(id))
                    .filter(&filter)
                    .and_then(|reg| reg.data::<ReflectComponent>())
                    .and_then(|reflect| reflect.reflect(entity));

                if let Some(reflect) = reflect {
                    entry.components.push(reflect.clone_value());
                }
            }

            entities.push(entry);
        }

        Self {
            resources,
            entities,
        }
    }
}

impl<'a, F> Applier<'a, &'a RawSnapshot, F>
where
    F: ReadOnlyWorldQuery,
{
    fn apply(self) -> Result<(), SaveableError> {
        let registry_arc = self.world.resource::<AppTypeRegistry>().clone();
        let registry = registry_arc.read();

        // Resources

        for resource in &self.snapshot.resources {
            let reg = registry
                .get_with_name(resource.type_name())
                .ok_or_else(|| SaveableError::UnregisteredType {
                    type_name: resource.type_name().to_string(),
                })?;

            let data = reg.data::<ReflectResource>().ok_or_else(|| {
                SaveableError::UnregisteredResource {
                    type_name: resource.type_name().to_string(),
                }
            })?;

            data.insert(self.world, resource.as_reflect());

            if let Some(mapper) = reg.data::<ReflectMapEntities>() {
                mapper
                    .map_entities(self.world, &self.map)
                    .map_err(SaveableError::MapEntitiesError)?;
            }
        }

        // Entities

        match &self.despawn {
            DespawnMode::Missing | DespawnMode::MissingWith(_) => {
                let valid = self
                    .snapshot
                    .entities
                    .iter()
                    .map(|e| e.try_map(&self.map))
                    .collect::<HashSet<_>>();

                let mut invalid = self
                    .world
                    .iter_entities()
                    .map(|e| e.id())
                    .filter(|e| !valid.contains(e))
                    .collect::<Vec<_>>();

                if let DespawnMode::MissingWith(_) = &self.despawn {
                    let matches = self
                        .world
                        .query_filtered::<Entity, F>()
                        .iter(self.world)
                        .collect::<HashSet<_>>();

                    invalid.retain(|e| matches.contains(e));
                }

                for entity in invalid {
                    self.world.despawn(entity);
                }
            }

            DespawnMode::Unmapped | DespawnMode::UnmappedWith(_) => {
                let valid = self
                    .snapshot
                    .entities
                    .iter()
                    .filter_map(|e| e.map(&self.map))
                    .collect::<HashSet<_>>();

                let mut invalid = self
                    .world
                    .iter_entities()
                    .map(|e| e.id())
                    .filter(|e| !valid.contains(e))
                    .collect::<Vec<_>>();

                if let DespawnMode::UnmappedWith(_) = &self.despawn {
                    let matches = self
                        .world
                        .query_filtered::<Entity, F>()
                        .iter(self.world)
                        .collect::<HashSet<_>>();

                    invalid.retain(|e| matches.contains(e));
                }

                for entity in invalid {
                    self.world.despawn(entity);
                }
            }
            DespawnMode::AllWith(_) => {
                let invalid = self
                    .world
                    .query_filtered::<Entity, F>()
                    .iter(self.world)
                    .collect::<Vec<_>>();

                for entity in invalid {
                    self.world.despawn(entity);
                }
            }
            DespawnMode::None => {}
        }

        let fallback = if let MappingMode::Simple = &self.mapping {
            let mut fallback = EntityMap::default();

            for entity in self.world.iter_entities() {
                fallback.insert(Entity::from_raw(entity.id().index()), entity.id());
            }

            fallback
        } else {
            EntityMap::default()
        };

        // Apply snapshot entities
        for saved in &self.snapshot.entities {
            let index = saved.entity;

            let entity = saved
                .map(&self.map)
                .or_else(|| fallback.get(Entity::from_raw(index)).ok())
                .unwrap_or_else(|| self.world.spawn_empty().id());

            let entity_mut = &mut self.world.entity_mut(entity);

            for component in &saved.components {
                let reg = registry
                    .get_with_name(component.type_name())
                    .ok_or_else(|| SaveableError::UnregisteredType {
                        type_name: component.type_name().to_string(),
                    })?;

                let data = reg.data::<ReflectComponent>().ok_or_else(|| {
                    SaveableError::UnregisteredComponent {
                        type_name: component.type_name().to_string(),
                    }
                })?;

                data.apply_or_insert(entity_mut, &**component);
            }
        }

        for reg in registry.iter() {
            if let Some(mapper) = reg.data::<ReflectMapEntities>() {
                mapper
                    .map_entities(self.world, &self.map)
                    .map_err(SaveableError::MapEntitiesError)?;
            }
        }

        Ok(())
    }
}

impl CloneReflect for RawSnapshot {
    fn clone_value(&self) -> Self {
        Self {
            resources: self.resources.clone_value(),
            entities: self.entities.iter().map(|e| e.clone_value()).collect(),
        }
    }
}

/// A rollback snapshot of the game state.
///
/// [`Rollback`] excludes types that opt out of rollback.
pub struct Rollback {
    pub(crate) snapshot: RawSnapshot,
}

impl Rollback {
    /// Returns a [`Rollback`] of the current [`World`] state.
    ///
    /// This excludes [`Rollbacks`] and any saveable that ignores rollbacking.
    pub fn from_world(world: &World) -> Self {
        Self::from_world_with_filter(world, |_| true)
    }

    /// Returns a [`Rollback`] of the current [`World`] state, filtered by `filter`.
    ///
    /// This excludes [`Rollbacks`] and any saveable that ignores rollbacking.
    pub fn from_world_with_filter<F>(world: &World, filter: F) -> Self
    where
        F: Fn(&&TypeRegistration) -> bool,
    {
        let registry = world.resource::<SaveableRegistry>();

        let snapshot = RawSnapshot::from_world_with_filter(world, |reg| {
            registry.can_rollback(reg.type_name()) && filter(reg)
        });

        Self { snapshot }
    }

    /// Apply the [`Rollback`] to the [`World`].
    ///
    /// # Errors
    /// - See [`SaveableError`]
    pub fn apply(&self, world: &mut World) -> Result<(), SaveableError> {
        self.applier(world).apply()
    }

    /// Create a [`Applier`] from the [`Rollback`] and the [`World`].
    ///
    /// # Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy::ecs::entity::EntityMap;
    /// # use bevy_save::prelude::*;
    /// # let mut app = App::new();
    /// # app.add_plugins(MinimalPlugins);
    /// # app.add_plugins(SavePlugins);
    /// # let world = &mut app.world;
    /// let rollback = Rollback::from_world(world);
    ///
    /// rollback
    ///     .applier(world)
    ///     .map(EntityMap::default())
    ///     .despawn(DespawnMode::default())
    ///     .mapping(MappingMode::default())
    ///     .apply();
    /// ```
    pub fn applier<'a>(&'a self, world: &'a mut World) -> Applier<'a, &'a Self> {
        Applier::new(world, self)
    }

    /// Create an owning [`Applier`] from the [`Rollback`] and the [`World`].
    pub fn into_applier(self, world: &mut World) -> Applier<Self> {
        Applier::new(world, self)
    }
}

macro_rules! impl_rollback_applier {
    ($t:ty) => {
        impl<'a, F> Applier<'a, $t, F>
        where
            F: ReadOnlyWorldQuery,
        {
            /// Apply the [`Rollback`].
            ///
            /// # Errors
            /// - See [`SaveableError`]
            pub fn apply(self) -> Result<(), SaveableError> {
                let applier = Applier {
                    world: self.world,
                    snapshot: &self.snapshot.snapshot,
                    map: self.map,
                    despawn: self.despawn,
                    mapping: self.mapping,
                };

                applier.apply()
            }
        }
    };
}

impl_rollback_applier!(Rollback);
impl_rollback_applier!(&'a Rollback);

impl CloneReflect for Rollback {
    fn clone_value(&self) -> Self {
        Self {
            snapshot: self.snapshot.clone_value(),
        }
    }
}

/// A complete snapshot of the game state.
///
/// Can be serialized via [`SnapshotSerializer`] and deserialized via [`SnapshotDeserializer`].
pub struct Snapshot {
    pub(crate) snapshot: RawSnapshot,
    pub(crate) rollbacks: Rollbacks,
}

impl Snapshot {
    /// Returns a [`Snapshot`] of the current [`World`] state.
    /// Includes [`Rollbacks`].
    pub fn from_world(world: &World) -> Self {
        Self::from_world_with_filter(world, |_| true)
    }

    /// Returns a [`Snapshot`] of the current [`World`] state filtered by `filter`.
    pub fn from_world_with_filter<F>(world: &World, filter: F) -> Self
    where
        F: Fn(&&TypeRegistration) -> bool,
    {
        let snapshot = RawSnapshot::from_world_with_filter(world, filter);
        let rollbacks = world.resource::<Rollbacks>().clone_value();

        Self {
            snapshot,
            rollbacks,
        }
    }

    /// Apply the [`Snapshot`] to the [`World`], restoring it to the saved state.
    ///
    /// # Errors
    /// - See [`SaveableError`]
    pub fn apply(&self, world: &mut World) -> Result<(), SaveableError> {
        self.applier(world).apply()
    }

    /// Create a [`Applier`] from the [`Snapshot`] and the [`World`].
    /// # Example
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy::ecs::entity::EntityMap;
    /// # use bevy_save::prelude::*;
    /// # let mut app = App::new();
    /// # app.add_plugins(MinimalPlugins);
    /// # app.add_plugins(SavePlugins);
    /// # let world = &mut app.world;
    /// let snapshot = Snapshot::from_world(world);
    ///
    /// snapshot
    ///     .applier(world)
    ///     .map(EntityMap::default())
    ///     .despawn(DespawnMode::default())
    ///     .mapping(MappingMode::default())
    ///     .apply();
    /// ```
    pub fn applier<'a>(&'a self, world: &'a mut World) -> Applier<&Self> {
        Applier::new(world, self)
    }

    /// Create an owning [`Applier`] from the [`Snapshot`] and the [`World`].
    pub fn into_applier(self, world: &mut World) -> Applier<Self> {
        Applier::new(world, self)
    }
}

macro_rules! impl_snapshot_applier {
    ($t:ty) => {
        impl<'a, F> Applier<'a, $t, F>
        where
            F: ReadOnlyWorldQuery,
        {
            /// Apply the [`Snapshot`].
            ///
            /// # Errors
            /// - See [`SaveableError`]
            pub fn apply(self) -> Result<(), SaveableError> {
                let applier = Applier {
                    world: self.world,
                    snapshot: &self.snapshot.snapshot,
                    map: self.map,
                    despawn: self.despawn,
                    mapping: self.mapping,
                };

                applier.apply()?;

                self.world
                    .insert_resource(self.snapshot.rollbacks.clone_value());

                Ok(())
            }
        }
    };
}

impl_snapshot_applier!(Snapshot);
impl_snapshot_applier!(&'a Snapshot);

impl CloneReflect for Snapshot {
    fn clone_value(&self) -> Self {
        Self {
            snapshot: self.snapshot.clone_value(),
            rollbacks: self.rollbacks.clone_value(),
        }
    }
}
