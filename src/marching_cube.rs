// use std::collections::BTreeMap;

use crate::{
    debug::WireframeObject,
    loading::LoadingTaskQueue,
};
use bevy::{
    asset::RenderAssetUsages,
    ecs::world::CommandQueue,
    math::vec3,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
    tasks::AsyncComputeTaskPool,
};
use itertools::Itertools;
use light_consts::lux;
use noise::{BasicMulti, NoiseFn, Perlin};

pub struct MarchingCubePlugin;

impl Plugin for MarchingCubePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_marching_cube_task);
    }
}
#[derive(Component)]
pub(crate) struct MarchedCube;

// TODO:
// [ ] - Compute Shader
// [ ] - ??? Minimimize uneeded triangles:
//   - greedy meshing: https://0fps.net/2012/06/30/meshing-in-a-minecraft-game/
//     - https://vodacek.zvb.cz/archiv/339.html
fn setup_marching_cube_task(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: lux::FULL_DAYLIGHT,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0., 20., 75.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
    ));

    let noise = BasicMulti::<Perlin>::new(1624);
    let thread_pool = AsyncComputeTaskPool::get();
    for (x, z) in (-1..=1).cartesian_product(-1..=1) {
        info!(?x, ?z);
        let noise = noise.clone();
        let entity = commands.spawn_empty().id();
        let task = thread_pool.spawn(async move {
            let chunk = TerrainChunk {
                coords: (x, 0, z),
                size: 100,
                noise,
            };
            let mut command_queue = CommandQueue::default();
            command_queue.push(move |world: &mut World| {
                let mesh = world
                    .get_resource_mut::<Assets<Mesh>>()
                    .expect("Mesh unavailable")
                    .add(chunk.mesh());
                let material = world
                    .get_resource_mut::<Assets<StandardMaterial>>()
                    .expect("StandardMaterial unavailable")
                    .add(Color::WHITE);

                world
                    .entity_mut(entity)
                    .insert((
                        MarchedCube,
                        Mesh3d(mesh),
                        MeshMaterial3d(material),
                        WireframeObject,
                        Transform::from_xyz((x * 100) as f32, 0., (z * 100) as f32),
                    ))
                    .remove::<LoadingTaskQueue>();
            });

            command_queue
        });
        commands.entity(entity).insert(LoadingTaskQueue(task));
    }
}

pub struct TerrainChunk {
    coords: (i32, i32, i32),
    size: u32,
    noise: BasicMulti<Perlin>,
}

impl TerrainChunk {
    fn value_from_noise(&self, translation: Vec3) -> f32 {
        let height = (self
            .noise
            .get([translation.x as f64 / 25., translation.z as f64 / 25.])
            * 10.)
            + 5.;

        if translation.y <= (height.floor() as f32).max(0.) {
            0.
        } else if translation.y >= height.ceil() as f32 {
            1.
        } else {
            height.fract() as f32
        }
    }

    fn mesh(&self) -> Mesh {
        let chunk_size = self.size;
        let iso_level = 0.7;

        let offset = Vec3::new(
            self.coords.0 as f32 * self.size as f32,
            self.coords.1 as f32 * self.size as f32,
            self.coords.2 as f32 * self.size as f32,
        );

        let triangles = (0..chunk_size)
            .flat_map(|x| {
                (0..chunk_size).flat_map(move |y| {
                    (0..chunk_size).flat_map(move |z| {
                        let translation = Vec3::new(x as f32, y as f32, z as f32);
                        let cell_points = OFFSET_TABLE.map(|point| {
                            let point = translation + point;
                            (point, self.value_from_noise(point + offset))
                        });

                        calculate_single_cube_polygons(&cell_points, iso_level)
                    })
                })
            })
            .collect_vec();

        // TODO: FIX: this duplicates vertices for EVERY triangle
        let vertices = triangles
            .iter()
            .flat_map(|triangle| [triangle.a, triangle.b, triangle.c])
            .map(|vector| [vector.x, vector.y, vector.z])
            .collect_vec();
        let indices = (0..vertices.len()).map(|index| index as u32).collect_vec();
        let uvs = (0..vertices.len()).map(|_| [0.0, 0.0]).collect_vec();
        let normals: Vec<[f32; 3]> = indices
            .chunks(3)
            .flat_map(|triangle| {
                let a = Vec3::from(vertices[(triangle)[0] as usize]);
                let b = Vec3::from(vertices[(triangle)[1] as usize]);
                let c = Vec3::from(vertices[(triangle)[2] as usize]);

                let normal = (b - a).cross(c - a).normalize();

                [normal.into(), normal.into(), normal.into()]
            })
            .collect();

        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
    }
}

