use bevy::{
    app::PluginGroupBuilder,
    prelude::*,
};

use crate::prelude::*;

/// Default plugins for `bevy_save`.
pub struct SavePlugins;

impl PluginGroup for SavePlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(SavePlugin)
            .add(SaverPlugin)
            .add(SaveablesPlugin)
    }
}

/// `bevy_save` core functionality.
pub struct SavePlugin;

#[rustfmt::skip]
impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<SaveableRegistry>()
            .init_resource::<Rollbacks>();
    }
}

/// Serialization and deserialization.
pub struct SaverPlugin;

#[rustfmt::skip]
impl Plugin for SaverPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<AppBackend>()
            .init_resource::<AppSaver>()
            .init_resource::<AppLoader>();
    }
}

/// Saveable registrations for common types.
pub struct SaveablesPlugin;

#[rustfmt::skip]
impl Plugin for SaveablesPlugin {
    fn build(&self, app: &mut App) {
        app
            .register_saveable::<GlobalTransform>()
            .register_saveable::<Transform>();
        
        #[cfg(feature = "bevy_render")]
        app
            .register_saveable::<Visibility>();

        #[cfg(all(feature = "bevy_render", feature = "bevy_asset"))]
        app
            .register_saveable::<Handle<Image>>();

        #[cfg(feature = "bevy_sprite")]
        app
            .register_saveable::<Sprite>()

            // Fix `bevy_reflect: Add ReflectComponent registration for Sprite #8206`
            .register_type_data::<Sprite, ReflectComponent>()
            .register_type::<Option<Vec2>>()
            .register_type::<Option<Rect>>();

        #[cfg(feature = "bevy_ecs_tilemap")]
        {
            use bevy_ecs_tilemap::{
                FrustumCulling,
                prelude::*
            };

            app
                // Tilemap
                .register_saveable::<FrustumCulling>()
                .register_saveable::<TileStorage>()
                .register_saveable::<TilemapGridSize>()
                .register_saveable::<TilemapSize>()
                .register_saveable::<TilemapSpacing>()
                .register_saveable::<TilemapTexture>()
                .register_saveable::<TilemapTileSize>()
                .register_saveable::<TilemapType>()
        
                // Tiles
                .register_saveable::<TileColor>()
                .register_saveable::<TileFlip>()
                .register_saveable::<TilePos>()
                .register_saveable::<TilePosOld>()
                .register_saveable::<TileTextureIndex>()
                .register_saveable::<TileVisible>()
                .register_saveable::<TilemapId>()

                .register_type::<Option<Entity>>()
                .register_type::<Vec<Option<Entity>>>();
        }
        
    }
}
