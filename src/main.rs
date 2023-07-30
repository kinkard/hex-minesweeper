//! Shows how to render a polygonal [`Mesh`], generated from a [`Quad`] primitive, in a 2D scene.

use bevy::{
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
    utils::{HashMap, HashSet},
    window::PrimaryWindow,
};
use hexx::{shapes, Hex, HexLayout, HexOrientation, PlaneMeshBuilder};

const TEXTURE_SIZE: Vec2 = Vec2::splat(26.0);
const HEX_SIZE: Vec2 = Vec2::splat(16.0);
const GRID_RADIUS: u32 = 16;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(PreStartup, load_sprites)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input)
        .run();
}

#[derive(Resource)]
struct HexGrid {
    layout: HexLayout,
    entities: HashMap<Hex, Entity>,

    with_numbers: HashSet<Hex>,
    with_mines: HashSet<Hex>,

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

    let entities: HashMap<_, _> = shapes::hexagon(Hex::ZERO, GRID_RADIUS)
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
        .filter(|(index, _)| index % 8 == 0)
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
    let numbers = numbers
        .into_iter()
        // we don't want to draw number over the mine
        .filter(|(hex, _number)| !mines.contains(hex))
        .filter_map(|(hex, number)| entities.get(&hex).map(|entity| (hex, number, entity)))
        .map(|(hex, number, entity)| {
            commands.entity(*entity).with_children(|parent| {
                parent.spawn(textures.numbers[number as usize].clone());
            });
            hex
        })
        .collect();

    commands.insert_resource(HexGrid {
        layout,
        entities,

        with_numbers: numbers,
        with_mines: mines,

        default_material,
        selected_material,
    });
}

fn handle_input(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut prev_hex: Local<Hex>,
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

    let curr_hex = grid.layout.world_pos_to_hex(cursor_pos);

    if buttons.just_pressed(MouseButton::Right) {
        if let Some(entity) = grid.entities.get(&curr_hex) {
            // plase sign on right click
            commands.entity(*entity).with_children(|parent| {
                parent.spawn(textures.sign.clone());
            });
        }
    }

    // Do nothing if selected hex didn't change
    if curr_hex == *prev_hex {
        return;
    }
    *prev_hex = curr_hex;

    // Reset all to not bother with additional hash set that keeps highlighted hexes
    for entity in grid.entities.values() {
        commands
            .entity(*entity)
            .insert(grid.default_material.clone());
    }

    // Highlight field of movement using flood fill algorithm, adjusted to the MineSweeper game logic
    let mut visited = HashSet::<Hex>::from([curr_hex]);
    // todo: `with_mines` check will became redundant once this logic became 'on-click'
    if curr_hex.ulength() <= GRID_RADIUS
        && !grid.with_numbers.contains(&curr_hex)
        && !grid.with_mines.contains(&curr_hex)
    {
        // this buffer stores the current line of expansion of the flood fill
        let mut buffer = vec![curr_hex];
        while !buffer.is_empty() {
            buffer = buffer
                .into_iter()
                // take neighbors
                .flat_map(|hex| hex.ring(1))
                // Simplified version of check that this hex is within our map
                .filter(|neighbor| neighbor.ulength() <= GRID_RADIUS)
                // Contains+Insert in a single insert, which with the following check against
                // `grid.with_numbers` implements the core game logic - we add adjusted numbers to the `visited`,
                // but we expand only those neighbor who are not numbers
                .filter(|neighbor| visited.insert(*neighbor))
                // don't need to check against `with_mines` as mines are always surrounded by numbers
                .filter(|neighbor| !grid.with_numbers.contains(neighbor))
                .collect();
        }
    }
    visited
        .into_iter()
        .filter_map(|hex| grid.entities.get(&hex))
        .for_each(|entity| {
            commands
                .entity(*entity)
                .insert(grid.selected_material.clone());
        });
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
