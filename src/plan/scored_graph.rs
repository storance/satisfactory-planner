use super::{
    find_by_product_child, find_by_product_node, find_input_node, find_production_node, ItemBitSet,
    NodeValue, PlanConfig,
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
use std::{cmp::Ordering, collections::HashMap, fmt, hash::Hash, ops::Index, rc::Rc, sync::atomic};

pub type ScoredGraphType = StableDiGraph<NodeValue, ScoredNodeEdge>;
pub type ChildrenByInput = Vec<(Rc<Item>, Vec<(EdgeIndex, NodeIndex)>)>;

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
    pub score: FloatType,
    pub chain: PathChain,
}

impl ScoredNodeEdge {
    #[inline]
    pub fn new(value: ItemValuePair, chain: PathChain) -> Self {
        Self {
            value,
            score: FloatType::INFINITY,
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
            round(self.score, 1),
            self.chain
        )
    }
}

#[derive(Debug, Clone)]
pub struct OutputNodeScore {
    pub output: ItemValuePair,
    pub index: NodeIndex,
    pub score: FloatType,
    pub unique_inputs: u8,
}

impl OutputNodeScore {
    #[inline]
    fn new(output: ItemValuePair, index: NodeIndex, score: FloatType, unique_inputs: u8) -> Self {
        Self {
            output,
            index,
            score,
            unique_inputs,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoredGraph<'a> {
    pub config: &'a PlanConfig,
    pub graph: ScoredGraphType,
    pub unique_inputs_by_item: HashMap<Rc<Item>, u8>,
    pub output_nodes: Vec<OutputNodeScore>,
}

impl<'a> ScoredGraph<'a> {
    #[inline]
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
            let node_index = self.graph.add_node(NodeValue::new_output(output.clone()));
            output_indices.push(node_index);
            self.create_children(node_index, output, &PathChain::empty());
        }

        let mut cached_inputs = HashMap::new();
        for node_index in output_indices {
            let output = self.graph[node_index].as_output().clone();
            let mut child_walker = self.graph.neighbors_directed(node_index, Incoming).detach();

            let mut score: FloatType = FloatType::INFINITY;
            while let Some((edge_index, _)) = child_walker.next(&self.graph) {
                score = score.min(self.score_edge(edge_index));
            }

            let item_combinations = self.calc_input_combinations(
                node_index,
                Rc::clone(&output.item),
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

        self.graph.add_edge(
            node_index,
            parent_index,
            ScoredNodeEdge::new(output.clone(), chain.next()),
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

        let recipe_output = recipe.find_output_by_item(&output.item).unwrap();
        let machine_count = output.ratio(recipe_output);
        let next_chain = chain.next();

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

        let recipe_output = recipe.find_output_by_item(&output.item).unwrap();
        let machine_count = output.ratio(recipe_output);
        let next_chain = chain.next();

        let node_index = match find_production_node(&self.graph, &recipe) {
            Some(existing_index) => {
                self.graph[existing_index].as_production_mut().machine_count += machine_count;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_production(Rc::clone(&recipe), machine_count)),
        };

        for o in &recipe.outputs {
            let by_product_parent_index: Option<NodeIndex> = if o.item == output.item {
                Some(parent_index)
            } else {
                None
            };
            self.create_by_product_node(
                by_product_parent_index,
                node_index,
                o.mul(machine_count),
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
        parent_index: Option<NodeIndex>,
        production_index: NodeIndex,
        output: ItemValuePair,
        chain: &PathChain,
    ) -> NodeIndex {
        let child_index = match find_by_product_node(&self.graph, &output.item) {
            Some(existing_index) => {
                *self.graph[existing_index].as_by_product_mut() += output.value;
                existing_index
            }
            None => self
                .graph
                .add_node(NodeValue::new_by_product(output.clone())),
        };

        if let Some(parent_index) = parent_index {
            self.graph.add_edge(
                child_index,
                parent_index,
                ScoredNodeEdge::new(output.clone(), chain.clone()),
            );
        }

        if let Some(edge_index) = self.graph.find_edge(production_index, child_index) {
            self.graph[edge_index].value += output;
        } else {
            self.graph.add_edge(
                production_index,
                child_index,
                ScoredNodeEdge::new(output.clone(), PathChain::empty()),
            );
        }

        child_index
    }

    pub fn score_edge(&mut self, edge_index: EdgeIndex) -> FloatType {
        let (child_index, _parent_index) = self.graph.edge_endpoints(edge_index).unwrap();
        let edge_weight = self.graph[edge_index].value.clone();

        let score = match self.graph[child_index] {
            NodeValue::ByProduct(..) => {
                let (child_edge_index, _) = find_by_product_child(child_index, &self.graph);
                self.score_edge(child_edge_index)
            }
            NodeValue::Input(..) => {
                if edge_weight.item.resource {
                    let input_limit = self.config.game_db.get_resource_limit(&edge_weight.item);
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
                let mut scores_by_input: HashMap<String, Vec<FloatType>> = HashMap::new();
                while let Some((child_edge_index, _)) = child_walker.next(&self.graph) {
                    if !self.is_same_path(edge_index, child_edge_index) {
                        continue;
                    }

                    let score = self.score_edge(child_edge_index);
                    scores_by_input
                        .entry(self.graph[child_edge_index].value.item.key.clone())
                        .or_default()
                        .push(score);
                }

                scores_by_input
                    .values()
                    .map(|scores| {
                        scores
                            .iter()
                            .copied()
                            .min_by(FloatType::total_cmp)
                            .unwrap_or(FloatType::INFINITY)
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
                    cached_inputs.insert(Rc::clone(&item), Rc::clone(&inputs_slice));
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
                let item_combinations = item_combinations.into();
                cached_inputs.insert(Rc::clone(&output_item), Rc::clone(&item_combinations));
                item_combinations
            }
            _ => Vec::new().into(),
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

    pub fn production_children(&self, node_index: NodeIndex, chain: &PathChain) -> ChildrenByInput {
        let production = self.graph[node_index].as_production();

        let mut children_by_item: HashMap<Rc<Item>, Vec<(EdgeIndex, NodeIndex)>> = production
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
                    .push((edge.id(), edge.source()));
            }
        }

        let mut sorted_children: ChildrenByInput = Vec::new();
        for (item, mut children_for_item) in children_by_item {
            children_for_item
                .sort_unstable_by(|a, b| self.graph[a.0].score.total_cmp(&self.graph[b.0].score));

            sorted_children.push((item, children_for_item));
        }
        sorted_children.sort_unstable_by_key(|(item, _)| {
            self.unique_inputs_by_item.get(item).copied().unwrap_or(0)
        });

        sorted_children
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
