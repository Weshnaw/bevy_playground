use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use bevy_asset_loader::prelude::*;
use iyes_progress::{ProgressPlugin, ProgressTracker};

use crate::ApplicationStates;
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            ProgressPlugin::<ApplicationStates>::new().with_state_transition(
                ApplicationStates::AssetLoading,
                ApplicationStates::LoadingComplete,
            ),
        );
        app.init_state::<ApplicationStates>();
        app.add_loading_state(
            LoadingState::new(ApplicationStates::AssetLoading)
                .load_collection::<LevelAssets>()
                .load_collection::<AudioAssets>()
                .load_collection::<PlayerAssets>(),
        );
        app.add_systems(OnEnter(ApplicationStates::AssetLoading), render_description);
        app.add_systems(
            OnEnter(ApplicationStates::LoadingComplete),
            cleanup_loading_render,
        );
        app.add_systems(
            Update,
            print_progress
                .run_if(in_state(ApplicationStates::AssetLoading))
                .after(LoadingStateSet(ApplicationStates::AssetLoading)),
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
    commands.spawn((
        Text::new(
            r#"
    See the console for progress output
    
    This window will close when progress completes..."#,
        ),
        LoadingRender,
    ));
}

fn cleanup_loading_render(
    mut commands: Commands,
    loading_entities: Query<Entity, With<LoadingRender>>,
) {
    info!("Tearing down loading camera");
    for entity in loading_entities.iter() {
        commands.entity(entity).despawn();
    }
}

fn print_progress(
    progress: Res<ProgressTracker<ApplicationStates>>,
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
