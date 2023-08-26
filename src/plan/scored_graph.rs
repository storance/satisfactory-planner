use super::{
    find_input_node, find_production_node, ItemBitSet, NodeValue, PlanConfig, DEFAULT_LIMITS,
};
use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::round_f64,
};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex, StableDiGraph},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::{cmp::Ordering, collections::HashMap, fmt, ops::Index, sync::atomic};

pub type ScoredGraphType<'a> = StableDiGraph<NodeValue<'a>, ScoredNodeEdge>;

#[derive(Debug, Clone)]
pub struct PathChain(Vec<u32>);

static ID_GENERATOR: atomic::AtomicU32 = atomic::AtomicU32::new(0);

#[allow(dead_code)]
impl PathChain {
    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn next(&self) -> Self {
        let mut chain = self.0.clone();
        let id = ID_GENERATOR.fetch_add(1, atomic::Ordering::Relaxed);
        chain.push(id);

        Self(chain)
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0.starts_with(self.0.as_slice())
    }

    pub fn id(&self) -> u32 {
        self.0.last().copied().unwrap()
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

#[derive(Debug, Clone)]
pub struct ScoredNodeEdge {
    pub value: ItemValuePair,
    pub score: f64,
    pub chain: PathChain,
}

impl ScoredNodeEdge {
    pub fn new(value: ItemValuePair, chain: PathChain) -> Self {
        Self {
            value,
            score: f64::INFINITY,
            chain,
        }
    }

    #[inline]
    pub fn item(&self) -> Item {
        self.value.item
    }
}

impl fmt::Display for ScoredNodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{} / min\nScore: {}\nChain: {}",
            self.value.item,
            round_f64(self.value.value, 3),
            round_f64(self.score, 1),
            self.chain
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub struct OutputNodeScore {
    pub output: ItemValuePair,
    pub index: NodeIndex,
    pub score: f64,
    pub unique_inputs: u8,
}

impl OutputNodeScore {
    fn new(output: ItemValuePair, index: NodeIndex, score: f64, unique_inputs: u8) -> Self {
        Self {
            output,
            index,
            score,
            unique_inputs,
        }
    }
}

pub struct ScoredGraph<'a> {
    pub config: &'a PlanConfig,
    pub graph: ScoredGraphType<'a>,
    pub unique_inputs_by_item: HashMap<Item, u8>,
    pub output_nodes: Vec<OutputNodeScore>,
}

impl<'a> ScoredGraph<'a> {
    pub fn new(config: &'a PlanConfig) -> Self {
        Self {
            config,
            graph: ScoredGraphType::new(),
            unique_inputs_by_item: HashMap::new(),
            output_nodes: Vec::new(),
        }
    }

    pub fn build(&mut self) {
        let mut output_indices: Vec<NodeIndex> = Vec::new();
        for output in &self.config.outputs {
            let node_index = self.graph.add_node(NodeValue::new_output(*output));
            output_indices.push(node_index);
            self.create_children(node_index, output, &PathChain::empty());
        }

        let mut cached_inputs: HashMap<Item, Vec<ItemBitSet>> = HashMap::new();
        for node_index in output_indices {
            let output = *self.graph[node_index].as_output();
            let mut child_walker = self.graph.neighbors_directed(node_index, Incoming).detach();

            let mut score: f64 = f64::INFINITY;
            while let Some((edge_index, _)) = child_walker.next(&self.graph) {
                score = score.min(self.score_edge(edge_index));
            }

            let item_combinations = self.calc_input_combinations(
                node_index,
                output.item,
                &PathChain::empty(),
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

            a.score.total_cmp(&b.score).reverse()
        });
    }

