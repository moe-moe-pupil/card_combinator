use std::time::Duration;

use bevy::prelude::{Rectangle, *};
use bevy::utils::{Entry, HashMap, HashSet};
use bevy::window::PrimaryWindow;
use bevy_rapier3d::prelude::*;

use crate::game::animate::{AnimateRange, Ease};
use crate::game::camera::PlayerCamera;
use crate::game::progress_bar::{ProgressBar, ProgressBarBundle};
use crate::game::tile::{HoveredTile, Tile};

pub struct CardPlugin;

impl Plugin for CardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectedCard>()
            .init_resource::<HoverPoint>()
            .init_resource::<StackRoots>()
            .init_resource::<CardData>()
            .add_systems(PostUpdate, on_spawn_card)
            .add_systems(Update, collide_cards)
            .add_systems(
                Update,
                select_card
                    .after(crate::game::camera::move_camera)
                    .after(collide_cards),
            )
            .add_systems(Update, move_cards.after(select_card))
            .add_systems(Update, evaluate_stacks.after(move_cards))
            .add_systems(Update, handle_enemies.after(evaluate_stacks))
            .add_systems(Update, combat.after(handle_enemies))
            .add_systems(Update, set_hearts.after(combat));
    }
}

#[derive(Component, Default)]
pub struct Card {
    pub animations: Animations,
    pub info: CardInfo,
    pub z: usize,
    pub combat_state: Option<CombatState>,
    pub stack_parent: Option<Entity>,
    pub stack_child: Option<Entity>,
    pub slotted_in_tile: Option<Entity>,
}

pub struct CombatState {
    cooldown: Timer,
    target: Entity,
}

impl From<CardType> for Card {
    fn from(card_type: CardType) -> Self {
        Self {
            info: card_type.into(),
            ..default()
        }
    }
}

impl Card {
    pub const ASPECT_RATIO: f32 = 50.0 / 60.0;
    pub const ART_WIDTH: f32 = 167.0;
    pub const ART_HEIGHT: f32 = 166.0;
    pub const ART_ASPECT: f32 = Self::ART_WIDTH / Self::ART_HEIGHT;
    pub const SPAWN_OFFSET: f32 = 1.0;

    pub fn card_type(&self) -> CardType {
        self.info.card_type
    }

    pub fn class(&self) -> CardClass {
        self.info.card_type.class()
    }

    pub fn is_stackable(&self) -> bool {
        self.slotted_in_tile.is_none() && !(self.class() == CardClass::Enemy)
    }

    pub fn is_player_controlled(&self) -> bool {
        match self.class() {
            CardClass::Villager => true,
            CardClass::Resource => true,
            CardClass::Enemy => false,
        }
    }

    pub fn in_stack(&self) -> bool {
        self.stack_parent.is_some() || self.stack_child.is_some()
    }
}

#[derive(Default, Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum CardType {
    #[default]
    Villager,
    Log,
    Goblin,
}

pub struct CardInfo {
    pub card_type: CardType,
    pub stats: CardStats,
}

impl Default for CardInfo {
    fn default() -> Self {
        CardType::default().into()
    }
}

impl From<CardType> for CardInfo {
    fn from(card_type: CardType) -> Self {
        let stats = card_type.get_initial_stats();
        Self { card_type, stats }
    }
}

impl CardType {
    pub fn class(&self) -> CardClass {
        match self {
            CardType::Villager { .. } => CardClass::Villager,
            CardType::Log => CardClass::Resource,
            CardType::Goblin { .. } => CardClass::Enemy,
        }
    }

    pub fn get_initial_stats(&self) -> CardStats {
        match self {
            CardType::Villager => CardStats {
                health: 3,
                max_health: 3,
                damage: 1,
            },
            CardType::Goblin => CardStats {
                health: 1,
                max_health: 1,
                damage: 1,
            },
            _ => CardStats {
                health: 0,
                max_health: 0,
                damage: 0,
            },
        }
    }
}

#[derive(Debug)]
pub struct CardStats {
    pub health: isize,
    pub max_health: usize,
    pub damage: usize,
}

