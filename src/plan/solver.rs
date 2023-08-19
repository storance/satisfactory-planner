use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::{NodeValue, PlanConfig, ItemBitSet, find_production_node};

use petgraph::graph::{Graph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Incoming;
use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use super::{GraphType, ScoredGraphType, ScoredNodeValue, find_input_node, find_output_node};

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("There is not enough {0} to produce the desired outputs")]
    InsufficientInput(Item),
    #[error("There was no recipe found that can produce {0} and it was not supplied as an input.")]
    UncraftableItem(Item),
}

pub type SolverResult<T> = Result<T, SolverError>;

pub fn solve<'a>(config: &PlanConfig<'a>) -> SolverResult<GraphType<'a>> {
    Solver::new(config).solve()
}

struct Solver<'a, 'b> {
    config: &'b PlanConfig<'a>,
    recipes_by_output: HashMap<Item, Vec<&'a Recipe>>,
    recipes_by_input: HashMap<Item, Vec<&'a Recipe>>,
}

impl<'a, 'b> Solver<'a, 'b> where 'a: 'b {
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

    fn has_input(&self, item: Item) -> bool {
        if item.is_extractable() {
            self.config.input_limits.get(&item).copied().unwrap_or(0.0) > 0.0
        } else {
            self.config.inputs.get(&item).copied().unwrap_or(0.0) > 0.0
        }
    }

    fn get_limit(&self, item: Item) -> Option<f64> {
        if item.is_extractable() {
            self.config.input_limits.get(&item).copied()
        } else {
            self.config.inputs.get(&item).copied()
        }
    }

    fn find_recipe_by_output(&self, item: Item) -> &Vec<&'a Recipe> {
        static EMPTY_VEC: Vec<&Recipe> = Vec::new();

        self.recipes_by_output.get(&item).unwrap_or(&EMPTY_VEC)
    }

    pub fn solve(&self) -> SolverResult<GraphType<'a>> {
        let mut output_graphs: Vec<SingleOutputGraph<'a>> = self
            .config
            .outputs
            .iter()
            .map(|output| SingleOutputGraph::new(self, *output))
            .collect();
        output_graphs.sort_by(|a, b| {
            match a.unique_inputs.cmp(&b.unique_inputs) {
                Ordering::Equal => {}
                ord => return ord,
            }
    
            a.overall_score.total_cmp(&b.overall_score).reverse()
        });

        let mut solved_graph: GraphType<'a> = Graph::new();
        for output_graph in output_graphs {
            Self::merge_optimal_path(
                &output_graph.graph,
                output_graph.root_index,
                &mut solved_graph)?;
        }

        Ok(solved_graph)
    }

    fn merge_optimal_path(
        src_graph: &ScoredGraphType<'a>,
        node_index: NodeIndex,
        dest_graph: &mut GraphType<'a>,
    ) -> SolverResult<NodeIndex> {

        let dest_node_index;
        let mut children_by_items: HashMap<Item, Vec<NodeIndex>> = HashMap::new();

        match src_graph[node_index].node {
            NodeValue::Input(input) => {
                dest_node_index = Self::merge_input_node(input, dest_graph);
            },
            NodeValue::Output(output, by_product) => {
                dest_node_index = Self::merge_output_node(output, by_product, dest_graph);
                children_by_items.insert(output.item, Vec::new());
            },
            NodeValue::Production(recipe, machine_count) => {
                dest_node_index = Self::merge_production_node(recipe, machine_count, dest_graph);
                children_by_items.extend(recipe.inputs.iter().map(|input| (input.item, Vec::new())));
            }
        };

        for edge in src_graph.edges_directed(node_index, Incoming) {
            children_by_items.entry(edge.weight().item)
                .or_default()
                .push(edge.source());
        }

        for (item, children) in children_by_items {
            if children.is_empty() {
                return Err(SolverError::UncraftableItem(item));
            }

            let best_child_index = children.iter().copied().min_by(|a, b| {
                let a_score = src_graph[*a].score.unwrap_or(f64::INFINITY);
                let b_score = src_graph[*b].score.unwrap_or(f64::INFINITY);

                a_score.total_cmp(&b_score)
            }).unwrap();

            let new_child_index = Self::merge_optimal_path(src_graph, best_child_index, dest_graph)?;
            let edge_index = src_graph.find_edge(best_child_index, node_index).unwrap();
            let input_value: ItemValuePair<f64> = src_graph[edge_index];

            if let Some(existing_edge) = dest_graph.find_edge(new_child_index, dest_node_index) {
                dest_graph[existing_edge].value += input_value.value;
            } else {
                dest_graph.add_edge(new_child_index, dest_node_index, input_value);
            }
        }

        Ok(dest_node_index)
    }

    fn merge_input_node(
        input: ItemValuePair<f64>,
        dest_graph: &mut GraphType<'a>) -> NodeIndex {
        
        if let Some(existing_index) = find_input_node(dest_graph, input.item) {
            dest_graph[existing_index].as_input_mut().value += input.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::Input(input))
        }
    }

    fn merge_output_node(
        output: ItemValuePair<f64>,
        by_product: bool,
        dest_graph: &mut GraphType<'a>) -> NodeIndex {
        
        if let Some(existing_index) = find_output_node(dest_graph, output.item) {
            let (existing_output, _) = dest_graph[existing_index].as_output_mut();
            existing_output.value += output.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::Output(output, by_product))
        }
    }

    fn merge_production_node(
        recipe: &'a Recipe,
        machine_count: f64,
        dest_graph: &mut GraphType<'a>) -> NodeIndex {
        
        if let Some(existing_index) = find_production_node(dest_graph, recipe) {
            let (_, existing_machine_count) = dest_graph[existing_index].as_production_mut();
            *existing_machine_count += machine_count;

            existing_index
        } else {
            dest_graph.add_node(NodeValue::Production(recipe, machine_count))
        }
    }

    fn propagate_production_changes(
        node_index: NodeIndex,
        additional_output: ItemValuePair<f64>,
        graph: &mut GraphType<'a>) {
        let additional_inputs: Vec<ItemValuePair<f64>> = match &mut graph[node_index] {
            NodeValue::Production(recipe, machine_count) => {
                let output = recipe.find_output_by_item(additional_output.item).unwrap();
                let new_machine_count = additional_output.value / output.amount_per_minute;

                *machine_count += new_machine_count;

                recipe
                    .inputs
                    .iter()
                    .map(|input| {
                        ItemValuePair::new(input.item, input.amount_per_minute * new_machine_count)
                    })
                    .collect()
            }
            NodeValue::Input(input) => {
                input.value += additional_output.value;
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
            Self::propagate_production_changes(source_node_index, *input, graph);
        }
    }
}

