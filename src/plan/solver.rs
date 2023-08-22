use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::{find_production_node, ItemBitSet, NodeValue, PlanConfig};

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::EdgeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Incoming;
use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use super::{
    find_input_node, find_output_node, GraphType, NodeEdge, Production, ScoredGraphType,
    ScoredNodeValue,
};

#[derive(Error, Debug)]
#[error("Unsolvable Plan: Unable to craft the desired quantity of `{item}`")]
pub struct SolverError {
    item: Item,
}

struct SingleOutputGraph<'a> {
    output: ItemValuePair,
    graph: ScoredGraphType<'a>,
    root_index: NodeIndex,
    overall_score: f64,
    unique_inputs: usize,
}

pub type SolverResult<T> = Result<T, SolverError>;

impl<'a> SingleOutputGraph<'a> {
    pub fn new(config: &'a PlanConfig, output: ItemValuePair) -> Self {
        let (mut graph, root_index) = build_single_output_graph(config, output);

        let overall_score = score_node(config, &mut graph, root_index);
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

pub fn solve<'a>(config: &'a PlanConfig) -> SolverResult<GraphType<'a>> {
    let mut output_graphs: Vec<SingleOutputGraph<'a>> = config
        .outputs
        .iter()
        .map(|(item, value)| SingleOutputGraph::new(config, ItemValuePair::new(*item, *value)))
        .collect();
    output_graphs.sort_by(|a, b| {
        match a.unique_inputs.cmp(&b.unique_inputs) {
            Ordering::Equal => {}
            ord => return ord,
        }

        a.overall_score.total_cmp(&b.overall_score).reverse()
    });

    let mut solved_graph: GraphType<'a> = GraphType::new();
    let mut remaining_inputs = config.inputs.clone();
    for output_graph in output_graphs {
        merge_optimal_path(
            &output_graph.graph,
            output_graph.root_index,
            output_graph.output,
            &mut solved_graph,
            &mut remaining_inputs,
        )?;
    }

    Ok(solved_graph)
}

fn merge_optimal_path<'a>(
    src_graph: &ScoredGraphType<'a>,
    node_index: NodeIndex,
    desired_output: ItemValuePair,
    dest_graph: &mut GraphType<'a>,
    remaining_inputs: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    match src_graph[node_index].node {
        NodeValue::Input(input) => {
            assert!(desired_output.item == input.item);
            merge_input_node(desired_output, dest_graph, remaining_inputs)
        }
        NodeValue::Output(output) => {
            assert!(desired_output.item == output.item);
            merge_output_node(
                src_graph,
                node_index,
                desired_output,
                dest_graph,
                remaining_inputs,
            )
        }
        NodeValue::Production(production) => merge_production_node(
            src_graph,
            node_index,
            production,
            desired_output,
            dest_graph,
            remaining_inputs,
        ),
        NodeValue::ByProduct(..) => todo!(),
    }
}

fn merge_input_node(
    input: ItemValuePair,
    dest_graph: &mut GraphType,
    remaining_inputs: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let limit = remaining_inputs.get(&input.item).copied().unwrap_or(0.0);
    let available_input = ItemValuePair::new(input.item, input.value.min(limit).max(0.0));
    let leftover_input =
        ItemValuePair::new(input.item, 0.0f64.max(input.value - available_input.value));

    if available_input.value <= 0.0 {
        return Err(SolverError { item: input.item });
    }

    let node_index = if let Some(existing_index) = find_input_node(dest_graph, available_input.item)
    {
        dest_graph[existing_index].as_input_mut().value += available_input.value;
        existing_index
    } else {
        dest_graph.add_node(NodeValue::Input(available_input))
    };

    *remaining_inputs.entry(available_input.item).or_default() -= available_input.value;
    Ok((node_index, leftover_input))
}