#[derive(PartialEq, Eq)]
pub enum CardClass {
    Villager,
    Resource,
    Enemy,
}

#[derive(Default, PartialEq, Eq, Copy, Clone, Resource)]
pub enum SelectedCard {
    Some(Entity),
    #[default]
    None,
}

impl SelectedCard {
    fn is_selected(self, entity: Entity) -> bool {
        match self {
            SelectedCard::Some(e) => e == entity,
            SelectedCard::None => false,
        }
    }
}

#[derive(Default, Resource)]
pub enum HoverPoint {
    Some(Vec3),
    #[default]
    None,
}

#[derive(Bundle)]
pub struct CardBundle {
    pub card: Card,
    pub collider: Collider,
    pub sensor: Sensor,
    pub rigid_body: RigidBody,
    pub active_events: ActiveEvents,
    pub active_collision_types: ActiveCollisionTypes,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub visibility: Visibility,
    pub computed_visibiltiy: InheritedVisibility,
}

#[derive(Debug)]
pub enum StackType {
    Pending,
    Nothing,
    Breed { progress_bar: Entity },
}

#[derive(Default, Resource)]
pub struct StackRoots {
    roots: HashMap<Entity, StackType>,
    queued_stack_recomputations: HashSet<Entity>,
}

impl Default for CardBundle {
    fn default() -> Self {
        Self {
            collider: Collider::cuboid(Card::ASPECT_RATIO / 2.0, 1.0 / 2.0, 0.2),
            sensor: Sensor,
            active_events: ActiveEvents::COLLISION_EVENTS,
            active_collision_types: ActiveCollisionTypes::all(),
            rigid_body: RigidBody::Fixed,
            card: Default::default(),
            transform: Default::default(),
            global_transform: Default::default(),
            visibility: Default::default(),
            computed_visibiltiy: Default::default(),
        }
    }
}

#[derive(Resource)]
pub struct CardData {
    mesh: Handle<Mesh>,
    portrait_mesh: Handle<Mesh>,
    heart_mesh: Handle<Mesh>,
    villager_base: Handle<StandardMaterial>,
    resource_base: Handle<StandardMaterial>,
    enemy_base: Handle<StandardMaterial>,
    villager_portrait_base: Handle<StandardMaterial>,
    log_portrait_base: Handle<StandardMaterial>,
    goblin_portrait_base: Handle<StandardMaterial>,
    heart_material: Handle<StandardMaterial>,
    removed_heart_material: Handle<StandardMaterial>,
}

impl FromWorld for CardData {
    fn from_world(world: &mut World) -> Self {
        let world = world.cell();
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let asset_server = world.resource::<AssetServer>();
        let card_base_material = StandardMaterial {
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            base_color_texture: Some(asset_server.load("card_base.png")),
            ..default()
        };
        let villager_base = StandardMaterial {
            base_color: Color::rgb(0.4, 0.4, 0.4),
            ..card_base_material.clone()
        };
        let resource_base = StandardMaterial {
            base_color: Color::rgb(0.7, 0.7, 0.4),
            ..card_base_material.clone()
        };
        let enemy_base = StandardMaterial {
            base_color: Color::rgb(0.7, 0.4, 0.4),
            ..card_base_material.clone()
        };
        Self {
            mesh: meshes.add(Rectangle {
                half_size: Vec2::new(Card::ASPECT_RATIO, 1.0),
                ..default()
            }),
            portrait_mesh: meshes.add(Rectangle {
                half_size: Vec2::new(Card::ART_ASPECT, 1.0) * 0.65,
                ..default()
            }),
            heart_mesh: meshes.add(Rectangle {
                half_size: Vec2::new(HEART_WIDTH, HEART_HEIGHT),
                ..default()
            }),
            villager_portrait_base: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("villager.png")),
                ..villager_base.clone()
            }),
            log_portrait_base: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("log.png")),
                ..resource_base.clone()
            }),
            goblin_portrait_base: materials.add(StandardMaterial {
                base_color_texture: Some(asset_server.load("goblin.png")),
                ..enemy_base.clone()
            }),
            heart_material: materials.add(StandardMaterial {
                base_color: Color::rgba_u8(200, 90, 90, 255),
                base_color_texture: Some(asset_server.load("heart.png")),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                depth_bias: 0.1,
                ..default()
            }),
            removed_heart_material: materials.add(StandardMaterial {
                base_color: Color::rgba(0.1, 0.1, 0.1, 0.5),
                base_color_texture: Some(asset_server.load("heart.png")),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                depth_bias: 0.1,
                ..default()
            }),
            villager_base: materials.add(villager_base),
            resource_base: materials.add(resource_base),
            enemy_base: materials.add(enemy_base),
        }
    }
}

