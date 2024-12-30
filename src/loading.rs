use bevy::{
    ecs::world::CommandQueue,
    prelude::*,
    tasks::{Task, block_on, futures_lite::future},
};

// TODO: figure out how to allow for some startup tasks to happen async, so they don't stall the main thread

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingState>();
        app.add_systems(Update, handle_loading_tasks);
        app.add_systems(OnEnter(LoadingState::AssetsLoading), loading_scene);
        app.add_systems(
            OnEnter(LoadingState::LoadingComplete),
            cleanup_loading_scene,
        );
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub(crate) enum LoadingState {
    #[default]
    AssetsLoading,
    LoadingComplete,
}

#[derive(Component)]
pub(crate) struct LoadingTaskQueue(pub(crate) Task<CommandQueue>);
fn handle_loading_tasks(
    mut commands: Commands,
    state: Res<State<LoadingState>>,
    mut next_state: ResMut<NextState<LoadingState>>,
    mut transform_tasks: Query<&mut LoadingTaskQueue>,
) {
    if !transform_tasks.is_empty() {
        info!("New loading tasks!");
        next_state.set(LoadingState::AssetsLoading);
        for mut task in &mut transform_tasks {
            if let Some(mut commands_queue) = block_on(future::poll_once(&mut task.0)) {
                // append the returned command queue to have it execute later
                commands.append(&mut commands_queue);
            }
        }
    } else if state.get() != &LoadingState::LoadingComplete {
        info!("Loading Completed");
        next_state.set(LoadingState::LoadingComplete);
    }
}

#[derive(Component)]
pub(crate) struct LoadingScene;

pub(crate) fn loading_scene(mut commands: Commands) {
    info!("Setting up loading camera");
    commands.spawn((
        Camera2d,
        Camera {
            order: 500,
            ..Default::default()
        },
        LoadingScene,
    ));
    commands.spawn((
        Text::new(
            r#"
    See the console for progress output
    
    This window will close when progress completes..."#,
        ),
        LoadingScene,
    ));

    // TODO: FIX: Not entirely sure why but this screen doesn't show up during startup
    // I'm assuming it's due to the cube mesh tasks taking up all the resources
    // so maybe it will go away once GPU compute is online
}

pub(crate) fn cleanup_loading_scene(
    mut commands: Commands,
    loading_entities: Query<Entity, With<LoadingScene>>,
) {
    info!("Tearing down loading camera");
    for entity in loading_entities.iter() {
        commands.entity(entity).despawn();
    }
}
