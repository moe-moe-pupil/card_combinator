use std::time::Duration;

use bevy::{prelude::*, utils::HashMap};
use bevy_rapier3d::prelude::Collider;

use crate::game::{
    card::{Card, CardBundle, CardClass, CardType, HoverPoint, SelectedCard},
    progress_bar::{self, ProgressBar, ProgressBarBundle, ProgressBarStatus},
};

pub struct TilePlugin;

impl Plugin for TilePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileData>()
            .init_resource::<TileGrid>()
            .init_resource::<HoveredTile>()
            .add_systems(Startup, spawn_tiles)
            .add_systems(PostUpdate, on_spawn_tile)
            .add_systems(Update, hover_tile.after(crate::game::card::select_card))
            .add_systems(Update, evaluate_tiles.after(hover_tile));
    }
}

fn spawn_tiles(mut commands: Commands, tile_data: Res<TileData>) {
    for x in -1..2 {
        for y in -1..2 {
            commands.spawn(TileBundle {
                tile: Tile::Woods {
                    slotted_villager: None,
                    progress_bar: None,
                },
                tile_grid_location: TileGridLocation(IVec2::new(x, y)),
                ..default()
            });
        }
    }
    commands.spawn(TileBundle {
        tile: Tile::Enemies { progress_bar: None },
        tile_grid_location: TileGridLocation(IVec2::new(0, 2)),
        ..default()
    });
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    Woods {
        slotted_villager: Option<Entity>,
        progress_bar: Option<Entity>,
    },
    Enemies {
        progress_bar: Option<Entity>,
    },
}

impl Default for Tile {
    fn default() -> Self {
        Self::Woods {
            slotted_villager: None,
            progress_bar: None,
        }
    }
}

impl Tile {
    pub const SIZE: Vec2 = Vec2::from_array([3.0, 3.0]);
    pub const OFFSET: Vec2 = Vec2::from_array([-0.05, -0.05]);
    pub const TILE_SLOT_ASPECT_RATIO: f32 = 50.0 / 60.0;
    pub const TILE_SLOT_SIZE: f32 = 1.2;
    pub const SPAWN_OFFSET: f32 = 0.95;

    pub fn grid_to_translation(grid_location: IVec2) -> Vec3 {
        (grid_location.as_vec2() * (Self::SIZE + Self::OFFSET)).extend(0.0)
    }

    pub fn translation_to_grid(translation: Vec3) -> IVec2 {
        let size = Self::SIZE + Self::OFFSET;
        let sign = translation.truncate().signum();
        let grid = (translation.truncate() + sign * size / 2.0) / size;
        grid.as_ivec2()
    }

    pub fn slot_size() -> Vec2 {
        Tile::TILE_SLOT_SIZE * Vec2::new(Tile::TILE_SLOT_ASPECT_RATIO, 1.0)
    }

    pub fn has_slot(&self) -> bool {
        match self {
            Tile::Woods {
                slotted_villager,
                progress_bar,
            } => true,
            Tile::Enemies { progress_bar } => false,
        }
    }

    pub fn try_slotting_card(
        &mut self,
        commands: &mut Commands,
        tile_entity: Entity,
        card_entity: Entity,
        card: &Card,
    ) -> bool {
        match self {
            Tile::Woods {
                slotted_villager,
                progress_bar,
            } => {
                if slotted_villager.is_none() && card.class() == CardClass::Villager {
                    *slotted_villager = Some(card_entity);
                    let mut new_progress_bar = None;
                    commands.entity(tile_entity).with_children(|parent| {
                        new_progress_bar = Some(
                            parent
                                .spawn(ProgressBarBundle {
                                    progress_bar: ProgressBar {
                                        current: 0.0,
                                        total: 15.0,
                                        width: 0.85,
                                        height: 0.15,
                                        padding: 0.05,
                                    },
                                    transform: Transform::from_xyz(0.0, 1.0, 0.0),
                                    ..default()
                                })
                                .id(),
                        );
                    });
                    *progress_bar = new_progress_bar;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[derive(Component, Default, Clone, Copy, PartialEq, Eq, Deref, DerefMut)]
pub struct TileGridLocation(IVec2);

#[derive(Component)]
pub struct TileSlotEffect(Entity);

#[derive(Bundle, Default)]
pub struct TileBundle {
    pub tile: Tile,
    pub tile_grid_location: TileGridLocation,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibiltiy: InheritedVisibility,
}

#[derive(Resource)]
pub struct TileData {
    mesh: Handle<Mesh>,
    woods_material: Handle<StandardMaterial>,
    enemies_material: Handle<StandardMaterial>,
    tile_slot_mesh: Handle<Mesh>,
    tile_slot_material: Handle<StandardMaterial>,
}

impl FromWorld for TileData {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let asset_server = world.resource::<AssetServer>();
        Self {
            mesh: meshes.add(Rectangle {
                half_size: Vec2::new(3.0, 3.0),
                ..default()
            }),
            tile_slot_mesh: meshes.add(Rectangle {
                half_size: Tile::slot_size(),
                ..default()
            }),
            woods_material: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("tile_woods.png")),
                base_color: Color::rgb_u8(90, 110, 90),
                unlit: true,
                depth_bias: -10.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            enemies_material: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("tile_woods.png")),
                base_color: Color::rgb_u8(60, 60, 60),
                unlit: true,
                depth_bias: -10.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            tile_slot_material: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("tile_slot.png")),
                base_color: Color::rgba_u8(255, 255, 255, 100),
                unlit: true,
                depth_bias: -9.0,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
        }
    }
}

#[derive(Default, Deref, DerefMut, Resource)]
pub struct TileGrid(HashMap<IVec2, Entity>);

fn on_spawn_tile(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    tile_data: Res<TileData>,
    mut tile_grid: ResMut<TileGrid>,
    mut tiles: Query<(Entity, &mut Tile, &TileGridLocation, &mut Transform), Added<Tile>>,
) {
    for (entity, mut tile, location, mut transform) in &mut tiles {
        tile_grid.insert(location.0, entity);
        transform.translation = Tile::grid_to_translation(location.0);
        let mut tile_slot = None;
        match &mut *tile {
            Tile::Woods {
                slotted_villager,
                progress_bar,
            } => {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn(PbrBundle {
                        material: tile_data.woods_material.clone(),
                        mesh: tile_data.mesh.clone(),
                        ..default()
                    });
                });
            }
            Tile::Enemies { progress_bar } => {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn(PbrBundle {
                        material: tile_data.enemies_material.clone(),
                        mesh: tile_data.mesh.clone(),
                        ..default()
                    });

                    *progress_bar = Some(
                        parent
                            .spawn(ProgressBarBundle {
                                progress_bar: ProgressBar {
                                    current: 0.0,
                                    total: 20.0,
                                    width: 1.0,
                                    height: 0.15,
                                    padding: 0.05,
                                },
                                transform: Transform::from_xyz(0.0, 1.0, 0.0),
                                ..default()
                            })
                            .id(),
                    );
                });
            }
        }

        if tile.has_slot() {
            commands.entity(entity).with_children(|parent| {
                tile_slot = Some(
                    parent
                        .spawn(PbrBundle {
                            material: tile_data.tile_slot_material.clone(),
                            mesh: tile_data.tile_slot_mesh.clone(),
                            transform: Transform::from_xyz(0.0, 0.0, 0.001),
                            visibility: Visibility::Hidden,
                            ..default()
                        })
                        .id(),
                );
            });
        }
        if let Some(tile_slot) = tile_slot {
            commands.entity(entity).insert(TileSlotEffect(tile_slot));
        }
    }
}

