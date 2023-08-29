use super::{
    find_by_product_node, find_input_node, find_production_node, ItemBitSet, NodeValue, PlanConfig,
};
use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::{round, FloatType},
};
use im::Vector;
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex, StableDiGraph},
    visit::EdgeRef,
    Direction::Incoming,
};
use std::{
    cmp::Ordering,
    collections::HashMap,
    fmt,
    hash::Hash,
    ops::{Add, Index},
    rc::Rc,
};

pub type ScoredGraphType = StableDiGraph<NodeValue, ScoredNodeEdge>;
pub type ChildrenByInput = Vec<(Rc<Item>, Vec<EdgeIndex>)>;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct PathChain(Vector<u32>);

#[derive(Debug, Default)]
pub struct PathChainGenerator(u32);

#[derive(Debug, Clone)]
pub struct ScoredNodeEdge {
    pub value: ItemValuePair,
    pub score: Score,
    pub chain: PathChain,
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Score {
    pub resource_score: FloatType,
    pub power_score: FloatType,
    pub floor_area_score: FloatType,
    pub volume_score: FloatType,
}

#[derive(Debug, Clone)]
pub struct OutputNodeScore {
    pub output: ItemValuePair,
    pub index: NodeIndex,
    pub score: Score,
    pub unique_inputs: u8,
}

#[derive(Debug)]
pub struct ScoredGraph<'a> {
    pub config: &'a PlanConfig,
    pub graph: ScoredGraphType,
    pub path_chain_gen: PathChainGenerator,
    pub unique_inputs_by_item: HashMap<Rc<Item>, u8>,
    pub output_nodes: Vec<OutputNodeScore>,
}

impl PathChainGenerator {
    pub fn next(&mut self, chain: &PathChain) -> PathChain {
        let id = self.0;
        self.0 += 1;

        let mut new_chain = chain.0.clone();
        new_chain.push_back(id);
        PathChain(new_chain)
    }
}

impl PathChain {
    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0.len() >= self.0.len() && other.0.iter().zip(self.0.iter()).all(|(a, b)| a == b)
    }
}

impl fmt::Display for PathChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}]",
            self.0
                .iter()
                .map(|i| format!("{}", i))
                .collect::<Vec<String>>()
                .join(",")
        )
    }
}

impl ScoredNodeEdge {
    #[inline]
    pub fn new(value: ItemValuePair, chain: PathChain) -> Self {
        Self {
            value,
            score: Score::infinity(),
            chain,
        }
    }
}

impl fmt::Display for ScoredNodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{} / min\nScore: {}\nChain: {}",
            self.value.item,
            round(self.value.value, 3),
            self.score,
            self.chain
        )
    }
}

impl OutputNodeScore {
    #[inline]
    fn new(output: ItemValuePair, index: NodeIndex, score: Score, unique_inputs: u8) -> Self {
        Self {
            output,
            index,
            score,
            unique_inputs,
        }
    }
}

const RESOURCE_SCORE_SCALE_FACTOR: FloatType = 10_000.0;

impl Score {
    pub fn infinity() -> Self {
        Self {
            resource_score: FloatType::INFINITY,
            power_score: FloatType::INFINITY,
            floor_area_score: FloatType::INFINITY,
            volume_score: FloatType::INFINITY,
        }
    }

    pub fn for_input_node(input: &ItemValuePair, limit: FloatType) -> Self {
        let resource_score = if limit == 0.0 || limit.is_infinite() {
            0.0
        } else {
            input.value / limit * RESOURCE_SCORE_SCALE_FACTOR
        };

        Self {
            resource_score,
            power_score: 0.0,
            floor_area_score: 0.0,
            volume_score: 0.0,
        }
    }

    pub fn for_production_node(recipe: &Recipe, machine_count: FloatType) -> Score {
        let power_score = recipe.average_mw(100.0) * machine_count;
        let floor_area_score = recipe.building.floor_area() * machine_count.ceil();
        let volume_score = recipe.building.volume() * machine_count.ceil();

        Self {
            resource_score: 0.0,
            power_score,
            floor_area_score,
            volume_score,
        }
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
        self.volume_score.partial_cmp(&other.volume_score)
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
        }
    }
}

