use crate::game::{Recipe, Item, ItemValuePair};
use crate::plan::{PlanConfig, NodeValue};
use petgraph::dot::Dot;
use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::{Directed, Incoming};
use std::collections::HashMap;
use std::iter::repeat;
use std::slice::Iter;
use thiserror::Error;

use super::{ScoredNodeValue};

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Insufficient or missing input item `{0}`")]
    MissingInput(Item),
    #[error("No recipe found that produces item `{0}`")]
    NoMatchingRecipes(Item),
}

pub type GraphType<'a> = Graph<NodeValue<'a>, ItemValuePair<f64>, Directed>;
pub type ScoredGraphType<'a> = Graph<ScoredNodeValue<'a>, ItemValuePair<f64>, Directed>;
pub type SolverResult<T> = Result<T, SolverError>;

pub fn solve<'a>(config: &PlanConfig<'a>) -> SolverResult<GraphType<'a>> {
    Solver::new(config).solve()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ItemBitSet(u16);

struct Solver<'a, 'b> {
    config: &'b PlanConfig<'a>,
    recipes_by_output: HashMap<Item, Vec<&'a Recipe>>,
    recipes_by_input: HashMap<Item, Vec<&'a Recipe>>,
}

impl ItemBitSet {
    pub fn new(item: Item) -> Self {
        Self(Self::item_to_u16(item))
    }

    pub fn set(&mut self, item: Item) {
        self.0 |= Self::item_to_u16(item)
    }

    pub fn contains(&self, item: Item) -> bool {
        let item_bit = Self::item_to_u16(item);
        self.0 & item_bit == item_bit
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0 & self.0 == self.0
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    pub fn len(&self) -> u32 {
        self.0.count_ones()
    }

    fn item_to_u16(item: Item) -> u16 {
        match item {
            Item::Bauxite => 1,
            Item::CateriumOre => 2,
            Item::Coal => 4,
            Item::CopperOre => 8,
            Item::CrudeOil => 16,
            Item::IronOre => 32,
            Item::Limestone => 64,
            Item::NitrogenGas => 128,
            Item::RawQuartz => 256,
            Item::Sulfur =>  512,
            Item::Uranium => 1024,
            Item::Water => 2048,
            Item::SAMOre => 4096,
            _ => { panic!("Item `{}` not supported in ItemBitSet", item) },
        }
    }
}

impl<'a, 'b> Solver<'a, 'b> {
    pub fn new(config: &'b PlanConfig<'a>) -> Self {
        let mut recipes_by_output: HashMap<Item, Vec<&Recipe>> = HashMap::new();
        let mut recipes_by_input: HashMap<Item, Vec<&Recipe>> = HashMap::new();
        for recipe in &config.recipes {
            for output in &recipe.outputs {
                recipes_by_output
                    .entry(output.item)
                    .and_modify(|recipes| recipes.push(*recipe))
                    .or_insert_with(|| vec![*recipe]);
            }

            for input in &recipe.inputs {
                recipes_by_input
                    .entry(input.item)
                    .and_modify(|recipes| recipes.push(*recipe))
                    .or_insert_with(|| vec![*recipe]);
            }
        }

        Self {
            config,
            recipes_by_output,
            recipes_by_input,
        }
    }

    pub fn solve(&self) -> SolverResult<GraphType<'a>> {
        let mut graph = self.build_graph();

        self.score_graph(&mut graph);

        graph.node_indices().filter(|i| graph[*i].node.is_output()).for_each(|i| {
            if let NodeValue::OutputNode(item, _) = graph[i].node {
                println!("{}: {}", item.item, self.calculate_unique_inputs_count(&graph, i));
            }
            
        });

        // println!("{}", Dot::new(&graph));

        // get order outputs
        //let ordered_outputs = Vec::new();

        // find optimal path
        todo!()
    }

    pub fn build_graph(&self) -> ScoredGraphType<'a> {
        let mut graph = Graph::new();

        let mut nodes: Vec<NodeIndex> = Vec::new();

        self.config.outputs.iter().for_each(|output| {
            let output_node = ScoredNodeValue::from(NodeValue::new_output(*output, false));
            nodes.push(graph.add_node(output_node));
        });

