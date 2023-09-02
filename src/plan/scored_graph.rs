use super::{
    find_by_product_node, find_input_node, find_production_node, ItemBitSet, Node, PlanConfig,
};
use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::{round, FloatType},
};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex, StableDiGraph},
    visit::EdgeRef,
    Direction::Incoming,
};
use std::{
    cmp::Ordering,
    fmt,
    ops::{Add, AddAssign, Index, Mul},
    rc::Rc,
    vec,
};

pub type ScoredGraphType = StableDiGraph<ScoredNodeValue, ScoredNodeEdge>;

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Score {
    pub resource_score: FloatType,
    pub power_score: FloatType,
    pub floor_area_score: FloatType,
    pub volume_score: FloatType,
    pub complexity: u32,
}

#[derive(Debug, Clone)]
pub struct ScoredByProduct {
    pub item: Rc<Item>,
    pub score: Score,
    pub unique_resources: u32,
    pub resource_combinations: Rc<[ItemBitSet]>,
    pub partial: bool,
}

#[derive(Debug, Clone)]
pub enum ScoredNodeValue {
    Output(Rc<Item>),
    Production(Rc<Recipe>),
    ByProduct(ScoredByProduct),
    Input(Rc<Item>),
}

#[derive(Debug, Clone)]
pub struct ScoredNodeEdge {
    pub item: Rc<Item>,
    pub score: Score,
    pub unique_resources: u32,
    pub resource_combinations: Rc<[ItemBitSet]>,
}

impl From<&ScoredByProduct> for ScoredNodeEdge {
    fn from(value: &ScoredByProduct) -> Self {
        Self {
            item: Rc::clone(&value.item),
            score: value.score,
            unique_resources: value.unique_resources,
            resource_combinations: Rc::clone(&value.resource_combinations),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutputNode {
    pub index: NodeIndex,
    pub value: ItemValuePair,
    pub score: Score,
    pub unique_resources: u32,
}

#[derive(Debug)]
pub struct ScoredGraph<'a> {
    pub config: &'a PlanConfig,
    pub graph: ScoredGraphType,
    pub output_nodes: Vec<OutputNode>,
}

impl ScoredByProduct {
    pub fn copy_score(&mut self, edge_weight: &ScoredNodeEdge, partial: bool) {
        self.score = edge_weight.score;
        self.unique_resources = edge_weight.unique_resources;
        self.resource_combinations = Rc::clone(&edge_weight.resource_combinations);
        self.partial = partial;
    }
}

#[allow(dead_code)]
impl ScoredNodeValue {
    #[inline]
    pub fn new_input(item: Rc<Item>) -> Self {
        Self::Input(item)
    }

    #[inline]
    pub fn new_output(item: Rc<Item>) -> Self {
        Self::Output(item)
    }

    #[inline]
    pub fn new_by_product(item: Rc<Item>) -> Self {
        Self::ByProduct(ScoredByProduct {
            item,
            score: Score::default(),
            unique_resources: 0,
            resource_combinations: vec![].into(),
            partial: true,
        })
    }

    #[inline]
    pub fn new_production(recipe: Rc<Recipe>) -> Self {
        Self::Production(recipe)
    }

    #[inline]
    pub fn as_input(&self) -> Rc<Item> {
        match self {
            Self::Input(i) => Rc::clone(i),
            _ => panic!("Node is not an Input"),
        }
    }

    #[inline]
    pub fn as_output(&self) -> Rc<Item> {
        match self {
            Self::Output(i) => Rc::clone(i),
            _ => panic!("Node is not an Output"),
        }
    }

    #[inline]
    pub fn as_by_product(&self) -> &ScoredByProduct {
        match self {
            Self::ByProduct(bp) => bp,
            _ => panic!("Node is not an ByProduct"),
        }
    }

    #[inline]
    pub fn as_by_product_mut(&mut self) -> &mut ScoredByProduct {
        match self {
            Self::ByProduct(bp) => bp,
            _ => panic!("Node is not an ByProduct"),
        }
    }

    #[inline]
    pub fn as_production(&self) -> Rc<Recipe> {
        match self {
            Self::Production(r) => Rc::clone(r),
            _ => panic!("Node is not an Production"),
        }
    }
}

impl Node for ScoredNodeValue {
    #[inline]
    fn is_input(&self) -> bool {
        matches!(self, Self::Input(..))
    }

    #[inline]
    fn is_input_resource(&self) -> bool {
        matches!(self, Self::Input(i, ..) if i.resource)
    }

    #[inline]
    fn is_output(&self) -> bool {
        matches!(self, Self::Output(..))
    }

    #[inline]
    fn is_by_product(&self) -> bool {
        matches!(self, Self::ByProduct(..))
    }

    #[inline]
    fn is_production(&self) -> bool {
        matches!(self, Self::Production(..))
    }

    #[inline]
    fn is_input_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::Input(i) if **i == *item)
    }

