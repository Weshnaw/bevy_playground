use bevy::{
    diagnostic::{
        Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
    render::extract_resource::{ExtractResource, ExtractResourcePlugin},
};
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{self, Ui},
};

use crate::shader::CHUNK_SIZE;

// use crate::compute::CHUNK_SIZE;
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            WireframePlugin,
            bevy::diagnostic::FrameTimeDiagnosticsPlugin,
            bevy::diagnostic::EntityCountDiagnosticsPlugin,
            bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            EguiPlugin,
            ExtractResourcePlugin::<DebugResource>::default(),
        ));

        app.init_state::<DebugState>();
        app.init_state::<WireframeState>();
        app.init_resource::<DebugResource>();

        app.add_systems(
            Update,
            (
                toggle_debug,
                toggle_wireframe,
                egui_debug.run_if(in_state(DebugState::Open)),
            ),
        );
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum DebugState {
    Open,
    #[default]
    Closed,
}

#[derive(Default, Resource, Debug, Clone, ExtractResource)]
pub struct DebugResource {
    pub value: u32,
}

fn egui_debug(
    mut contexts: EguiContexts,
    mut debug: ResMut<DebugResource>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let fps = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS);
    let frame_count = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_COUNT);
    let frame_time = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME);
    let cpu = diagnostics.get(&SystemInformationDiagnosticsPlugin::CPU_USAGE);
    let mem = diagnostics.get(&SystemInformationDiagnosticsPlugin::MEM_USAGE);
    egui::Window::new("DEBUG").show(contexts.ctx_mut(), move |ui| {
        create_label("FPS: ", ui, fps, 2, Diagnostic::smoothed);
        create_label("MIN FPS: ", ui, fps, 2, |fps| {
            fps.values().cloned().reduce(|a, b| a.min(b))
        });
        create_label("Frame Count: ", ui, frame_count, 0, Diagnostic::smoothed);
        create_label("Avg. Frame Time: ", ui, frame_time, 2, Diagnostic::average);
        create_label("CPU Usage: ", ui, cpu, 2, Diagnostic::smoothed);
        create_label("Mem Usage: ", ui, mem, 2, Diagnostic::smoothed);

        let mut value: u32 = debug.value;
        ui.add(egui::Slider::new(&mut value, 0..=CHUNK_SIZE - 1).text("layer"));

        if value != debug.value {
            debug.value = value;
        }
    });
}

fn create_label(
    label: &str,
    ui: &mut Ui,
    diagnostic: Option<&Diagnostic>,
    precision: usize,
    calc: fn(&Diagnostic) -> Option<f64>,
) {
    if let Some(diagnostic) = diagnostic {
        let diagnostic = match calc(diagnostic) {
            Some(val) => format!("{:.1$}", val, &precision),
            None => "N/A".into(),
        };
        ui.horizontal(|ui| {
            ui.label(label);
            ui.label(diagnostic);
        });
    }
}

fn toggle_debug(
    // mut commands: Commands,
    kbd: Res<ButtonInput<KeyCode>>,
    state: Res<State<DebugState>>,
    mut next_state: ResMut<NextState<DebugState>>,
) {
    if kbd.just_pressed(KeyCode::F12) {
        info!("Toggling DEBUG");
        match state.get() {
            DebugState::Open => next_state.set(DebugState::Closed),
            DebugState::Closed => next_state.set(DebugState::Open),
        }
    }
}

#[derive(Component)]
pub(crate) struct WireframeObject;

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
enum WireframeState {
    On,
    #[default]
    Off,
}

fn toggle_wireframe(
    mut commands: Commands,
    objs: Query<Entity, With<WireframeObject>>,
    kbd: Res<ButtonInput<KeyCode>>,
    state: Res<State<WireframeState>>,
    mut next_state: ResMut<NextState<WireframeState>>,
) {
    if kbd.just_pressed(KeyCode::F11) {
        match state.get() {
            WireframeState::On => {
                // Should move obj logic to be seperate from the kbd / state change logic
                // so that we can do things like also allow for a egui button for wireframes
                for obj in &objs {
                    commands.entity(obj).remove::<Wireframe>();
                }
                next_state.set(WireframeState::Off)
            }
            WireframeState::Off => {
                for obj in &objs {
                    commands.entity(obj).insert(Wireframe);
                }
                next_state.set(WireframeState::On)
            }
        }
    }
}
