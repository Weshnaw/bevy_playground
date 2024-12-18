use bevy::{a11y::AccessibilityPlugin, app::{PanicHandlerPlugin, TerminalCtrlCHandlerPlugin}, core_pipeline::CorePipelinePlugin, input::InputPlugin, log::LogPlugin, pbr::PbrPlugin, prelude::*, render::{settings::{RenderCreation, WgpuFeatures, WgpuSettings}, RenderPlugin}, sprite::SpritePlugin, text::TextPlugin, ui::UiPlugin, winit::{WakeUp, WinitPlugin}};
use bevy_playground::{camera::CameraPlugin, debug::DebugPlugin, terrain_gen::TerrainPlugin};

fn main() {
    App::new()
        .add_plugins(MinimalPlugins)
        .add_plugins(PanicHandlerPlugin::default())
        .add_plugins(LogPlugin::default())
        .add_plugins(TerminalCtrlCHandlerPlugin::default())
        .add_plugins(TransformPlugin::default())
        .add_plugins(InputPlugin::default())
        .add_plugins(WindowPlugin::default())
        .add_plugins(AccessibilityPlugin::default())
        .add_plugins(AssetPlugin::default())
        .add_plugins(WinitPlugin::<WakeUp>::default())
        .add_plugins(RenderPlugin {
            render_creation: RenderCreation::Automatic(WgpuSettings {
                features: WgpuFeatures::POLYGON_MODE_LINE,
                ..default()
            }),
            ..Default::default()
        })
        .add_plugins(ImagePlugin::default())
        .add_plugins(CorePipelinePlugin::default())
        .add_plugins(SpritePlugin ::default())
        .add_plugins(TextPlugin::default())
        .add_plugins(UiPlugin::default())
        .add_plugins(PbrPlugin::default())
        .add_plugins(DebugPlugin)
        .add_plugins(TerrainPlugin)
        .add_plugins(CameraPlugin)
        .run();
}