impl CardData {
    pub fn class_material(&self, card_class: CardClass) -> Handle<StandardMaterial> {
        match card_class {
            CardClass::Villager => self.villager_base.clone(),
            CardClass::Resource => self.resource_base.clone(),
            CardClass::Enemy => self.enemy_base.clone(),
        }
    }
    pub fn portrait_material(&self, card_type: CardType) -> Handle<StandardMaterial> {
        match card_type {
            CardType::Villager { .. } => self.villager_portrait_base.clone(),
            CardType::Log => self.log_portrait_base.clone(),
            CardType::Goblin { .. } => self.goblin_portrait_base.clone(),
        }
    }
}

const HEART_WIDTH: f32 = 0.11;
const HEART_HEIGHT: f32 = 0.1;
const HEART_PANEL_WIDTH: f32 = 0.6;

fn on_spawn_card(
    mut commands: Commands,
    card_data: Res<CardData>,
    cards: Query<(Entity, &Card), Added<Card>>,
) {
    for (entity, card) in &cards {
        println!("{:#?}", card.info.stats);
        commands.entity(entity).with_children(|parent| {
            parent.spawn(PbrBundle {
                material: card_data.class_material(card.class()),
                mesh: card_data.mesh.clone(),
                ..default()
            });
            parent.spawn(PbrBundle {
                material: card_data.portrait_material(card.card_type()),
                mesh: card_data.portrait_mesh.clone(),
                transform: Transform::from_xyz(0.0, -0.08, 0.001),
                ..default()
            });
            parent
                .spawn(SpatialBundle::default())
                .with_children(|parent| {
                    let max = card.info.stats.max_health;
                    let offset = HEART_PANEL_WIDTH / max as f32;
                    let width = (max as f32 - 1.0) * offset;
                    for i in 0..max {
                        parent.spawn(PbrBundle {
                            material: card_data.heart_material.clone(),
                            mesh: card_data.heart_mesh.clone(),
                            transform: Transform::from_xyz(
                                i as f32 * offset - width / 2.0,
                                0.37,
                                0.01,
                            ),
                            ..default()
                        });
                    }
                });
        });
    }
}

fn set_hearts(
    card_data: Res<CardData>,
    cards: Query<(&Card, &Children)>,
    children: Query<&Children>,
    mut materials: Query<&mut Handle<StandardMaterial>>,
) {
    for (card, card_children) in &cards {
        if card.info.stats.max_health > 0 {
            let heart_children = children.get(card_children[2]).unwrap();
            for i in 0..card.info.stats.max_health {
                let child = heart_children[i];
                let mut material = materials.get_mut(child).unwrap();
                if i < card.info.stats.health as usize {
                    *material = card_data.heart_material.clone();
                } else {
                    *material = card_data.removed_heart_material.clone();
                }
            }
        }
    }
}

