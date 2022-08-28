use bevy::{prelude::*, render::texture::ImageSettings, time::Stopwatch, utils::HashMap};
use bevy_prototype_lyon::prelude::*;
use bevy_rapier2d::prelude::*;
use itertools::Itertools;
use rand::{
    distributions::{Distribution, Uniform},
    thread_rng, Rng,
};

const WINDOW_WIDTH: f32 = 1280.;
const WINDOW_HEIGHT: f32 = 720.;

const PIXELS_PER_METER: f32 = 30.;

const MAIN_LAYER: f32 = 2.;
const DRAG_LAYER: f32 = 5.;
const SHAPE_LAYER: f32 = 7.;

const SLIME_RADIUS_PX: f32 = 14.;
const SLIME_SIZE_MIN: u32 = 1;
const SLIME_SIZE_MAX: u32 = 5;

const SPIDER_RADIUS_PX: f32 = 18.;

const GARDEN_X: f32 = -WINDOW_WIDTH / 2. + 32. * 5.;

fn main() {
    App::new()
        .insert_resource(WindowDescriptor { ..default() })
        .insert_resource(ImageSettings::default_nearest())
        .insert_resource(MousePosition(None))
        .add_event::<SpawnSlimeEvent>()
        .add_event::<SpawnSpiderEvent>()
        .add_event::<CombineEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(
            PIXELS_PER_METER,
        ))
        .add_state(AppState::PreGame)
        .add_startup_system(setup)
        // .add_startup_system(draw_garden_line)
        .add_startup_system(setup_physics)
        .add_startup_system(spawn_background_tiles)
        .add_system(sync_mouse_position)
        .add_system(despawn_old_slime_text)
        .add_system(despawn_old_spider_text)
        .add_system_set(SystemSet::on_enter(AppState::PreGame).with_system(setup_main_menu))
        .add_system_set(SystemSet::on_update(AppState::PreGame).with_system(start_game_on_click))
        .add_system_set(SystemSet::on_exit(AppState::PreGame).with_system(despawn_main_menu))
        .add_system_set(
            SystemSet::on_enter(AppState::InGame)
                .with_system(spawn_initial_slimes)
                .with_system(setup_spider_spawn_timer)
                .with_system(reset_score),
        )
        .add_system_set(
            SystemSet::on_update(AppState::InGame)
                .with_system(animate_sprites)
                .with_system(slime_drag_animation)
                .with_system(add_activation_circle)
                .with_system(drag_start)
                .with_system(drag_update)
                .with_system(drag_end)
                .with_system(mouse_hover)
                .with_system(color_on_hover)
                .with_system(slime_spawner)
                .with_system(random_movement)
                .with_system(combine)
                .with_system(sync_slime_text_position)
                .with_system(sync_spider_text_position)
                .with_system(spider_spawner)
                .with_system(spider_spawn_timer)
                .with_system(end_if_spider_reaches_garden),
        )
        .add_system_set(
            SystemSet::on_enter(AppState::GameOver)
                .with_system(setup_game_over_menu)
                .with_system(set_all_velocities_to_zero)
                .with_system(remove_all_hover)
                .with_system(despawn_other_text),
        )
        .add_system_set(SystemSet::on_update(AppState::GameOver).with_system(restart_game_on_click))
        .add_system_set(SystemSet::on_exit(AppState::GameOver).with_system(despawn_game_over_menu).with_system(despawn_all_entities))
        .run();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    PreGame,
    InGame,
    GameOver,
}

#[derive(Component)]
struct MainMenu;

#[derive(Component)]
struct GameOverMenu;

const INSTRUCTIONS: [&str; 4] = [
    "Drag  slimes  together  to  form  new  slimes.",
    "Drag  slimes  onto  spiders  to  attack  them.",
    "Defeat  spiders  before  they  reach  the  garden.",
    "Click anywhere to begin.",
];

fn setup_main_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            color: Color::rgba(0.0, 0.0, 0.0, 0.9).into(),
            ..default()
        })
        .insert(MainMenu)
        .with_children(|parent| {
            parent.spawn_bundle(
                TextBundle::from_section(
                    INSTRUCTIONS.iter().join("\n\n"),
                    TextStyle {
                        font: asset_server.load("fonts/Kenney Pixel.ttf"),
                        font_size: 32.,
                        color: Color::WHITE,
                    },
                )
                .with_style(Style {
                    margin: UiRect::all(Val::Px(5.0)),
                    ..default()
                }),
            );
        });
}

fn start_game_on_click(mouse_input: Res<Input<MouseButton>>, mut state: ResMut<State<AppState>>) {
    if mouse_input.just_pressed(MouseButton::Left) {
        state.set(AppState::InGame).expect("could not set state");
    }
}