fn merge_output_node<'a>(
    src_graph: &ScoredGraphType<'a>,
    node_index: NodeIndex,
    desired_output: ItemValuePair,
    dest_graph: &mut GraphType<'a>,
    remaining_inputs: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let mut children: Vec<NodeIndex> = src_graph.neighbors_directed(node_index, Incoming).collect();
    children.sort_by(|a, b| src_graph[*a].score.total_cmp(&src_graph[*b].score));

    let mut remaining_output = desired_output;
    let mut new_children: Vec<(NodeIndex, ItemValuePair)> = Vec::new();
    for child_index in children {
        if remaining_output.value <= 0.0 {
            break;
        }

        if let Ok((child_index, leftover_output)) = merge_optimal_path(
            src_graph,
            child_index,
            remaining_output,
            dest_graph,
            remaining_inputs,
        ) {
            new_children.push((child_index, remaining_output - leftover_output.value));
            remaining_output = leftover_output;
        }
    }

    if remaining_output.value > 0.0 {
        return Err(SolverError {
            item: desired_output.item,
        });
    }

    let new_node_index =
        if let Some(existing_index) = find_output_node(dest_graph, desired_output.item) {
            dest_graph[existing_index].as_output_mut().value += desired_output.value;
            existing_index
        } else {
            dest_graph.add_node(NodeValue::new_output(desired_output))
        };

    for (order, (child_index, item_value)) in new_children.iter().enumerate() {
        create_or_update_edge(
            *child_index,
            new_node_index,
            *item_value,
            order as u32,
            dest_graph,
        );
    }

    Ok((new_node_index, remaining_output))
}

fn merge_production_node<'a>(
    src_graph: &ScoredGraphType<'a>,
    node_index: NodeIndex,
    production: Production<'a>,
    desired_output: ItemValuePair,
    dest_graph: &mut GraphType<'a>,
    remaining_inputs: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let chosen_output = production
        .recipe
        .find_output_by_item(desired_output.item)
        .unwrap();

    let mut children_by_items: HashMap<Item, Vec<NodeIndex>> = production
        .recipe
        .inputs
        .iter()
        .copied()
        .map(|input| (input.item, Vec::<NodeIndex>::new()))
        .collect();

    for edge in src_graph.edges_directed(node_index, Incoming) {
        children_by_items
            .entry(edge.weight().item)
            .or_default()
            .push(edge.source());
    }

    let new_machine_count = desired_output.value / chosen_output.amount_per_minute;
    let mut min_machine_count = new_machine_count;
    let mut new_children_by_items: HashMap<Item, Vec<(NodeIndex, ItemValuePair)>> = HashMap::new();
    for (item, mut children) in children_by_items {
        let recipe_input = production.recipe.find_input_by_item(item).unwrap();
        let initial_output = recipe_input.to_amount_per_minute_pair() * new_machine_count;
        let mut remaining_output = initial_output;

        children.sort_by(|a, b| src_graph[*a].score.total_cmp(&src_graph[*b].score));
        for child_index in children {
            if remaining_output.value <= 0.0 {
                break;
            }

            let merge_result = merge_optimal_path(
                src_graph,
                child_index,
                remaining_output,
                dest_graph,
                remaining_inputs,
            );
            if let Ok((child_index, leftover_output)) = merge_result {
                new_children_by_items
                    .entry(item)
                    .or_default()
                    .push((child_index, remaining_output - leftover_output.value));
                remaining_output = leftover_output;
            }
        }

        let used_output = initial_output - remaining_output.value;
        min_machine_count =
            min_machine_count.min(used_output.value / recipe_input.amount_per_minute);
    }

    let new_node_index = match find_production_node(dest_graph, production.recipe) {
        Some(existing_index) => {
            dest_graph[existing_index].as_production_mut().machine_count += new_machine_count;
            existing_index
        }
        None => dest_graph.add_node(NodeValue::new_production(
            production.recipe,
            new_machine_count,
        )),
    };

    for children in new_children_by_items.values() {
        for (order, (child_index, item_value)) in children.iter().enumerate() {
            create_or_update_edge(
                *child_index,
                new_node_index,
                *item_value,
                order as u32,
                dest_graph,
            );
        }
    }

    let machine_count_diff = f64::max(0.0, new_machine_count - min_machine_count);
    let reduced_output = chosen_output.to_amount_per_minute_pair() * machine_count_diff;
    reduce_node_output(new_node_index, reduced_output, dest_graph, remaining_inputs);

    Ok((
        new_node_index,
        desired_output - (chosen_output.amount_per_minute * min_machine_count),
    ))
}

