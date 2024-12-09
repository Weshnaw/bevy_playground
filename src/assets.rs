use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, prelude::*
};
use bevy_asset_loader::prelude::*;
use iyes_progress::{Progress, ProgressPlugin, ProgressReturningSystem, ProgressTracker};

use crate::MyStates;
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            ProgressPlugin::<MyStates>::new()
                .with_state_transition(MyStates::AssetLoading, MyStates::Next),
        );
        app.init_state::<MyStates>();
        app.add_loading_state(
            LoadingState::new(MyStates::AssetLoading)
                .load_collection::<LevelAssets>()
                .load_collection::<AudioAssets>()
                .load_collection::<PlayerAssets>(),
        );
        app.add_systems(OnEnter(MyStates::AssetLoading), render_description);
        app.add_systems(OnEnter(MyStates::Next), cleanup_loading_render);
        app.add_systems(
            Update,
            (
                track_fake_long_task.track_progress::<MyStates>(),
                print_progress,
            )
                .chain()
                .run_if(in_state(MyStates::AssetLoading))
                .after(LoadingStateSet(MyStates::AssetLoading)),
        );
    }
}
#[derive(AssetCollection, Resource)]
struct AudioAssets {
    // #[asset(path = "audio/background.ogg")]
    // background: Handle<AudioSource>,
}

#[derive(AssetCollection, Resource)]
pub struct LevelAssets {
    #[asset(path = "level_01.glb#Scene0")]
    pub level_01: Handle<Scene>,
}

#[derive(AssetCollection, Resource)]
pub struct PlayerAssets {
    #[asset(path = "level_01.glb#Scene1")]
    pub player: Handle<Scene>,
}


#[derive(Component)]
struct LoadingRender;

fn render_description(mut commands: Commands) {
    commands.spawn((Camera2d, LoadingRender));
    commands.spawn((Text::new(
        r#"
    See the console for progress output
    
    This window will close when progress completes..."#,
    ), LoadingRender));
}

fn cleanup_loading_render(mut commands: Commands, loading_entities: Query<Entity, With<LoadingRender>>) {
    info!("Tearing down loading camera");
    for entity in loading_entities.iter() {
        commands.entity(entity).despawn();
    }
}

// Time in seconds to complete a custom long-running task.
// If assets are loaded earlier, the current state will not
// be changed until the 'fake long task' is completed (thanks to 'iyes_progress')
const DURATION_LONG_TASK_IN_SECS: f64 = 1.0;

fn track_fake_long_task(time: Res<Time>) -> Progress {
    if time.elapsed_secs_f64() > DURATION_LONG_TASK_IN_SECS {
        debug!("Long fake task is completed");
        true.into()
    } else {
        false.into()
    }
}
fn print_progress(
    progress: Res<ProgressTracker<MyStates>>,
    diagnostics: Res<DiagnosticsStore>,
    mut last_done: Local<u32>,
) {
    let progress = progress.get_global_progress();
    if progress.done > *last_done {
        *last_done = progress.done;
        info!(
            "[Frame {}] Changed progress: {:?}",
            diagnostics
                .get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT)
                .map(|diagnostic| diagnostic.value().unwrap_or(0.))
                .unwrap_or(0.),
            progress
        );
    }
}