#[derive(Debug)]
pub struct Triangle {
    pub a: Vec3,
    pub b: Vec3,
    pub c: Vec3,
}

fn calculate_single_cube_polygons(point_map: &[(Vec3, f32); 8], iso: f32) -> Vec<Triangle> {
    let mut idx: usize = 0;
    for (i, (_, point)) in point_map.iter().enumerate() {
        if point < &iso {
            idx |= 1 << i;
        }
    }

    let edge = EDGE_LOOKUP[idx];

    if edge == 0 {
        return vec![];
    }

    let mut vertex_list: [Option<Vec3>; 12] = [None; 12];
    for (v_id, (a, b)) in INTERPOLATION_TABLE.iter().enumerate() {
        if edge & (1 << v_id) > 0 {
            vertex_list[v_id] = Some(interpolate(iso, &point_map[*a], &point_map[*b]))
        }
    }

    TRIANGULATION_LOOKUP[idx]
        .chunks(3)
        .map(|chunk| {
            let a = vertex_list[chunk[0] as usize].expect("Missing interpolated vertex for a");
            let b = vertex_list[chunk[1] as usize].expect("Missing interpolated vertex for b");
            let c = vertex_list[chunk[2] as usize].expect("Missing interpolated vertex for c");

            Triangle { a, b, c }
        })
        .collect_vec()
}

fn interpolate(_iso: f32, a: &(Vec3, f32), b: &(Vec3, f32)) -> Vec3 {
    (a.0 + b.0) / vec3(2., 2., 2.)
}

#[cfg(test)]
mod test {
    use bevy::{log::LogPlugin, state::app::StatesPlugin};

    use crate::loading::LoadingPlugin;

    use super::*;

    #[rstest::fixture]
    fn app() -> App {
        let mut app = App::new();

        app.add_plugins((
            MinimalPlugins,
            StatesPlugin::default(),
            AssetPlugin::default(),
            LogPlugin::default(),
            LoadingPlugin,
        ));

        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());

        app.add_plugins(MarchingCubePlugin);

        app
    }

    #[rstest::rstest]
    #[test]
    fn test_cube(mut app: App) {
        app.update();

        // TODO: something to actually test
    }
}

