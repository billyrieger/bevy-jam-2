use bevy::{prelude::*, render::texture::ImageSettings, utils::HashMap};
use bevy_prototype_lyon::prelude::*;
use bevy_rapier2d::prelude::*;
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
const SLIME_SIZE_MAX: u32 = 6;

fn main() {
    App::new()
        .insert_resource(WindowDescriptor { ..default() })
        .insert_resource(ImageSettings::default_nearest())
        .insert_resource(MousePosition(None))
        .add_event::<SpawnSlimeEvent>()
        .add_event::<CombineEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(
            PIXELS_PER_METER,
        ))
        .add_startup_system(setup)
        .add_startup_system(setup_physics)
        .add_startup_system(spawn_background_tiles)
        .add_startup_system(spawn_initial_slimes)
        .add_system(animate_sprites)
        .add_system(sync_mouse_position)
        .add_system(slime_drag_animation)
        .add_system(add_activation_circle)
        .add_system(drag_start)
        .add_system(drag_update)
        .add_system(drag_end)
        .add_system(mouse_hover)
        .add_system(color_on_hover)
        .add_system(slime_spawner)
        .add_system(random_movement)
        .add_system(combine)
        .run();
}

#[derive(Default)]
struct MousePosition(Option<Vec2>);

#[derive(Default)]
struct SlimeResources {
    texture_atlases: HashMap<SlimeColor, Handle<TextureAtlas>>,
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
    mut interactable: Query<(&Transform, &Interactable, &DragActive, &mut HoverActive)>,
) {
    if let Some(mouse_pos) = mouse_position.0 {
        for (transform, interactable, drag_active, mut hover_active) in interactable.iter_mut() {
            if transform.translation.truncate().distance(mouse_pos) < interactable.activation_radius
                && !drag_active.0
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
    mut query: Query<(
        Entity,
        &mut Transform,
        &mut DragActive,
        &HoverActive,
        &mut CollisionGroups,
        &mut Velocity,
    )>,
    mut events: EventWriter<CombineEvent>,
) {
    if mouse_input.just_released(MouseButton::Left) {
        let mut addition_entity: Option<Entity> = None;
        let mut base_entity: Option<Entity> = None;
        for (
            entity,
            mut transform,
            mut drag_active,
            hover_active,
            mut collision_groups,
            mut velocity,
        ) in &mut query
        {
            if drag_active.0 {
                drag_active.0 = false;
                transform.translation.z = MAIN_LAYER;
                collision_groups.filters = !0;
                *velocity = Velocity::zero();
                addition_entity = Some(entity);
            } else if hover_active.0 {
                base_entity = Some(entity);
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
    mut combine_events: EventReader<CombineEvent>,
    slime_query: Query<&Slime>,
    mut slime_events: EventWriter<SpawnSlimeEvent>,
) {
    let mut rng = rand::thread_rng();
    for ev in combine_events.iter() {
        if let Ok([base_slime, addition_slime]) = slime_query.get_many([ev.base, ev.addition]) {
            let new_size = base_slime.size + addition_slime.size;
            if new_size > SLIME_SIZE_MAX {
                let overflow = (new_size - SLIME_SIZE_MAX).clamp(SLIME_SIZE_MIN, SLIME_SIZE_MAX);
                    let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                    slime_events.send(SpawnSlimeEvent {
                        slime: Slime {
                            color: SlimeColor::Yellow,
                            size: overflow,
                        },
                        position: ev.location + offset,
                    });
                for _ in 0..2 {
                    let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                    slime_events.send(SpawnSlimeEvent {
                        slime: Slime {
                            color: SlimeColor::Blue,
                            size: SLIME_SIZE_MAX / 2,
                        },
                        position: ev.location + offset,
                    });
                }
            } else {
                let offset = Vec2::new(rng.gen(), rng.gen()) * 20.;
                slime_events.send(SpawnSlimeEvent {
                    slime: Slime {
                        color: SlimeColor::Red,
                        size: new_size,
                    },
                    position: ev.location + offset,
                });
            }
        }
        commands.entity(ev.base).despawn_recursive();
        commands.entity(ev.addition).despawn_recursive();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SlimeColor {
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
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
        Self::Magenta,
        Self::Yellow,
        Self::Black,
        Self::White,
    ];
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
}

#[derive(Component)]
struct SlimeAnimation;

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
    // the grass tiles are the first four rows, 4 * 8 = 32.
    let index_distribution = Uniform::from(0..32);
    let mut rng = rand::thread_rng();
    for x in -10..=10 {
        for y in -10..=10 {
            commands.spawn_bundle(SpriteSheetBundle {
                sprite: TextureAtlasSprite {
                    index: index_distribution.sample(&mut rng),
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

fn slime_spawner(
    mut commands: Commands,
    slime_resources: Res<SlimeResources>,
    mut events: EventReader<SpawnSlimeEvent>,
) {
    for ev in events.iter() {
        let scale = 1. + ev.slime.size as f32;
        let radius_px = scale * SLIME_RADIUS_PX;
        commands
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
                        transform: Transform::from_xyz(-14.5 * scale, 1. * scale, 2.)
                            .with_scale(Vec3::splat(scale)),
                        ..default()
                    })
                    .insert(AnimationTimer(Timer::from_seconds(0.2, true)))
                    .insert(SpriteAnimation::slime_idle());
            });
    }
}

fn spawn_initial_slimes(
    windows: Res<Windows>,
    keys: Res<Input<KeyCode>>,
    mut events: EventWriter<SpawnSlimeEvent>,
) {
    if keys.just_pressed(KeyCode::Space) {
        let mut rng = rand::thread_rng();
        let window = windows.get_primary().unwrap();
        for size in [2, 2, 2, 2, 2, 2, 2, 2] {
            let x = rng.gen_range(0.0..window.width()) - window.width() / 2.;
            let y = rng.gen_range(0.0..window.height()) - window.height() / 2.;
            let color = SlimeColor::ALL[rng.gen_range(0..8)];
            events.send(SpawnSlimeEvent {
                slime: Slime { color, size },
                position: 0.5 * Vec2::new(x, y),
            });
        }
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
        (SlimeColor::Magenta, "purple"),
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
