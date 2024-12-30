use bevy::{prelude::*, utils::HashSet};
use uuid::Uuid;

// TODO: figure out how to allow for some startup tasks to happen async, so they don't stall the main thread

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<LoadingState>();
        app.init_resource::<LoadingResource>();
        app.add_systems(Update, check_loading_status);
        app.add_systems(OnEnter(LoadingState::AssetsLoading), render_description);
        app.add_systems(
            OnEnter(LoadingState::LoadingComplete),
            cleanup_loading_render,
        );
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub(crate) enum LoadingState {
    #[default]
    AssetsLoading,
    LoadingComplete,
}

#[derive(Debug, Default, Resource)]
pub(crate) struct LoadingResource {
    pub(crate) loading_steps: HashSet<Uuid>,
}

fn check_loading_status(
    loading_state: Res<LoadingResource>,
    mut next_state: ResMut<NextState<LoadingState>>,
) {
    if loading_state.is_changed() {
        if loading_state.loading_steps.is_empty() {
            next_state.set(LoadingState::LoadingComplete);
        } else {
            next_state.set(LoadingState::AssetsLoading);
        }
    }
}

#[derive(Component)]
struct LoadingScene;

fn render_description(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
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
}

fn cleanup_loading_render(
    mut commands: Commands,
    loading_entities: Query<Entity, With<LoadingScene>>,
) {
    info!("Tearing down loading camera");
    for entity in loading_entities.iter() {
        commands.entity(entity).despawn();
    }
}