const TRIANGULATION_LOOKUP: [&[u8]; 256] = [
    &[],
    &[0, 8, 3],
    &[0, 1, 9],
    &[1, 8, 3, 9, 8, 1],
    &[1, 2, 10],
    &[0, 8, 3, 1, 2, 10],
    &[9, 2, 10, 0, 2, 9],
    &[2, 8, 3, 2, 10, 8, 10, 9, 8],
    &[3, 11, 2],
    &[0, 11, 2, 8, 11, 0],
    &[1, 9, 0, 2, 3, 11],
    &[1, 11, 2, 1, 9, 11, 9, 8, 11],
    &[3, 10, 1, 11, 10, 3],
    &[0, 10, 1, 0, 8, 10, 8, 11, 10],
    &[3, 9, 0, 3, 11, 9, 11, 10, 9],
    &[9, 8, 10, 10, 8, 11],
    &[4, 7, 8],
    &[4, 3, 0, 7, 3, 4],
    &[0, 1, 9, 8, 4, 7],
    &[4, 1, 9, 4, 7, 1, 7, 3, 1],
    &[1, 2, 10, 8, 4, 7],
    &[3, 4, 7, 3, 0, 4, 1, 2, 10],
    &[9, 2, 10, 9, 0, 2, 8, 4, 7],
    &[2, 10, 9, 2, 9, 7, 2, 7, 3, 7, 9, 4],
    &[8, 4, 7, 3, 11, 2],
    &[11, 4, 7, 11, 2, 4, 2, 0, 4],
    &[9, 0, 1, 8, 4, 7, 2, 3, 11],
    &[4, 7, 11, 9, 4, 11, 9, 11, 2, 9, 2, 1],
    &[3, 10, 1, 3, 11, 10, 7, 8, 4],
    &[1, 11, 10, 1, 4, 11, 1, 0, 4, 7, 11, 4],
    &[4, 7, 8, 9, 0, 11, 9, 11, 10, 11, 0, 3],
    &[4, 7, 11, 4, 11, 9, 9, 11, 10],
    &[9, 5, 4],
    &[9, 5, 4, 0, 8, 3],
    &[0, 5, 4, 1, 5, 0],
    &[8, 5, 4, 8, 3, 5, 3, 1, 5],
    &[1, 2, 10, 9, 5, 4],
    &[3, 0, 8, 1, 2, 10, 4, 9, 5],
    &[5, 2, 10, 5, 4, 2, 4, 0, 2],
    &[2, 10, 5, 3, 2, 5, 3, 5, 4, 3, 4, 8],
    &[9, 5, 4, 2, 3, 11],
    &[0, 11, 2, 0, 8, 11, 4, 9, 5],
    &[0, 5, 4, 0, 1, 5, 2, 3, 11],
    &[2, 1, 5, 2, 5, 8, 2, 8, 11, 4, 8, 5],
    &[10, 3, 11, 10, 1, 3, 9, 5, 4],
    &[4, 9, 5, 0, 8, 1, 8, 10, 1, 8, 11, 10],
    &[5, 4, 0, 5, 0, 11, 5, 11, 10, 11, 0, 3],
    &[5, 4, 8, 5, 8, 10, 10, 8, 11],
    &[9, 7, 8, 5, 7, 9],
    &[9, 3, 0, 9, 5, 3, 5, 7, 3],
    &[0, 7, 8, 0, 1, 7, 1, 5, 7],
    &[1, 5, 3, 3, 5, 7],
    &[9, 7, 8, 9, 5, 7, 10, 1, 2],
    &[10, 1, 2, 9, 5, 0, 5, 3, 0, 5, 7, 3],
    &[8, 0, 2, 8, 2, 5, 8, 5, 7, 10, 5, 2],
    &[2, 10, 5, 2, 5, 3, 3, 5, 7],
    &[7, 9, 5, 7, 8, 9, 3, 11, 2],
    &[9, 5, 7, 9, 7, 2, 9, 2, 0, 2, 7, 11],
    &[2, 3, 11, 0, 1, 8, 1, 7, 8, 1, 5, 7],
    &[11, 2, 1, 11, 1, 7, 7, 1, 5],
    &[9, 5, 8, 8, 5, 7, 10, 1, 3, 10, 3, 11],
    &[5, 7, 0, 5, 0, 9, 7, 11, 0, 1, 0, 10, 11, 10, 0],
    &[11, 10, 0, 11, 0, 3, 10, 5, 0, 8, 0, 7, 5, 7, 0],
    &[11, 10, 5, 7, 11, 5],
    &[10, 6, 5],
    &[0, 8, 3, 5, 10, 6],
    &[9, 0, 1, 5, 10, 6],
    &[1, 8, 3, 1, 9, 8, 5, 10, 6],
    &[1, 6, 5, 2, 6, 1],
    &[1, 6, 5, 1, 2, 6, 3, 0, 8],
    &[9, 6, 5, 9, 0, 6, 0, 2, 6],
    &[5, 9, 8, 5, 8, 2, 5, 2, 6, 3, 2, 8],
    &[2, 3, 11, 10, 6, 5],
    &[11, 0, 8, 11, 2, 0, 10, 6, 5],
    &[0, 1, 9, 2, 3, 11, 5, 10, 6],
    &[5, 10, 6, 1, 9, 2, 9, 11, 2, 9, 8, 11],
    &[6, 3, 11, 6, 5, 3, 5, 1, 3],
    &[0, 8, 11, 0, 11, 5, 0, 5, 1, 5, 11, 6],
    &[3, 11, 6, 0, 3, 6, 0, 6, 5, 0, 5, 9],
    &[6, 5, 9, 6, 9, 11, 11, 9, 8],
    &[5, 10, 6, 4, 7, 8],
    &[4, 3, 0, 4, 7, 3, 6, 5, 10],
    &[1, 9, 0, 5, 10, 6, 8, 4, 7],
    &[10, 6, 5, 1, 9, 7, 1, 7, 3, 7, 9, 4],
    &[6, 1, 2, 6, 5, 1, 4, 7, 8],
    &[1, 2, 5, 5, 2, 6, 3, 0, 4, 3, 4, 7],
    &[8, 4, 7, 9, 0, 5, 0, 6, 5, 0, 2, 6],
    &[7, 3, 9, 7, 9, 4, 3, 2, 9, 5, 9, 6, 2, 6, 9],
    &[3, 11, 2, 7, 8, 4, 10, 6, 5],
    &[5, 10, 6, 4, 7, 2, 4, 2, 0, 2, 7, 11],
    &[0, 1, 9, 4, 7, 8, 2, 3, 11, 5, 10, 6],
    &[9, 2, 1, 9, 11, 2, 9, 4, 11, 7, 11, 4, 5, 10, 6],
    &[8, 4, 7, 3, 11, 5, 3, 5, 1, 5, 11, 6],
    &[5, 1, 11, 5, 11, 6, 1, 0, 11, 7, 11, 4, 0, 4, 11],
    &[0, 5, 9, 0, 6, 5, 0, 3, 6, 11, 6, 3, 8, 4, 7],
    &[6, 5, 9, 6, 9, 11, 4, 7, 9, 7, 11, 9],
    &[10, 4, 9, 6, 4, 10],
    &[4, 10, 6, 4, 9, 10, 0, 8, 3],
    &[10, 0, 1, 10, 6, 0, 6, 4, 0],
    &[8, 3, 1, 8, 1, 6, 8, 6, 4, 6, 1, 10],
    &[1, 4, 9, 1, 2, 4, 2, 6, 4],
    &[3, 0, 8, 1, 2, 9, 2, 4, 9, 2, 6, 4],
    &[0, 2, 4, 4, 2, 6],
    &[8, 3, 2, 8, 2, 4, 4, 2, 6],
    &[10, 4, 9, 10, 6, 4, 11, 2, 3],
    &[0, 8, 2, 2, 8, 11, 4, 9, 10, 4, 10, 6],
    &[3, 11, 2, 0, 1, 6, 0, 6, 4, 6, 1, 10],
    &[6, 4, 1, 6, 1, 10, 4, 8, 1, 2, 1, 11, 8, 11, 1],
    &[9, 6, 4, 9, 3, 6, 9, 1, 3, 11, 6, 3],
    &[8, 11, 1, 8, 1, 0, 11, 6, 1, 9, 1, 4, 6, 4, 1],
    &[3, 11, 6, 3, 6, 0, 0, 6, 4],
    &[6, 4, 8, 11, 6, 8],
    &[7, 10, 6, 7, 8, 10, 8, 9, 10],
    &[0, 7, 3, 0, 10, 7, 0, 9, 10, 6, 7, 10],
    &[10, 6, 7, 1, 10, 7, 1, 7, 8, 1, 8, 0],
    &[10, 6, 7, 10, 7, 1, 1, 7, 3],
    &[1, 2, 6, 1, 6, 8, 1, 8, 9, 8, 6, 7],
    &[2, 6, 9, 2, 9, 1, 6, 7, 9, 0, 9, 3, 7, 3, 9],
    &[7, 8, 0, 7, 0, 6, 6, 0, 2],
    &[7, 3, 2, 6, 7, 2],
    &[2, 3, 11, 10, 6, 8, 10, 8, 9, 8, 6, 7],
    &[2, 0, 7, 2, 7, 11, 0, 9, 7, 6, 7, 10, 9, 10, 7],
    &[1, 8, 0, 1, 7, 8, 1, 10, 7, 6, 7, 10, 2, 3, 11],
    &[11, 2, 1, 11, 1, 7, 10, 6, 1, 6, 7, 1],
    &[8, 9, 6, 8, 6, 7, 9, 1, 6, 11, 6, 3, 1, 3, 6],
    &[0, 9, 1, 11, 6, 7],
    &[7, 8, 0, 7, 0, 6, 3, 11, 0, 11, 6, 0],
    &[7, 11, 6],
    &[7, 6, 11],
    &[3, 0, 8, 11, 7, 6],
    &[0, 1, 9, 11, 7, 6],
    &[8, 1, 9, 8, 3, 1, 11, 7, 6],
    &[10, 1, 2, 6, 11, 7],
    &[1, 2, 10, 3, 0, 8, 6, 11, 7],
    &[2, 9, 0, 2, 10, 9, 6, 11, 7],
    &[6, 11, 7, 2, 10, 3, 10, 8, 3, 10, 9, 8],
    &[7, 2, 3, 6, 2, 7],
    &[7, 0, 8, 7, 6, 0, 6, 2, 0],
    &[2, 7, 6, 2, 3, 7, 0, 1, 9],
    &[1, 6, 2, 1, 8, 6, 1, 9, 8, 8, 7, 6],
    &[10, 7, 6, 10, 1, 7, 1, 3, 7],
    &[10, 7, 6, 1, 7, 10, 1, 8, 7, 1, 0, 8],
    &[0, 3, 7, 0, 7, 10, 0, 10, 9, 6, 10, 7],
    &[7, 6, 10, 7, 10, 8, 8, 10, 9],
    &[6, 8, 4, 11, 8, 6],
    &[3, 6, 11, 3, 0, 6, 0, 4, 6],
    &[8, 6, 11, 8, 4, 6, 9, 0, 1],
    &[9, 4, 6, 9, 6, 3, 9, 3, 1, 11, 3, 6],
    &[6, 8, 4, 6, 11, 8, 2, 10, 1],
    &[1, 2, 10, 3, 0, 11, 0, 6, 11, 0, 4, 6],
    &[4, 11, 8, 4, 6, 11, 0, 2, 9, 2, 10, 9],
    &[10, 9, 3, 10, 3, 2, 9, 4, 3, 11, 3, 6, 4, 6, 3],
    &[8, 2, 3, 8, 4, 2, 4, 6, 2],
    &[0, 4, 2, 4, 6, 2],
    &[1, 9, 0, 2, 3, 4, 2, 4, 6, 4, 3, 8],
    &[1, 9, 4, 1, 4, 2, 2, 4, 6],
    &[8, 1, 3, 8, 6, 1, 8, 4, 6, 6, 10, 1],
    &[10, 1, 0, 10, 0, 6, 6, 0, 4],
    &[4, 6, 3, 4, 3, 8, 6, 10, 3, 0, 3, 9, 10, 9, 3],
    &[10, 9, 4, 6, 10, 4],
    &[4, 9, 5, 7, 6, 11],
    &[0, 8, 3, 4, 9, 5, 11, 7, 6],
    &[5, 0, 1, 5, 4, 0, 7, 6, 11],
    &[11, 7, 6, 8, 3, 4, 3, 5, 4, 3, 1, 5],
    &[9, 5, 4, 10, 1, 2, 7, 6, 11],
    &[6, 11, 7, 1, 2, 10, 0, 8, 3, 4, 9, 5],
    &[7, 6, 11, 5, 4, 10, 4, 2, 10, 4, 0, 2],
    &[3, 4, 8, 3, 5, 4, 3, 2, 5, 10, 5, 2, 11, 7, 6],
    &[7, 2, 3, 7, 6, 2, 5, 4, 9],
    &[9, 5, 4, 0, 8, 6, 0, 6, 2, 6, 8, 7],
    &[3, 6, 2, 3, 7, 6, 1, 5, 0, 5, 4, 0],
    &[6, 2, 8, 6, 8, 7, 2, 1, 8, 4, 8, 5, 1, 5, 8],
    &[9, 5, 4, 10, 1, 6, 1, 7, 6, 1, 3, 7],
    &[1, 6, 10, 1, 7, 6, 1, 0, 7, 8, 7, 0, 9, 5, 4],
    &[4, 0, 10, 4, 10, 5, 0, 3, 10, 6, 10, 7, 3, 7, 10],
    &[7, 6, 10, 7, 10, 8, 5, 4, 10, 4, 8, 10],
    &[6, 9, 5, 6, 11, 9, 11, 8, 9],
    &[3, 6, 11, 0, 6, 3, 0, 5, 6, 0, 9, 5],
    &[0, 11, 8, 0, 5, 11, 0, 1, 5, 5, 6, 11],
    &[6, 11, 3, 6, 3, 5, 5, 3, 1],
    &[1, 2, 10, 9, 5, 11, 9, 11, 8, 11, 5, 6],
    &[0, 11, 3, 0, 6, 11, 0, 9, 6, 5, 6, 9, 1, 2, 10],
    &[11, 8, 5, 11, 5, 6, 8, 0, 5, 10, 5, 2, 0, 2, 5],
    &[6, 11, 3, 6, 3, 5, 2, 10, 3, 10, 5, 3],
    &[5, 8, 9, 5, 2, 8, 5, 6, 2, 3, 8, 2],
    &[9, 5, 6, 9, 6, 0, 0, 6, 2],
    &[1, 5, 8, 1, 8, 0, 5, 6, 8, 3, 8, 2, 6, 2, 8],
    &[1, 5, 6, 2, 1, 6],
    &[1, 3, 6, 1, 6, 10, 3, 8, 6, 5, 6, 9, 8, 9, 6],
    &[10, 1, 0, 10, 0, 6, 9, 5, 0, 5, 6, 0],
    &[0, 3, 8, 5, 6, 10],
    &[10, 5, 6],
    &[11, 5, 10, 7, 5, 11],
    &[11, 5, 10, 11, 7, 5, 8, 3, 0],
    &[5, 11, 7, 5, 10, 11, 1, 9, 0],
    &[10, 7, 5, 10, 11, 7, 9, 8, 1, 8, 3, 1],
    &[11, 1, 2, 11, 7, 1, 7, 5, 1],
    &[0, 8, 3, 1, 2, 7, 1, 7, 5, 7, 2, 11],
    &[9, 7, 5, 9, 2, 7, 9, 0, 2, 2, 11, 7],
    &[7, 5, 2, 7, 2, 11, 5, 9, 2, 3, 2, 8, 9, 8, 2],
    &[2, 5, 10, 2, 3, 5, 3, 7, 5],
    &[8, 2, 0, 8, 5, 2, 8, 7, 5, 10, 2, 5],
    &[9, 0, 1, 5, 10, 3, 5, 3, 7, 3, 10, 2],
    &[9, 8, 2, 9, 2, 1, 8, 7, 2, 10, 2, 5, 7, 5, 2],
    &[1, 3, 5, 3, 7, 5],
    &[0, 8, 7, 0, 7, 1, 1, 7, 5],
    &[9, 0, 3, 9, 3, 5, 5, 3, 7],
    &[9, 8, 7, 5, 9, 7],
    &[5, 8, 4, 5, 10, 8, 10, 11, 8],
    &[5, 0, 4, 5, 11, 0, 5, 10, 11, 11, 3, 0],
    &[0, 1, 9, 8, 4, 10, 8, 10, 11, 10, 4, 5],
    &[10, 11, 4, 10, 4, 5, 11, 3, 4, 9, 4, 1, 3, 1, 4],
    &[2, 5, 1, 2, 8, 5, 2, 11, 8, 4, 5, 8],
    &[0, 4, 11, 0, 11, 3, 4, 5, 11, 2, 11, 1, 5, 1, 11],
    &[0, 2, 5, 0, 5, 9, 2, 11, 5, 4, 5, 8, 11, 8, 5],
    &[9, 4, 5, 2, 11, 3],
    &[2, 5, 10, 3, 5, 2, 3, 4, 5, 3, 8, 4],
    &[5, 10, 2, 5, 2, 4, 4, 2, 0],
    &[3, 10, 2, 3, 5, 10, 3, 8, 5, 4, 5, 8, 0, 1, 9],
    &[5, 10, 2, 5, 2, 4, 1, 9, 2, 9, 4, 2],
    &[8, 4, 5, 8, 5, 3, 3, 5, 1],
    &[0, 4, 5, 1, 0, 5],
    &[8, 4, 5, 8, 5, 3, 9, 0, 5, 0, 3, 5],
    &[9, 4, 5],
    &[4, 11, 7, 4, 9, 11, 9, 10, 11],
    &[0, 8, 3, 4, 9, 7, 9, 11, 7, 9, 10, 11],
    &[1, 10, 11, 1, 11, 4, 1, 4, 0, 7, 4, 11],
    &[3, 1, 4, 3, 4, 8, 1, 10, 4, 7, 4, 11, 10, 11, 4],
    &[4, 11, 7, 9, 11, 4, 9, 2, 11, 9, 1, 2],
    &[9, 7, 4, 9, 11, 7, 9, 1, 11, 2, 11, 1, 0, 8, 3],
    &[11, 7, 4, 11, 4, 2, 2, 4, 0],
    &[11, 7, 4, 11, 4, 2, 8, 3, 4, 3, 2, 4],
    &[2, 9, 10, 2, 7, 9, 2, 3, 7, 7, 4, 9],
    &[9, 10, 7, 9, 7, 4, 10, 2, 7, 8, 7, 0, 2, 0, 7],
    &[3, 7, 10, 3, 10, 2, 7, 4, 10, 1, 10, 0, 4, 0, 10],
    &[1, 10, 2, 8, 7, 4],
    &[4, 9, 1, 4, 1, 7, 7, 1, 3],
    &[4, 9, 1, 4, 1, 7, 0, 8, 1, 8, 7, 1],
    &[4, 0, 3, 7, 4, 3],
    &[4, 8, 7],
    &[9, 10, 8, 10, 11, 8],
    &[3, 0, 9, 3, 9, 11, 11, 9, 10],
    &[0, 1, 10, 0, 10, 8, 8, 10, 11],
    &[3, 1, 10, 11, 3, 10],
    &[1, 2, 11, 1, 11, 9, 9, 11, 8],
    &[3, 0, 9, 3, 9, 11, 1, 2, 9, 2, 11, 9],
    &[0, 2, 11, 8, 0, 11],
    &[3, 2, 11],
    &[2, 3, 8, 2, 8, 10, 10, 8, 9],
    &[9, 10, 2, 0, 9, 2],
    &[2, 3, 8, 2, 8, 10, 0, 1, 8, 1, 10, 8],
    &[1, 10, 2],
    &[1, 3, 8, 9, 1, 8],
    &[0, 9, 1],
    &[0, 3, 8],
    &[],
];