    #[inline]
    fn is_output_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::Output(i) if **i == *item)
    }

    #[inline]
    fn is_by_product_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::ByProduct(bp) if *bp.item == *item)
    }

    #[inline]
    fn is_production_for_recipe(&self, recipe: &Recipe) -> bool {
        matches!(self, Self::Production(r) if **r == *recipe)
    }
}

impl fmt::Display for ScoredNodeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(item) => {
                write!(f, "{}", item,)
            }
            Self::Production(recipe) => {
                write!(f, "{}\n{}", recipe, recipe.building,)
            }
            Self::ByProduct(byproduct) => {
                write!(f, "{}", byproduct.item,)
            }
            Self::Output(item) => {
                write!(f, "{}", item,)
            }
        }
    }
}

impl ScoredNodeEdge {
    #[inline]
    pub fn new(item: Rc<Item>, score: Score, resource_combinations: Rc<[ItemBitSet]>) -> Self {
        Self {
            item,
            score,
            unique_resources: count_unique_resources(&resource_combinations),
            resource_combinations,
        }
    }

    #[inline]
    pub fn without_score(item: Rc<Item>) -> Self {
        Self {
            item,
            score: Score::default(),
            unique_resources: 0,
            resource_combinations: vec![].into(),
        }
    }
}

impl fmt::Display for ScoredNodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\nScore: {}\n", self.item, self.score)
    }
}

const NORMALIZED_OUTPUT: FloatType = 60.0;
const RESOURCE_SCORE_SCALE_FACTOR: FloatType = 10_000.0;

impl Score {
    #[inline]
    pub fn infinity() -> Self {
        Self {
            resource_score: FloatType::INFINITY,
            power_score: FloatType::INFINITY,
            floor_area_score: FloatType::INFINITY,
            volume_score: FloatType::INFINITY,
            complexity: 0,
        }
    }

    #[inline]
    pub fn for_input_node(limit: FloatType) -> Self {
        let resource_score = if limit == 0.0 || limit.is_infinite() {
            0.0
        } else {
            NORMALIZED_OUTPUT / limit * RESOURCE_SCORE_SCALE_FACTOR
        };

        Self {
            resource_score,
            power_score: 0.0,
            floor_area_score: 0.0,
            volume_score: 0.0,
            complexity: 0,
        }
    }

    #[inline]
    pub fn add_production_step(&mut self, recipe: &Recipe, building_count: FloatType) {
        self.power_score += recipe.average_mw(100.0) * building_count;
        self.floor_area_score += recipe.building.floor_area() * building_count;
        self.volume_score += recipe.building.volume() * building_count;
        self.complexity += 1;
    }
}

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.resource_score.partial_cmp(&other.resource_score) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }
        match self.power_score.partial_cmp(&other.power_score) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }
        match self.floor_area_score.partial_cmp(&other.floor_area_score) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }

        match self.volume_score.partial_cmp(&other.volume_score) {
            Some(Ordering::Equal) => {}
            ord => return ord,
        }

        self.complexity.partial_cmp(&other.complexity)
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Add<Score> for Score {
    type Output = Self;

    fn add(self, rhs: Score) -> Self::Output {
        Self {
            resource_score: self.resource_score + rhs.resource_score,
            power_score: self.power_score + rhs.power_score,
            floor_area_score: self.floor_area_score + rhs.floor_area_score,
            volume_score: self.volume_score + rhs.volume_score,
            complexity: self.complexity.max(rhs.complexity),
        }
    }
}

