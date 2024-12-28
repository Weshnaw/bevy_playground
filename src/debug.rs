use bevy::{
    diagnostic::{
        Diagnostic, DiagnosticsStore, FrameTimeDiagnosticsPlugin,
        SystemInformationDiagnosticsPlugin,
    },
    pbr::wireframe::{Wireframe, WireframePlugin},
    prelude::*,
};
use bevy_egui::{
    EguiContexts, EguiPlugin,
    egui::{self, Ui},
};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        // Keyboard toggle checks
        app.add_systems(Update, toggle_debug);
        app.add_systems(Update, toggle_wireframe);

        // Allow wireframe view
        app.add_plugins(WireframePlugin);
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin);
        app.add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin);

        // egui
        app.init_resource::<EGUIState>();
        app.add_plugins(EguiPlugin);
        app.add_systems(Update, egui_debug.after(toggle_debug));
    }
}

#[derive(Default, Resource)]
struct EGUIState {
    open: bool,
}
fn egui_debug(
    mut contexts: EguiContexts,
    ui_state: Res<EGUIState>,
    diagnostics: Res<DiagnosticsStore>,
) {
    if ui_state.open {
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
        });
    }
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
    mut ui_state: ResMut<EGUIState>,
) {
    if kbd.just_pressed(KeyCode::F12) {
        info!("Toggling DEBUG");
        ui_state.open = !ui_state.open;
    }
}

#[derive(Component)]
pub(crate) struct WireframeObject;

fn toggle_wireframe(
    mut commands: Commands,
    obj_wireframes: Query<Entity, (With<WireframeObject>, With<Wireframe>)>,
    objs: Query<Entity, (With<WireframeObject>, Without<Wireframe>)>,
    kbd: Res<ButtonInput<KeyCode>>,
) {
    if kbd.just_pressed(KeyCode::F11) {
        for obj in &objs {
            commands.entity(obj).insert(Wireframe);
        }
        for obj in &obj_wireframes {
            commands.entity(obj).remove::<Wireframe>();
        }
    }
}
