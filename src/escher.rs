//! Data model of escher JSON maps
//! TODO: borrow strings
use crate::data::{MetaboliteData, ReactionData};
use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use bevy_prototype_lyon::prelude::*;
use itertools::Itertools;
use serde::Deserialize;
use serde_json;
use std::{cmp::Ordering, collections::HashMap};

pub struct EscherPlugin;

impl Plugin for EscherPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<EscherMap>()
            .add_asset_loader(CustomAssetLoader::<EscherMap>::new(vec!["json"]))
            .add_asset_loader(CustomAssetLoader::<ReactionData>::new(vec![
                "reaction.json",
            ]))
            .add_asset_loader(CustomAssetLoader::<MetaboliteData>::new(vec![
                "metabolite.json",
            ]))
            .add_system(load_map);
    }
}

#[derive(Resource)]
pub struct MapState {
    pub escher_map: Handle<EscherMap>,
    pub loaded: bool,
}

#[derive(Deserialize, TypeUuid, Default)]
#[uuid = "413be529-bfeb-41b3-9db0-4b8b380a2c46"]
pub struct EscherMap {
    #[allow(dead_code)]
    info: EscherInfo,
    metabolism: Metabolism,
}

#[derive(Default)]
pub struct CustomAssetLoader<A> {
    extensions: Vec<&'static str>,
    _mark: std::marker::PhantomData<A>,
}

impl<A> AssetLoader for CustomAssetLoader<A>
where
    for<'de> A: serde::Deserialize<'de> + bevy::asset::Asset,
{
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let custom_asset = serde_json::from_slice::<A>(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(custom_asset));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &self.extensions
    }
}

impl<A> CustomAssetLoader<A> {
    fn new(extensions: Vec<&'static str>) -> Self {
        Self {
            extensions,
            _mark: std::marker::PhantomData::<A>,
        }
    }
}