impl AddAssign<Score> for Score {
    fn add_assign(&mut self, rhs: Score) {
        self.resource_score += rhs.resource_score;
        self.power_score += rhs.power_score;
        self.floor_area_score += rhs.floor_area_score;
        self.volume_score += rhs.volume_score;
        self.complexity = self.complexity.max(rhs.complexity);
    }
}

impl Mul<FloatType> for Score {
    type Output = Self;

    fn mul(self, rhs: FloatType) -> Self::Output {
        Self {
            resource_score: self.resource_score * rhs,
            power_score: self.power_score * rhs,
            floor_area_score: self.floor_area_score * rhs,
            volume_score: self.volume_score * rhs,
            complexity: self.complexity,
        }
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {} MW, {} m^2, {} m^3, {} depth)",
            round(self.resource_score, 1),
            round(self.power_score, 1),
            round(self.floor_area_score, 1),
            round(self.volume_score, 1),
            self.complexity
        )
    }
}

impl OutputNode {
    #[inline]
    pub fn new(
        index: NodeIndex,
        value: ItemValuePair,
        score: Score,
        unique_resources: u32,
    ) -> Self {
        Self {
            index,
            value,
            score,
            unique_resources,
        }
    }
}

impl<'a> ScoredGraph<'a> {
    #[inline]
    pub fn new(config: &'a PlanConfig) -> Self {
        Self {
            config,
            graph: ScoredGraphType::new(),
            output_nodes: Vec::with_capacity(config.outputs.len()),
        }
    }

    pub fn build(&mut self) {
        for output in &self.config.outputs {
            let node_index = self
                .graph
                .add_node(ScoredNodeValue::new_output(Rc::clone(&output.item)));
            let (score, resources) = self.create_children(node_index, Rc::clone(&output.item));

            self.output_nodes.push(OutputNode::new(
                node_index,
                output.clone(),
                score,
                count_unique_resources(&resources),
            ));
        }

        self.output_nodes
            .sort_unstable_by_key(|o| o.unique_resources);
    }

    fn create_children(
        &mut self,
        parent_idx: NodeIndex,
        item: Rc<Item>,
    ) -> (Score, Rc<[ItemBitSet]>) {
        if item.resource {
            self.create_input_node(parent_idx, item)
        } else {
            self.create_production_by_product(parent_idx, Rc::clone(&item))
        }
    }

    fn create_input_node(
        &mut self,
        parent_idx: NodeIndex,
        item: Rc<Item>,
    ) -> (Score, Rc<[ItemBitSet]>) {
        let idx = match find_input_node(&self.graph, &item) {
            Some(idx) => idx,
            None => self
                .graph
                .add_node(ScoredNodeValue::new_input(Rc::clone(&item))),
        };

        let resources: Rc<[ItemBitSet]> = if item.resource {
            vec![ItemBitSet::new(&item)].into()
        } else {
            vec![].into()
        };

        let limit = self.config.game_db.get_resource_limit(&item);
        let score = Score::for_input_node(limit);
        self.graph.add_edge(
            idx,
            parent_idx,
            ScoredNodeEdge::new(item, score, Rc::clone(&resources)),
        );

        (score, resources)
    }

