use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::{find_production_node, ItemBitSet, NodeValue, PlanConfig};

use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Incoming;
use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use super::{find_input_node, find_output_node, GraphType, ScoredGraphType, ScoredNodeValue, Production, find_by_product_node};

#[derive(Error, Debug)]
#[allow(dead_code)]
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

#[allow(dead_code)]
struct Solver<'a, 'b> {
    config: &'b PlanConfig<'a>,
    recipes_by_output: HashMap<Item, Vec<&'a Recipe>>,
    recipes_by_input: HashMap<Item, Vec<&'a Recipe>>,
}

impl<'a, 'b> Solver<'a, 'b>
where
    'a: 'b,
{
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

        let mut solved_graph: GraphType<'a> = GraphType::new();
        for output_graph in output_graphs {
            Self::merge_optimal_path(
                &output_graph.graph,
                output_graph.root_index,
                &mut solved_graph,
            )?;
        }

        Ok(solved_graph)
    }

    fn merge_optimal_path(
        src_graph: &ScoredGraphType<'a>,
        node_index: NodeIndex,
        dest_graph: &mut GraphType<'a>,
    ) -> SolverResult<NodeIndex> {
        let children_by_items: HashMap<Item, Vec<NodeIndex>> = group_children_by_input(node_index, src_graph);

        let dest_node_index = match src_graph[node_index].node {
            NodeValue::Input(input) => Self::merge_input_node(input, dest_graph),
            NodeValue::Output(output) => Self::merge_output_node(output, dest_graph),
            NodeValue::ByProduct(output) => Self::merge_by_product_node(output, dest_graph),
            NodeValue::Production(production) => Self::merge_production_node(production, dest_graph),
        };

        for (item, children) in children_by_items {
            if children.is_empty() {
                return Err(SolverError::UncraftableItem(item));
            }

            let best_child_index = children
                .iter()
                .copied()
                .min_by(|a, b| src_graph[*a].score.total_cmp(&src_graph[*b].score))
                .unwrap();

            let new_child_index =
                Self::merge_optimal_path(src_graph, best_child_index, dest_graph)?;
            let edge_index = src_graph.find_edge(best_child_index, node_index).unwrap();
            let input_value: ItemValuePair = src_graph[edge_index];

            if let Some(existing_edge) = dest_graph.find_edge(new_child_index, dest_node_index) {
                dest_graph[existing_edge].value += input_value.value;
            } else {
                dest_graph.add_edge(new_child_index, dest_node_index, input_value);
            }
        }

        Ok(dest_node_index)
    }

    fn merge_input_node(input: ItemValuePair, dest_graph: &mut GraphType<'a>) -> NodeIndex {
        if let Some(existing_index) = find_input_node(dest_graph, input.item) {
            dest_graph[existing_index].as_input_mut().value += input.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::Input(input))
        }
    }

    fn merge_output_node(
        output: ItemValuePair,
        dest_graph: &mut GraphType<'a>,
    ) -> NodeIndex {
        if let Some(existing_index) = find_by_product_node(dest_graph, output.item) {
            dest_graph[existing_index].as_output_mut().value += output.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::new_by_product(output))
        }
    }

    fn merge_by_product_node(
        output: ItemValuePair,
        dest_graph: &mut GraphType<'a>,
    ) -> NodeIndex {
        if let Some(existing_index) = find_output_node(dest_graph, output.item) {
            dest_graph[existing_index].as_output_mut().value += output.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::new_output(output))
        }
    }

    fn merge_production_node(
        production: Production<'a>,
        dest_graph: &mut GraphType<'a>,
    ) -> NodeIndex {
        if let Some(existing_index) = find_production_node(dest_graph, production.recipe) {
            dest_graph[existing_index].as_production_mut().machine_count += production.machine_count;

            existing_index
        } else {
            dest_graph.add_node(NodeValue::Production(production))
        }
    }
}

#[allow(dead_code)]
struct SingleOutputGraph<'a> {
    output: ItemValuePair,
    graph: ScoredGraphType<'a>,
    root_index: NodeIndex,
    overall_score: f64,
    unique_inputs: usize,
}

impl<'a> SingleOutputGraph<'a> {
    pub fn new<'b>(solver: &Solver<'a, 'b>, output: ItemValuePair) -> Self {
        let (mut graph, root_index) = build_graph(solver, output);