    fn create_children(
        &mut self,
        parent_index: NodeIndex,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        if self.config.has_input(output.item) {
            self.create_input_node(parent_index, output, chain);
        }

        if !output.item.is_extractable() {
            for recipe in self.config.recipes.find_recipes_by_output(output.item) {
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
        let node_index = match find_input_node(&self.graph, output.item) {
            Some(existing_index) => {
                *self.graph[existing_index].as_input_mut() += output;
                existing_index
            }
            None => self.graph.add_node(NodeValue::new_input(*output)),
        };

        self.graph.add_edge(
            node_index,
            parent_index,
            ScoredNodeEdge::new(*output, chain.next()),
        );
    }

    fn create_production_node(
        &mut self,
        parent_index: NodeIndex,
        recipe: &'a Recipe,
        output: &ItemValuePair,
        chain: &PathChain,
    ) {
        let recipe_output = recipe.find_output_by_item(output.item).unwrap();
        let machine_count = *output / *recipe_output;
        let next_chain = chain.next();

        let node_index = match find_production_node(&self.graph, recipe) {
            Some(existing_index) => {
                self.graph[existing_index].as_production_mut().machine_count += machine_count;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_production(recipe, machine_count)),
        };
        self.graph.add_edge(
            node_index,
            parent_index,
            ScoredNodeEdge::new(*output, next_chain.clone()),
        );

        for output in &recipe.outputs {
            if recipe_output.item == output.item {
                continue;
            }
            self.create_by_product_node(node_index, *output * machine_count, &next_chain);
        }

        for input in &recipe.inputs {
            let desired_output = *input * machine_count;
            self.create_children(node_index, &desired_output, &next_chain);
        }
    }

    pub fn create_by_product_node(
        &mut self,
        parent_index: NodeIndex,
        output: ItemValuePair,
        chain: &PathChain,
    ) {
        let child_index = self.graph.add_node(NodeValue::new_by_product(output));
        self.graph.add_edge(
            parent_index,
            child_index,
            ScoredNodeEdge::new(output, chain.next()),
        );
    }

    pub fn score_edge(&mut self, edge_index: EdgeIndex) -> f64 {
        let (child_index, _parent_index) = self.graph.edge_endpoints(edge_index).unwrap();
        let edge_weight = self.graph[edge_index].value;

        let score = match self.graph[child_index] {
            NodeValue::ByProduct(..) => 0.0,
            NodeValue::Input(..) => {
                if edge_weight.item.is_extractable() {
                    let input_limit = DEFAULT_LIMITS
                        .iter()
                        .find(|(i, _)| *i == edge_weight.item)
                        .map(|(_, v)| *v)
                        .unwrap_or(0.0);
                    edge_weight.value / input_limit * 10000.0
                } else {
                    0.0
                }
            }
            NodeValue::Production(..) => {
                let mut child_walker = self
                    .graph
                    .neighbors_directed(child_index, Incoming)
                    .detach();
                let mut scores_by_input: HashMap<Item, Vec<f64>> = HashMap::new();
                while let Some((child_edge_index, _)) = child_walker.next(&self.graph) {
                    if !self.is_same_path(edge_index, child_edge_index) {
                        continue;
                    }

                    let score = self.score_edge(child_edge_index);
                    scores_by_input
                        .entry(self.graph[child_edge_index].value.item)
                        .or_default()
                        .push(score);
                }

                scores_by_input
                    .values()
                    .map(|scores| {
                        scores
                            .iter()
                            .copied()
                            .min_by(f64::total_cmp)
                            .unwrap_or(f64::INFINITY)
                    })
                    .sum()
            }
            NodeValue::Output(..) => panic!("Unexpectedly encountered an output node"),
        };

        self.graph[edge_index].score = score;
        score
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
        output_item: Item,
        chain: &PathChain,
        cached_inputs: &mut HashMap<Item, Vec<ItemBitSet>>,
    ) -> Vec<ItemBitSet> {
        if let Some(existing) = cached_inputs.get(&output_item) {
            return existing.clone();
        }

        match self.graph[node_index] {
            NodeValue::Input(input) => {
                if input.item.is_extractable() {
                    vec![ItemBitSet::new(input.item)]
                } else {
                    Vec::new()
                }
            }
            NodeValue::Production(_production) => {
                let mut inputs_by_item: HashMap<Item, Vec<ItemBitSet>> = HashMap::new();
                for edge in self.graph.edges_directed(node_index, Incoming) {
                    if !chain.is_subset_of(&edge.weight().chain) {
                        continue;
                    }

                    let child_item = edge.weight().value.item;
                    let child_inputs = self.calc_input_combinations(
                        edge.source(),
                        child_item,
                        &edge.weight().chain,
                        cached_inputs,
                    );

                    inputs_by_item
                        .entry(child_item)
                        .or_default()
                        .extend(child_inputs);
                }

                for (item, inputs) in &mut inputs_by_item {
                    inputs.sort_by_key(|i| i.len());
                    cached_inputs.insert(*item, inputs.clone());
                }

                item_combinations(&inputs_by_item)
            }
            NodeValue::Output(..) => {
                let mut item_combinations: Vec<ItemBitSet> = Vec::new();
                for edge in self.graph.edges_directed(node_index, Incoming) {
                    item_combinations.extend(self.calc_input_combinations(
                        edge.source(),
                        output_item,
                        &edge.weight().chain,
                        cached_inputs,
                    ));
                }

                item_combinations.sort_by_key(|i| i.len());
                cached_inputs.insert(output_item, item_combinations.clone());
                item_combinations
            }
            _ => Vec::new(),
        }
    }

    pub fn output_children(
        &self,
        node_index: NodeIndex,
        chain: &PathChain,
    ) -> Vec<(EdgeIndex, NodeIndex)> {
        assert!(self.graph[node_index].is_output());

        let mut children: Vec<(EdgeIndex, NodeIndex)> = Vec::new();

        for edge in self.graph.edges_directed(node_index, Incoming) {
            if chain.is_subset_of(&edge.weight().chain) {
                children.push((edge.id(), edge.source()));
            }
        }

        children.sort_by(|a, b| self.graph[a.0].score.total_cmp(&self.graph[b.0].score));
        children
    }

    pub fn production_children(
        &self,
        node_index: NodeIndex,
        chain: &PathChain,
    ) -> Vec<(Item, Vec<(EdgeIndex, NodeIndex)>)> {
        let production = self.graph[node_index].as_production();

        let mut children_by_item: HashMap<Item, Vec<(EdgeIndex, NodeIndex)>> = production
            .recipe
            .inputs
            .iter()
            .map(|i| (i.item, Vec::new()))
            .collect();

        for edge in self.graph.edges_directed(node_index, Incoming) {
            if chain.is_subset_of(&edge.weight().chain) {
                let edge_item = edge.weight().value.item;

                children_by_item
                    .entry(edge_item)
                    .or_default()
                    .push((edge.id(), edge.source()));
            }
        }

        let mut sorted_children: Vec<(Item, Vec<(EdgeIndex, NodeIndex)>)> = Vec::new();
        for (item, mut children_for_item) in children_by_item {
            children_for_item
                .sort_by(|a, b| self.graph[a.0].score.total_cmp(&self.graph[b.0].score));

            sorted_children.push((item, children_for_item));
        }
        sorted_children
            .sort_by_key(|(item, _)| self.unique_inputs_by_item.get(item).copied().unwrap_or(0));

        sorted_children
    }

    pub fn production_by_products(
        &self,
        node_index: NodeIndex,
        chain: &PathChain,
    ) -> Vec<(EdgeIndex, NodeIndex)> {
        let mut children: Vec<(EdgeIndex, NodeIndex)> = Vec::new();

        for edge in self.graph.edges_directed(node_index, Outgoing) {
            if !self.graph[edge.target()].is_by_product()
                || !chain.is_subset_of(&edge.weight().chain)
            {
                continue;
            }

            children.push((edge.id(), edge.target()));
        }

        children
    }
}

impl<'a> Index<EdgeIndex> for ScoredGraph<'a> {
    type Output = ScoredNodeEdge;

    fn index(&self, index: EdgeIndex) -> &ScoredNodeEdge {
        &self.graph[index]
    }
}

impl<'a> Index<NodeIndex> for ScoredGraph<'a> {
    type Output = NodeValue<'a>;

    fn index(&self, index: NodeIndex) -> &NodeValue<'a> {
        &self.graph[index]
    }
}

fn item_combinations(inputs_by_item: &HashMap<Item, Vec<ItemBitSet>>) -> Vec<ItemBitSet> {
    let mut combinations: Vec<ItemBitSet> = inputs_by_item
        .values()
        .next()
        .cloned()
        .unwrap_or(Vec::new());

    for inputs in inputs_by_item.values().skip(1) {
        let prev_combinations = combinations;
        let capacity = prev_combinations.len() * inputs.len();
        combinations = Vec::with_capacity(capacity);

        for prev_combination in &prev_combinations {
            for input in inputs {
                combinations.push(prev_combination.union(input));
            }
        }
    }

    combinations.sort();
    combinations.dedup();
    combinations
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_item_combinations_two_inputs_simple() {
        let mut inputs_by_item = HashMap::new();
        inputs_by_item.insert(Item::IronIngot, vec![ItemBitSet::from(&[Item::IronOre])]);
        inputs_by_item.insert(Item::Coal, vec![ItemBitSet::from(&[Item::Coal])]);

        assert_eq!(
            item_combinations(&inputs_by_item),
            vec![ItemBitSet::from(&[Item::IronOre, Item::Coal])]
        );
    }

    #[test]
    fn test_item_combinations_two_inputs_dedupes() {
        let mut inputs_by_item = HashMap::new();
        inputs_by_item.insert(
            Item::IronIngot,
            vec![
                ItemBitSet::from(&[Item::IronOre]),
                ItemBitSet::from(&[Item::IronOre, Item::Water]),
            ],
        );
        inputs_by_item.insert(
            Item::CopperIngot,
            vec![
                ItemBitSet::from(&[Item::CopperOre]),
                ItemBitSet::from(&[Item::CopperOre, Item::Water]),
            ],
        );

        assert_eq!(
            item_combinations(&inputs_by_item),
            vec![
                ItemBitSet::from(&[Item::IronOre, Item::CopperOre]),
                ItemBitSet::from(&[Item::IronOre, Item::CopperOre, Item::Water]),
            ],
        );
    }

    #[test]
    fn test_item_combinations_three_inputs() {
        let mut inputs_by_item = HashMap::new();
        inputs_by_item.insert(
            Item::IronIngot,
            vec![
                ItemBitSet::from(&[Item::IronOre]),
                ItemBitSet::from(&[Item::IronOre, Item::Water]),
            ],
        );
        inputs_by_item.insert(
            Item::CopperIngot,
            vec![
                ItemBitSet::from(&[Item::CopperOre]),
                ItemBitSet::from(&[Item::CateriumOre]),
            ],
        );

        inputs_by_item.insert(
            Item::AluminumCasing,
            vec![
                ItemBitSet::from(&[Item::Bauxite, Item::Coal, Item::RawQuartz]),
                ItemBitSet::from(&[Item::Bauxite, Item::Coal, Item::RawQuartz, Item::Limestone]),
            ],
        );

        let mut expected = vec![
            ItemBitSet::from(&[
                Item::IronOre,
                Item::CopperOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::CopperOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
                Item::Limestone,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::CateriumOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::CateriumOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
                Item::Limestone,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::Water,
                Item::CopperOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::Water,
                Item::CopperOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
                Item::Limestone,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::Water,
                Item::CateriumOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
            ]),
            ItemBitSet::from(&[
                Item::IronOre,
                Item::Water,
                Item::CateriumOre,
                Item::Bauxite,
                Item::Coal,
                Item::RawQuartz,
                Item::Limestone,
            ]),
        ];
        expected.sort();

        assert_eq!(item_combinations(&inputs_by_item), expected);
    }
}
