use bevy::prelude::*;

pub mod assets;
pub mod debug;
pub mod terrain_gen;
pub mod camera;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub(crate) enum ApplicationStates {
    #[default]
    AssetLoading,
    LoadingComplete,
}