    pub fn create_production_by_product(
        &mut self,
        parent_idx: NodeIndex,
        item: Rc<Item>,
    ) -> (Score, Rc<[ItemBitSet]>) {
        let (idx, mut score, mut resources) = match find_by_product_node(&self.graph, &item) {
            Some(idx) => {
                let by_product = self.graph[idx].as_by_product();
                if !by_product.partial {
                    let weight = ScoredNodeEdge::from(by_product);
                    self.graph.add_edge(idx, parent_idx, weight.clone());
                    return (weight.score, weight.resource_combinations);
                }

                let mut resources: Vec<ItemBitSet> = Vec::new();
                resources.extend(by_product.resource_combinations.iter());

                (idx, by_product.score, resources)
            }
            None => {
                let idx = self
                    .graph
                    .add_node(ScoredNodeValue::new_by_product(Rc::clone(&item)));

                (idx, Score::infinity(), Vec::new())
            }
        };

        for recipe in self.config.game_db.find_recipes_by_output(&item) {
            let (child_score, child_resources) =
                self.create_production_node(idx, recipe, Rc::clone(&item));

            score = score.min(child_score);
            resources.extend(child_resources.iter());
        }
        resources.sort_unstable();
        resources.dedup();
        let resources = resources.into();

        if self.config.has_input(&item) {
            let (child_score, _) = self.create_input_node(idx, Rc::clone(&item));
            score = score.min(child_score);
        }

        let edge_weight = ScoredNodeEdge::new(Rc::clone(&item), score, Rc::clone(&resources));

        self.graph[idx]
            .as_by_product_mut()
            .copy_score(&edge_weight, false);
        self.graph.add_edge(idx, parent_idx, edge_weight);

        (score, resources)
    }

    fn create_production_node(
        &mut self,
        parent_idx: NodeIndex,
        recipe: Rc<Recipe>,
        item: Rc<Item>,
    ) -> (Score, Rc<[ItemBitSet]>) {
        match find_production_node(&self.graph, &recipe) {
            Some(idx) => {
                let edge_idx = self.graph.find_edge(idx, parent_idx).unwrap();
                (
                    self.graph[edge_idx].score,
                    Rc::clone(&self.graph[edge_idx].resource_combinations),
                )
            }
            None => {
                let idx = self
                    .graph
                    .add_node(ScoredNodeValue::new_production(Rc::clone(&recipe)));

                let building_count = recipe
                    .find_output_by_item(&item)
                    .map(|o| NORMALIZED_OUTPUT / o.value)
                    .unwrap();

                let mut other_by_products = Vec::with_capacity(recipe.outputs.len() - 1);
                for recipe_output in &recipe.outputs {
                    if recipe_output.item != item {
                        let (e, n) = self
                            .create_partial_by_product_node(idx, Rc::clone(&recipe_output.item));
                        other_by_products.push((recipe_output, e, n));
                    }
                }

                let mut score = Score::default();
                let mut resources = Vec::new();
                for input in &recipe.inputs {
                    let scale = input.value * building_count / NORMALIZED_OUTPUT;
                    let (child_score, child_resources) =
                        self.create_children(idx, Rc::clone(&input.item));

                    score += child_score * scale;
                    resources = resource_combinations(&resources, &child_resources);
                }
                score.add_production_step(&recipe, building_count);
                resources.sort_unstable();
                let resources: Rc<[ItemBitSet]> = resources.into();

                for (recipe_output, e, n) in other_by_products {
                    let score_scale = NORMALIZED_OUTPUT / (recipe_output.value * building_count);
                    let edge_weight = ScoredNodeEdge::new(
                        Rc::clone(&recipe_output.item),
                        score * score_scale,
                        Rc::clone(&resources),
                    );
                    self.graph[n]
                        .as_by_product_mut()
                        .copy_score(&edge_weight, true);
                    self.graph[e] = edge_weight;
                }

                let edge_weight =
                    ScoredNodeEdge::new(Rc::clone(&item), score, Rc::clone(&resources));
                self.graph.add_edge(idx, parent_idx, edge_weight);

                (score, resources)
            }
        }
    }

    pub fn create_partial_by_product_node(
        &mut self,
        child_idx: NodeIndex,
        item: Rc<Item>,
    ) -> (EdgeIndex, NodeIndex) {
        let idx = self
            .graph
            .add_node(ScoredNodeValue::new_by_product(Rc::clone(&item)));

        let edge_idx = self
            .graph
            .add_edge(child_idx, idx, ScoredNodeEdge::without_score(item));

        (edge_idx, idx)
    }

    pub fn output_child(&self, idx: NodeIndex) -> Option<NodeIndex> {
        self.graph.neighbors_directed(idx, Incoming).next()
    }