fn move_cards(
    time: Res<Time>,
    selected: Res<SelectedCard>,
    hover_point: Res<HoverPoint>,
    stack_roots: Res<StackRoots>,
    mut cards: Query<(Entity, &mut Card, &mut Transform)>,
    mut transforms: Query<&Transform, Without<Card>>,
) {
    for (entity, mut card, mut transform) in &mut cards {
        let mut z_offset = 0.0;
        if selected.is_selected(entity) {
            z_offset += card.animations.select.tick(time.delta());
            if let HoverPoint::Some(hover_point) = *hover_point {
                transform.translation.x = hover_point.x;
                transform.translation.y = hover_point.y;
            }
        } else {
            z_offset += card.animations.deselect.tick(time.delta());
        }

        if let Some(tile) = card.slotted_in_tile {
            let tile_transform = transforms.get(tile).unwrap();
            transform.translation.x = tile_transform.translation.x;
            transform.translation.y = tile_transform.translation.y;
        }
        transform.translation.z = z_offset;
    }

    for root in stack_roots.roots.keys() {
        let result = cards
            .get(*root)
            .ok()
            .and_then(|(_, card, transform)| card.stack_child.map(|e| (e, transform.translation)));
        if let Some((child, translation)) = result {
            position_stack(&mut cards, child, translation, 1);
        }
    }
}

fn position_stack(
    cards: &mut Query<(Entity, &mut Card, &mut Transform)>,
    entity: Entity,
    root_position: Vec3,
    depth: usize,
) {
    let child = if let Ok((_, card, mut transform)) = cards.get_mut(entity) {
        transform.translation =
            root_position + Vec3::new(0.0, -0.3 * depth as f32, 0.01 * depth as f32);
        card.stack_child
    } else {
        None
    };

    if let Some(child) = child {
        position_stack(cards, child, root_position, depth + 1);
    }
}