impl fmt::Display for Score {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}, {} MW, {} m^2, {} m^3)",
            round(self.resource_score, 1),
            round(self.power_score, 1),
            round(self.floor_area_score, 1),
            round(self.volume_score, 1)
        )
    }
}

impl<'a> ScoredGraph<'a> {
    #[inline]
    pub fn new(config: &'a PlanConfig) -> Self {
        Self {
            config,
            graph: ScoredGraphType::new(),
            unique_inputs_by_item: HashMap::new(),
            path_chain_gen: PathChainGenerator::default(),
            output_nodes: Vec::new(),
        }
    }

    pub fn build(&mut self) {
        let mut output_indices: Vec<NodeIndex> = Vec::new();
        for output in &self.config.outputs {
            let node_index = self.graph.add_node(NodeValue::new_output(output.clone()));
            output_indices.push(node_index);
            self.create_children(node_index, output, &PathChain::default());
        }

        let mut cached_inputs = HashMap::new();
        for node_index in output_indices {
            let output = self.graph[node_index].as_output().clone();
            let mut child_walker = self.graph.neighbors_directed(node_index, Incoming).detach();

            let mut score = Score::infinity();
            while let Some((edge_index, _)) = child_walker.next(&self.graph) {
                score = score.min(self.score_edge(edge_index));
            }

            let item_combinations = self.calc_input_combinations(
                node_index,
                Rc::clone(&output.item),
                &PathChain::default(),
                &mut cached_inputs,
            );
            self.output_nodes.push(OutputNodeScore::new(
                output,
                node_index,
                score,
                self.count_unique_inputs(&item_combinations),
            ));
        }

        for (item, inputs) in cached_inputs {
            self.unique_inputs_by_item
                .insert(item, self.count_unique_inputs(&inputs));
        }

        self.output_nodes.sort_by(|a, b| {
            match a.unique_inputs.cmp(&b.unique_inputs) {
                Ordering::Equal => {}
                ord => return ord,
            }

            a.score.cmp(&b.score).reverse()
        });
    }

    fn create_children(
        &mut self,
        parent_index: NodeIndex,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        if self.config.has_input(&output.item) {
            self.create_input_node(parent_index, output, chain);
        }

        if !output.item.resource {
            for recipe in self.config.game_db.find_recipes_by_output(&output.item) {
                self.create_production_node(parent_index, recipe, output, chain);
            }
        }
    }

    fn create_input_node(
        &mut self,
        parent_index: NodeIndex,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        let node_index = match find_input_node(&self.graph, &output.item) {
            Some(existing_index) => {
                *self.graph[existing_index].as_input_mut() += output;
                existing_index
            }
            None => self.graph.add_node(NodeValue::new_input(output.clone())),
        };

        let next_chain = self.next_path(chain);
        self.graph.add_edge(
            node_index,
            parent_index,
            ScoredNodeEdge::new(output.clone(), next_chain),
        );
    }

    fn create_production_node(
        &mut self,
        parent_index: NodeIndex,
        recipe: Rc<Recipe>,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        if recipe.outputs.len() == 1 {
            self.create_single_output_production_node(parent_index, recipe, output, chain);
        } else {
            self.create_multiple_output_production_node(parent_index, recipe, output, chain);
        }
    }

    fn create_single_output_production_node(
        &mut self,
        parent_index: NodeIndex,
        recipe: Rc<Recipe>,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        assert!(recipe.outputs.len() == 1);

        let machine_count = recipe.calc_buildings_for_output(output).unwrap();
        let next_chain = self.next_path(chain);

        let node_index = match find_production_node(&self.graph, &recipe) {
            Some(existing_index) => {
                self.graph[existing_index].as_production_mut().machine_count += machine_count;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_production(Rc::clone(&recipe), machine_count)),
        };
        self.graph.add_edge(
            node_index,
            parent_index,
            ScoredNodeEdge::new(output.clone(), next_chain.clone()),
        );

        for input in &recipe.inputs {
            let desired_output = input.mul(machine_count);
            self.create_children(node_index, &desired_output, &next_chain);
        }
    }