        prune_impossible(root_index, &mut graph);
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
    output: ItemValuePair,
) -> (ScoredGraphType<'a>, NodeIndex) {
    let mut graph = ScoredGraphType::new();
    let output_node = ScoredNodeValue::new_output(output);
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
        let inputs_to_solve: Vec<ItemValuePair> = match graph[*node_index].node {
            NodeValue::Production(production) => production.recipe
                .inputs
                .iter()
                .map(|input| ItemValuePair::new(input.item, input.amount_per_minute * production.machine_count))
                .collect(),
            NodeValue::Output(output, ..) => vec![output],
            _ => vec![],
        };

        for input in inputs_to_solve {
            if solver.has_input(input.item) {
                next_nodes.push(create_input_node(input, *node_index, graph));
            }
            if !input.item.is_extractable() {
                next_nodes.extend(create_production_nodes(solver, input, *node_index, graph));
            }
        }
    }

    next_nodes
}

fn create_input_node(
    item_value: ItemValuePair,
    parent_index: NodeIndex,
    graph: &mut ScoredGraphType<'_>,
) -> NodeIndex {
    let child_node = ScoredNodeValue::new_input(item_value);
    let child_index = graph.add_node(child_node);
    graph.add_edge(child_index, parent_index, item_value);

    child_index
}

fn create_production_nodes<'a>(
    solver: &Solver<'a, '_>,
    item_value: ItemValuePair,
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
                ScoredNodeValue::new_production(recipe, machine_count);
            let child_index = graph.add_node(child_node);
            graph.add_edge(child_index, parent_index, item_value);

            child_index
        }).collect()
}

fn prune_impossible(node_index: NodeIndex, graph: &mut ScoredGraphType) -> bool {
    if graph[node_index].is_input() {
        return false;
    }

    let children_by_input = group_children_by_input(node_index, graph);
    let mut delete_self = false;
    
    for children in children_by_input.values() {
        let remaining_children: Vec<NodeIndex> = children
            .iter()
            .copied()
            .filter(|child_index| !prune_impossible(*child_index, graph))
            .collect();

        if remaining_children.is_empty() {
            delete_self = true;
            break;
        }
    }

    if delete_self {
        prune_node(node_index, graph);
    }
    delete_self
}

fn prune_node(node_index: NodeIndex, graph: &mut ScoredGraphType) {
    let mut neighbor_walker = graph.neighbors_directed(node_index, Incoming).detach();

    while let Some(child_index) = neighbor_walker.next_node(graph) {
        prune_node(child_index, graph);
    }

    graph.remove_node(node_index);
}

fn group_children_by_input(node_index: NodeIndex, graph: &ScoredGraphType) -> HashMap<Item, Vec<NodeIndex>> {
    

    let expected_items = match graph[node_index].node {
        NodeValue::Input(..) => Vec::new(),
        NodeValue::Output(output) => vec![output.item],
        NodeValue::ByProduct(output) => vec![output.item],
        NodeValue::Production(production) => production.recipe.inputs.iter().map(|input| input.item).collect(),
    };

    let mut children_by_items: HashMap<Item, Vec<NodeIndex>> = expected_items
        .iter()
        .copied()
        .map(|item| (item, Vec::<NodeIndex>::new()))
        .collect();

    for edge in graph.edges_directed(node_index, Incoming) {
        children_by_items
            .entry(edge.weight().item)
            .or_default()
            .push(edge.source());
    }

    children_by_items
}

fn score_node(solver: &Solver, graph: &mut ScoredGraphType, node_index: NodeIndex) -> f64 {
    let score = match graph[node_index].node {
        NodeValue::Input(input) => score_input_node(solver, &input),
        NodeValue::Production(production) => score_production_node(solver, graph, node_index, production.recipe),
        NodeValue::Output(..) => score_output_node(solver, graph, node_index),
        NodeValue::ByProduct(..) => score_output_node(solver, graph, node_index),
    };

    graph[node_index].score = score;
    score
}

fn score_input_node(solver: &Solver, input: &ItemValuePair) -> f64 {
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
    recipe: &Recipe,
) -> f64 {
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

fn score_output_node(solver: &Solver, graph: &mut ScoredGraphType, node_index: NodeIndex) -> f64 {
    let mut score = f64::INFINITY;
    let mut children = graph.neighbors_directed(node_index, Incoming).detach();

    while let Some(child_node) = children.next_node(graph) {
        score = score.min(score_node(solver, graph, child_node));
    }

    score
}

fn count_unique_inputs(graph: &ScoredGraphType, node_index: NodeIndex) -> usize {
    let mut unique_inputs = Vec::new();
    calc_input_combinations(graph, node_index)
        .iter()
        .for_each(|a| {
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
        NodeValue::ByProduct(..) => Vec::new()
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