struct SingleOutputGraph<'a> {
    output: ItemValuePair<f64>,
    graph: ScoredGraphType<'a>,
    root_index: NodeIndex,
    overall_score: f64,
    unique_inputs: usize,
}

impl<'a> SingleOutputGraph<'a> {
    pub fn new<'b>(solver: &Solver<'a, 'b>, output: ItemValuePair<f64>) -> Self {
        let (mut graph, root_index) = build_graph(solver, output);

        let overall_score = score_node(solver, &mut graph, root_index);
        let unique_inputs = count_unique_inputs(&graph, root_index);

        Self {
            output,
            graph,
            root_index,
            overall_score,
            unique_inputs,
        }
    }
}

fn build_graph<'a>(
    solver: &Solver<'a, '_>,
    output: ItemValuePair<f64>
) -> (ScoredGraphType<'a>, NodeIndex) {
    let mut graph = Graph::new();
    let output_node = ScoredNodeValue::from(NodeValue::new_output(output));
    let root_index = graph.add_node(output_node);

    let mut node_indices = vec![root_index];
    loop {
        node_indices = build_graph_level(solver, &mut graph, &node_indices);

        if node_indices
            .iter()
            .all(|node_index| graph[*node_index].node.is_input())
        {
            break;
        }
    }

    (graph, root_index)
}

fn build_graph_level<'a>(
    solver: &Solver<'a, '_>,
    graph: &mut ScoredGraphType<'a>,
    node_indices: &Vec<NodeIndex>,
) -> Vec<NodeIndex> {
    let mut next_nodes = Vec::new();

    for node_index in node_indices {
        let inputs_to_solve: Vec<ItemValuePair<f64>> =
            match graph[*node_index].node {
                NodeValue::Production(recipe, machine_count) => {
                    recipe.inputs.iter().map(|input| {
                        ItemValuePair::new(input.item, input.amount_per_minute * machine_count)
                    }).collect()
                }
                NodeValue::Output(output, ..) => vec![output],
                _ => vec![]
            };

        for input in inputs_to_solve {
            if solver.has_input(input.item) {
                next_nodes.push(create_input_node(input, *node_index, graph));
            }
            if !input.item.is_extractable() {
                next_nodes.extend(create_production_nodes(
                    solver,
                    input,
                    *node_index,
                    graph,
                ));
            }
        }
    }

    next_nodes
}

