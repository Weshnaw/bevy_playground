use bevy::{
    color::palettes::tailwind::{AMBER_800, GREEN_400},
    prelude::*,
    render::mesh::VertexAttributeValues,
    utils::HashMap,
};
use itertools::Itertools;
use light_consts::lux;
use noise::{BasicMulti, NoiseFn, Perlin};
use uuid::Uuid;

use crate::{camera::Player, debug::WireframeObject, loading::LoadingResource};

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TerrainStore::default());
        app.add_systems(Startup, setup_terrain);
        app.add_systems(Update, manage_chunks);
    }
}

#[derive(Component)]
pub(crate) struct Terrain;

fn setup_terrain(mut commands: Commands, mut loading_state: ResMut<LoadingResource>) {
    let uuid = Uuid::new_v4();
    loading_state.loading_steps.insert(uuid);
    commands.spawn((
        DirectionalLight {
            illuminance: lux::OVERCAST_DAY,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0., 20., 75.).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
    ));

    info!("Generating initial grid");
    for (x, y) in (-1..=1).cartesian_product(-1..=1) {
        debug!(?x, ?y);
        commands.queue(SpawnTerrain(IVec2::new(x, y)));
    }
    loading_state.loading_steps.remove(&uuid);
}

fn manage_chunks(
    mut commands: Commands,
    mut current_chunk: Local<IVec2>,
    player: Query<&Transform, With<Player>>,
    mut terrain_store: ResMut<TerrainStore>,
    terrain_entities: Query<(Entity, &Mesh3d), With<Terrain>>,
) {
    let size = 1000.;

    let Ok(transform) = player.get_single() else {
        warn!("No player!");
        return;
    };

    let xz = (transform.translation.xz() / size).trunc().as_ivec2();

    if *current_chunk != xz {
        *current_chunk = xz;
        let chunks_to_render: Vec<_> = (-1..=1)
            .cartesian_product(-1..=1)
            .map(|(x, y)| *current_chunk + IVec2::new(x, y))
            .collect();

        let chunks_to_despawn: Vec<_> = terrain_store
            .0
            .extract_if(|k, _| !chunks_to_render.contains(k))
            .collect();

        for (chunk, handle) in chunks_to_despawn {
            let Some((e, _)) = terrain_entities.iter().find(|(_, mesh)| handle == ***mesh) else {
                continue;
            };

            commands.entity(e).despawn_recursive();
            terrain_store.0.remove(&chunk);
        }

        for chunk in chunks_to_render {
            commands.queue(SpawnTerrain(chunk));
        }
    }
}

#[derive(Resource, Default)]
struct TerrainStore(HashMap<IVec2, Handle<Mesh>>);

struct SpawnTerrain(IVec2);

impl Command for SpawnTerrain {
    fn apply(self, world: &mut World) {
        let noise = BasicMulti::<Perlin>::new(1624);
        let normalizer = 300.;
        let scalar = 70.;
        let sub_div = 200;
        let size = 1000.;

        if world
            .get_resource_mut::<TerrainStore>()
            .expect("TerrainStore unavailable")
            .0
            .get(&self.0)
            .is_some()
        {
            debug!("Mesh already exists");
            return;
        }

        let mut terrain = Mesh::from(
            Plane3d::default()
                .mesh()
                .size(size, size)
                .subdivisions(sub_div),
        );

        if let Some(VertexAttributeValues::Float32x3(pos)) =
            terrain.attribute_mut(Mesh::ATTRIBUTE_POSITION)
        {
            for pos in pos.iter_mut() {
                let x = pos[0] as f64 + (size as f64 * self.0.x as f64);
                let y = pos[2] as f64 + (size as f64 * self.0.y as f64);

                pos[1] = noise.get([x / normalizer, y / normalizer]) as f32 * scalar;
            }

            let colors: Vec<[f32; 4]> = pos
                .iter()
                .map(|[_, g, _]| {
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
                    }
                    .to_linear()
                    .to_f32_array()
                })
                .collect();

            terrain.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

            terrain.compute_normals();
        }

        let mesh = world
            .get_resource_mut::<Assets<Mesh>>()
            .expect("Mesh unavailable")
            .add(terrain);
        let material = world
            .get_resource_mut::<Assets<StandardMaterial>>()
            .expect("StandardMaterial unavailable")
            .add(Color::WHITE);

        world
            .get_resource_mut::<TerrainStore>()
            .expect("TerrainStore unavailable")
            .0
            .insert(self.0, mesh.clone());

        world.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_xyz(self.0.x as f32 * size, 0., self.0.y as f32 * size),
            Terrain,
            WireframeObject,
        ));
    }
}

#[cfg(test)]
mod test {
    use bevy::state::app::StatesPlugin;

    use super::*;

    fn minimal_app() -> App {
        let mut app = App::new();

        app.add_plugins((
            MinimalPlugins,
            StatesPlugin::default(),
            AssetPlugin::default(),
        ));

        app.insert_resource(Assets::<Mesh>::default());
        app.insert_resource(Assets::<StandardMaterial>::default());

        app
    }

    #[test]
    fn generate_terrain() {
        let mut app = minimal_app();

        app.add_plugins(TerrainPlugin);

        assert!(app.world().get_resource::<TerrainStore>().is_some());
        assert_eq!(
            app.world().get_resource::<TerrainStore>().unwrap().0.len(),
            0
        );

        app.update();

        assert!(app.world().get_resource::<TerrainStore>().is_some());
        assert_eq!(
            app.world().get_resource::<TerrainStore>().unwrap().0.len(),
            9
        );
    }
}