fn create_input_node(
    item_value: ItemValuePair,
    parent_index: NodeIndex,
    graph: &mut ScoredGraphType,
) -> NodeIndex {
    let child_node = ScoredNodeValue::new_input(item_value);
    let child_index = graph.add_node(child_node);
    graph.add_edge(child_index, parent_index, item_value);

    child_index
}

fn create_production_nodes<'a>(
    config: &'a PlanConfig,
    item_value: ItemValuePair,
    parent_index: NodeIndex,
    graph: &mut ScoredGraphType<'a>,
) -> Vec<NodeIndex> {
    config
        .find_recipe_by_output(item_value.item)
        .map(|recipe| {
            let output = recipe.find_output_by_item(item_value.item).unwrap();
            let machine_count = item_value.value / output.amount_per_minute;

            let child_node = ScoredNodeValue::new_production(recipe, machine_count);
            let child_index = graph.add_node(child_node);
            graph.add_edge(child_index, parent_index, item_value);

            child_index
        })
        .collect()
}

fn create_or_update_edge(
    child_index: NodeIndex,
    parent_index: NodeIndex,
    item_value: ItemValuePair,
    order: u32,
    graph: &mut GraphType,
) {
    if let Some(edge_index) = graph.find_edge(child_index, parent_index) {
        assert!(graph[edge_index].value.item == item_value.item);
        graph[edge_index].value += item_value.value;
    } else {
        graph.add_edge(child_index, parent_index, NodeEdge::new(item_value, order));
    }
}

fn reduce_node_output(
    node_index: NodeIndex,
    reduce_amount: ItemValuePair,
    graph: &mut GraphType<'_>,
    remaining_inputs: &mut HashMap<Item, f64>,
) -> bool {
    if reduce_amount.value <= 0.0 {
        return false;
    }

    match graph[node_index] {
        NodeValue::Input(input) => {
            assert!(input.item == reduce_amount.item);
            let new_value = f64::max(0.0, input.value - reduce_amount.value);

            if new_value <= 0.0 {
                prune_node(node_index, graph, remaining_inputs);
                return true;
            }
            *remaining_inputs.entry(input.item).or_default() +=
                f64::min(reduce_amount.value, input.value);
            graph[node_index].as_input_mut().value = new_value;
        }
        NodeValue::Production(production) => {
            let output = production
                .recipe
                .find_output_by_item(reduce_amount.item)
                .unwrap();
            let reduced_machine_count = reduce_amount.value / output.amount_per_minute;
            graph[node_index].as_production_mut().machine_count -= reduced_machine_count;

            if production.machine_count <= 0.0 {
                prune_node(node_index, graph, remaining_inputs);
                return true;
            }

            let mut children_by_inputs: HashMap<Item, Vec<(EdgeIndex, NodeIndex)>> = HashMap::new();
            for edge in graph.edges_directed(node_index, Incoming) {
                children_by_inputs
                    .entry(edge.weight().value.item)
                    .or_default()
                    .push((edge.id(), edge.source()));
            }

            for (item, mut children) in children_by_inputs {
                let recipe_input = production.recipe.find_input_by_item(item).unwrap();
                let total_output = children
                    .iter()
                    .map(|e| graph[e.0].value.value)
                    .reduce(|acc, e| acc + e)
                    .unwrap_or(0.0);

                let desired_output = recipe_input.amount_per_minute * production.machine_count;
                let mut to_prune = total_output - desired_output;
                children.sort_by(|a, b| graph[a.0].order.cmp(&graph[b.0].order).reverse());
                for (edge_index, child_index) in children {
                    let reduce_amount = graph[edge_index].value - to_prune;
                    to_prune = f64::max(0.0, to_prune - reduce_amount.value);
                    reduce_node_output(child_index, reduce_amount, graph, remaining_inputs);
                }
            }
        }
        _ => {
            panic!("Unexpected node found");
        }
    };

    false
}