fn create_input_node(
    item_value: ItemValuePair<f64>,
    parent_index: NodeIndex,
    graph: &mut ScoredGraphType<'_>,
) -> NodeIndex {
    let child_node = ScoredNodeValue::from(NodeValue::new_input(item_value));
    let child_index = graph.add_node(child_node);
    graph.add_edge(child_index, parent_index, item_value);

    child_index
}

fn create_production_nodes<'a>(
    solver: &Solver<'a, '_>,
    item_value: ItemValuePair<f64>,
    parent_index: NodeIndex,
    graph: &mut ScoredGraphType<'a>,
) -> Vec<NodeIndex> {
    solver
        .find_recipe_by_output(item_value.item)
        .iter()
        .copied()
        .map(|recipe| {
            let output = recipe.find_output_by_item(item_value.item).unwrap();
            let machine_count = item_value.value / output.amount_per_minute;

            let child_node =
                ScoredNodeValue::from(NodeValue::new_production(recipe, machine_count));
            let child_index = graph.add_node(child_node);
            graph.add_edge(child_index, parent_index, item_value);

            child_index
        }).collect()
}

fn score_node(
    solver: &Solver,
    graph: &mut ScoredGraphType,
    node_index: NodeIndex,
) -> f64 {
    if let Some(score) = graph[node_index].score {
        return score;
    }

    let score = match graph[node_index].node {
        NodeValue::Input(input) => score_input_node(solver, &input),
        NodeValue::Production(recipe, ..) => score_production_node(solver, graph, node_index, recipe),
        NodeValue::Output(..) => score_output_node(solver, graph, node_index)
    };

    graph[node_index].score = Some(score);
    score
}

fn score_input_node(solver: &Solver, input: &ItemValuePair<f64>) -> f64 {
    if input.item.is_extractable() {
        let input_limit = solver.get_limit(input.item).unwrap();
        input.value / input_limit * 10000.0
    } else {
        0.0
    }
}

fn score_production_node(
    solver: &Solver,
    graph: &mut ScoredGraphType,
    node_index: NodeIndex,
    recipe: &Recipe) -> f64 {
    let mut scores_by_input: HashMap<Item, f64> = recipe
        .inputs
        .iter()
        .map(|input| (input.item, f64::INFINITY))
        .collect();

    let mut children = graph.neighbors_directed(node_index, Incoming).detach();
    while let Some((edge_index, child_index)) = children.next(graph) {
        let score = score_node(solver, graph, child_index);

        scores_by_input
            .entry(graph[edge_index].item)
            .and_modify(|e| *e = e.min(score))
            .or_insert(score);
    }

    scores_by_input.values().fold(0.0, |acc, f| acc + *f)
}

fn score_output_node(
    solver: &Solver,
    graph: &mut ScoredGraphType,
    node_index: NodeIndex,
) -> f64 {
    let mut score = f64::INFINITY;
    let mut children = graph.neighbors_directed(node_index, Incoming).detach();

    while let Some(child_node) = children.next_node(graph) {
        score = score.min(score_node(solver, graph, child_node));
    }

    score
}

fn count_unique_inputs(graph: &ScoredGraphType, node_index: NodeIndex) -> usize {
    let mut unique_inputs = Vec::new();
    calc_input_combinations(graph, node_index).iter().for_each(|a| {
        if !unique_inputs
            .iter()
            .any(|b| a.is_subset_of(b) || b.is_subset_of(a))
        {
            unique_inputs.push(*a);
        }
    });

    unique_inputs.len()
}

fn calc_input_combinations(graph: &ScoredGraphType, node_index: NodeIndex) -> Vec<ItemBitSet> {
    match graph[node_index].node {
        NodeValue::Input(input) => {
            if input.item.is_extractable() {
                vec![ItemBitSet::new(input.item)]
            } else {
                Vec::new()
            }
        }
        NodeValue::Production(_recipe, ..) => {
            let mut inputs_by_item: HashMap<Item, Vec<ItemBitSet>> = HashMap::new();
            graph.edges_directed(node_index, Incoming).for_each(|edge| {
                inputs_by_item
                    .entry(edge.weight().item)
                    .or_default()
                    .extend(calc_input_combinations(graph, edge.source()));
            });

            item_combinations(&inputs_by_item)
        }
        NodeValue::Output(..) => graph
            .neighbors_directed(node_index, Incoming)
            .flat_map(|child_index| calc_input_combinations(graph, child_index))
            .collect(),
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

    combinations
}