    pub fn create_multiple_output_production_node(
        &mut self,
        parent_index: NodeIndex,
        recipe: Rc<Recipe>,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        assert!(recipe.outputs.len() > 1);

        let machine_count = recipe.calc_buildings_for_output(output).unwrap();
        let next_chain = self.next_path(chain);

        let node_index = match find_production_node(&self.graph, &recipe) {
            Some(existing_index) => {
                self.graph[existing_index].as_production_mut().machine_count += machine_count;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_production(Rc::clone(&recipe), machine_count)),
        };

        for recipe_output in &recipe.outputs {
            let by_product_parent_index = if recipe_output.item == output.item {
                Some(parent_index)
            } else {
                None
            };
            self.create_by_product_node(
                by_product_parent_index,
                node_index,
                recipe_output.mul(machine_count),
                &next_chain,
            );
        }

        for input in &recipe.inputs {
            let desired_output = input.mul(machine_count);
            self.create_children(node_index, &desired_output, &next_chain);
        }
    }

    pub fn create_by_product_node(
        &mut self,
        parent_idx: Option<NodeIndex>,
        production_idx: NodeIndex,
        output: ItemValuePair,
        chain: &PathChain,
    ) -> NodeIndex {
        let by_product_idx = match find_by_product_node(&self.graph, &output.item) {
            Some(existing_index) => {
                *self.graph[existing_index].as_by_product_mut() += output.value;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_by_product(output.clone())),
        };

        if let Some(parent_index) = parent_idx {
            self.graph.add_edge(
                by_product_idx,
                parent_index,
                ScoredNodeEdge::new(output.clone(), chain.clone()),
            );
        }

        let next_chain = self.next_path(chain);
        self.graph.add_edge(
            production_idx,
            by_product_idx,
            ScoredNodeEdge::new(output.clone(), next_chain),
        );

        by_product_idx
    }

    pub fn score_edge(&mut self, edge_index: EdgeIndex) -> Score {
        let child_index = self.source_node(edge_index).unwrap();
        let output = self.graph[edge_index].value.clone();

        let score = match self.graph[child_index] {
            NodeValue::ByProduct(..) => {
                let child_edge_index = self
                    .by_product_edge(child_index, &self.graph[edge_index].chain)
                    .unwrap();
                self.score_edge(child_edge_index)
            }
            NodeValue::Input(..) => {
                if output.item.resource {
                    let input_limit = self.config.game_db.get_resource_limit(&output.item);
                    Score::for_input_node(&output, input_limit)
                } else {
                    Score::default()
                }
            }
            NodeValue::Production(..) => {
                let mut child_walker = self
                    .graph
                    .neighbors_directed(child_index, Incoming)
                    .detach();
                let mut scores_by_input: HashMap<String, Vec<Score>> = HashMap::new();
                while let Some(child_edge_index) = child_walker.next_edge(&self.graph) {
                    if !self.is_same_path(edge_index, child_edge_index) {
                        continue;
                    }

                    let score = self.score_edge(child_edge_index);
                    scores_by_input
                        .entry(self.graph[child_edge_index].value.item.key.clone())
                        .or_default()
                        .push(score);
                }

                if scores_by_input.is_empty() {
                    Score::infinity()
                } else {
                    let recipe = &self.graph[child_index].as_production().recipe;
                    let machine_count = recipe.calc_buildings_for_output(&output).unwrap();
                    let recipe_score = Score::for_production_node(recipe, machine_count);
                    scores_by_input
                        .values()
                        .map(|scores| scores.iter().min().copied().unwrap_or(Score::infinity()))
                        .fold(recipe_score, |acc, e| acc + e)
                }
            }
            NodeValue::Output(..) => panic!("Unexpectedly encountered an output node"),
        };

        self.graph[edge_index].score = score;
        score
    }

    fn next_path(&mut self, chain: &PathChain) -> PathChain {
        self.path_chain_gen.next(chain)
    }

    fn is_same_path(&self, parent_edge_index: EdgeIndex, child_edge_index: EdgeIndex) -> bool {
        let parent_weight = &self.graph[parent_edge_index];
        let child_weight = &self.graph[child_edge_index];

        parent_weight.chain.is_subset_of(&child_weight.chain)
    }

