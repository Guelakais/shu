//! Gui (windows and panels) to upload data and hover.

use crate::data::{Data, ReactionState};
use crate::escher::{EscherMap, Hover, MapState};
use crate::geom::{AnyTag, Drag, HistTag, VisCondition, Xaxis};
use bevy::prelude::*;
use bevy_egui::egui::color_picker::{color_edit_button_rgba, Alpha};
use bevy_egui::egui::epaint::color::Rgba;
use bevy_egui::{egui, EguiContext, EguiPlugin};
use itertools::Itertools;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub struct GuiPlugin;

impl Plugin for GuiPlugin {
    fn build(&self, app: &mut App) {
        let building = app
            .add_plugin(EguiPlugin)
            .insert_resource(UiState::default())
            .add_event::<SaveEvent>()
            .add_system(ui_settings)
            .add_system(show_hover)
            .add_system(follow_mouse_on_drag)
            .add_system(follow_mouse_on_drag_ui)
            .add_system(follow_mouse_on_rotate)
            .add_system(scale_ui)
            .add_system(mouse_click_system)
            .add_system(mouse_click_ui_system);

        // file drop and file system does not work in WASM
        #[cfg(not(target_arch = "wasm32"))]
        building.add_system(file_drop).add_system(save_file);

        #[cfg(target_arch = "wasm32")]
        building
            .add_system(listen_js_escher)
            .add_system(listen_js_data);
    }
}

/// Retrieve a mutable reference to the color or insert the color
/// that is already in the map at the empty string.
pub fn or_color<'m>(key: &str, map: &'m mut HashMap<String, Rgba>) -> &'m mut Rgba {
    let def_color = *map.get("").unwrap();
    match map.entry(key.to_string()) {
        Entry::Occupied(v) => v.into_mut(),
        Entry::Vacant(v) => v.insert(def_color),
    }
}

/// Global appeareance settings.
#[derive(Resource)]
pub struct UiState {
    pub min_reaction: f32,
    pub max_reaction: f32,
    pub zero_white: bool,
    pub min_reaction_color: Rgba,
    pub max_reaction_color: Rgba,
    pub min_metabolite: f32,
    pub max_metabolite: f32,
    pub min_metabolite_color: Rgba,
    pub max_metabolite_color: Rgba,
    pub max_left: f32,
    pub max_right: f32,
    pub max_top: f32,
    pub color_left: HashMap<String, Rgba>,
    pub color_right: HashMap<String, Rgba>,
    pub color_top: HashMap<String, Rgba>,
    pub condition: String,
    pub conditions: Vec<String>,
    pub save_path: String,
    pub map_path: String,
    pub data_path: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            min_reaction_color: Rgba::from_srgba_unmultiplied(164, 191, 232, 255),
            max_reaction_color: Rgba::from_srgba_unmultiplied(42, 98, 183, 255),
            min_metabolite_color: Rgba::from_srgba_unmultiplied(183, 110, 42, 255),
            max_metabolite_color: Rgba::from_srgba_unmultiplied(186, 148, 113, 255),
            zero_white: false,
            min_reaction: 20.,
            max_reaction: 60.,
            min_metabolite: 20.,
            max_metabolite: 60.,
            max_left: 100.,
            max_right: 100.,
            max_top: 100.,
            color_left: {
                let mut color = HashMap::new();
                color.insert(
                    String::from(""),
                    Rgba::from_srgba_unmultiplied(218, 150, 135, 124),
                );
                color
            },
            color_right: {
                let mut color = HashMap::new();
                color.insert(
                    String::from(""),
                    Rgba::from_srgba_unmultiplied(125, 206, 96, 124),
                );
                color
            },
            color_top: {
                let mut color = HashMap::new();
                color.insert(
                    String::from(""),
                    Rgba::from_srgba_unmultiplied(161, 134, 216, 124),
                );
                color
            },
            condition: String::from(""),
            conditions: vec![String::from("")],
            save_path: String::from("this_map.json"),
            map_path: String::from("my_map.json"),
            data_path: String::from("my_data.metabolism.json"),
        }
    }
}