const EDGE_LOOKUP: [u16; 256] = [
    0x0, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c, 0x80c, 0x905, 0xa0f, 0xb06, 0xc0a, 0xd03,
    0xe09, 0xf00, 0x190, 0x99, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c, 0x99c, 0x895, 0xb9f,
    0xa96, 0xd9a, 0xc93, 0xf99, 0xe90, 0x230, 0x339, 0x33, 0x13a, 0x636, 0x73f, 0x435, 0x53c,
    0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30, 0x3a0, 0x2a9, 0x1a3, 0xaa, 0x7a6,
    0x6af, 0x5a5, 0x4ac, 0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0, 0x460, 0x569,
    0x663, 0x76a, 0x66, 0x16f, 0x265, 0x36c, 0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963, 0xa69,
    0xb60, 0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0xff, 0x3f5, 0x2fc, 0xdfc, 0xcf5, 0xfff, 0xef6,
    0x9fa, 0x8f3, 0xbf9, 0xaf0, 0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x55, 0x15c, 0xe5c,
    0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950, 0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6, 0x2cf,
    0x1c5, 0xcc, 0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0, 0x8c0, 0x9c9, 0xac3,
    0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc, 0xcc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9, 0x7c0,
    0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c, 0x15c, 0x55, 0x35f, 0x256, 0x55a,
    0x453, 0x759, 0x650, 0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc, 0x2fc, 0x3f5,
    0xff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0, 0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f, 0xd65,
    0xc6c, 0x36c, 0x265, 0x16f, 0x66, 0x76a, 0x663, 0x569, 0x460, 0xca0, 0xda9, 0xea3, 0xfaa,
    0x8a6, 0x9af, 0xaa5, 0xbac, 0x4ac, 0x5a5, 0x6af, 0x7a6, 0xaa, 0x1a3, 0x2a9, 0x3a0, 0xd30,
    0xc39, 0xf33, 0xe3a, 0x936, 0x83f, 0xb35, 0xa3c, 0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x33,
    0x339, 0x230, 0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c, 0x69c, 0x795, 0x49f,
    0x596, 0x29a, 0x393, 0x99, 0x190, 0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x0,
];

const INTERPOLATION_TABLE: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

const OFFSET_TABLE: [Vec3; 8] = [
    vec3(-0.5, -0.5, -0.5),
    vec3(0.5, -0.5, -0.5),
    vec3(0.5, -0.5, 0.5),
    vec3(-0.5, -0.5, 0.5),
    vec3(-0.5, 0.5, -0.5),
    vec3(0.5, 0.5, -0.5),
    vec3(0.5, 0.5, 0.5),
    vec3(-0.5, 0.5, 0.5),
];