    pub fn production_children(&self, idx: NodeIndex) -> Vec<(EdgeIndex, NodeIndex)> {
        let mut children: Vec<(EdgeIndex, NodeIndex)> = self
            .graph
            .edges_directed(idx, Incoming)
            .map(|e| (e.id(), e.source()))
            .collect();

        children.sort_unstable_by_key(|(e, _)| self.graph[*e].unique_resources);

        let recipe = self.graph[idx].as_production();
        assert!(
            recipe.inputs.len() == children.len(),
            "Missing child nodes on production node {:?}",
            self.graph[idx]
        );
        children
    }

    pub fn by_product_children(&self, idx: NodeIndex) -> Vec<(EdgeIndex, NodeIndex)> {
        let mut children: Vec<(EdgeIndex, NodeIndex)> = self
            .graph
            .edges_directed(idx, Incoming)
            .map(|e| (e.id(), e.source()))
            .collect();

        children.sort_unstable_by_key(|(e, _)| self.graph[*e].score);
        children
    }
}

impl<'a> Index<EdgeIndex> for ScoredGraph<'a> {
    type Output = ScoredNodeEdge;

    fn index(&self, index: EdgeIndex) -> &Self::Output {
        &self.graph[index]
    }
}

impl<'a> Index<NodeIndex> for ScoredGraph<'a> {
    type Output = ScoredNodeValue;

    fn index(&self, index: NodeIndex) -> &Self::Output {
        &self.graph[index]
    }
}

fn count_unique_resources(resource_combinations: &[ItemBitSet]) -> u32 {
    if resource_combinations.is_empty() {
        return 0;
    }

    let mut unique_resources = Vec::new();
    resource_combinations.iter().for_each(|a| {
        if !unique_resources
            .iter()
            .any(|b| a.is_subset_of(b) || b.is_subset_of(a))
        {
            unique_resources.push(*a);
        }
    });

    unique_resources.len() as u32
}

fn resource_combinations(left: &[ItemBitSet], right: &[ItemBitSet]) -> Vec<ItemBitSet> {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => vec![],
        (false, true) => Vec::from(right),
        (true, false) => Vec::from(left),
        (false, false) => {
            let mut combinations = Vec::with_capacity(right.len() * left.len());
            for i in left {
                for j in right {
                    let union = i.union(j);
                    if !combinations.contains(&union) {
                        combinations.push(union);
                    }
                }
            }

            combinations
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{game::test::get_test_game_db, plan::test::create_bit_set};

    use super::*;

    #[test]
    fn resource_combinations_both_empty() {}

    #[test]
    fn resource_combinations_left_empty() {}

    #[test]
    fn resource_combinations_right_empty() {}

    #[test]
    fn resource_combinations_simple() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let coal = game_db.find_item("Desc_Coal_C").unwrap();

        assert_eq!(
            resource_combinations(
                &vec![create_bit_set(&[&iron_ore])],
                &vec![create_bit_set(&[&coal])]
            ),
            vec![create_bit_set(&[&iron_ore, &coal])]
        );
    }

    #[test]
    fn resource_combinations_complex() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let coal = game_db.find_item("Desc_Coal_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        assert_eq!(
            resource_combinations(
                &vec![
                    create_bit_set(&[&iron_ore]),
                    create_bit_set(&[&iron_ore, &coal])
                ],
                &vec![
                    create_bit_set(&[&copper_ore]),
                    create_bit_set(&[&copper_ore, &water])
                ]
            ),
            vec![
                create_bit_set(&[&iron_ore, &copper_ore]),
                create_bit_set(&[&iron_ore, &copper_ore, &water]),
                create_bit_set(&[&iron_ore, &coal, &copper_ore]),
                create_bit_set(&[&iron_ore, &coal, &copper_ore, &water]),
            ]
        );
    }

    #[test]
    fn resource_combinations_dedupes() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        assert_eq!(
            resource_combinations(
                &vec![
                    create_bit_set(&[&iron_ore]),
                    create_bit_set(&[&iron_ore, &water]),
                ],
                &vec![
                    create_bit_set(&[&copper_ore]),
                    create_bit_set(&[&copper_ore, &water]),
                ]
            ),
            vec![
                create_bit_set(&[&iron_ore, &copper_ore]),
                create_bit_set(&[&iron_ore, &copper_ore, &water]),
            ],
        );
    }
}
