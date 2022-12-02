use crate::escher::{load_map, ArrowTag, CircleTag, Hover};
use crate::funcplot::{
    lerp, lerp_hsv, max_f32, min_f32, path_to_vec, plot_box_point, plot_hist, plot_kde, plot_scales,
};
use crate::geom::{
    AnyTag, GeomArrow, GeomHist, GeomMetabolite, HistPlot, HistTag, PopUp, Side, Xaxis,
};
use crate::gui::UiState;
use itertools::Itertools;
use std::collections::HashMap;

use bevy::prelude::*;
use bevy_prototype_lyon::prelude::{
    shapes, DrawMode, FillMode, GeometryBuilder, Path, ShapePath, StrokeMode,
};

pub struct AesPlugin;

impl Plugin for AesPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(plot_arrow_size)
            .add_system(plot_arrow_color)
            .add_system(plot_arrow_size_dist)
            .add_system(plot_metabolite_color)
            .add_system(build_axes.before(load_map))
            .add_system(build_hover_axes.before(load_map))
            .add_system(build_point_axes.before(load_map))
            .add_system(plot_side_hist.before(load_map))
            .add_system(plot_side_box.before(load_map))
            .add_system(plot_hover_hist.before(load_map))
            .add_system(normalize_histogram_height)
            .add_system(unscale_histogram_children)
            .add_system(fill_conditions)
            .add_system(filter_histograms)
            .add_system(follow_the_axes)
            .add_system(plot_metabolite_size);
    }
}

#[derive(Component)]
pub struct Aesthetics {
    /// flag to filter out the plotting
    /// it will be moved to the Geoms since more than one group of Aes
    /// can be a plotted with different geoms.
    pub plotted: bool,
    /// ordered identifers that each aesthetic will be plotted at
    pub identifiers: Vec<String>,
    /// ordered condition identifiers
    pub condition: Option<String>,
}

#[derive(Component)]
pub struct Gx {}

#[derive(Component)]
pub struct Gy {}

/// Data from the variables is allocated here.
#[derive(Component)]
pub struct Point<T>(pub Vec<T>);
#[derive(Component)]
pub struct Distribution<T>(pub Vec<Vec<T>>);
#[derive(Component)]
pub struct Categorical<T>(Vec<T>);

#[derive(Component)]
pub struct Gsize {}

#[derive(Component)]
pub struct Gcolor {}

/// Marker to avoid scaling some Entities with HistTag.
#[derive(Component)]
struct Unscale;

/// Plot arrow size.
pub fn plot_arrow_size(
    ui_state: Res<UiState>,
    mut query: Query<(&mut DrawMode, &ArrowTag)>,
    mut aes_query: Query<(&Point<f32>, &Aesthetics), (With<GeomArrow>, With<Gsize>)>,
) {
    for (sizes, aes) in aes_query.iter_mut() {
        if let Some(condition) = &aes.condition {
            if condition != &ui_state.condition {
                continue;
            }
        }
        let min_val = min_f32(&sizes.0);
        let max_val = max_f32(&sizes.0);
        for (mut draw_mode, arrow) in query.iter_mut() {
            if let DrawMode::Stroke(StrokeMode {
                ref mut options, ..
            }) = *draw_mode
            {
                if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                    let unscaled_width = sizes.0[index];
                    options.line_width = lerp(
                        unscaled_width,
                        min_val,
                        max_val,
                        ui_state.min_reaction,
                        ui_state.max_reaction,
                    );
                } else {
                    options.line_width = 10.;
                }
            }
        }
    }
}

/// For arrows (reactions) sizes, distributions are summarised as the mean.
pub fn plot_arrow_size_dist(
    ui_state: Res<UiState>,
    mut query: Query<(&mut DrawMode, &ArrowTag)>,
    mut aes_query: Query<(&Distribution<f32>, &Aesthetics), (With<GeomArrow>, With<Gsize>)>,
) {
    for (sizes, aes) in aes_query.iter_mut() {
        if let Some(condition) = &aes.condition {
            if condition != &ui_state.condition {
                continue;
            }
        }
        for (mut draw_mode, arrow) in query.iter_mut() {
            let min_val = min_f32(&sizes.0.iter().flatten().copied().collect::<Vec<f32>>());
            let max_val = max_f32(&sizes.0.iter().flatten().copied().collect::<Vec<f32>>());
            if let DrawMode::Stroke(StrokeMode {
                ref mut options, ..
            }) = *draw_mode
            {
                if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                    let unscaled_width =
                        sizes.0[index].iter().sum::<f32>() / sizes.0[index].len() as f32;
                    options.line_width = lerp(
                        unscaled_width,
                        min_val,
                        max_val,
                        ui_state.min_reaction,
                        ui_state.max_reaction,
                    );
                } else {
                    options.line_width = 10.;
                }
            }
        }
    }
}