impl EscherMap {
    pub fn get_components(&self) -> (Vec<Reaction>, Vec<Metabolite>) {
        (
            self.metabolism.reactions.clone().into_values().collect(),
            self.metabolism
                .nodes
                .clone()
                .into_iter()
                .filter_map(|(_, met)| match met {
                    Node::Metabolite(met) => Some(met),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Get the coordinates of a metabolite given a node id
    pub fn met_coords(&self, met_id: &str) -> Option<Vec2> {
        let met = self.metabolism.nodes.get(&met_id.parse().unwrap())?;
        match met {
            Node::Metabolite(Metabolite { x, y, .. })
            | Node::Multimarker { x, y }
            | Node::Midmarker { x, y } => Some(Vec2::new(*x, *y)),
        }
    }

    /// Reaction direction as defined by the vector that follows the longest segment.
    /// This is needed to calculate rotation angles for elements at the side of the
    /// reactions.
    pub fn main_direction(&self, reac: &Reaction) -> Vec2 {
        reac.segments
            .values()
            .filter_map(|seg| {
                match self
                    .metabolism
                    .nodes
                    .get(&seg.from_node_id.parse().unwrap())
                {
                    Some(node) => Some(node),
                    _ => None,
                }
            })
            .chain(reac.segments.values().filter_map(|seg| {
                match self.metabolism.nodes.get(&seg.to_node_id.parse().unwrap()) {
                    Some(node) => Some(node),
                    _ => None,
                }
            }))
            .filter_map(|node| match node {
                Node::Metabolite(Metabolite {
                    x,
                    y,
                    node_is_primary,
                    ..
                }) if *node_is_primary => Some(Vec2::new(*x, *y)),
                _ => None,
            })
            .combinations(2)
            .map(|vec| vec[1] - vec[0])
            // avoid zero vectors
            .filter(|vec| vec.max_element() > 1e-5)
            // .inspect(|vec| info!("{vec}"))
            .max_by(|x, y| {
                if x.length() - y.length() > 1e-5 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            })
            .unwrap_or(Vec2::Y)
            .normalize()
    }
    pub fn _main_direction(&self, reac: &Reaction) -> Vec2 {
        reac.segments
            .values()
            .filter_map(|seg| {
                match (
                    self.met_coords(&seg.from_node_id),
                    self.met_coords(&seg.to_node_id),
                ) {
                    (Some(node), Some(node2)) => Some((node, node2)),
                    _ => None,
                }
            })
            .map(|(from, to)| Vec2::new(from.x - to.x, from.y - to.y))
            .max_by(|from, to| {
                if from.length() - to.length() > 1e-5 {
                    Ordering::Less
                } else {
                    Ordering::Greater
                }
            })
            .unwrap_or(Vec2::Y)
            .normalize()
    }
}

#[derive(Deserialize, Default)]
struct EscherInfo {
    map_name: String,
    map_id: String,
    map_description: String,
    homepage: String,
    schema: String,
}

#[derive(Deserialize, Default)]
struct Metabolism {
    reactions: HashMap<u64, Reaction>,
    nodes: HashMap<u64, Node>,
}

/// Component for Bevy that will be rendered on screen.
/// Rendered as arrow.
#[derive(Component, Deserialize, Clone)]
pub struct Reaction {
    name: String,
    pub bigg_id: String,
    reversibility: bool,
    label_x: f32,
    label_y: f32,
    gene_reaction_rule: String,
    genes: Vec<HashMap<String, String>>,
    metabolites: Vec<MetRef>,
    pub segments: HashMap<u32, Segment>,
}

#[derive(Deserialize, Clone)]
struct MetRef {
    coefficient: f32,
    bigg_id: String,
}

#[derive(Deserialize, Clone)]
pub struct Segment {
    pub from_node_id: String,
    pub to_node_id: String,
    pub b1: Option<BezierHandle>,
    pub b2: Option<BezierHandle>,
}

#[derive(Deserialize, Clone)]
pub struct BezierHandle {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "node_type", rename_all = "lowercase")]
enum Node {
    Metabolite(Metabolite),
    Multimarker { x: f32, y: f32 },
    Midmarker { x: f32, y: f32 },
}

/// Component for Bevy that will be rendered on screen.
/// Rendered as circles.
#[derive(Component, Deserialize, Clone)]
pub struct Metabolite {
    pub x: f32,
    pub y: f32,
    label_x: f32,
    label_y: f32,
    name: String,
    pub bigg_id: String,
    pub node_is_primary: bool,
}

/// Component to differentiate circles via identifier (bigg_id in [`Metabolite`]).
#[derive(Component, Deserialize, Clone)]
pub struct CircleTag {
    pub id: String,
}
/// Component to differentiate arrows via identifier (bigg_id in [`Reaction`]).
#[derive(Component, Deserialize, Clone)]
pub struct ArrowTag {
    pub id: String,
    pub direction: Vec2,
}

/// Load escher map once the asset is available.
/// The colors correspond to the default escher colors.
fn load_map(
    mut commands: Commands,
    mut state: ResMut<MapState>,
    mut custom_assets: ResMut<Assets<EscherMap>>,
) {
    let custom_asset = custom_assets.get_mut(&mut state.escher_map);
    if state.loaded || custom_asset.is_none() {
        return;
    }
    let my_map = custom_asset.unwrap();
    let (reactions, metabolites) = my_map.get_components();
    // center all metabolites positions
    let (total_x, total_y) = metabolites
        .iter()
        .map(|met| (met.x, met.y))
        .fold((0., 0.), |(acc_x, acc_y), (x, y)| (acc_x + x, acc_y + y));
    let (center_x, center_y) = (
        total_x / metabolites.len() as f32,
        total_y / metabolites.len() as f32,
    );
    for met in metabolites {
        let shape = shapes::RegularPolygon {
            sides: 6,
            feature: shapes::RegularPolygonFeature::Radius(if met.node_is_primary {
                20.0
            } else {
                10.0
            }),
            ..shapes::RegularPolygon::default()
        };
        commands
            .spawn(GeometryBuilder::build_as(
                &shape,
                DrawMode::Outlined {
                    fill_mode: FillMode::color(Color::rgb(224. / 255., 137. / 255., 101. / 255.)),
                    outline_mode: StrokeMode::new(
                        Color::rgb(162. / 255., 69. / 255., 16. / 255.),
                        4.0,
                    ),
                },
                Transform::from_xyz(met.x - center_x, -met.y + center_y, 2.),
            ))
            .insert(CircleTag { id: met.bigg_id });
    }
    for reac in reactions {
        let mut path_builder = PathBuilder::new();
        // origin of the figure as the center of mass
        let ori: Vec2 = reac
            .segments
            .iter()
            .map(|(_, seg)| {
                (
                    my_map.met_coords(&seg.from_node_id),
                    my_map.met_coords(&seg.to_node_id),
                )
            })
            .filter_map(|(from, to)| match (from, to) {
                (Some(f), Some(t)) => Some(f + t),
                _ => None,
            })
            .sum::<Vec2>()
            / (2. * reac.segments.len() as f32);
        let direction = my_map.main_direction(&reac);
        for (_, segment) in reac.segments {
            if let (Some(from), Some(to)) = (
                my_map.met_coords(&segment.from_node_id),
                my_map.met_coords(&segment.to_node_id),
            ) {
                path_builder.move_to(Vec2::new(from.x - ori.x, -from.y + ori.y));
                match (segment.b1, segment.b2) {
                    (Some(BezierHandle { x, y }), None) | (None, Some(BezierHandle { x, y })) => {
                        path_builder.quadratic_bezier_to(
                            Vec2::new(x - ori.x, -y + ori.y),
                            Vec2::new(to.x - ori.x, -to.y + ori.y),
                        );
                    }
                    (Some(BezierHandle { x: x1, y: y1 }), Some(BezierHandle { x: x2, y: y2 })) => {
                        path_builder.cubic_bezier_to(
                            Vec2::new(x1 - ori.x, -y1 + ori.y),
                            Vec2::new(x2 - ori.x, -y2 + ori.y),
                            Vec2::new(to.x - ori.x, -to.y + ori.y),
                        );
                    }
                    (None, None) => {
                        let v = Vec2::new(to.x - ori.x, -to.y + ori.y);
                        path_builder.line_to(v);
                    }
                }
            }
        }
        let line = path_builder.build();
        commands.spawn((
            GeometryBuilder::build_as(
                &line,
                DrawMode::Stroke(StrokeMode::new(
                    Color::rgb(51. / 255., 78. / 255., 101. / 255.),
                    10.0,
                )),
                Transform::from_xyz(ori.x - center_x, -ori.y + center_y, 1.),
            ),
            ArrowTag {
                id: reac.bigg_id.clone(),
                direction,
            },
        ));
    }
    info!("Map loaded!");

    state.loaded = true;
}