impl UiState {
    fn get_geom_params_mut(&mut self, extreme: &str, geom: &str) -> (&mut Rgba, &mut f32) {
        match (extreme, geom) {
            ("min", "Reaction") => (&mut self.min_reaction_color, &mut self.min_reaction),
            ("max", "Reaction") => (&mut self.max_reaction_color, &mut self.max_reaction),
            ("min", "Metabolite") => (&mut self.min_metabolite_color, &mut self.min_metabolite),
            ("max", "Metabolite") => (&mut self.max_metabolite_color, &mut self.max_metabolite),
            ("left", _) => (or_color(geom, &mut self.color_left), &mut self.max_left),
            ("right", _) => (or_color(geom, &mut self.color_right), &mut self.max_left),
            ("top", _) => (or_color(geom, &mut self.color_top), &mut self.max_left),
            _ => panic!("Unknown side"),
        }
    }

    fn get_mut_paths(&mut self, label: &str) -> &mut String {
        match label {
            "Map" => &mut self.map_path,
            "Data" => &mut self.data_path,
            _ => panic!("Unknown label"),
        }
    }
}

struct SaveEvent(String);

/// Settings for appearance of map and plots.
/// This is managed by [`bevy_egui`] and it is separate from the rest of the GUI.
fn ui_settings(
    windows: Res<Windows>,
    mut egui_context: ResMut<EguiContext>,
    mut state: ResMut<UiState>,
    mut save_events: EventWriter<SaveEvent>,
    mut load_events: EventWriter<FileDragAndDrop>,
) {
    egui::Window::new("Settings").show(egui_context.ctx_mut(), |ui| {
        for (geom, ext) in ["Reaction", "Metabolite"]
            .into_iter()
            .cartesian_product(["min", "max"])
        {
            if "min" == ext {
                ui.label(format!("{geom} scale"));
            }
            let (color, value) = state.get_geom_params_mut(ext, geom);
            ui.horizontal(|ui| {
                color_edit_button_rgba(ui, color, Alpha::Opaque);
                ui.add(egui::Slider::new(value, 5.0..=90.0).text(ext));
            });
        }

        ui.label("Histogram scale");
        let condition = state.condition.clone();
        for side in ["left", "right", "top"] {
            ui.horizontal(|ui| {
                let (color, value) = state.get_geom_params_mut(side, &condition);
                color_edit_button_rgba(ui, color, Alpha::BlendOrAdditive);
                ui.add(egui::Slider::new(value, 1.0..=300.0).text(side));
            });
        }

        ui.checkbox(&mut state.zero_white, "Zero as white");

        if let Some(first_cond) = state.conditions.get(0) {
            if !((first_cond.is_empty()) & (state.conditions.len() == 1)) {
                let conditions = state.conditions.clone();
                let condition = &mut state.condition;
                egui::ComboBox::from_label("Condition")
                    .selected_text(condition.clone())
                    .show_ui(ui, |ui| {
                        for cond in conditions.iter() {
                            ui.selectable_value(condition, cond.clone(), cond.clone());
                        }
                    });
            }
        }
        // direct interactions with the file system are not supported in WASM
        // for loading, direct wasm bindings are being used.
        #[cfg(not(target_arch = "wasm32"))]
        ui.collapsing("Export", |ui| {
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    save_events.send(SaveEvent(state.save_path.clone()))
                }
                ui.text_edit_singleline(&mut state.save_path);
            })
        });
        #[cfg(not(target_arch = "wasm32"))]
        ui.collapsing("Import", |ui| {
            let win = windows.get_primary().expect("no primary window");
            for label in ["Map", "Data"] {
                let path = state.get_mut_paths(label);
                ui.horizontal(|ui| {
                    if ui.button(label).clicked() {
                        // piggyback on file_drop()
                        load_events.send(FileDragAndDrop::DroppedFile {
                            id: win.id(),
                            path_buf: path.clone().into(),
                        });
                    }
                    ui.text_edit_singleline(path);
                });
            }
        });

        ui.add(egui::Hyperlink::from_label_and_url(
            "How to use?",
            // "https://shu.readthedocs.io",
            "https://carrascomj.github.io/shu/docs/plotting.html",
        ));
    });
}