/// Plot Color as numerical variable in arrows.
pub fn plot_arrow_color(
    ui_state: Res<UiState>,
    mut query: Query<(&mut DrawMode, &ArrowTag)>,
    mut aes_query: Query<(&Point<f32>, &Aesthetics), (With<GeomArrow>, With<Gcolor>)>,
) {
    for (colors, aes) in aes_query.iter_mut() {
        if let Some(condition) = &aes.condition {
            if condition != &ui_state.condition {
                continue;
            }
        }
        let min_val = min_f32(&colors.0);
        let max_val = max_f32(&colors.0);
        for (mut draw_mode, arrow) in query.iter_mut() {
            if let DrawMode::Stroke(StrokeMode { ref mut color, .. }) = *draw_mode {
                if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                    *color = lerp_hsv(
                        (colors.0[index] - min_val) / (max_val - min_val),
                        ui_state.min_reaction_color,
                        ui_state.max_reaction_color,
                    );
                } else {
                    *color = Color::rgb(0.85, 0.85, 0.85);
                }
            }
        }
    }
}

/// Plot size as numerical variable in metabolic circles.
pub fn plot_metabolite_size(
    ui_state: Res<UiState>,
    mut query: Query<(&mut Path, &CircleTag)>,
    mut aes_query: Query<(&Point<f32>, &Aesthetics), (With<GeomMetabolite>, With<Gsize>)>,
) {
    for (sizes, aes) in aes_query.iter_mut() {
        let min_val = min_f32(&sizes.0);
        let max_val = max_f32(&sizes.0);
        for (mut path, arrow) in query.iter_mut() {
            let radius = if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                lerp(
                    sizes.0[index],
                    min_val,
                    max_val,
                    ui_state.min_metabolite,
                    ui_state.max_metabolite,
                )
            } else {
                20.
            };
            let polygon = shapes::RegularPolygon {
                sides: 6,
                feature: shapes::RegularPolygonFeature::Radius(radius),
                ..shapes::RegularPolygon::default()
            };
            *path = ShapePath::build_as(&polygon);
        }
    }
}

/// Plot Color as numerical variable in metabolic circles.
pub fn plot_metabolite_color(
    ui_state: Res<UiState>,
    mut query: Query<(&mut DrawMode, &CircleTag)>,
    mut aes_query: Query<(&Point<f32>, &Aesthetics), (With<GeomMetabolite>, With<Gcolor>)>,
) {
    for (colors, aes) in aes_query.iter_mut() {
        let min_val = min_f32(&colors.0);
        let max_val = max_f32(&colors.0);
        for (mut draw_mode, arrow) in query.iter_mut() {
            if let DrawMode::Outlined {
                fill_mode: FillMode { ref mut color, .. },
                ..
            } = *draw_mode
            {
                if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                    *color = lerp_hsv(
                        (colors.0[index] - min_val) / (max_val - min_val),
                        ui_state.min_metabolite_color,
                        ui_state.max_metabolite_color,
                    );
                } else {
                    *color = Color::rgb(0.85, 0.85, 0.85);
                }
            }
        }
    }
}

