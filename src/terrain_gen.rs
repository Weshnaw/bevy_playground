use bevy::{color::palettes::tailwind::{AMBER_800, GREEN_400}, prelude::*, render::mesh::VertexAttributeValues};
use light_consts::lux;
use noise::{BasicMulti, NoiseFn, Perlin};

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

    let mut terrain = Mesh::from(
        Plane3d::default()
            .mesh()
            .size(1000., 1000.)
            .subdivisions(200),
    );

    if let Some(VertexAttributeValues::Float32x3(pos)) =
        terrain.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        let noise = BasicMulti::<Perlin>::new(1624);
        let normalizer = 300.;
        let scalar = 70.;
        for pos in pos.iter_mut() {
            pos[1] =
                noise.get([pos[0] as f64 / normalizer, pos[2] as f64 / normalizer]) as f32 * scalar;
        }

        let colors: Vec<[f32; 4]> = pos.iter().map(|[_, g, _]| {
            let g = *g / scalar * 2.;

            if g > 0.8 {
                Color::LinearRgba(LinearRgba {
                    red: 20.,
                    green: 20.,
                    blue: 20.,
                    alpha: 1.,
                })
            } else if g > 0.3 {
                Color::from(AMBER_800)
            } else if g < -0.8 {
                Color::BLACK
            } else {
                Color::from(GREEN_400)
            }.to_linear().to_f32_array()
        }).collect();

        terrain.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

        terrain.compute_normals();
    }


    commands.spawn((
        Mesh3d(meshes.add(terrain)),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Terrain,
    ));
}