fn collide_cards(
    mut commands: Commands,
    mut collisions: EventReader<CollisionEvent>,
    mut stack_roots: ResMut<StackRoots>,
    mut selected: Res<SelectedCard>,
    mut cards: Query<&mut Card>,
    transforms: Query<&Transform>,
) {
    let mut stack_x_on_y = Vec::new();
    for collision in collisions.read() {
        match *collision {
            CollisionEvent::Started(e1, e2, _) => {
                if selected.is_selected(e1) || selected.is_selected(e2) {
                    continue;
                }
                if let (Ok([mut c1, mut c2]), Ok([t1, t2])) =
                    (cards.get_many_mut([e1, e2]), transforms.get_many([e1, e2]))
                {
                    if t1.translation.z > t2.translation.z {
                        if c1.stack_parent.is_none() {
                            stack_x_on_y.push((e1, e2));
                        }
                    } else {
                        if c2.stack_parent.is_none() {
                            stack_x_on_y.push((e2, e1));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for (ex, ey) in stack_x_on_y {
        let top = find_stack_top(&cards.to_readonly(), ey);
        if let Ok([mut cx, mut ctop]) = cards.get_many_mut([ex, top]) {
            if cx.stack_parent.is_none()
                && ctop.stack_child.is_none()
                && ctop.is_stackable()
                && cx.is_stackable()
            {
                // update pointers
                ctop.stack_child = Some(ex);
                cx.stack_parent = Some(top);

                match stack_roots.roots.entry(top) {
                    // if stack root is already a stack, queue recalculation
                    Entry::Occupied(_) => {
                        stack_roots.queued_stack_recomputations.insert(top);
                    }
                    // if parent is newly stacked, make it a stack root and recompute
                    Entry::Vacant(mut entry) => {
                        entry.insert(StackType::Pending);
                        stack_roots.queued_stack_recomputations.insert(top);
                    }
                }

                match stack_roots.roots.entry(ex) {
                    // if newly stacked card is a stack, queue it for recomputation (and therefore removal)
                    Entry::Occupied(_) => {
                        stack_roots.queued_stack_recomputations.insert(ex);
                    }
                    // if newly stacked card is not a stack, do nothing
                    Entry::Vacant(_) => {}
                }
            }
        }
    }
}

fn find_stack_top(cards: &Query<&Card>, mut current_entity: Entity) -> Entity {
    loop {
        if let Ok(card) = cards.get(current_entity) {
            if let Some(child) = card.stack_child {
                current_entity = child;
            } else {
                return current_entity;
            }
        } else {
            return current_entity;
        }
    }
}

fn find_stack_root(cards: &Query<&Card>, mut current_entity: Entity) -> Entity {
    loop {
        if let Ok(card) = cards.get(current_entity) {
            if let Some(parent) = card.stack_parent {
                current_entity = parent;
            } else {
                return current_entity;
            }
        } else {
            return current_entity;
        }
    }
}

pub fn select_card(
    mut commands: Commands,
    context: Res<RapierContext>,
    windows: Query<&Window, With<PrimaryWindow>>,
    hovered_tile: Res<HoveredTile>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut selected_card: ResMut<SelectedCard>,
    mut stack_roots: ResMut<StackRoots>,
    mut hover_point: ResMut<HoverPoint>,
    cameras: Query<(&Camera, &Transform), With<PlayerCamera>>,
    mut cards: Query<&mut Card>,
    mut tiles: Query<(&mut Tile, &Transform)>,
) {
    let window = windows.single();
    if let Some(mut cursor) = window.cursor_position() {
        let (camera, camera_transform) = cameras.single();

        let view = camera_transform.compute_matrix();

        let Rect {
            min: viewport_min,
            max: viewport_max,
        } = camera.logical_viewport_rect().unwrap();
        let screen_size = camera.logical_target_size().unwrap();
        let viewport_size = viewport_max - viewport_min;
        let adj_cursor_pos = cursor - Vec2::new(viewport_min.x, screen_size.y - viewport_max.y);
        println!("{:?}, {:?}", cursor, adj_cursor_pos);
        let projection = camera.projection_matrix();
        let far_ndc = projection.project_point3(Vec3::NEG_Z).z;
        let near_ndc = projection.project_point3(Vec3::Z).z;
        let mut cursor_ndc = (adj_cursor_pos / viewport_size) * 2.0 - Vec2::ONE;
        cursor_ndc.y *= -1.0;
        let ndc_to_world: Mat4 = view * projection.inverse();
        let mut near = ndc_to_world.project_point3(cursor_ndc.extend(near_ndc));
        let mut far = ndc_to_world.project_point3(cursor_ndc.extend(far_ndc));
        let direction = far - near;
        println!("{:?}, {:?}", near, far);
        let denom = Vec3::Z.dot(direction);
        if denom.abs() > 0.0001 {
            let t = (Vec3::ZERO - near).dot(Vec3::Z) / denom;
            if t >= 0.0 {
                *hover_point = HoverPoint::Some(near + direction * t);
            } else {
                *hover_point = HoverPoint::None;
            }
        } else {
            *hover_point = HoverPoint::None;
        }

        if mouse.just_pressed(MouseButton::Left) {
            let result = context.cast_ray(near, direction, 50.0, true, QueryFilter::new());

            if let Some((entity, toi)) = result {
                if cards.get(entity).unwrap().is_player_controlled() {
                    let (parent, child) = {
                        let mut card = cards.get_mut(entity).unwrap();
                        // unslot from tile
                        if let Some(tile_entity) = card.slotted_in_tile {
                            card.slotted_in_tile = None;
                            let (mut tile, _) = tiles.get_mut(tile_entity).unwrap();
                            match &mut *tile {
                                Tile::Woods {
                                    slotted_villager,
                                    progress_bar,
                                } => {
                                    *slotted_villager = None;
                                    if let Some(progress_bar) = *progress_bar {
                                        commands.entity(progress_bar).despawn_recursive();
                                    }
                                }
                                _ => {}
                            }
                        }
                        card.animations.select.reset();
                        *selected_card = SelectedCard::Some(entity);
                        let parent = card.stack_parent;
                        card.stack_parent = None;
                        (parent, card.stack_child)
                    };
                    // finish unstack
                    if let Some(parent) = parent {
                        let mut card = cards.get_mut(parent).unwrap();
                        card.stack_child = None;
                        // queue parent for recomputation
                        stack_roots.queued_stack_recomputations.insert(parent);

                        // unstacked card is now a stack root, create a new stack root as pending and recompute
                        if child.is_some() {
                            stack_roots.roots.insert(entity, StackType::Pending);
                            stack_roots.queued_stack_recomputations.insert(entity);
                        }
                    }
                }
            }
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let SelectedCard::Some(entity) = *selected_card {
            let mut card = cards.get_mut(entity).unwrap();
            card.animations.deselect.reset();
            *selected_card = SelectedCard::None;
            // try stacking on a tile
            if !card.in_stack() {
                if let Some(tile_entity) = hovered_tile.0 {
                    if let Ok((mut tile, transform)) = tiles.get_mut(tile_entity) {
                        if let HoverPoint::Some(hover_point) = *hover_point {
                            let slot_size = Tile::slot_size();
                            if transform.translation.x - slot_size.x / 2.0 < hover_point.x
                                && hover_point.x < transform.translation.x + slot_size.x / 2.0
                                && transform.translation.y - slot_size.y / 2.0 < hover_point.y
                                && hover_point.y < transform.translation.y + slot_size.y / 2.0
                            {
                                if tile.try_slotting_card(&mut commands, tile_entity, entity, &card)
                                {
                                    card.slotted_in_tile = Some(tile_entity);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn evaluate_stacks(
    mut commands: Commands,
    time: Res<Time>,
    mut stack_roots: ResMut<StackRoots>,
    cards: Query<&Card>,
    mut progress_bars: Query<&mut ProgressBar>,
    transforms: Query<&Transform>,
) {
    let stack_roots = &mut *stack_roots;
    for entity in stack_roots.queued_stack_recomputations.drain() {
        let root = find_stack_root(&cards, entity);
        let mut cancelled_stack_types = Vec::new();
        if root != entity {
            // if the queued entity is no longer a root, remove the root and cancel the current stack_type
            if let Some(stack_type) = stack_roots.roots.remove(&entity) {
                cancelled_stack_types.push(stack_type);
            }
        }
        // if the queued root is still a root, recompute the stack type
        let card_types = get_cards_types(root, &cards);
        let villagers = card_types.get(&CardType::Villager).unwrap_or(&0);
        let new_stack_type = if *villagers == 2 && card_types.len() == 1 {
            let mut progress_bar = None;
            commands.entity(root).with_children(|parent| {
                progress_bar = Some(
                    parent
                        .spawn(ProgressBarBundle {
                            progress_bar: ProgressBar {
                                current: 0.0,
                                total: 5.0,
                                width: 0.7,
                                height: 0.15,
                                padding: 0.05,
                            },
                            transform: Transform::from_xyz(0.0, 0.55, 0.0),
                            ..default()
                        })
                        .id(),
                );
            });
            StackType::Breed {
                progress_bar: progress_bar.unwrap(),
            }
        } else {
            StackType::Nothing
        };

        // insert the new stack type and cancel the old one, if it exists
        if let Some(stack_type) = stack_roots.roots.insert(root, new_stack_type) {
            cancelled_stack_types.push(stack_type);
        }

        for stack_type in cancelled_stack_types {
            match stack_type {
                StackType::Pending => {}
                StackType::Nothing => {}
                StackType::Breed { progress_bar } => {
                    commands.entity(progress_bar).despawn_recursive();
                }
            }
        }
    }

    let mut queued_recomputations = Vec::new();
    for (root, stack_type) in stack_roots.roots.iter_mut() {
        let mut should_reset = false;
        match stack_type {
            StackType::Pending => {}
            StackType::Nothing => {}
            StackType::Breed { progress_bar } => {
                if let Ok(mut bar) = progress_bars.get_mut(*progress_bar) {
                    bar.add(time.delta_seconds());
                    if bar.finished() {
                        commands.entity(*progress_bar).despawn_recursive();
                        if let Ok(transform) = transforms.get(*root) {
                            commands.spawn(CardBundle {
                                card: Card {
                                    info: CardType::Villager.into(),
                                    ..default()
                                },
                                transform: Transform::from_xyz(
                                    transform.translation.x + Card::SPAWN_OFFSET,
                                    transform.translation.y,
                                    0.0,
                                ),
                                ..default()
                            });
                        }
                        should_reset = true;
                    }
                }
            }
        }
        if should_reset {
            *stack_type = StackType::Pending;
            queued_recomputations.push(*root);
        }
    }

    stack_roots
        .queued_stack_recomputations
        .extend(queued_recomputations);
}

fn get_cards_types(root: Entity, cards: &Query<&Card>) -> HashMap<CardType, usize> {
    let mut current = root;
    let mut card_types = HashMap::new();
    while let Ok(card) = cards.get(current) {
        let mut count = card_types.entry(card.card_type()).or_insert(0);
        *count += 1;
        if let Some(child) = card.stack_child {
            current = child;
        } else {
            break;
        }
    }

    card_types
}

pub struct Animations {
    select: AnimateRange,
    deselect: AnimateRange,
    attack_in: AnimateRange,
    attack_out: AnimateRange,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            select: AnimateRange::new(Duration::from_secs_f32(0.2), Ease::Linear, 0.0..0.5, false),
            deselect: AnimateRange::new(
                Duration::from_secs_f32(0.2),
                Ease::Linear,
                0.5..0.0,
                false,
            ),
            attack_in: AnimateRange::new(
                Duration::from_secs_f32(0.2),
                Ease::Linear,
                1.0..1.5,
                false,
            ),
            attack_out: AnimateRange::new(
                Duration::from_secs_f32(0.2),
                Ease::Linear,
                1.5..1.0,
                false,
            ),
        }
    }
}

pub fn handle_enemies(time: Res<Time>, mut cards: Query<(Entity, &mut Card, &mut Transform)>) {
    let mut enemy_targets = Vec::new();
    for (entity, card, transform) in &cards {
        if card.combat_state.is_some() {
            continue;
        }
        if let CardClass::Enemy = card.class() {
            let mut current_target: Option<(Entity, Vec3)> = None;
            for (target_entity, target_card, target_transform) in &cards {
                if target_card.class() == CardClass::Villager {
                    if let Some((_, current_translation)) = current_target {
                        if current_translation.distance_squared(transform.translation)
                            > target_transform
                                .translation
                                .distance_squared(transform.translation)
                        {
                            current_target = Some((target_entity, target_transform.translation));
                        }
                    } else {
                        current_target = Some((target_entity, target_transform.translation));
                    }
                }
            }

            if let Some((target, translation)) = current_target {
                enemy_targets.push((entity, target, translation))
            }
        }
    }

    for (enemy, target, target_translation) in enemy_targets {
        let [(_, mut card, mut transform), (_, mut target_card, _)] =
            cards.get_many_mut([enemy, target]).unwrap();
        let distance = target_translation - transform.translation;
        // move until close
        if distance.length() > 1.0 {
            let direction = distance.normalize();
            transform.translation += direction * time.delta_seconds();
            card.combat_state = None;
        } else {
            card.combat_state = Some(CombatState {
                cooldown: Timer::from_seconds(1.0, TimerMode::Repeating),
                target,
            });

            // if target_card.combat_state.is_none() {
            //     target_card.combat_state = Some(CombatState {
            //         // villagers attack faster than enemies
            //         cooldown: Timer::from_seconds(0.9, true),
            //         target: enemy,
            //     });
            // }
        }
    }
}

fn combat(
    mut commands: Commands,
    time: Res<Time>,
    mut cards: Query<&mut Card>,
    card_entities: Query<Entity, With<Card>>,
) {
    for entity in &card_entities {
        let result = {
            let mut card = cards.get_mut(entity).unwrap();
            if let Some(combat_state) = &mut card.combat_state {
                if combat_state.cooldown.tick(time.delta()).just_finished() {
                    Some((combat_state.target, card.info.stats.damage))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((damaged_entity, damage)) = result {
            if let Ok([mut target_card, mut card]) = cards.get_many_mut([damaged_entity, entity]) {
                target_card.info.stats.health =
                    (target_card.info.stats.health - damage as isize).max(0);
                if target_card.combat_state.is_none() {
                    target_card.combat_state = Some(CombatState {
                        cooldown: Timer::from_seconds(0.9, TimerMode::Repeating),
                        target: entity,
                    });
                }
                if target_card.info.stats.health == 0 {
                    card.combat_state = None;
                    commands.entity(damaged_entity).despawn_recursive();
                }
            } else {
                cards.get_mut(entity).unwrap().combat_state = None;
            }
        }
    }
}