/// Build axes for histograms, summarising all external information.
/// Each Side of an arrow is assigned a different axis, shared across conditions.
fn build_axes(
    mut commands: Commands,
    mut query: Query<(&Transform, &ArrowTag, &Path)>,
    mut aes_query: Query<
        (&Distribution<f32>, &Aesthetics, &mut GeomHist),
        (With<Gy>, Without<PopUp>),
    >,
) {
    let mut axes: HashMap<String, HashMap<Side, (Xaxis, Transform)>> = HashMap::new();
    // first gather all x-limits for different conditions and the arrow and side
    for (dist, aes, mut geom) in aes_query.iter_mut() {
        if geom.in_axis {
            continue;
        }
        for (trans, arrow, path) in query.iter_mut() {
            if let Some(index) = aes.identifiers.iter().position(|r| r == &arrow.id) {
                let this_dist = match dist.0.get(index) {
                    Some(d) => d,
                    None => continue,
                };
                let size = path_to_vec(path).length();
                let xlimits = (min_f32(this_dist), max_f32(this_dist));
                let (rotation_90, away) = match geom.side {
                    Side::Right => (-Vec2::Y.angle_between(arrow.direction.perp()), -30.),
                    Side::Left => (-Vec2::NEG_Y.angle_between(arrow.direction.perp()), 30.),
                    _ => {
                        warn!("Tried to plot Up direction for non-popup '{}'", arrow.id);
                        continue;
                    }
                };
                let transform: Transform = if let Some(Some(ser_transform)) =
                    arrow.hists.as_ref().map(|x| x.get(&geom.side))
                {
                    // there were saved histogram positions
                    ser_transform.clone().into()
                } else {
                    // histogram perpendicular to the direction of the arrow
                    // the arrow direction is decided by a fallible heuristic!
                    let mut transform =
                        Transform::from_xyz(trans.translation.x, trans.translation.y, 0.5)
                            .with_rotation(Quat::from_rotation_z(rotation_90));
                    transform.translation.x += arrow.direction.perp().x * away;
                    transform.translation.y += arrow.direction.perp().y * away;
                    transform
                };
                let mut axis_entry = axes
                    .entry(arrow.id.clone())
                    .or_insert(HashMap::new())
                    .entry(geom.side.clone())
                    .or_insert((
                        Xaxis {
                            id: arrow.id.clone(),
                            arrow_size: size,
                            xlimits,
                            side: geom.side.clone(),
                            plot: geom.plot.clone(),
                            node_id: arrow.node_id,
                            dragged: false,
                            rotating: false,
                            conditions: Vec::new(),
                        },
                        transform,
                    ));
                axis_entry.0.xlimits = (
                    f32::min(axis_entry.0.xlimits.0, xlimits.0),
                    f32::max(axis_entry.0.xlimits.1, xlimits.1),
                );
                if let Some(cond) = aes.condition.as_ref() {
                    axis_entry.0.conditions.push(cond.clone());
                }
                geom.in_axis = true;
            }
        }
    }

    for (axis, trans) in axes.into_values().flat_map(|side| side.into_values()) {
        commands
            .spawn(axis)
            .insert(trans)
            .insert(VisibilityBundle::default());
    }
}

// Build axis
fn build_point_axes(
    mut commands: Commands,
    mut query: Query<(&Transform, &ArrowTag, &Path)>,
    mut aes_query: Query<
        (&Aesthetics, &mut GeomHist),
        (With<Gy>, Without<PopUp>, With<Point<f32>>),
    >,
) {
    let mut axes: HashMap<String, HashMap<Side, (Xaxis, Transform)>> = HashMap::new();
    // first gather all x-limits for different conditions and the arrow and side
    for (aes, mut geom) in aes_query.iter_mut() {
        if geom.in_axis {
            continue;
        }
        for (trans, arrow, path) in query.iter_mut() {
            if aes.identifiers.iter().any(|r| r == &arrow.id) {
                let size = path_to_vec(path).length();
                let (rotation_90, away) = match geom.side {
                    Side::Right => (-Vec2::Y.angle_between(arrow.direction.perp()), -30.),
                    Side::Left => (-Vec2::NEG_Y.angle_between(arrow.direction.perp()), 30.),
                    _ => {
                        warn!("Tried to plot Up direction for non-popup '{}'", arrow.id);
                        continue;
                    }
                };
                let transform: Transform = if let Some(Some(ser_transform)) =
                    arrow.hists.as_ref().map(|x| x.get(&geom.side))
                {
                    // there were saved histogram positions
                    ser_transform.clone().into()
                } else {
                    // histogram perpendicular to the direction of the arrow
                    // the arrow direction is decided by a fallible heuristic!
                    let mut transform =
                        Transform::from_xyz(trans.translation.x, trans.translation.y, 0.5)
                            .with_rotation(Quat::from_rotation_z(rotation_90));
                    transform.translation.x += arrow.direction.perp().x * away;
                    transform.translation.y += arrow.direction.perp().y * away;
                    transform
                };
                let axis_entry = axes
                    .entry(arrow.id.clone())
                    .or_insert(HashMap::new())
                    .entry(geom.side.clone())
                    .or_insert((
                        Xaxis {
                            id: arrow.id.clone(),
                            arrow_size: size,
                            xlimits: (0., 0.),
                            side: geom.side.clone(),
                            plot: geom.plot.clone(),
                            node_id: arrow.node_id,
                            dragged: false,
                            rotating: false,
                            conditions: Vec::new(),
                        },
                        transform,
                    ));
                if let Some(cond) = aes.condition.as_ref() {
                    axis_entry.0.conditions.push(cond.clone());
                }
                geom.in_axis = true;
            }
        }
    }

    for (axis, trans) in axes.into_values().flat_map(|side| side.into_values()) {
        commands
            .spawn(axis)
            .insert(trans)
            .insert(Unscale {})
            .insert(VisibilityBundle::default());
    }
}