fn despawn_main_menu(mut commands: Commands, main_menu_query: Query<Entity, With<MainMenu>>) {
    for entity in &main_menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn setup_game_over_menu(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    score: Res<ScoreResource>,
) {
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            color: Color::rgba(0.0, 0.0, 0.0, 0.9).into(),
            ..default()
        })
        .insert(GameOverMenu)
        .with_children(|parent| {
            parent.spawn_bundle(
                TextBundle::from_section(
                    format!(
                        "GAME  OVER\n\nSpiders  defeated:  {}\n\nClick anywhere to play again.",
                        score.spiders_killed
                    ),
                    TextStyle {
                        font: asset_server.load("fonts/Kenney Pixel.ttf"),
                        font_size: 32.,
                        color: Color::WHITE,
                    },
                )
                .with_style(Style {
                    margin: UiRect::all(Val::Px(5.0)),
                    ..default()
                }),
            );
        });
}

fn restart_game_on_click(mut mouse_input: ResMut<Input<MouseButton>>, mut state: ResMut<State<AppState>>) {
    if mouse_input.just_pressed(MouseButton::Left) {
        mouse_input.reset_all();
        state.set(AppState::InGame).expect("could not set state");
    }
}

fn despawn_game_over_menu(
    mut commands: Commands,
    game_over_menu_query: Query<Entity, With<GameOverMenu>>,
) {
    for entity in &game_over_menu_query {
        commands.entity(entity).despawn_recursive();
    }
}

fn despawn_all_entities(
    mut commands: Commands,
    query: Query<Entity, Or<(With<Slime>, With<Spider>)>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

fn despawn_other_text(
    mut commands: Commands,
    query: Query<Entity, Or<(With<SlimeText>, With<SpiderText>)>>,
) {
    for entity in &query {
        commands.entity(entity).despawn_recursive();
    }
}

#[derive(Default)]
struct MousePosition(Option<Vec2>);

#[derive(Default)]
struct SlimeResources {
    texture_atlases: HashMap<SlimeColor, Handle<TextureAtlas>>,
}

#[derive(Default)]
struct SpiderResources {
    texture_atlas: Handle<TextureAtlas>,
}

#[derive(Component)]
struct GardenLine;

#[derive(Component)]
struct Spider {
    level: u32,
    weakness: SlimeColor,
    speed: f32,
}

struct SpiderSpawnTimer(Timer);

struct ScoreResource {
    survival_time: Stopwatch,
    spiders_killed: u32,
}

#[derive(Component)]
struct Interactable {
    activation_radius: f32,
}

#[derive(Component, Deref, DerefMut)]
struct DragActive(bool);

#[derive(Component, Deref, DerefMut)]
struct HoverActive(bool);

#[derive(Component)]
struct ActivationCircle;

fn mouse_hover(
    mouse_position: Res<MousePosition>,
    mut interactable: Query<(
        &Transform,
        &Interactable,
        Option<&DragActive>,
        &mut HoverActive,
    )>,
) {
    if let Some(mouse_pos) = mouse_position.0 {
        for (transform, interactable, drag_active, mut hover_active) in interactable.iter_mut() {
            if transform.translation.truncate().distance(mouse_pos) < interactable.activation_radius
                && !drag_active.map(|x| x.0).unwrap_or(false)
            {
                if !hover_active.0 {
                    hover_active.0 = true;
                }
            } else {
                if hover_active.0 {
                    hover_active.0 = false;
                }
            }
        }
    }
}

fn color_on_hover(
    hover_query: Query<(&HoverActive, &Children), Changed<HoverActive>>,
    mut circle_query: Query<&mut DrawMode, With<ActivationCircle>>,
) {
    for (hover_active, children) in hover_query.iter() {
        for &child in children.iter() {
            if let Ok(DrawMode::Outlined {
                ref mut fill_mode, ..
            }) = circle_query.get_mut(child).as_deref_mut()
            {
                *fill_mode = if hover_active.0 {
                    bevy_prototype_lyon::prelude::FillMode::color(Color::rgba(0.5, 0.5, 0.5, 0.5))
                } else {
                    bevy_prototype_lyon::prelude::FillMode::color(Color::NONE)
                }
            }
        }
    }
}

fn add_activation_circle(
    mut commands: Commands,
    interactable_query: Query<(Entity, &Interactable), Added<Interactable>>,
) {
    for (entity, interactable) in &interactable_query {
        let shape = shapes::Circle {
            radius: interactable.activation_radius,
            ..default()
        };
        let circle_entity = commands
            .spawn_bundle(GeometryBuilder::build_as(
                &shape,
                DrawMode::Outlined {
                    fill_mode: bevy_prototype_lyon::prelude::FillMode::color(Color::NONE),
                    outline_mode: StrokeMode::new(Color::NONE, 3.0),
                },
                Transform::from_xyz(0., 0., SHAPE_LAYER),
            ))
            .insert(ActivationCircle)
            .id();
        commands.entity(entity).add_child(circle_entity);
    }
}

fn drag_start(
    mouse_input: Res<Input<MouseButton>>,
    mouse_position: Res<MousePosition>,
    mut draggable_query: Query<(
        &mut Transform,
        &Interactable,
        &mut DragActive,
        &mut HoverActive,
        &mut CollisionGroups,
    )>,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        let mouse_pos = mouse_position.0.unwrap();
        for (mut transform, draggable, mut drag_active, mut hover_active, mut collision_groups) in
            &mut draggable_query
        {
            if transform.translation.truncate().distance(mouse_pos) < draggable.activation_radius {
                drag_active.0 = true;
                hover_active.0 = false;
                transform.translation.z = DRAG_LAYER;
                collision_groups.filters = 0;
                // only drag one thing at a time.
                break;
            }
        }
    }
}

