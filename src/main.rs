//! Shows how to render a polygonal [`Mesh`], generated from a [`Quad`] primitive, in a 2D scene.

use bevy::{
    prelude::*,
    render::{mesh::Indices, render_resource::PrimitiveTopology},
    utils::HashMap,
    window::PrimaryWindow,
};
use hexx::{shapes, Hex, HexLayout, HexOrientation, PlaneMeshBuilder};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
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

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2dBundle::default());

    let layout = HexLayout {
        orientation: HexOrientation::Pointy,
        hex_size: Vec2::splat(11.0),
        ..default()
    };

    // materials
    let selected_material = materials.add(Color::RED.into());
    let default_material = materials.add(Color::WHITE.into());

    // mesh
    let mesh = hexagonal_plane(&layout);
    let mesh_handle = meshes.add(mesh);

    let entities = shapes::hexagon(Hex::ZERO, 20)
        .map(|hex| {
            println!("Hex: {hex:?}");
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

    commands.insert_resource(HexGrid {
        layout,
        entities,
        default_material,
        selected_material,
    });
}

fn handle_input(
    mut commands: Commands,
    // buttons: Res<Input<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut current: Local<Hex>,
    grid: ResMut<HexGrid>,
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

    // Do nothing if selected hex didn't change
    if hex_pos == *current {
        return;
    }

    if let Some(entity) = grid.entities.get(&*current).copied() {
        commands
            .entity(entity)
            .insert(grid.default_material.clone());
    };

    *current = hex_pos;

    if let Some(entity) = grid.entities.get(&*current).copied() {
        commands
            .entity(entity)
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
