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
const GRID_LAYOUT: HexLayout = HexLayout {
    orientation: HexOrientation::Pointy,
    hex_size: HEX_SIZE,
    origin: Vec2::ZERO,
    invert_x: false,
    invert_y: false,
};

fn is_hex_within_grid(hex: &Hex) -> bool {
    hex.ulength() <= GRID_RADIUS
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                // todo: derive from the `HEX_SIZE` and `GRID_RADIUS`
                resolution: (916.0, 800.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(PreStartup, load_sprites)
        .add_systems(Startup, setup)
        .add_systems(Update, handle_input)
        .run();
}

#[derive(Resource)]
struct HexGrid {
    entities: HashMap<Hex, Entity>,

    covered: HashSet<Hex>,
    numbers: HashMap<Hex, u8>,
    mines: HashSet<Hex>,
    flagged: HashSet<Hex>,

    covered_material: Handle<ColorMaterial>,
    uncovered_material: Handle<ColorMaterial>,
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
) {
    commands.spawn(Camera2dBundle::default());

    // materials
    let covered_material = materials.add(Color::DARK_GRAY.into());
    let uncovered_material = materials.add(Color::GRAY.into());
    let selected_material = materials.add(Color::WHITE.into());

    // mesh
    let mesh = hexagonal_plane(&GRID_LAYOUT);
    let mesh_handle = meshes.add(mesh);

    let entities: HashMap<_, _> = shapes::hexagon(Hex::ZERO, GRID_RADIUS)
        .map(|hex| {
            let pos = GRID_LAYOUT.hex_to_world_pos(hex);
            let id = commands
                .spawn(ColorMesh2dBundle {
                    transform: Transform::from_xyz(pos.x, pos.y, 0.0).with_scale(Vec3::splat(0.9)),
                    mesh: mesh_handle.clone().into(),
                    material: covered_material.clone(),
                    ..default()
                })
                .id();
            (hex, id)
        })
        .collect();

    // Add mines
    let mines: HashSet<_> = entities
        .keys()
        .enumerate()
        // todo: add random here
        .filter(|(index, _)| index % 6 == 0)
        .map(|(_index, hex)| *hex)
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
        .filter(|(hex, _number)| is_hex_within_grid(hex))
        .collect();

    // all hexes are covered by default
    let covered = entities.keys().cloned().collect();

    commands.insert_resource(HexGrid {
        entities,

        covered,
        numbers,
        mines,
        flagged: HashSet::new(),

        covered_material,
        uncovered_material,
        selected_material,
    });
}

fn handle_input(
    mut commands: Commands,
    buttons: Res<Input<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut prev_hex: Local<Hex>,
    mut grid: ResMut<HexGrid>,
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

    let curr_hex = GRID_LAYOUT.world_pos_to_hex(cursor_pos);

    if buttons.just_pressed(MouseButton::Right) && grid.covered.contains(&curr_hex) {
        let entity = grid.entities[&curr_hex];
        match grid.flagged.entry(curr_hex) {
            bevy::utils::hashbrown::hash_set::Entry::Occupied(occupied) => {
                commands.entity(entity).despawn_descendants();
                occupied.remove();
            }
            bevy::utils::hashbrown::hash_set::Entry::Vacant(vacant) => {
                commands.entity(entity).with_children(|parent| {
                    parent.spawn(textures.sign.clone());
                });
                vacant.insert();
            }
        }
    }

    // Core minesweeper logic
    if buttons.just_pressed(MouseButton::Left) && !grid.flagged.contains(&curr_hex) {
        if grid.covered.contains(&curr_hex) {
            let entity = grid.entities.get(&curr_hex).unwrap();

            if grid.mines.contains(&curr_hex) {
                // todo: explode!
                commands.entity(*entity).with_children(|parent| {
                    parent.spawn(textures.mine.clone());
                });
            } else if let Some(number) = grid.numbers.get(&curr_hex) {
                commands.entity(*entity).with_children(|parent| {
                    parent.spawn(textures.numbers[*number as usize].clone());
                });
            } else {
                // Flood fill algorithm, adjusted to the MineSweeper game logic
                let mut visited = HashSet::<Hex>::from([curr_hex]);

                // this buffer stores the current line of expansion of the flood fill
                let mut buffer = vec![curr_hex];
                while !buffer.is_empty() {
                    buffer = buffer
                        .into_iter()
                        // take neighbors
                        .flat_map(|hex| hex.ring(1))
                        // Simplified version of check that this hex is within our map
                        .filter(is_hex_within_grid)
                        // Contains+Insert in a single insert, which with the following check against
                        // `grid.with_numbers` implements the core game logic - we add adjusted numbers to the `visited`,
                        // but we expand only those neighbor who are not numbers
                        .filter(|neighbor| visited.insert(*neighbor))
                        // don't need to check against `with_mines` as mines are always surrounded by numbers
                        // so we just stop exporation on numbers
                        .filter(|neighbor| !grid.numbers.contains_key(neighbor))
                        .collect();
                }

                for hex in visited {
                    if !grid.flagged.contains(&hex) {
                        grid.covered.remove(&hex);
                        commands
                            .entity(grid.entities[&hex])
                            .insert(grid.uncovered_material.clone());
                        if let Some(number) = grid.numbers.get(&hex) {
                            commands
                                .entity(grid.entities[&hex])
                                .with_children(|parent| {
                                    parent.spawn(textures.numbers[*number as usize].clone());
                                });
                        }
                    }
                }
            }
            grid.covered.remove(&curr_hex);
        }
    }

    // Do nothing if selected hex didn't change
    if curr_hex == *prev_hex {
        return;
    }

    // Remove highlighting from the prev_hex
    if let Some(entity) = grid.entities.get(&*prev_hex) {
        let material = if grid.covered.contains(&*prev_hex) {
            grid.covered_material.clone()
        } else {
            grid.uncovered_material.clone()
        };
        commands.entity(*entity).insert(material);
    }

    *prev_hex = curr_hex;

    // Highlight current hex
    if let Some(entity) = grid.entities.get(&curr_hex) {
        commands
            .entity(*entity)
            .insert(grid.selected_material.clone());
    }
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