fn drag_update(
    mouse_position: Res<MousePosition>,
    mut draggable_query: Query<(&DragActive, &mut Transform), With<Interactable>>,
) {
    if let Some(mouse_coords) = mouse_position.0 {
        for (drag_active, mut transform) in &mut draggable_query {
            if drag_active.0 {
                transform.translation.x = mouse_coords.x;
                transform.translation.y = mouse_coords.y;
            }
        }
    }
}

struct CombineEvent {
    location: Vec2,
    base: Entity,
    addition: Entity,
}

fn drag_end(
    mouse_position: Res<MousePosition>,
    mouse_input: Res<Input<MouseButton>>,
    mut drag_query: Query<(
        Entity,
        &mut Transform,
        &mut DragActive,
        &mut CollisionGroups,
        &mut Velocity,
    )>,
    hover_query: Query<(Entity, &HoverActive)>,
    mut events: EventWriter<CombineEvent>,
) {
    if mouse_input.just_released(MouseButton::Left) {
        let mut addition_entity: Option<Entity> = None;
        let mut base_entity: Option<Entity> = None;
        for (entity, mut transform, mut drag_active, mut collision_groups, mut velocity) in
            &mut drag_query
        {
            if drag_active.0 {
                drag_active.0 = false;
                transform.translation.z = MAIN_LAYER;
                collision_groups.filters = !0;
                *velocity = Velocity::zero();
                addition_entity = Some(entity);
                break;
            }
        }
        for (entity, hover_active) in &hover_query {
            if hover_active.0 {
                base_entity = Some(entity);
                break;
            }
        }
        if let (Some(addition), Some(base)) = (addition_entity, base_entity) {
            events.send(CombineEvent {
                base,
                addition,
                location: mouse_position.0.unwrap(),
            })
        }
    }
}

