use bevy::prelude::*;
use iyes_perf_ui::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin);
        app.add_plugins(PerfUiPlugin);
        app.add_systems(Update, toggle.before(iyes_perf_ui::PerfUiSet::Setup));
    }
}

fn perf_ui_entries() -> impl Bundle {
    // (
    //     PerfUiRoot {
    //         display_labels: true,
    //         layout_horizontal: false,
    //         ..default()
    //     },
    //     PerfUiEntryFPS::default(),
    //     PerfUiEntryFPSWorst::default(),
    // )

    PerfUiAllEntries::default()
}

fn toggle(
    mut commands: Commands,
    q_root: Query<Entity, With<PerfUiRoot>>,
    kbd: Res<ButtonInput<KeyCode>>,
) {
    if kbd.just_pressed(KeyCode::F12) {
        if let Ok(e) = q_root.get_single() {
            // despawn the existing Perf UI
            commands.entity(e).despawn_recursive();
        } else {
            // create a simple Perf UI with default settings
            // and all entries provided by the crate:
            commands.spawn(perf_ui_entries());
        }
    }
}