    fn count_unique_inputs(&self, input_combinations: &[ItemBitSet]) -> u8 {
        let mut unique_inputs = Vec::new();
        input_combinations.iter().for_each(|a| {
            if !unique_inputs
                .iter()
                .any(|b| a.is_subset_of(b) || b.is_subset_of(a))
            {
                unique_inputs.push(*a);
            }
        });

        unique_inputs.len() as u8
    }

    fn calc_input_combinations(
        &self,
        node_index: NodeIndex,
        output_item: Rc<Item>,
        chain: &PathChain,
        cached_inputs: &mut HashMap<Rc<Item>, Rc<[ItemBitSet]>>,
    ) -> Rc<[ItemBitSet]> {
        if let Some(existing) = cached_inputs.get(&output_item) {
            return Rc::clone(existing);
        }

        match &self.graph[node_index] {
            NodeValue::Input(input) => {
                assert!(output_item == input.item);
                if input.item.resource {
                    vec![ItemBitSet::new(&input.item)].into()
                } else {
                    Vec::new().into()
                }
            }
            NodeValue::Production(_production) => {
                let mut inputs_by_item: HashMap<Rc<Item>, Vec<ItemBitSet>> = HashMap::new();
                for edge in self.graph.edges_directed(node_index, Incoming) {
                    if !chain.is_subset_of(&edge.weight().chain) {
                        continue;
                    }

                    let child_item = &edge.weight().value.item;
                    let child_inputs = self.calc_input_combinations(
                        edge.source(),
                        Rc::clone(child_item),
                        &edge.weight().chain,
                        cached_inputs,
                    );

                    inputs_by_item
                        .entry(Rc::clone(child_item))
                        .or_default()
                        .extend(child_inputs.iter());
                }

                inputs_by_item
                    .values_mut()
                    .for_each(|inputs| inputs.sort_unstable_by_key(|i| i.len()));

                let mut slice_inputs_by_item = HashMap::new();
                for (item, inputs) in inputs_by_item {
                    let inputs_slice = inputs.into();

                    cached_inputs
                        .entry(Rc::clone(&item))
                        .or_insert_with(|| Rc::clone(&inputs_slice));
                    slice_inputs_by_item.insert(item, inputs_slice);
                }

                item_combinations(&slice_inputs_by_item)
            }
            NodeValue::Output(..) => {
                let mut item_combinations: Vec<ItemBitSet> = Vec::new();
                for edge in self.graph.edges_directed(node_index, Incoming) {
                    item_combinations.extend(
                        self.calc_input_combinations(
                            edge.source(),
                            Rc::clone(&output_item),
                            &edge.weight().chain,
                            cached_inputs,
                        )
                        .iter(),
                    );
                }

                item_combinations.sort_unstable_by_key(|i| i.len());
                let item_combinations_slice = item_combinations.into();
                cached_inputs
                    .entry(Rc::clone(&output_item))
                    .or_insert_with(|| Rc::clone(&item_combinations_slice));
                item_combinations_slice
            }
            NodeValue::ByProduct(..) => {
                let child_edge_index = self.by_product_edge(node_index, chain).unwrap();
                let child_index = self.source_node(child_edge_index).unwrap();

                self.calc_input_combinations(
                    child_index,
                    Rc::clone(&output_item),
                    &self.graph[child_edge_index].chain,
                    cached_inputs,
                )
            }
        }
    }

    pub fn output_edges(&self, node_index: NodeIndex, chain: &PathChain) -> Vec<EdgeIndex> {
        assert!(self.graph[node_index].is_output());

        let mut children = Vec::new();

        for edge in self.graph.edges_directed(node_index, Incoming) {
            if chain.is_subset_of(&edge.weight().chain) {
                children.push(edge.id());
            }
        }

        children.sort_unstable_by(|a, b| self.graph[*a].score.cmp(&self.graph[*b].score));
        children
    }