fn combine(
    mut commands: Commands,
    mut score: ResMut<ScoreResource>,
    mut combine_events: EventReader<CombineEvent>,
    slime_query: Query<&Slime>,
    spider_query: Query<&Spider>,
    mut slime_events: EventWriter<SpawnSlimeEvent>,
) {
    let mut rng = rand::thread_rng();
    for ev in combine_events.iter() {
        if let Ok([base_slime, addition_slime]) = slime_query.get_many([ev.base, ev.addition]) {
            let new_size = base_slime.size + addition_slime.size;
            let new_color = addition_slime.color;
            let random_color = SlimeColor::ALL[rng.gen_range(0..8)];
            if new_size > SLIME_SIZE_MAX {
                let overflow = (new_size - SLIME_SIZE_MAX).clamp(SLIME_SIZE_MIN, SLIME_SIZE_MAX);
                for (color, size) in [
                    (new_color, SLIME_SIZE_MAX / 2),
                    (new_color, SLIME_SIZE_MAX - SLIME_SIZE_MAX / 2),
                    (new_color, overflow),
                    (random_color, 1),
                ] {
                    let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                    slime_events.send(SpawnSlimeEvent {
                        slime: Slime { color, size },
                        position: ev.location + offset,
                    });
                }
            } else {
                let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                slime_events.send(SpawnSlimeEvent {
                    slime: Slime {
                        color: new_color,
                        size: new_size,
                    },
                    position: ev.location + offset,
                });
            }
            commands.entity(ev.base).despawn_recursive();
            commands.entity(ev.addition).despawn_recursive();
        } else if let (Ok(spider), Ok(slime)) =
            (spider_query.get(ev.base), slime_query.get(ev.addition))
        {
            if spider.level <= slime.size && spider.weakness == slime.color {
                score.spiders_killed += 1;
                commands.entity(ev.base).despawn_recursive();
            }
            for size in [slime.size / 2, slime.size - slime.size / 2] {
                if size > 0 {
                    let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                    slime_events.send(SpawnSlimeEvent {
                        slime: Slime {
                            color: slime.color,
                            size,
                        },
                        position: ev.location + offset,
                    });
                }
            }
            commands.entity(ev.addition).despawn_recursive();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SlimeColor {
    Red,
    Green,
    Blue,
    Cyan,
    Purple,
    Yellow,
    White,
    Black,
}

impl SlimeColor {
    const ALL: [Self; 8] = [
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Cyan,
        Self::Purple,
        Self::Yellow,
        Self::Black,
        Self::White,
    ];

    fn name(&self) -> &'static str {
        match self {
            SlimeColor::Red => "red",
            SlimeColor::Green => "green",
            SlimeColor::Blue => "blue",
            SlimeColor::Cyan => "cyan",
            SlimeColor::Purple => "purple",
            SlimeColor::Yellow => "yellow",
            SlimeColor::White => "white",
            SlimeColor::Black => "black",
        }
    }

    fn color(&self) -> Color {
        match self {
            SlimeColor::Red => Color::rgb_u8(224, 84, 66),
            SlimeColor::Green => Color::rgb_u8(79, 175, 73),
            SlimeColor::Blue => Color::rgb_u8(69, 140, 192),
            SlimeColor::Cyan => Color::rgb_u8(0, 200, 221),
            SlimeColor::Purple => Color::rgb_u8(159, 84, 205),
            SlimeColor::Yellow => Color::rgb_u8(232, 208, 85),
            SlimeColor::White => Color::rgb_u8(217, 217, 217),
            SlimeColor::Black => Color::rgb_u8(11, 11, 11),
        }
    }
}

#[derive(Debug, Component)]
struct Slime {
    color: SlimeColor,
    size: u32,
}

#[derive(Component)]
struct RandomMovement {
    chance_to_move: f32,
    speed: f32,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component)]
struct SpriteAnimation {
    frames: Vec<usize>,
    current: usize,
}

impl SpriteAnimation {
    fn slime_idle() -> Self {
        Self {
            frames: vec![0, 1, 2, 3],
            current: 0,
        }
    }

    fn slime_drag() -> Self {
        Self {
            frames: vec![24, 25, 26, 27],
            current: 0,
        }
    }

    fn spider_walk() -> Self {
        Self {
            frames: vec![16, 17, 18, 19, 20, 21],
            current: 0,
        }
    }
}

#[derive(Component)]
struct SlimeAnimation;

#[derive(Component)]
struct SlimeText {
    slime: Entity,
}

#[derive(Component)]
struct SpiderText {
    spider: Entity,
    above: bool,
}

fn animate_sprites(
    time: Res<Time>,
    mut query: Query<(
        &mut AnimationTimer,
        &mut TextureAtlasSprite,
        &mut SpriteAnimation,
    )>,
) {
    for (mut timer, mut sprite, mut animation) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() {
            animation.current = (animation.current + 1) % animation.frames.len();
        }
        sprite.index = animation.frames[animation.current];
    }
}