/// Open `.metabolism.json` and `.reactions.json` files when dropped on the window.
pub fn file_drop(
    mut dnd_evr: EventReader<FileDragAndDrop>,
    asset_server: Res<AssetServer>,
    mut reaction_resource: ResMut<ReactionState>,
    mut escher_resource: ResMut<MapState>,
) {
    for ev in dnd_evr.iter() {
        if let FileDragAndDrop::DroppedFile { path_buf, .. } = ev {
            println!("Dropped file with path: {:?}", path_buf);

            if path_buf.to_str().unwrap().ends_with("metabolism.json") {
                let reaction_handle: Handle<Data> = asset_server.load(path_buf.to_str().unwrap());
                reaction_resource.reaction_data = Some(reaction_handle);
                reaction_resource.reac_loaded = false;
                reaction_resource.met_loaded = false;
                info! {"Reactions dropped!"};
            } else {
                //an escher map
                let escher_handle: Handle<EscherMap> =
                    asset_server.load(path_buf.to_str().unwrap());
                escher_resource.escher_map = escher_handle;
                escher_resource.loaded = false;
            }
        }
    }
}

/// Cursor to mouse position. Stolen from bevy cheatbook.
fn get_pos(win: &Window, camera: &Camera, camera_transform: &GlobalTransform) -> Option<Vec2> {
    // get the size of the window
    let window_size = Vec2::new(win.width(), win.height());
    if let Some(screen_pos) = win.cursor_position() {
        // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
        let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;
        // matrix for undoing the projection and camera transform
        let ndc_to_world = camera_transform.compute_matrix() * camera.projection_matrix().inverse();
        // use it to convert ndc to world-space coordinates
        let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));
        // reduce it to a 2D value
        Some(world_pos.truncate())
    } else {
        None
    }
}

/// Show hovered data on cursor enter.
fn show_hover(
    ui_state: Res<UiState>,
    windows: Res<Windows>,
    hover_query: Query<(&Transform, &Hover)>,
    mut popup_query: Query<(&mut Visibility, &AnyTag, &VisCondition), With<HistTag>>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
) {
    let (camera, camera_transform) = q_camera.single();
    let win = windows.get_primary().expect("no primary window");
    if let Some(world_pos) = get_pos(win, camera, camera_transform) {
        for (trans, hover) in hover_query.iter() {
            if (world_pos - Vec2::new(trans.translation.x, trans.translation.y)).length_squared()
                < 5000.
            {
                for (mut vis, tag, hist) in popup_query.iter_mut() {
                    let cond_if = hist
                        .condition
                        .as_ref()
                        .map(|c| (c == &ui_state.condition) || (ui_state.condition == "ALL"))
                        .unwrap_or(true);
                    if (hover.node_id == tag.id) & cond_if {
                        *vis = Visibility::VISIBLE;
                    }
                }
            } else {
                for (mut vis, tag, hist) in popup_query.iter_mut() {
                    let cond_if = hist
                        .condition
                        .as_ref()
                        .map(|c| (c != &ui_state.condition) & (ui_state.condition != "ALL"))
                        .unwrap_or(false);
                    if (hover.node_id == tag.id) || cond_if {
                        *vis = Visibility::INVISIBLE;
                    }
                }
            }
        }
    }
}