        loop {
            nodes = self.build_next_tier(&mut graph, &nodes);

            if nodes.iter().all(|node_index| graph[*node_index].node.is_input()) {
                break;
            }
        };

        graph
    }

    pub fn build_next_tier(&self, graph: &mut ScoredGraphType<'a>, nodes: &Vec<NodeIndex>) -> Vec<NodeIndex> {
        let mut next_nodes = Vec::new();

        for node_index in nodes {
            let inputs_to_solve: Vec<ItemValuePair<f64>> = match graph[*node_index].node {
                NodeValue::OutputNode(item_value, ..) => vec![item_value],
                NodeValue::ProductionNode(recipe, machine_count) => recipe
                    .inputs
                    .iter()
                    .map(|input| ItemValuePair::new(input.item, input.amount_per_minute * machine_count))
                    .collect(),
                _ => Vec::new(),
            };

            for input in inputs_to_solve {
                next_nodes.extend(if input.item.is_extractable() {
                    self.create_input_node(input,  *node_index, graph)
                } else if self.config.inputs.contains_key(&input.item) {
                   self.create_input_node(input,  *node_index, graph)
                } else {
                    self.create_production_nodes(input, *node_index, graph)
                });
            }
        }

        next_nodes
    } 

    fn create_input_node(
        &self,
        item_value: ItemValuePair<f64>,
        parent_index: NodeIndex,
        graph: &mut ScoredGraphType<'a>,
    ) -> Vec<NodeIndex> {
        let mut child_nodes: Vec<NodeIndex> = Vec::new();

        let child_node = ScoredNodeValue::from(NodeValue::new_input(item_value));
        let child_index = graph.add_node(child_node);
        graph.add_edge(child_index, parent_index, item_value);
        child_nodes.push(child_index);

        if !item_value.item.is_extractable() {
            child_nodes.extend(self.create_production_nodes(item_value, parent_index, graph));
        }

        child_nodes
    }

    fn create_production_nodes(
        &self,
        item_value: ItemValuePair<f64>,
        parent_index: NodeIndex,
        graph: &mut ScoredGraphType<'a>,
    ) -> Vec<NodeIndex> {
        self.recipes_by_output.get(&item_value.item)
            .unwrap_or(&Vec::new())
            .iter()
            .map(|recipe| {
                let output = recipe.find_output_by_item(item_value.item).unwrap();
                let machine_count = item_value.value / output.amount_per_minute;

                let child_node = ScoredNodeValue::from(NodeValue::new_production(*recipe, machine_count));
                let child_index = graph.add_node(child_node);
                graph.add_edge(child_index, parent_index, item_value);
                
                child_index
            })
            .collect()
    }

    fn score_graph(&self, graph: &mut ScoredGraphType<'a>) {
        let output_nodes: Vec<NodeIndex> = graph.node_indices()
            .filter(|i| graph[*i].node.is_output())
            .collect();

        for output_node in output_nodes {
            self.score_node(graph, output_node);
        }
    }

    // TODO: track visited nodes
    fn score_node(&self, graph: &mut ScoredGraphType<'a>, node: NodeIndex) -> f64 {
        if let Some(score) = graph[node].score {
            return score;
        }

        let score = match graph[node].node {
            NodeValue::InputNode(input) => 
                if input.item.is_extractable() {
                    let input_limit = self.config.input_limits.get(&input.item).unwrap();
                    input.value / input_limit * 10000.0
                } else {
                    let input_limit = self.config.inputs.get(&input.item).unwrap();
                    input.value / input_limit * 10000.0
                },
            NodeValue::ProductionNode( recipe, .. ) => {
                let mut children = graph.neighbors_directed(node, Incoming).detach();

                let mut scores_by_input: HashMap<Item, f64> = recipe.inputs.iter()
                    .map(|input| (input.item, f64::INFINITY))
                    .collect();

                while let Some(child_node) = children.next_node(graph) {
                    if let Some(edge) = graph.find_edge(child_node, node) {
                        let score = self.score_node(graph, child_node);
                        scores_by_input.entry(graph[edge].item)
                            .and_modify(|e| { *e = e.min(score); })
                            .or_insert(score);
                    } else {
                        panic!("Missing edge between {:?} and {:?}", node, child_node);
                    }
                }

                scores_by_input.values().fold(0.0, |acc, f| acc + *f)
            },
            NodeValue::OutputNode( .. ) =>  {
                let mut score = f64::INFINITY;
                let mut children = graph.neighbors_directed(node, Incoming).detach();

                while let Some(child_node) = children.next_node(graph) {
                    let child_score = self.score_node(graph, child_node);

                    
                    if child_score < score {
                        score = child_score
                    }
                }

                score
            }
        };

        graph[node].score = Some(score);
        score
    }

    fn calculate_unique_inputs_count(&self, graph: &ScoredGraphType<'a>, node: NodeIndex) -> usize {
        let mut unique_inputs = Vec::new();
        self.calculate_inputs(graph, node).iter().for_each(|a| {
            if !unique_inputs.iter().any(|b| a.is_subset_of(b) || b.is_subset_of(a)) {
                unique_inputs.push(*a);
            }
        });

        unique_inputs.len()
    }

    fn calculate_inputs(&self, graph: &ScoredGraphType<'a>, node: NodeIndex) -> Vec<ItemBitSet> {
        match graph[node].node {
            NodeValue::InputNode(input) => {
                if input.item.is_extractable() {
                    vec![ItemBitSet::new(input.item)]
                } else {
                    Vec::new()
                }
            },
            NodeValue::ProductionNode( recipe, .. ) => {
                let mut input_items: HashMap<Item, Vec<ItemBitSet>> = HashMap::new();
                graph.edges_directed(node, Incoming)
                    .for_each(|edge| {
                        input_items.entry(edge.weight().item)
                            .or_default()
                            .extend(self.calculate_inputs(graph, edge.source()));
                    });

                let mut input_combinations: Vec<ItemBitSet> = Vec::new();
                for inputs in input_items.values() {
                    let prev_combinations = input_combinations;
                    if prev_combinations.is_empty() {
                        input_combinations = inputs.clone();
                    } else {
                        input_combinations = Vec::with_capacity(prev_combinations.len() * inputs.len());
                        for prev_combination in &prev_combinations {
                            for input in inputs {
                                input_combinations.push(prev_combination.union(input));
                            }
                        }
                    }
                }

                input_combinations
            },
            NodeValue::OutputNode( .. ) => {
                graph.neighbors_directed(node, Incoming)
                    .flat_map(|child_index| self.calculate_inputs(graph, child_index))
                    .collect()
            }
        }
    }

    fn propagate_production_changes(
        &self,
        node_index: NodeIndex,
        additional_output: ItemValuePair<f64>,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<()> {
        let additional_inputs: Vec<ItemValuePair<f64>> = match &mut graph[node_index] {
            NodeValue::ProductionNode(existing_recipe, existing_machine_count) => {
                let output = existing_recipe
                    .find_output_by_item(additional_output.item)
                    .unwrap();
                let machine_count = additional_output.value / output.amount_per_minute;

                *existing_machine_count += machine_count;

                existing_recipe
                    .inputs
                    .iter()
                    .map(|input| {
                        ItemValuePair::new(
                            input.item,
                            input.amount_per_minute * machine_count,
                        )
                    })
                    .collect()
            }
            NodeValue::InputNode(resource_value) => {
                resource_value.value += additional_output.value;
                Vec::new()
            }
            _ => {
                panic!("Unexpected node");
            }
        };

        let mut walker = graph.neighbors_directed(node_index, Incoming).detach();
        while let Some((edge_index, source_node_index)) = walker.next(graph) {
            let item = graph[edge_index].item;
            let input = additional_inputs
                .iter()
                .find(|input| input.item == item)
                .unwrap();

            graph[edge_index].value += input.value;
            self.propagate_production_changes(source_node_index, *input, graph)?;
        }

        Ok(())
    }
}

fn find_input_node<'a>(graph: &GraphType<'a>, item: Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_input_for_item(item))
}

fn find_production_node<'a>(graph: &GraphType<'a>, recipe: &'a Recipe) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_production_for_recipe(recipe))
}