fn spawn_background_tiles(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // spawn the background tiles by randomly choosing an index for each tile.
    let background_texture = asset_server.load("tiles/TX Tileset Grass.png");
    let background_atlas = TextureAtlas::from_grid(background_texture, Vec2::new(32.0, 32.0), 8, 8);
    let background_atlas_handle = texture_atlases.add(background_atlas);
    // the grass tiles are the first four tiles of the first four rows, 4 * 4 = 16.
    let index_distribution = Uniform::from(0..16);
    let mut rng = rand::thread_rng();
    for x in -10..=10 {
        for y in -10..=10 {
            let index = index_distribution.sample(&mut rng);
            let tile_row = index / 4;
            let tile_col = index % 4;
            let true_index = tile_row * 8 + tile_col;
            commands.spawn_bundle(SpriteSheetBundle {
                sprite: TextureAtlasSprite {
                    index: true_index,
                    ..default()
                },
                texture_atlas: background_atlas_handle.clone(),
                transform: Transform::from_translation(Vec3::new(0., 0., 1.))
                    * Transform::from_scale(Vec3::splat(2.))
                    * Transform::from_translation(Vec3::new(x as f32 * 32., y as f32 * 32., 0.)),
                ..default()
            });
        }
    }
    let left_col = -10;
    let path_col = -7;
    for x in left_col..(left_col + 3) {
        for y in -10..=10 {
            let index = [12, 13, 14, 20, 21, 22, 28, 29, 30][rng.gen_range(0..9)];
            commands.spawn_bundle(SpriteSheetBundle {
                sprite: TextureAtlasSprite { index, ..default() },
                texture_atlas: background_atlas_handle.clone(),
                transform: Transform::from_translation(Vec3::new(0., 0., 1.))
                    * Transform::from_scale(Vec3::splat(2.))
                    * Transform::from_translation(Vec3::new(x as f32 * 32., y as f32 * 32., 0.)),
                ..default()
            });
        }
    }
    for x in [path_col] {
        for y in -10..=10 {
            let index = [40, 41, 48, 49][rng.gen_range(0..4)];
            commands.spawn_bundle(SpriteSheetBundle {
                sprite: TextureAtlasSprite { index, ..default() },
                texture_atlas: background_atlas_handle.clone(),
                transform: Transform::from_translation(Vec3::new(0., 0., 1.))
                    * Transform::from_scale(Vec3::splat(2.))
                    * Transform::from_translation(Vec3::new(x as f32 * 32., y as f32 * 32., 0.)),
                ..default()
            });
        }
    }
}

fn slime_drag_animation(
    slime_query: Query<(&Slime, &DragActive, &Children), Changed<DragActive>>,
    mut sprite_query: Query<(&mut SpriteAnimation, &mut TextureAtlasSprite)>,
) {
    for (_slime, drag_active, children) in &slime_query {
        for &child in children.iter() {
            if let Ok((mut animation, mut sprite)) = sprite_query.get_mut(child) {
                if drag_active.0 {
                    sprite.color = Color::rgba(1., 1., 1., 0.5);
                    *animation = SpriteAnimation::slime_drag();
                } else {
                    sprite.color = Color::WHITE;
                    *animation = SpriteAnimation::slime_idle();
                }
            }
        }
    }
}

fn random_movement(mut query: Query<(&RandomMovement, &mut Velocity)>) {
    let mut rng = thread_rng();
    for (random_movement, mut velocity) in &mut query {
        if rng.gen::<f32>() < random_movement.chance_to_move {
            let angle = rng.gen::<f32>() * std::f32::consts::TAU;
            *velocity =
                Velocity::linear(velocity.linvel + Vec2::from_angle(angle) * random_movement.speed);
        }
    }
}

struct SpawnSlimeEvent {
    slime: Slime,
    position: Vec2,
}

struct SpawnSpiderEvent {
    spider: Spider,
    position: Vec2,
}