fn build_hover_axes(
    mut query: Query<&mut Hover>,
    mut aes_query: Query<(&Distribution<f32>, &Aesthetics, &mut GeomHist), (With<Gy>, With<PopUp>)>,
) {
    let mut axes: HashMap<u64, (f32, f32)> = HashMap::new();
    // first gather all x-limits for different conditions and the arrow and side
    for (dist, aes, mut geom) in aes_query.iter_mut() {
        if geom.in_axis {
            continue;
        }
        for hover in query.iter() {
            if hover.xlimits.is_some() {
                continue;
            }
            if let Some(index) = aes.identifiers.iter().position(|r| r == &hover.id) {
                let this_dist = match dist.0.get(index) {
                    Some(d) => d,
                    None => continue,
                };
                let xlimits = (min_f32(this_dist), max_f32(this_dist));
                let axis_entry = axes.entry(hover.node_id).or_insert(xlimits);
                *axis_entry = (
                    f32::min(axis_entry.0, xlimits.0),
                    f32::max(axis_entry.1, xlimits.1),
                );
                geom.in_axis = true;
            }
        }
    }

    for (node_id, xlimits) in axes {
        for mut hover in query.iter_mut().filter(|h| h.node_id == node_id) {
            hover.xlimits = Some(xlimits)
        }
    }
}

/// Plot histogram as numerical variable next to arrows.
fn plot_side_hist(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut aes_query: Query<
        (&Distribution<f32>, &Aesthetics, &mut GeomHist),
        (With<Gy>, Without<PopUp>),
    >,
    mut query: Query<(Entity, &Transform, &Xaxis)>,
) {
    'outer: for (dist, aes, mut geom) in aes_query.iter_mut() {
        if geom.rendered {
            continue;
        }
        let font = asset_server.load("fonts/FiraSans-Bold.ttf");
        for (_e, trans, axis) in query.iter_mut() {
            if let Some(index) = aes
                .identifiers
                .iter()
                .position(|r| (r == &axis.id) & (geom.side == axis.side))
            {
                let this_dist = match dist.0.get(index) {
                    Some(d) => d,
                    None => continue,
                };
                let line = match geom.plot {
                    HistPlot::Hist => plot_hist(this_dist, 30, axis.arrow_size, axis.xlimits),
                    HistPlot::Kde => plot_kde(this_dist, 200, axis.arrow_size, axis.xlimits),
                    HistPlot::BoxPoint => {
                        warn!("Tried to plot a BoxPoint from a Distributions. Not Implemented! Consider using a Point as input");
                        continue 'outer;
                    }
                };
                if line.is_none() {
                    println!("line not plotted for geom {}", geom.side);
                    continue 'outer;
                }
                let line = line.unwrap();
                let hex = match geom.side {
                    // TODO: this should be a setting
                    Side::Right => "7dce9688",
                    Side::Left => "DA968788",
                    _ => {
                        warn!("Tried to plot Up direction for non-popup '{}'", axis.id);
                        continue;
                    }
                };
                // TODO(carrascomj): just y, x should be by axis
                let scales = plot_scales(
                    this_dist,
                    axis.arrow_size,
                    font.clone(),
                    12.,
                    match geom.plot {
                        HistPlot::Kde => 0.1,
                        _ => 80.,
                    },
                );

                commands
                    // .entity(e)
                    .spawn(GeometryBuilder::build_as(
                        &line,
                        DrawMode::Fill(FillMode::color(Color::hex(hex).unwrap())),
                        *trans,
                    ))
                    .insert(HistTag {
                        side: geom.side.clone(),
                        condition: aes.condition.clone(),
                        node_id: axis.node_id,
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.x_0);
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.x_n);
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.y);
                    });
            }
            geom.rendered = true;
        }
    }
}