    pub fn production_edges(&self, node_index: NodeIndex, chain: &PathChain) -> ChildrenByInput {
        let production = self.graph[node_index].as_production();

        let mut children_by_item: HashMap<Rc<Item>, Vec<EdgeIndex>> = production
            .recipe
            .inputs
            .iter()
            .map(|i| (Rc::clone(&i.item), Vec::new()))
            .collect();

        for edge in self.graph.edges_directed(node_index, Incoming) {
            if chain.is_subset_of(&edge.weight().chain) {
                let edge_item = &edge.weight().value.item;

                children_by_item
                    .entry(Rc::clone(edge_item))
                    .or_default()
                    .push(edge.id());
            }
        }

        let mut sorted_children: ChildrenByInput = Vec::with_capacity(children_by_item.len());
        for (item, mut children) in children_by_item {
            children.sort_unstable_by(|a, b| self.graph[*a].score.cmp(&self.graph[*b].score));

            sorted_children.push((item, children));
        }
        sorted_children.sort_unstable_by_key(|(item, _)| {
            self.unique_inputs_by_item.get(item).copied().unwrap_or(0)
        });

        sorted_children
    }

    pub fn by_product_edge(&self, node_index: NodeIndex, chain: &PathChain) -> Option<EdgeIndex> {
        self.graph
            .edges_directed(node_index, Incoming)
            .find(|edge| chain.is_subset_of(&edge.weight().chain))
            .map(|edge| edge.id())
    }

    pub fn source_node(&self, edge_index: EdgeIndex) -> Option<NodeIndex> {
        self.graph.edge_endpoints(edge_index).map(|e| e.0)
    }
}

impl<'a> Index<EdgeIndex> for ScoredGraph<'a> {
    type Output = ScoredNodeEdge;

    fn index(&self, index: EdgeIndex) -> &ScoredNodeEdge {
        &self.graph[index]
    }
}

impl<'a> Index<NodeIndex> for ScoredGraph<'a> {
    type Output = NodeValue;

    fn index(&self, index: NodeIndex) -> &NodeValue {
        &self.graph[index]
    }
}

fn item_combinations<K: Eq + Hash>(
    inputs_by_item: &HashMap<K, Rc<[ItemBitSet]>>,
) -> Rc<[ItemBitSet]> {
    let mut combinations = Vec::new();
    if let Some(bit_sets) = inputs_by_item.values().next() {
        combinations.extend(bit_sets.iter());
    } else {
        return combinations.into();
    }

    for inputs in inputs_by_item.values().skip(1) {
        let prev_combinations = combinations;
        let capacity = prev_combinations.len() * inputs.len();
        combinations = Vec::with_capacity(capacity);

        for prev_combination in &prev_combinations {
            for input in inputs.iter() {
                combinations.push(prev_combination.union(input));
            }
        }
    }

    combinations.sort_unstable();
    combinations.dedup();
    combinations.into()
}

#[cfg(test)]
mod test {
    use crate::{game::test::get_test_game_db, plan::test::create_bit_set};

    use super::*;

    #[test]
    fn path_chain_generator_next() {
        let mut chain = PathChain::default();
        let mut gen = PathChainGenerator::default();

        chain = gen.next(&chain);
        assert_eq!(chain, PathChain(Vector::from(vec![0])));

        chain = gen.next(&chain);
        assert_eq!(chain, PathChain(Vector::from(vec![0, 1])));

        chain = gen.next(&chain);
        assert_eq!(chain, PathChain(Vector::from(vec![0, 1, 2])));

        gen.next(&PathChain::default());
        gen.next(&PathChain::default());

        chain = gen.next(&chain);
        assert_eq!(chain, PathChain(Vector::from(vec![0, 1, 2, 5])));
    }

    #[test]
    fn path_chain_is_subset_of() {
        let chain = PathChain(Vector::from(vec![1, 2, 3]));

        assert!(PathChain(Vector::from(vec![1])).is_subset_of(&chain));
        assert!(PathChain(Vector::from(vec![1, 2])).is_subset_of(&chain));
        assert!(PathChain(Vector::from(vec![1, 2, 3])).is_subset_of(&chain));
        assert!(!PathChain(Vector::from(vec![1, 2, 3, 4])).is_subset_of(&chain));
        assert!(!PathChain(Vector::from(vec![5, 6])).is_subset_of(&chain));
        assert!(!PathChain(Vector::from(vec![1, 2, 7])).is_subset_of(&chain));
    }