fn slime_spawner(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    slime_resources: Res<SlimeResources>,
    mut events: EventReader<SpawnSlimeEvent>,
) {
    for ev in events.iter() {
        let scale = 1. + ev.slime.size as f32;
        let radius_px = scale * SLIME_RADIUS_PX;
        let slime_entity = commands
            .spawn_bundle(SpatialBundle {
                transform: Transform::from_translation(ev.position.extend(0.)),
                ..default()
            })
            .insert(Slime { ..ev.slime })
            .insert(Interactable {
                activation_radius: radius_px,
            })
            .insert(DragActive(false))
            .insert(HoverActive(false))
            .insert(RandomMovement {
                chance_to_move: 5e-3,
                speed: 200.,
            })
            // rapier components
            .insert(RigidBody::Dynamic)
            .insert(Collider::ball(radius_px))
            .insert(LockedAxes::ROTATION_LOCKED)
            .insert(CollisionGroups::default())
            .insert(Restitution::coefficient(0.5))
            .insert(Velocity::zero())
            .insert(Damping {
                linear_damping: 2.,
                ..default()
            })
            .with_children(|parent| {
                parent
                    .spawn_bundle(SpriteSheetBundle {
                        texture_atlas: slime_resources
                            .texture_atlases
                            .get(&ev.slime.color)
                            .expect("texture atlas not found")
                            .clone(),
                        transform: Transform::from_xyz(-14.5 * scale, 1. * scale, MAIN_LAYER)
                            .with_scale(Vec3::splat(scale)),
                        ..default()
                    })
                    .insert(AnimationTimer(Timer::from_seconds(0.2, true)))
                    .insert(SpriteAnimation::slime_idle());
            })
            .id();
        let lvl_text = TextSection {
            value: "LVL ".to_owned(),
            style: TextStyle {
                font: asset_server.load("fonts/Kenney Pixel Square.ttf"),
                font_size: 16.,
                color: Color::rgba(1., 1., 1., 0.5),
            },
        };
        let number_text = TextSection {
            value: format!("{}", ev.slime.size),
            style: TextStyle {
                font: asset_server.load("fonts/Kenney Pixel Square.ttf"),
                font_size: 32.,
                color: Color::WHITE,
            },
        };
        commands
            .spawn_bundle(TextBundle {
                node: Node {
                    size: Vec2::new(radius_px * 2., radius_px * 2.),
                    ..default()
                },
                text: Text::from_sections([lvl_text, number_text]),
                style: Style {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                // transform: Transform::from_translation(Vec3::new(0., 0., 10.)),
                ..default()
            })
            .insert(SlimeText {
                slime: slime_entity,
            });
    }
}

fn spider_spawner(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    spider_resources: Res<SpiderResources>,
    mut events: EventReader<SpawnSpiderEvent>,
) {
    for ev in events.iter() {
        let scale = 1. + ev.spider.level as f32;
        let radius_px = scale * SPIDER_RADIUS_PX;
        let spider_entity = commands
            .spawn_bundle(SpatialBundle {
                transform: Transform::from_translation(ev.position.extend(0.)),
                ..default()
            })
            .insert(Spider { ..ev.spider })
            .insert(Interactable {
                activation_radius: radius_px,
            })
            .insert(HoverActive(false))
            // rapier components
            .insert(RigidBody::KinematicVelocityBased)
            .insert(Collider::ball(radius_px))
            .insert(LockedAxes::ROTATION_LOCKED)
            .insert(CollisionGroups::default())
            .insert(Restitution::coefficient(0.5))
            .insert(Velocity::linear(Vec2::new(-ev.spider.speed, 0.)))
            .with_children(|parent| {
                parent
                    .spawn_bundle(SpriteSheetBundle {
                        texture_atlas: spider_resources.texture_atlas.clone(),
                        transform: Transform::from_translation(Vec3::new(-1., 0., MAIN_LAYER))
                            .with_scale(Vec3::splat(scale))
                            .with_rotation(Quat::from_axis_angle(
                                Vec3::Z,
                                -std::f32::consts::FRAC_PI_2,
                            )),
                        ..default()
                    })
                    .insert(AnimationTimer(Timer::from_seconds(0.2, true)))
                    .insert(SpriteAnimation::spider_walk());
            })
            .id();
        let font = asset_server.load("fonts/Kenney Pixel Square.ttf");
        let lvl_text = TextSection {
            value: "LVL ".to_owned(),
            style: TextStyle {
                font: font.clone(),
                font_size: 16.,
                color: Color::rgba(1.0, 1.0, 1.0, 0.5),
            },
        };
        let number_text = TextSection {
            value: format!("{}", ev.spider.level),
            style: TextStyle {
                font: font.clone(),
                font_size: 32.,
                color: Color::WHITE,
            },
        };
        let weakness_text = TextSection {
            value: "WEAK TO ".to_owned(),
            style: TextStyle {
                font: font.clone(),
                font_size: 16.,
                color: Color::rgba(1.0, 1.0, 1.0, 0.5),
            },
        };
        let color_text = TextSection {
            value: ev.spider.weakness.name().to_owned(),
            style: TextStyle {
                font: font.clone(),
                font_size: 32.,
                color: ev.spider.weakness.color(),
            },
        };
        commands
            .spawn_bundle(TextBundle {
                node: Node {
                    size: Vec2::new(radius_px * 2., radius_px * 2.),
                    ..default()
                },
                text: Text::from_sections([lvl_text, number_text]),
                style: Style {
                    position_type: PositionType::Absolute,
                    position: UiRect {
                        left: Val::Px(
                            WINDOW_WIDTH / 2. + ev.position.x - scale * SPIDER_RADIUS_PX / 2.,
                        ),
                        top: Val::Px(
                            WINDOW_HEIGHT / 2. - ev.position.y - scale * SPIDER_RADIUS_PX - 16.,
                        ),
                        ..default()
                    },
                    ..default()
                },
                ..default()
            })
            .insert(SpiderText {
                spider: spider_entity,
                above: true,
            });
        commands
            .spawn_bundle(TextBundle {
                node: Node {
                    size: Vec2::new(radius_px * 2., radius_px * 2.),
                    ..default()
                },
                text: Text::from_sections([weakness_text, color_text]),
                style: Style {
                    position_type: PositionType::Absolute,
                    position: UiRect {
                        left: Val::Px(
                            WINDOW_WIDTH / 2. + ev.position.x
                                - scale * SPIDER_RADIUS_PX / 2.
                                - scale * 8.,
                        ),
                        top: Val::Px(
                            WINDOW_HEIGHT / 2. - ev.position.y + scale * SPIDER_RADIUS_PX - 16.,
                        ),
                        ..default()
                    },
                    ..default()
                },
                ..default()
            })
            .insert(SpiderText {
                spider: spider_entity,
                above: false,
            });
    }
}

fn sync_slime_text_position(
    mut text_query: Query<(&mut Text, &mut Style, &SlimeText)>,
    slime_query: Query<(&Transform, &Slime)>,
) {
    for (mut text, mut style, slime_text) in &mut text_query {
        if let Ok((transform, slime)) = slime_query.get(slime_text.slime) {
            let x = transform.translation.x;
            let y = transform.translation.y;
            style.position = UiRect {
                left: Val::Px(
                    WINDOW_WIDTH / 2. + x - (1. + slime.size as f32) * SLIME_RADIUS_PX / 2.,
                ),
                top: Val::Px(
                    WINDOW_HEIGHT / 2. - y - (1. + slime.size as f32) * SLIME_RADIUS_PX - 16.,
                ),
                ..default()
            };
            text.sections[0].style.font_size = 12. + slime.size as f32 * 4.;
            text.sections[1].style.font_size = 24. + slime.size as f32 * 8.;
        }
    }
}

fn sync_spider_text_position(
    mut text_query: Query<(&mut Text, &mut Style, &SpiderText)>,
    spider_query: Query<(&Transform, &Spider)>,
) {
    for (mut text, mut style, spider_text) in &mut text_query {
        if let Ok((transform, spider)) = spider_query.get(spider_text.spider) {
            let x = transform.translation.x;
            let y = transform.translation.y;
            let scale = 1. + spider.level as f32;
            style.position = if spider_text.above {
                UiRect {
                    left: Val::Px(WINDOW_WIDTH / 2. + x - scale * SPIDER_RADIUS_PX / 2.),
                    top: Val::Px(WINDOW_HEIGHT / 2. - y - scale * SPIDER_RADIUS_PX - 16.),
                    ..default()
                }
            } else {
                UiRect {
                    left: Val::Px(
                        WINDOW_WIDTH / 2. + x - scale * SPIDER_RADIUS_PX / 2. - scale * 8.,
                    ),
                    top: Val::Px(WINDOW_HEIGHT / 2. - y + scale * SPIDER_RADIUS_PX - 16.),
                    ..default()
                }
            };
            text.sections[0].style.font_size = 12. + spider.level as f32 * 4.;
            text.sections[1].style.font_size = 24. + spider.level as f32 * 8.;
        }
    }
}

fn despawn_old_slime_text(
    mut commands: Commands,
    mut text_query: Query<(Entity, &SlimeText)>,
    slime_query: Query<&Slime>,
) {
    for (entity, slime_text) in &mut text_query {
        if slime_query.get(slime_text.slime).is_err() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn despawn_old_spider_text(
    mut commands: Commands,
    mut text_query: Query<(Entity, &SpiderText)>,
    spider_query: Query<&Spider>,
) {
    for (entity, spider_text) in &mut text_query {
        if spider_query.get(spider_text.spider).is_err() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn spawn_initial_slimes(windows: Res<Windows>, mut events: EventWriter<SpawnSlimeEvent>) {
    let mut rng = rand::thread_rng();
    let window = windows.get_primary().unwrap();
    for _ in 0..2 {
        for &color in SlimeColor::ALL.iter() {
            let x = rng.gen_range(0.0..window.width()) - window.width() / 2.;
            let y = rng.gen_range(0.0..window.height()) - window.height() / 2.;
            events.send(SpawnSlimeEvent {
                slime: Slime { color, size: 1 },
                position: 0.9 * Vec2::new(x, y),
            });
        }
    }
}

fn setup_spider_spawn_timer(mut commands: Commands) {
    commands.insert_resource(SpiderSpawnTimer(Timer::new(
        std::time::Duration::from_secs_f32(5.),
        true,
    )));
}

fn spider_spawn_timer(
    time: Res<Time>,
    mut timer: ResMut<SpiderSpawnTimer>,
    mut events: EventWriter<SpawnSpiderEvent>,
) {
    let mut rng = thread_rng();
    let level = rng.gen_range(1..5);
    if timer.0.tick(time.delta()).just_finished() {
        events.send(SpawnSpiderEvent {
            spider: Spider {
                level,
                weakness: SlimeColor::ALL[rng.gen_range(0..8)],
                speed: 60.,
                // speed: rng.gen_range(40.0..70.0),
            },
            position: Vec2::new(
                WINDOW_WIDTH / 2. + (1. + level as f32) * SPIDER_RADIUS_PX,
                rng.gen_range((-WINDOW_HEIGHT / 3.)..WINDOW_HEIGHT / 3.),
            ),
        });
    }
}

// fn draw_garden_line(mut commands: Commands) {
//     let shape = shapes::Line(
//         Vec2::new(GARDEN_X, WINDOW_HEIGHT / 2.),
//         Vec2::new(GARDEN_X, -WINDOW_HEIGHT / 2.),
//     );
//     commands
//         .spawn_bundle(GeometryBuilder::build_as(
//             &shape,
//             DrawMode::Stroke(StrokeMode::new(Color::BLACK, 3.0)),
//             Transform::from_xyz(0., 0., SHAPE_LAYER),
//         ))
//         .insert(GardenLine);
// }

fn end_if_spider_reaches_garden(
    mut state: ResMut<State<AppState>>,
    spider_query: Query<(&Transform, &Spider)>,
) {
    for (transform, _spider) in &spider_query {
        if transform.translation.x < GARDEN_X {
            state.set(AppState::GameOver).unwrap();
        }
    }
}

fn set_all_velocities_to_zero(mut query: Query<&mut Velocity>) {
    for mut velocity in &mut query {
        *velocity = Velocity::zero();
    }
}

fn remove_all_hover(mut query: Query<&mut DrawMode, With<ActivationCircle>>) {
    for mut draw_mode in &mut query {
        *draw_mode = DrawMode::Fill(bevy_prototype_lyon::prelude::FillMode::color(Color::NONE));
    }
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // Spawn the camera.
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);

    // Load all the slime textures and insert them as a resource.
    let mut slime_texture_atlases = HashMap::new();
    for (color, color_str) in [
        (SlimeColor::White, "white"),
        (SlimeColor::Black, "black"),
        (SlimeColor::Red, "red"),
        (SlimeColor::Blue, "blue"),
        (SlimeColor::Green, "green"),
        (SlimeColor::Yellow, "yellow"),
        (SlimeColor::Purple, "purple"),
        (SlimeColor::Cyan, "aqua"),
    ] {
        let texture = asset_server.load(&format!("slime/slime_{color_str}.png"));
        let atlas = TextureAtlas::from_grid(texture, Vec2::new(64.0, 32.0), 6, 6);
        let atlas_handle = texture_atlases.add(atlas);
        slime_texture_atlases.insert(color, atlas_handle);
    }
    commands.insert_resource(SlimeResources {
        texture_atlases: slime_texture_atlases,
    });

    // spider resources
    let texture = asset_server.load("spider/spider_gray.png");
    let atlas = TextureAtlas::from_grid(texture, Vec2::new(40.0, 40.0), 8, 7);
    let atlas_handle = texture_atlases.add(atlas);
    commands.insert_resource(SpiderResources {
        texture_atlas: atlas_handle,
    });
}

fn reset_score(mut commands: Commands) {
    commands.insert_resource(ScoreResource {
        survival_time: Stopwatch::new(),
        spiders_killed: 0,
    });
}

fn setup_physics(mut rapier_config: ResMut<RapierConfiguration>, mut commands: Commands) {
    rapier_config.gravity = Vec2::ZERO;
    let wall_size = 20.;
    for (width_x, width_y, pos_x, pos_y) in [
        (wall_size, WINDOW_HEIGHT, -WINDOW_WIDTH / 2., 0.),
        (wall_size, WINDOW_HEIGHT, WINDOW_WIDTH / 2., 0.),
        (WINDOW_WIDTH, wall_size, 0., -WINDOW_HEIGHT / 2.),
        (WINDOW_WIDTH, wall_size, 0., WINDOW_HEIGHT / 2.),
    ] {
        commands
            .spawn()
            .insert(Collider::cuboid(width_x / 2., width_y / 2.))
            .insert(CollisionGroups::default())
            .insert_bundle(TransformBundle::from(Transform::from_xyz(pos_x, pos_y, 0.)));
    }
}

#[derive(Component)]
struct MainCamera;

fn sync_mouse_position(
    windows: Res<Windows>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mouse_position: ResMut<MousePosition>,
) {
    // taken from https://bevy-cheatbook.github.io/cookbook/cursor2world.html
    let (camera, camera_transform) = camera_query.single();
    let window = windows.get_primary().unwrap();
    if let Some(screen_pos) = window.cursor_position() {
        let window_size = Vec2::new(window.width() as f32, window.height() as f32);
        let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
        let ndc_to_world = camera_transform.compute_matrix() * camera.projection_matrix().inverse();
        let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));
        let world_pos: Vec2 = world_pos.truncate();
        mouse_position.0 = Some(world_pos);
    } else {
        mouse_position.0 = None;
    }
}