pub fn enemy_tile_spawner(
    mut commands: Commands,
    mut timer: Local<Option<Timer>>,
    mut grid_size: Local<UVec2>,
    time: Res<Time>,
) {
    if *grid_size == UVec2::new(0, 0) {
        *grid_size = UVec2::new(3, 3);
    }
    let timer = timer.get_or_insert(Timer::new(
        Duration::from_secs_f32(60.0),
        TimerMode::Repeating,
    ));
    if timer.tick(time.delta()).just_finished() {}
}

#[derive(Default, Resource)]
pub struct HoveredTile(pub Option<Entity>);

pub fn hover_tile(
    hover_point: Res<HoverPoint>,
    tile_grid: Res<TileGrid>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut hovered_tile: ResMut<HoveredTile>,
    selected_card: Res<SelectedCard>,
    mut visibilities: Query<&mut Visibility>,
    tile_slots: Query<&TileSlotEffect>,
    tiles: Query<(&Tile, &TileSlotEffect)>,
) {
    if let Some(tile_entity) = hovered_tile.0 {
        if let Ok(tile_slot) = tile_slots.get(tile_entity) {
            let mut visibility = visibilities.get_mut(tile_slot.0).unwrap();
            *visibility = Visibility::Hidden;
        }
    }
    for (tile, tile_slot) in tiles.iter() {
        match tile {
            Tile::Woods {
                slotted_villager, ..
            } => {
                let mut visibility = visibilities.get_mut(tile_slot.0).unwrap();
                *visibility = if slotted_villager.is_some() {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                };
            }
            _ => {}
        }
    }

    if let SelectedCard::Some(_) = *selected_card {
        if let HoverPoint::Some(point) = *hover_point {
            let location = Tile::translation_to_grid(point);
            if let Some(tile_entity) = tile_grid.get(&location) {
                hovered_tile.0 = Some(*tile_entity);
                let tile_slot = tile_slots.get(*tile_entity).unwrap().0;
                let mut visibility = visibilities.get_mut(tile_slot).unwrap();
                *visibility = Visibility::Visible;
            } else {
                hovered_tile.0 = None;
            }
        } else {
            hovered_tile.0 = None;
        }
    }
}

fn evaluate_tiles(
    mut commands: Commands,
    time: Res<Time>,
    mut tiles: Query<(&mut Tile, &Transform)>,
    mut progress_bars: Query<&mut ProgressBar>,
) {
    for (mut tile, transform) in &mut tiles {
        match &mut *tile {
            Tile::Woods {
                slotted_villager,
                progress_bar,
            } => {
                if let Some(bar_entity) = *progress_bar {
                    if let Ok(mut bar) = progress_bars.get_mut(bar_entity) {
                        bar.add(time.delta_seconds());
                        if bar.finished() {
                            commands.spawn(CardBundle {
                                card: Card::from(CardType::Log),
                                transform: Transform::from_xyz(
                                    transform.translation.x + Tile::SPAWN_OFFSET,
                                    transform.translation.y,
                                    0.0,
                                ),
                                ..default()
                            });
                            bar.reset();
                        }
                    }
                }
            }
            Tile::Enemies { progress_bar } => {
                if let Some(bar_entity) = *progress_bar {
                    if let Ok(mut bar) = progress_bars.get_mut(bar_entity) {
                        bar.add(time.delta_seconds());
                        if bar.finished() {
                            commands.spawn(CardBundle {
                                card: Card::from(CardType::Goblin),
                                transform: Transform::from_xyz(
                                    transform.translation.x,
                                    transform.translation.y,
                                    0.0,
                                ),
                                ..default()
                            });
                            bar.reset();
                        }
                    }
                }
            }
        }
    }
}