fn prune_node(
    node_index: NodeIndex,
    graph: &mut GraphType,
    remaining_inputs: &mut HashMap<Item, f64>,
) {
    let mut neighbor_walker = graph.neighbors_directed(node_index, Incoming).detach();

    while let Some(child_index) = neighbor_walker.next_node(graph) {
        prune_node(child_index, graph, remaining_inputs);
    }

    if graph[node_index].is_input() {
        let input = graph[node_index].as_input();
        *remaining_inputs.entry(input.item).or_default() += input.value;
    }

    graph.remove_node(node_index);
}

fn build_single_output_graph(
    config: &PlanConfig,
    output: ItemValuePair,
) -> (ScoredGraphType<'_>, NodeIndex) {
    let mut graph = ScoredGraphType::new();
    let output_node = ScoredNodeValue::new_output(output);
    let root_index = graph.add_node(output_node);

    let mut node_indices = vec![root_index];
    loop {
        node_indices = build_single_output_graph_level(config, &mut graph, &node_indices);

        if node_indices
            .iter()
            .all(|node_index| graph[*node_index].node.is_input())
        {
            break;
        }
    }

    (graph, root_index)
}

fn build_single_output_graph_level<'a>(
    config: &'a PlanConfig,
    graph: &mut ScoredGraphType<'a>,
    parent_indices: &Vec<NodeIndex>,
) -> Vec<NodeIndex> {
    let mut next_nodes = Vec::new();

    for node_index in parent_indices {
        let inputs_to_solve: Vec<ItemValuePair> = match graph[*node_index].node {
            NodeValue::Production(production) => production
                .recipe
                .inputs
                .iter()
                .map(|input| input.to_amount_per_minute_pair() * production.machine_count)
                .collect(),
            NodeValue::Output(output, ..) => vec![output],
            _ => vec![],
        };

        for input in inputs_to_solve {
            if config.has_input(input.item) {
                next_nodes.push(create_input_node(input, *node_index, graph));
            }
            if !input.item.is_extractable() {
                next_nodes.extend(create_production_nodes(config, input, *node_index, graph));
            }
        }
    }

    next_nodes
}

fn score_node(config: &PlanConfig, graph: &mut ScoredGraphType, node_index: NodeIndex) -> f64 {
    let score = match graph[node_index].node {
        NodeValue::Input(input) => score_input_node(config, &input),
        NodeValue::Production(production) => {
            score_production_node(config, graph, node_index, production.recipe)
        }
        NodeValue::Output(..) => score_output_node(config, graph, node_index),
        NodeValue::ByProduct(..) => score_output_node(config, graph, node_index),
    };

    graph[node_index].score = score;
    score
}

fn score_input_node(config: &PlanConfig, input: &ItemValuePair) -> f64 {
    if input.item.is_extractable() {
        let input_limit = config.find_input(input.item);
        input.value / input_limit * 10000.0
    } else {
        0.0
    }
}

fn score_production_node(
    config: &PlanConfig,
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
        let score = score_node(config, graph, child_index);

        scores_by_input
            .entry(graph[edge_index].item)
            .and_modify(|e| *e = e.min(score))
            .or_insert(score);
    }

    scores_by_input.values().fold(0.0, |acc, f| acc + *f)
}

fn score_output_node(
    config: &PlanConfig,
    graph: &mut ScoredGraphType,
    node_index: NodeIndex,
) -> f64 {
    let mut score = f64::INFINITY;
    let mut children = graph.neighbors_directed(node_index, Incoming).detach();

    while let Some(child_node) = children.next_node(graph) {
        score = score.min(score_node(config, graph, child_node));
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
        NodeValue::ByProduct(..) => Vec::new(),
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
