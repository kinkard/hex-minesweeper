//! Shows how to render a polygonal [`Mesh`], generated from a [`Quad`] primitive, in a 2D scene.

use bevy::{
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
    utils::{HashMap, HashSet},
    window::PrimaryWindow,
};
use hexx::{shapes, Hex, HexLayout, HexOrientation, PlaneMeshBuilder};

const TEXTURE_SIZE: Vec2 = Vec2::splat(16.0);
const HEX_SIZE: Vec2 = Vec2::splat(11.0);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(PreStartup, load_sprites)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input)
        .add_systems(Update, bevy::window::close_on_esc)
        .run();
}

#[derive(Resource)]
struct HexGrid {
    layout: HexLayout,
    entities: HashMap<Hex, Entity>,

    default_material: Handle<ColorMaterial>,
    selected_material: Handle<ColorMaterial>,
}

#[derive(Resource)]
struct Sprites {
    /// Textures to display numbers. Number 1 lives under index 0 and so on.
    numbers: [SpriteBundle; 6],
    mine: SpriteBundle,
    sign: SpriteBundle,
}

fn load_sprites(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load_sprite = |path: &str| SpriteBundle {
        texture: asset_server.load(path),
        sprite: Sprite {
            custom_size: Some(TEXTURE_SIZE),
            ..default()
        },
        transform: Transform::from_xyz(0.0, 0.0, 1.0),
        ..default()
    };

    commands.insert_resource(Sprites {
        numbers: (1..=6).map(|i| format!("{i}.png")).enumerate().fold(
            Default::default(),
            |mut acc, (i, path)| {
                acc[i] = load_sprite(&path);
                acc
            },
        ),
        mine: load_sprite("mine.png"),
        sign: load_sprite("sign.png"),
    });
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    textures: Res<Sprites>,
) {
    commands.spawn(Camera2dBundle::default());

    let layout = HexLayout {
        orientation: HexOrientation::Pointy,
        hex_size: HEX_SIZE,
        ..default()
    };

    // materials
    let selected_material = materials.add(Color::WHITE.into());
    let default_material = materials.add(Color::GRAY.into());

    // mesh
    let mesh = hexagonal_plane(&layout);
    let mesh_handle = meshes.add(mesh);

    let entities: HashMap<_, _> = shapes::hexagon(Hex::ZERO, 20)
        .map(|hex| {
            let pos = layout.hex_to_world_pos(hex);
            let id = commands
                .spawn(ColorMesh2dBundle {
                    transform: Transform::from_xyz(pos.x, pos.y, 0.0).with_scale(Vec3::splat(0.9)),
                    mesh: mesh_handle.clone().into(),
                    material: default_material.clone(),
                    ..default()
                })
                .id();
            (hex, id)
        })
        .collect();

    // Add mines
    let mines: HashSet<_> = entities
        .iter()
        .enumerate()
        // todo: add random here
        .filter(|(index, _)| index % 4 == 0)
        .map(|(_index, (hex, entity))| {
            // Spawn mine
            commands.entity(*entity).with_children(|parent| {
                parent.spawn(textures.mine.clone());
            });
            hex.clone()
        })
        .collect();

    // Count neighbor mines simply iterating over all mines and increment counter for each neigbor
    let numbers = mines.iter().fold(
        HashMap::with_capacity(entities.len() / 2),
        |mut acc, hex| {
            hex.ring(1).for_each(|hex| {
                acc.entry(hex)
                    // keep count-1 to as we store numbers as number-1
                    .and_modify(|count| *count += 1)
                    .or_insert(0);
            });
            acc
        },
    );

    // Add child entities with numbers
    numbers
        .into_iter()
        // we don't want to draw number over the mine
        .filter(|(hex, _number)| !mines.contains(hex))
        .filter_map(|(hex, number)| entities.get(&hex).map(|entity| (entity, number)))
        .for_each(|(entity, number)| {
            commands.entity(*entity).with_children(|parent| {
                parent.spawn(textures.numbers[number as usize].clone());
            });
        });

    commands.insert_resource(HexGrid {
        layout,
        entities,
        default_material,
        selected_material,
    });
}

fn handle_input(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut current: Local<Hex>,
    grid: ResMut<HexGrid>,
    textures: Res<Sprites>,
) {
    let window = windows.single();
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    // Transform from (0,0) in top right corner to the world coordinates with (0,0) in the center
    let cursor_pos = Vec2::new(
        cursor_pos.x - window.width() / 2.0,
        window.height() / 2.0 - cursor_pos.y,
    );

    let hex_pos = grid.layout.world_pos_to_hex(cursor_pos);

    if buttons.just_pressed(MouseButton::Right) {
        if let Some(entity) = grid.entities.get(&hex_pos) {
            // plase sign on right click
            commands.entity(*entity).with_children(|parent| {
                parent.spawn(textures.sign.clone());
            });
        }
    }

    // Do nothing if selected hex didn't change
    if hex_pos == *current {
        return;
    }

    if let Some(entity) = grid.entities.get(&*current) {
        commands
            .entity(*entity)
            .insert(grid.default_material.clone());
    };

    *current = hex_pos;

    if let Some(entity) = grid.entities.get(&*current) {
        commands
            .entity(*entity)
            .insert(grid.selected_material.clone());
    };
}

/// Compute a bevy mesh from the layout
fn hexagonal_plane(hex_layout: &HexLayout) -> Mesh {
    let mesh_info = PlaneMeshBuilder::new(hex_layout).facing(Vec3::Z).build();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, mesh_info.vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_info.normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh_info.uvs);
    mesh.set_indices(Some(Indices::U16(mesh_info.indices)));
    mesh
}