fn plot_side_box(
    mut commands: Commands,
    ui_state: Res<UiState>,
    mut aes_query: Query<(&Point<f32>, &Aesthetics, &mut GeomHist), (With<Gy>, Without<PopUp>)>,
    mut query: Query<(&mut Transform, &Xaxis), With<Unscale>>,
) {
    for (colors, aes, mut geom) in aes_query.iter_mut() {
        if geom.rendered {
            continue;
        }
        let min_val = min_f32(&colors.0);
        let max_val = max_f32(&colors.0);

        for (mut trans, axis) in query.iter_mut() {
            if let Some(index) = aes
                .identifiers
                .iter()
                .position(|r| (r == &axis.id) & (geom.side == axis.side))
            {
                let color = lerp_hsv(
                    (colors.0[index] - min_val) / (max_val - min_val),
                    ui_state.min_reaction_color,
                    ui_state.max_reaction_color,
                );
                match geom.plot {
                    HistPlot::Hist | HistPlot::Kde => {
                        warn!(
                            "Tried to plot a distribution from one point. Coercing to a Box Point!"
                        );
                    }
                    _ => (),
                };

                let line_box = plot_box_point(
                    axis.conditions.len(),
                    axis.conditions
                        .iter()
                        .position(|x| x == aes.condition.as_ref().unwrap_or(&String::from("")))
                        .unwrap_or(0),
                );
                trans.translation.z += 10.;
                commands
                    .spawn(GeometryBuilder::build_as(
                        &line_box,
                        DrawMode::Outlined {
                            fill_mode: FillMode::color(color),
                            outline_mode: StrokeMode::new(Color::BLACK, 2.),
                        },
                        *trans,
                    ))
                    .insert(HistTag {
                        side: geom.side.clone(),
                        condition: aes.condition.clone(),
                        node_id: axis.node_id,
                    })
                    .insert(Unscale {});
            }
            geom.rendered = true;
        }
    }
}

/// Plot hovered histograms of both metabolites and reactions.
fn plot_hover_hist(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut query: Query<(&Transform, &Hover)>,
    mut aes_query: Query<(&Distribution<f32>, &Aesthetics, &mut GeomHist), (With<Gy>, With<PopUp>)>,
) {
    'outer: for (dist, aes, mut geom) in aes_query.iter_mut() {
        if geom.rendered {
            continue;
        }
        let font = asset_server.load("fonts/FiraSans-Bold.ttf");
        for (trans, hover) in query.iter_mut() {
            if hover.xlimits.is_none() {
                continue;
            }
            if let Some(index) = aes.identifiers.iter().position(|r| r == &hover.id) {
                let this_dist = match dist.0.get(index) {
                    Some(d) => d,
                    None => continue,
                };
                let xlimits = hover.xlimits.as_ref().unwrap();
                let line = match geom.plot {
                    HistPlot::Hist => plot_hist(this_dist, 30, 600., *xlimits),
                    HistPlot::Kde => plot_kde(this_dist, 200, 600., *xlimits),
                    HistPlot::BoxPoint => {
                        warn!("Tried to plot a BoxPoint from a Distributions. Not Implemented! Consider using a Point as input");
                        continue 'outer;
                    }
                };
                if line.is_none() {
                    continue 'outer;
                }
                let line = line.unwrap();
                let transform =
                    Transform::from_xyz(trans.translation.x + 150., trans.translation.y + 150., 5.);
                let mut geometry = GeometryBuilder::build_as(
                    &line,
                    DrawMode::Fill(FillMode::color(Color::hex("ffb73388").unwrap())),
                    transform,
                );
                geometry.visibility = Visibility::INVISIBLE;
                let scales = plot_scales(
                    this_dist,
                    600.,
                    font.clone(),
                    12.,
                    match geom.plot {
                        HistPlot::Kde => 0.15,
                        _ => 120.,
                    },
                );
                commands
                    .spawn(HistTag {
                        side: geom.side.clone(),
                        condition: aes.condition.clone(),
                        node_id: hover.node_id,
                    })
                    .insert(geometry)
                    .with_children(|p| {
                        p.spawn(SpriteBundle {
                            texture: asset_server.load("hover.png"),
                            transform: Transform::from_xyz(0., 0., -0.4),
                            ..default()
                        });
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.x_0);
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.x_n);
                    })
                    .with_children(|parent| {
                        parent.spawn(scales.y);
                    })
                    .insert(AnyTag { id: hover.node_id });
            }
            geom.rendered = true;
        }
    }
}

