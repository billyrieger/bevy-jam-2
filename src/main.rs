use bevy::{prelude::*, render::texture::ImageSettings, utils::HashMap};
use bevy_prototype_lyon::prelude::*;
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};

const MAIN_LAYER: f32 = 2.;
const DRAG_LAYER: f32 = 5.;
const SHAPE_LAYER: f32 = 7.;

fn main() {
    App::new()
        .insert_resource(ImageSettings::default_nearest())
        .insert_resource(MousePosition(None))
        .add_event::<SpawnSlimeEvent>()
        .add_plugins(DefaultPlugins)
        .add_plugin(ShapePlugin)
        .add_startup_system(setup)
        .add_startup_system(spawn_background_tiles)
        .add_system(animate_sprites)
        .add_system(sync_mouse_position)
        .add_system(slime_drag_animation)
        .add_system(draw_activation_circle)
        .add_system(drag_start)
        .add_system(drag_update)
        .add_system(drag_end)
        .add_system(slime_spawner)
        .add_system(spawn_slime_on_keypress)
        .run();
}

#[derive(Default)]
struct MousePosition(Option<Vec2>);

#[derive(Default)]
struct SlimeResources {
    texture_atlases: HashMap<SlimeColor, Handle<TextureAtlas>>,
}

#[derive(Component)]
struct Draggable {
    activation_radius: f32,
}

#[derive(Component)]
struct DragActive(bool);

#[derive(Component)]
struct ActivationCircle;

fn draw_activation_circle(
    mut commands: Commands,
    draggable_query: Query<(Entity, &Draggable), Added<Draggable>>,
) {
    for (entity, draggable) in &draggable_query {
        let shape = shapes::Circle {
            radius: draggable.activation_radius,
            ..default()
        };
        let circle_entity = commands
            .spawn_bundle(GeometryBuilder::build_as(
                &shape,
                DrawMode::Outlined {
                    fill_mode: FillMode::color(Color::NONE),
                    outline_mode: StrokeMode::new(Color::GRAY, 3.0),
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
    mut draggable_query: Query<(&mut Transform, &Draggable, &mut DragActive)>,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        let mouse_pos = mouse_position.0.unwrap();
        for (mut transform, draggable, mut drag_active) in &mut draggable_query {
            if transform.translation.truncate().distance(mouse_pos) < draggable.activation_radius {
                drag_active.0 = true;
                transform.translation.z = DRAG_LAYER;
                // only drag one thing at a time.
                break;
            }
        }
    }
}

fn drag_update(
    mouse_position: Res<MousePosition>,
    mut draggable_query: Query<(&DragActive, &mut Transform), With<Draggable>>,
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

fn drag_end(
    mouse_input: Res<Input<MouseButton>>,
    mut draggable_query: Query<(&mut Transform, &mut DragActive, &Children)>,
    mut circle_query: Query<&mut DrawMode, With<ActivationCircle>>,
) {
    if mouse_input.just_released(MouseButton::Left) {
        for (mut transform, mut drag_active, children) in &mut draggable_query {
            drag_active.0 = false;
            transform.translation.z = MAIN_LAYER;
            for &child in children.iter() {
                if let Ok(mut draw_mode) = circle_query.get_mut(child) {
                    if let DrawMode::Stroke(StrokeMode { ref mut color, .. }) = *draw_mode {
                        *color = Color::GRAY;
                    }
                }
            }
        }
    }
}

const SLIME_RADIUS_PX: f32 = 14.;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Component)]
struct Slime {
    color: SlimeColor,
    size: u32,
}

#[derive(Component)]
struct Dragging;

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component)]
struct SpriteAnimation {
    frames: Vec<usize>,
    current: usize,
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
            if drag_active.0 {
                if let Ok((mut animation, mut sprite)) = sprite_query.get_mut(child) {
                    sprite.color = Color::rgba(1., 1., 1., 0.5);
                    animation.frames = vec![24, 25, 26, 27];
                }
            } else {
                if let Ok((mut animation, mut sprite)) = sprite_query.get_mut(child) {
                    sprite.color = Color::WHITE;
                    animation.frames = vec![0, 1, 2, 3];
                }
            }
        }
    }
}

// fn end_slime_drag(
//     mut commands: Commands,
//     mouse_position: Res<MousePosition>,
//     mouse_buttons: Res<Input<MouseButton>>,
//     slime_query: Query<(Entity, &Children), (With<Slime>, With<Dragging>)>,
//     mut sprite_query: Query<(&mut SpriteAnimation, &mut TextureAtlasSprite)>,
// ) {
//     if mouse_position.0.is_none() || mouse_buttons.just_released(MouseButton::Left) {
//         for (entity, children) in &slime_query {
//             commands.entity(entity).remove::<Dragging>();
//             for &child in children.iter() {
//                 let (mut animation, mut sprite) = sprite_query.get_mut(child).unwrap();
//                 sprite.color = Color::default();
//                 animation.frames = vec![0, 1, 2, 3];
//             }
//         }
//     }
// }

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
        commands
            .spawn_bundle(SpatialBundle {
                transform: Transform::from_translation(ev.position.extend(0.)),
                ..default()
            })
            .insert(Slime { ..ev.slime })
            .insert(Draggable {
                activation_radius: ev.slime.size as f32 * SLIME_RADIUS_PX,
            })
            .insert(DragActive(false))
            .with_children(|parent| {
                parent
                    .spawn_bundle(SpriteSheetBundle {
                        texture_atlas: slime_resources
                            .texture_atlases
                            .get(&ev.slime.color)
                            .expect("texture atlas not found")
                            .clone(),
                        transform: Transform::from_xyz(
                            -14.5 * ev.slime.size as f32,
                            1. * ev.slime.size as f32,
                            2.,
                        )
                        .with_scale(Vec3::splat(ev.slime.size as f32)),
                        ..default()
                    })
                    .insert(AnimationTimer(Timer::from_seconds(0.2, true)))
                    .insert(SpriteAnimation {
                        frames: vec![0, 1, 2, 3],
                        current: 0,
                    });
            });
    }
}

fn spawn_slime_on_keypress(
    windows: Res<Windows>,
    keys: Res<Input<KeyCode>>,
    mut events: EventWriter<SpawnSlimeEvent>,
) {
    if keys.just_pressed(KeyCode::Space) {
        let mut rng = rand::thread_rng();
        let window = windows.get_primary().unwrap();
        for size in [1, 3, 5, 7] {
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