/// Register an non-UI entity (histogram) as being dragged by center or right button.
fn mouse_click_system(
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut drag_query: Query<(&Transform, &mut Drag), Without<AnyTag>>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
) {
    if mouse_button_input.just_pressed(MouseButton::Middle) {
        for (trans, mut drag) in drag_query.iter_mut() {
            let (camera, camera_transform) = q_camera.single();
            let win = windows.get_primary().expect("no primary window");
            if let Some(world_pos) = get_pos(win, camera, camera_transform) {
                if (world_pos - Vec2::new(trans.translation.x, trans.translation.y))
                    .length_squared()
                    < 5000.
                {
                    drag.dragged = true;
                }
            }
        }
    }

    if mouse_button_input.just_released(MouseButton::Middle) {
        for (_, mut drag) in drag_query.iter_mut() {
            drag.dragged = false;
        }
    }
    if mouse_button_input.just_pressed(MouseButton::Right) {
        for (trans, mut drag) in drag_query.iter_mut() {
            let (camera, camera_transform) = q_camera.single();
            let win = windows.get_primary().expect("no primary window");
            if let Some(world_pos) = get_pos(win, camera, camera_transform) {
                if (world_pos - Vec2::new(trans.translation.x, trans.translation.y))
                    .length_squared()
                    < 5000.
                {
                    drag.rotating = true;
                }
            }
        }
    }

    if mouse_button_input.just_released(MouseButton::Right) {
        for (_, mut drag) in drag_query.iter_mut() {
            drag.rotating = false;
        }
    }
}

/// Register a UI Drag enity as being dragged by center or right button.
fn mouse_click_ui_system(
    mouse_button_input: Res<Input<MouseButton>>,
    mut drag_query: Query<(&mut Drag, &Interaction, &mut BackgroundColor)>,
    // mut drag_query: Query<(&mut Drag, &Interaction)>,
) {
    for (mut drag, interaction, mut background_color) in drag_query.iter_mut() {
        // for (mut drag, interaction) in drag_query.iter_mut() {
        match interaction {
            Interaction::Hovered | Interaction::Clicked => {
                drag.dragged = mouse_button_input.pressed(MouseButton::Middle);
                drag.rotating = mouse_button_input.pressed(MouseButton::Right);
                *background_color = BackgroundColor(Color::rgba(0.9, 0.9, 0.9, 0.2));
            }
            _ => {
                *background_color = BackgroundColor(Color::rgba(1.0, 1.0, 1.0, 0.0));
            }
        }
    }
}

/// Move the center-dragged interactable non-UI entities (histograms).
fn follow_mouse_on_drag(
    windows: Res<Windows>,
    mut drag_query: Query<(&mut Transform, &Drag), Without<Style>>,
    q_camera: Query<(&Camera, &GlobalTransform)>,
) {
    for (mut trans, drag) in drag_query.iter_mut() {
        if drag.dragged {
            let (camera, camera_transform) = q_camera.single();
            let win = windows.get_primary().expect("no primary window");
            if let Some(world_pos) = get_pos(win, camera, camera_transform) {
                trans.translation = Vec3::new(world_pos.x, world_pos.y, trans.translation.z);
            }
        }
    }
}

/// Move the center-dragged interactable UI entities.
fn follow_mouse_on_drag_ui(windows: Res<Windows>, mut drag_query: Query<(&mut Style, &Drag)>) {
    for (mut style, drag) in drag_query.iter_mut() {
        if drag.dragged {
            let win = windows.get_primary().expect("no primary window");
            if let Some(screen_pos) = win.cursor_position() {
                style.position = UiRect {
                    // arbitrary offset to make it feel more natural
                    left: Val::Px(screen_pos.x - 60.),
                    bottom: Val::Px(screen_pos.y),
                    ..Default::default()
                };
            }
        }
    }
}