/// Normalize the height of histograms to be comparable with each other.
/// It treats the two sides independently.
fn normalize_histogram_height(
    ui_state: Res<UiState>,
    mut query: Query<(&mut Transform, &mut Path, &mut DrawMode, &HistTag), Without<Unscale>>,
) {
    for (mut trans, path, mut draw_mode, hist) in query.iter_mut() {
        let height = max_f32(&path.0.iter().map(|ev| ev.to().y).collect::<Vec<f32>>());
        trans.scale.y = match hist.side {
            Side::Left => ui_state.max_left / height,
            Side::Right => ui_state.max_right / height,
            Side::Up => ui_state.max_top / height,
        };
        if let DrawMode::Fill(ref mut fill_mode) = *draw_mode {
            fill_mode.color = match hist.side {
                Side::Left => {
                    let color = ui_state.color_left;
                    Color::rgba_linear(color.r(), color.g(), color.b(), color.a())
                }
                Side::Right => {
                    let color = ui_state.color_right;
                    Color::rgba_linear(color.r(), color.g(), color.b(), color.a())
                }
                Side::Up => {
                    let color = ui_state.color_top;
                    Color::rgba_linear(color.r(), color.g(), color.b(), color.a())
                }
            }
        }
    }
}

/// Unscale up children of scaled histograms.
fn unscale_histogram_children(
    parents: Query<(Entity, &Children), With<HistTag>>,
    mut query: Query<&mut Transform>,
) {
    for (parent, children) in parents.iter() {
        let Ok(scale) = query.get_mut(parent).map(|trans| trans.scale.y) else {continue;};
        for child in children {
            let Ok(mut trans) = query.get_mut(*child) else {continue;};
            trans.scale.y = 1. / scale;
        }
    }
}

/// Fill conditions menu.
fn fill_conditions(mut ui_state: ResMut<UiState>, aesthetics: Query<&Aesthetics>) {
    let conditions = aesthetics
        .iter()
        .filter_map(|a| a.condition.clone())
        .unique()
        .collect::<Vec<String>>();
    if conditions
        .iter()
        .any(|cond| !ui_state.conditions.contains(cond))
    {
        if !conditions.is_empty() {
            ui_state.conditions = conditions;
            if !ui_state.conditions.contains(&String::from("ALL")) {
                ui_state.conditions.push(String::from("ALL"));
            }
        } else {
            ui_state.conditions = vec![String::from("")];
            ui_state.condition = String::from("");
        }
        if ui_state.condition.is_empty() {
            ui_state.condition = ui_state.conditions[0].clone();
        }
    }
}

/// Hide histograms that are not in the conditions.
pub fn filter_histograms(
    ui_state: Res<UiState>,
    mut query: Query<(&mut Visibility, &HistTag), Without<AnyTag>>,
) {
    for (mut vis, hist) in query.iter_mut() {
        if let Some(condition) = &hist.condition {
            if (condition != &ui_state.condition) & (ui_state.condition != "ALL") {
                *vis = Visibility::INVISIBLE;
            } else {
                *vis = Visibility::VISIBLE;
            }
        }
    }
}

/// Coordinate the position of histograms with their hovers.
fn follow_the_axes(
    axes: Query<(&Transform, &Xaxis), Changed<Transform>>,
    mut hists: Query<(&mut Transform, &HistTag), (Without<AnyTag>, Without<Xaxis>)>,
) {
    for (axis_trans, axis) in axes.iter() {
        for (mut trans, hist) in hists.iter_mut() {
            if (axis.node_id == hist.node_id) & (hist.side == axis.side) {
                trans.translation = axis_trans.translation;
                trans.rotation = axis_trans.rotation;
            }
        }
    }
}
