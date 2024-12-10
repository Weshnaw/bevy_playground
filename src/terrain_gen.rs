use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use light_consts::lux;

use crate::ApplicationStates;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(ApplicationStates::LoadingComplete), setup_terrain);
    }
}

#[derive(Component)]
pub(crate) struct Terrain;

fn setup_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        DirectionalLight {
            illuminance: lux::OVERCAST_DAY,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0., 20., 75.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
    ));

    let mut terrain = Mesh::from(Plane3d::default().mesh().size(50., 50.).subdivisions(100));

    if let Some(VertexAttributeValues::Float32x3(pos)) =
        terrain.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for pos in pos.iter_mut() {
            pos[1] = (pos[0] + pos[2]).sin();
        }
    }

    commands.spawn((
        Mesh3d(meshes.add(terrain)),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Terrain,
    ));
}