/// Rotate the right-dragged interactable (histograms and legend) entities.
fn follow_mouse_on_rotate(
    mut drag_query: Query<(&mut Transform, &Drag)>,
    mut mouse_motion_events: EventReader<bevy::input::mouse::MouseMotion>,
) {
    for ev in mouse_motion_events.iter() {
        for (mut trans, drag) in drag_query.iter_mut() {
            let pos = trans.translation;
            if drag.rotating {
                trans.rotate_around(pos, Quat::from_axis_angle(Vec3::Z, -ev.delta.y * 0.05));
                // clamping of angle to rect angles
                let (_, angle) = trans.rotation.to_axis_angle();
                const TOL: f32 = 0.06;
                if f32::abs(angle) < TOL {
                    trans.rotation = Quat::from_axis_angle(Vec3::Z, 0.);
                } else if f32::abs(angle - std::f32::consts::PI) < TOL {
                    trans.rotation = Quat::from_axis_angle(Vec3::Z, std::f32::consts::PI);
                } else if f32::abs(angle - std::f32::consts::PI / 2.) < TOL {
                    trans.rotation = Quat::from_axis_angle(Vec3::Z, std::f32::consts::PI / 2.);
                } else if f32::abs(angle - 3. * std::f32::consts::PI / 2.) < TOL {
                    trans.rotation = Quat::from_axis_angle(Vec3::Z, 3. * std::f32::consts::PI / 2.);
                }
            }
        }
    }
}

/// Change size of UI on +/-.
fn scale_ui(key_input: Res<Input<KeyCode>>, mut ui_scale: ResMut<UiScale>) {
    if key_input.just_pressed(KeyCode::Plus) {
        ui_scale.scale += 0.1;
    } else if key_input.just_pressed(KeyCode::Minus) {
        ui_scale.scale -= 0.1;
    }
}

/// Save map to arbitrary place, including (non-hover) hist transforms.
fn save_file(
    mut assets: ResMut<Assets<EscherMap>>,
    state: ResMut<MapState>,
    mut save_events: EventReader<SaveEvent>,
    hist_query: Query<(&Transform, &Xaxis), Without<AnyTag>>,
) {
    for save_event in save_events.iter() {
        let custom_asset = assets.get_mut(&state.escher_map);
        if custom_asset.is_none() {
            return;
        }
        let escher_map = custom_asset.unwrap();
        for (trans, axis) in hist_query.iter() {
            if let Some(reac) = escher_map.metabolism.reactions.get_mut(&axis.node_id) {
                reac.hist_position
                    .get_or_insert(HashMap::new())
                    .insert(axis.side.clone(), (*trans).into());
            }
        }
        safe_json_write(&save_event.0, escher_map)
            .unwrap_or_else(|e| warn!("Could not write the file: {}.", e));
    }
}

fn safe_json_write<P, C>(path: P, contents: C) -> std::io::Result<()>
where
    P: AsRef<std::path::Path>,
    C: serde::Serialize,
{
    std::fs::write(path, serde_json::to_string(&contents)?)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
/// WASM Part.
#[derive(Resource)]
pub struct ReceiverResource<T> {
    pub rx: async_std::channel::Receiver<T>,
}

#[cfg(target_arch = "wasm32")]
fn listen_js_escher(
    receiver: Res<ReceiverResource<EscherMap>>,
    mut escher_asset: ResMut<Assets<EscherMap>>,
    mut escher_resource: ResMut<MapState>,
) {
    if let Ok(escher_map) = receiver.rx.try_recv() {
        escher_resource.escher_map = escher_asset.add(escher_map);
        escher_resource.loaded = false;
    }
}

#[cfg(target_arch = "wasm32")]
fn listen_js_data(
    receiver: Res<ReceiverResource<Data>>,
    mut data_asset: ResMut<Assets<Data>>,
    mut data_resource: ResMut<ReactionState>,
) {
    if let Ok(escher_map) = receiver.rx.try_recv() {
        data_resource.reaction_data = Some(data_asset.add(escher_map));
        data_resource.reac_loaded = false;
        data_resource.met_loaded = false;
    }
}