    #[test]
    fn test_item_combinations_two_inputs_simple() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let coal = game_db.find_item("Desc_Coal_C").unwrap();

        let mut inputs_by_item: HashMap<String, Rc<[ItemBitSet]>> = HashMap::new();
        inputs_by_item.insert(
            iron_ore.key.clone(),
            vec![create_bit_set(&[&iron_ore])].into(),
        );
        inputs_by_item.insert(coal.key.clone(), vec![create_bit_set(&[&coal])].into());

        assert_eq!(
            item_combinations(&inputs_by_item),
            vec![create_bit_set(&[&iron_ore, &coal])].into()
        );
    }

    #[test]
    fn test_item_combinations_two_inputs_dedupes() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        let mut inputs_by_item: HashMap<String, Rc<[ItemBitSet]>> = HashMap::new();
        inputs_by_item.insert(
            String::from("Desc_IronIngot_C"),
            vec![
                create_bit_set(&[&iron_ore]),
                create_bit_set(&[&iron_ore, &water]),
            ]
            .into(),
        );
        inputs_by_item.insert(
            String::from("Desc_CopperIngot_C"),
            vec![
                create_bit_set(&[&copper_ore]),
                create_bit_set(&[&copper_ore, &water]),
            ]
            .into(),
        );

        assert_eq!(
            item_combinations(&inputs_by_item),
            vec![
                create_bit_set(&[&iron_ore, &copper_ore]),
                create_bit_set(&[&iron_ore, &copper_ore, &water]),
            ]
            .into(),
        );
    }

    #[test]
    fn test_item_combinations_three_inputs() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let limestone = game_db.find_item("Desc_Stone_C").unwrap();
        let raw_quartz = game_db.find_item("Desc_RawQuartz_C").unwrap();
        let coal = game_db.find_item("Desc_Coal_C").unwrap();
        let bauxite = game_db.find_item("Desc_OreBauxite_C").unwrap();

        let mut inputs_by_item: HashMap<String, Rc<[ItemBitSet]>> = HashMap::new();
        inputs_by_item.insert(
            String::from("Desc_IronIngot_C"),
            vec![
                create_bit_set(&[&iron_ore]),
                create_bit_set(&[&iron_ore, &water]),
            ]
            .into(),
        );
        inputs_by_item.insert(
            String::from("Desc_Wire_C"),
            vec![
                create_bit_set(&[&copper_ore]),
                create_bit_set(&[&caterium_ore]),
            ]
            .into(),
        );

        inputs_by_item.insert(
            String::from("Desc_AluminumCasing_C"),
            vec![
                create_bit_set(&[&bauxite, &coal, &raw_quartz]),
                create_bit_set(&[&bauxite, &coal, &raw_quartz, &limestone]),
            ]
            .into(),
        );

        let mut expected = vec![
            create_bit_set(&[&iron_ore, &copper_ore, &bauxite, &coal, &raw_quartz]),
            create_bit_set(&[
                &iron_ore,
                &copper_ore,
                &bauxite,
                &coal,
                &raw_quartz,
                &limestone,
            ]),
            create_bit_set(&[&iron_ore, &caterium_ore, &bauxite, &coal, &raw_quartz]),
            create_bit_set(&[
                &iron_ore,
                &caterium_ore,
                &bauxite,
                &coal,
                &raw_quartz,
                &limestone,
            ]),
            create_bit_set(&[&iron_ore, &water, &copper_ore, &bauxite, &coal, &raw_quartz]),
            create_bit_set(&[
                &iron_ore,
                &water,
                &copper_ore,
                &bauxite,
                &coal,
                &raw_quartz,
                &limestone,
            ]),
            create_bit_set(&[
                &iron_ore,
                &water,
                &caterium_ore,
                &bauxite,
                &coal,
                &raw_quartz,
            ]),
            create_bit_set(&[
                &iron_ore,
                &water,
                &caterium_ore,
                &bauxite,
                &coal,
                &raw_quartz,
                &limestone,
            ]),
        ];
        expected.sort_unstable();

        assert_eq!(item_combinations(&inputs_by_item), expected.into());
    }
}
