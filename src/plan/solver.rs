use crate::game::{Item, ItemValuePair, Recipe, RecipeIO};
use crate::plan::{find_production_node, ItemBitSet, NodeValue, PlanConfig};

use petgraph::graph::NodeIndex;
use petgraph::stable_graph::{EdgeIndex, StableDiGraph};
use petgraph::visit::EdgeRef;
use petgraph::Incoming;
use std::cmp::Ordering;
use std::collections::HashMap;

use thiserror::Error;

use super::{
    find_input_node, find_output_node, GraphType, NodeEdge, Production, ScoredGraphType,
    ScoredNodeValue, DEFAULT_LIMITS,
};

const EPSILON: f64 = 0.00000001;

#[derive(Error, Debug)]
#[error("Unsolvable Plan: Unable to craft the desired quantity of `{0}`")]
pub struct SolverError(Item);

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
        .map(|output| SingleOutputGraph::new(config, *output))
        .collect();
    output_graphs.sort_by(|a, b| {
        match a.unique_inputs.cmp(&b.unique_inputs) {
            Ordering::Equal => {}
            ord => return ord,
        }

        a.overall_score.total_cmp(&b.overall_score).reverse()
    });

    let mut graph: GraphType<'a> = GraphType::new();
    let mut input_limits = config.inputs.clone();
    for output_graph in output_graphs {
        merge_optimal_path(
            output_graph.root_index,
            output_graph.output,
            &output_graph.graph,
            &mut graph,
            &mut input_limits,
        )?;
    }

    Ok(graph)
}

fn merge_optimal_path<'a>(
    node_index: NodeIndex,
    desired_output: ItemValuePair,
    src_graph: &ScoredGraphType<'a>,
    dest_graph: &mut GraphType<'a>,
    input_limits: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    match src_graph[node_index].node {
        NodeValue::Input(input) => {
            assert!(desired_output.item == input.item);
            merge_input_node(desired_output, dest_graph, input_limits)
        }
        NodeValue::Output(output) => {
            assert!(desired_output.item == output.item);
            merge_output_node(
                node_index,
                desired_output,
                src_graph,
                dest_graph,
                input_limits,
            )
        }
        NodeValue::Production(production) => merge_production_node(
            node_index,
            production,
            desired_output,
            src_graph,
            dest_graph,
            input_limits,
        ),
        NodeValue::ByProduct(..) => todo!(),
    }
}

fn merge_input_node(
    desired_input: ItemValuePair,
    graph: &mut GraphType,
    input_limits: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let available_input = f64::min(
        desired_input.value,
        *input_limits.get(&desired_input.item).unwrap_or(&0.0),
    );
    if available_input <= 0.0 {
        return Err(SolverError(desired_input.item));
    }

    let node_index = if let Some(existing_index) = find_input_node(graph, desired_input.item) {
        graph[existing_index].as_input_mut().value += available_input;
        existing_index
    } else {
        graph.add_node(NodeValue::Input(desired_input.with_value(available_input)))
    };

    *input_limits.entry(desired_input.item).or_default() -= available_input;
    Ok((node_index, desired_input - available_input))
}

fn merge_output_node<'a>(
    node_index: NodeIndex,
    desired_output: ItemValuePair,
    src_graph: &ScoredGraphType<'a>,
    dest_graph: &mut GraphType<'a>,
    input_limits: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let mut children: Vec<NodeIndex> = src_graph.neighbors_directed(node_index, Incoming).collect();
    children.sort_by(|a, b| src_graph[*a].score.total_cmp(&src_graph[*b].score));

    let mut remaining_output = desired_output;
    let mut new_children: Vec<(NodeIndex, ItemValuePair)> = Vec::new();
    for child_index in children {
        if remaining_output.value <= 0.0 {
            break;
        }

        let merge_result = merge_optimal_path(
            child_index,
            remaining_output,
            src_graph,
            dest_graph,
            input_limits,
        );
        if let Ok((child_index, leftover_output)) = merge_result {
            new_children.push((child_index, remaining_output - leftover_output.value));
            remaining_output = leftover_output;
        }
    }

    if remaining_output.value > EPSILON {
        println!("Still need {}", remaining_output);
        return Err(SolverError(desired_output.item));
    }

    let new_node_index = create_or_update_input_node(desired_output, dest_graph);
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
    node_index: NodeIndex,
    production: Production<'a>,
    desired_output: ItemValuePair,
    src_graph: &ScoredGraphType<'a>,
    dest_graph: &mut GraphType<'a>,
    input_limits: &mut HashMap<Item, f64>,
) -> SolverResult<(NodeIndex, ItemValuePair)> {
    let recipe_output = production
        .recipe
        .find_output_by_item(desired_output.item)
        .unwrap();

    let children_by_items = group_production_children(
        &production.recipe.inputs,
        node_index,
        |e| e.item,
        |a, b| f64::total_cmp(&src_graph[a.1].score, &src_graph[b.1].score),
        src_graph,
    );

    let machine_count = desired_output.value / recipe_output.amount_per_minute;
    let mut min_machine_count = machine_count;
    let mut new_children_by_inputs: Vec<Vec<(NodeIndex, ItemValuePair)>> = Vec::new();
    for (item, children) in children_by_items {
        let recipe_input = production.recipe.find_input_by_item(item).unwrap();

        let (new_children, actual_output) = merge_production_children(
            recipe_input.to_amount_per_minute_pair() * machine_count,
            children,
            src_graph,
            dest_graph,
            input_limits,
        );
        new_children_by_inputs.push(new_children);
        min_machine_count = f64::min(
            min_machine_count,
            actual_output.value / recipe_input.amount_per_minute,
        );
    }

    let new_node_index =
        create_or_update_production_node(production.recipe, machine_count, dest_graph);
    for children in new_children_by_inputs {
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

    let reduced_output = recipe_output.to_amount_per_minute_pair()
        * f64::max(0.0, machine_count - min_machine_count);
    reduce_node_output(new_node_index, reduced_output, dest_graph, input_limits);

    Ok((
        new_node_index,
        desired_output - (recipe_output.amount_per_minute * min_machine_count),
    ))
}

fn merge_production_children<'a>(
    desired_output: ItemValuePair,
    children: Vec<(EdgeIndex, NodeIndex)>,
    src_graph: &ScoredGraphType<'a>,
    dest_graph: &mut GraphType<'a>,
    input_limits: &mut HashMap<Item, f64>,
) -> (Vec<(NodeIndex, ItemValuePair)>, ItemValuePair) {
    let mut new_children: Vec<(NodeIndex, ItemValuePair)> = Vec::new();
    let mut remaining_output = desired_output;

    for (_, child_index) in children {
        if remaining_output.value <= 0.0 {
            break;
        }

        let merge_result = merge_optimal_path(
            child_index,
            remaining_output,
            src_graph,
            dest_graph,
            input_limits,
        );
        if let Ok((child_index, leftover_output)) = merge_result {
            new_children.push((child_index, remaining_output - leftover_output.value));
            remaining_output = leftover_output;
        }
    }

    (new_children, desired_output - remaining_output)
}

fn create_or_update_input_node(input: ItemValuePair, graph: &mut GraphType) -> NodeIndex {
    match find_output_node(graph, input.item) {
        Some(existing_index) => {
            graph[existing_index].as_output_mut().value += input.value;
            existing_index
        }
        None => graph.add_node(NodeValue::new_output(input)),
    }
}

fn create_or_update_production_node<'a>(
    recipe: &'a Recipe,
    machine_count: f64,
    graph: &mut GraphType<'a>,
) -> NodeIndex {
    match find_production_node(graph, recipe) {
        Some(existing_index) => {
            graph[existing_index].as_production_mut().machine_count += machine_count;
            existing_index
        }
        None => graph.add_node(NodeValue::new_production(recipe, machine_count)),
    }
}

fn group_production_children<N, E, I, S>(
    inputs: &[RecipeIO],
    index: NodeIndex,
    get_edge_item: I,
    mut sort_cmp: S,
    graph: &StableDiGraph<N, E>,
) -> HashMap<Item, Vec<(EdgeIndex, NodeIndex)>>
where
    I: Fn(&E) -> Item,
    S: FnMut(&(EdgeIndex, NodeIndex), &(EdgeIndex, NodeIndex)) -> Ordering,
{
    let mut children_by_items: HashMap<Item, Vec<(EdgeIndex, NodeIndex)>> = inputs
        .iter()
        .copied()
        .map(|input| (input.item, Vec::<(EdgeIndex, NodeIndex)>::new()))
        .collect();

    for edge in graph.edges_directed(index, Incoming) {
        children_by_items
            .entry(get_edge_item(edge.weight()))
            .or_default()
            .push((edge.id(), edge.source()));
    }

    for children in children_by_items.values_mut() {
        children.sort_by(&mut sort_cmp);
    }

    children_by_items
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
    input_limits: &mut HashMap<Item, f64>,
) -> bool {
    if reduce_amount.value <= 0.0 {
        return false;
    }

    match graph[node_index] {
        NodeValue::Input(input) => {
            reduce_input_node_output(input, node_index, reduce_amount, graph, input_limits)
        }
        NodeValue::Production(production) => reduce_production_node_output(
            production.recipe,
            node_index,
            reduce_amount,
            graph,
            input_limits,
        ),
        _ => {
            panic!("Output and ByProduct nodes can not be reduced");
        }
    }
}

fn reduce_input_node_output(
    input: ItemValuePair,
    node_index: NodeIndex,
    reduce_amount: ItemValuePair,
    graph: &mut GraphType<'_>,
    input_limits: &mut HashMap<Item, f64>,
) -> bool {
    assert!(input.item == reduce_amount.item);
    let new_value = f64::max(0.0, input.value - reduce_amount.value);

    *input_limits.entry(input.item).or_default() += input.value - new_value;
    if new_value <= 0.0 {
        graph.remove_node(node_index);
        true
    } else {
        graph[node_index].as_input_mut().value = new_value;
        false
    }
}

fn reduce_production_node_output(
    recipe: &Recipe,
    node_index: NodeIndex,
    reduce_amount: ItemValuePair,
    graph: &mut GraphType<'_>,
    input_limits: &mut HashMap<Item, f64>,
) -> bool {
    let recipe_output = recipe.find_output_by_item(reduce_amount.item).unwrap();
    let new_machine_count = {
        let machine_count = &mut graph[node_index].as_production_mut().machine_count;

        *machine_count = f64::max(
            0.0,
            *machine_count - reduce_amount.value / recipe_output.amount_per_minute,
        );
        *machine_count
    };

    let children_by_items = group_production_children(
        &recipe.inputs,
        node_index,
        |e| e.value.item,
        |a, b| graph[a.0].order.cmp(&graph[b.0].order).reverse(),
        graph,
    );

    for (item, children) in children_by_items {
        let recipe_input = recipe.find_input_by_item(item).unwrap();
        reduce_production_node_children(
            recipe_input.to_amount_per_minute_pair() * new_machine_count,
            children,
            graph,
            input_limits,
        );
    }

    if new_machine_count <= 0.0 {
        graph.remove_node(node_index);
        true
    } else {
        false
    }
}

fn reduce_production_node_children(
    desired_input: ItemValuePair,
    children: Vec<(EdgeIndex, NodeIndex)>,
    graph: &mut GraphType,
    input_limits: &mut HashMap<Item, f64>,
) {
    let total_output = children
        .iter()
        .map(|e| graph[e.0].value.value)
        .reduce(|acc, e| acc + e)
        .unwrap_or(0.0);

    let mut to_prune = total_output - desired_input.value;

    for (edge_index, child_index) in children {
        if to_prune <= 0.0 {
            break;
        }

        let reduce_amount = to_prune.min(graph[edge_index].value.value);
        to_prune = f64::max(0.0, to_prune - reduce_amount);

        graph[edge_index].value -= reduce_amount;
        reduce_node_output(
            child_index,
            ItemValuePair::new(desired_input.item, reduce_amount),
            graph,
            input_limits,
        );
    }
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

fn score_node(config: &PlanConfig, graph: &mut ScoredGraphType, node_index: NodeIndex) -> f64 {
    let score = match graph[node_index].node {
        NodeValue::Input(input) => score_input_node(&input),
        NodeValue::Production(production) => {
            score_production_node(config, graph, node_index, production.recipe)
        }
        NodeValue::Output(..) => score_output_node(config, graph, node_index),
        NodeValue::ByProduct(..) => score_output_node(config, graph, node_index),
    };

    graph[node_index].score = score;
    score
}

fn score_input_node(input: &ItemValuePair) -> f64 {
    if input.item.is_extractable() {
        let input_limit = DEFAULT_LIMITS
            .iter()
            .find(|(i, _)| *i == input.item)
            .map(|(_, v)| *v)
            .unwrap_or(0.0);
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

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::game::{Machine, RecipeIO};

    use super::*;

    #[test]
    fn single_production_node() {
        let mut recipes: Vec<Recipe> = build_recipe_db();
        recipes.retain_mut(|r| !r.alternate);

        let config = PlanConfig::new(
            vec![ItemValuePair::new(Item::IronIngot, 30.0)],
            recipes.clone(),
        );

        let mut expected_graph = GraphType::new();
        let output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Item::IronIngot,
            30.0,
        )));
        let smelter_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Ingot", &recipes),
            1.0,
        ));
        let input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::IronOre,
            30.0,
        )));

        expected_graph.add_edge(
            smelter_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronIngot, 30.0), 0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronOre, 30.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn single_production_node_optimizes_resources() {
        let recipes: Vec<Recipe> = build_recipe_db();

        let config = PlanConfig::new(
            vec![ItemValuePair::new(Item::IronIngot, 65.0)],
            recipes.clone(),
        );

        let mut expected_graph = GraphType::new();
        let output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Item::IronIngot,
            65.0,
        )));
        let refinery_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Pure Iron Ingot", &recipes),
            1.0,
        ));
        let ore_input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::IronOre,
            35.0,
        )));

        let water_input_idx =
            expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(Item::Water, 20.0)));

        expected_graph.add_edge(
            refinery_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronIngot, 65.0), 0),
        );
        expected_graph.add_edge(
            ore_input_idx,
            refinery_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronOre, 35.0), 0),
        );
        expected_graph.add_edge(
            water_input_idx,
            refinery_idx,
            NodeEdge::new(ItemValuePair::new(Item::Water, 20.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn multiple_outputs_graph() {
        let mut recipes: Vec<Recipe> = build_recipe_db();
        recipes.retain_mut(|r| !r.alternate);

        let config = PlanConfig::new(
            vec![
                ItemValuePair::new(Item::IronRod, 30.0),
                ItemValuePair::new(Item::IronPlate, 60.0),
            ],
            recipes.clone(),
        );

        let mut expected_graph = GraphType::new();
        let plate_output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Item::IronPlate,
            60.0,
        )));
        let rod_output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Item::IronRod,
            30.0,
        )));

        let plate_prod_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Plate", &recipes),
            3.0,
        ));
        let rod_prod_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Rod", &recipes),
            2.0,
        ));
        let smelter_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Ingot", &recipes),
            4.0,
        ));
        let input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::IronOre,
            120.0,
        )));

        expected_graph.add_edge(
            plate_prod_idx,
            plate_output_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronPlate, 60.0), 0),
        );

        expected_graph.add_edge(
            rod_prod_idx,
            rod_output_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronRod, 30.0), 0),
        );

        expected_graph.add_edge(
            smelter_idx,
            rod_prod_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronIngot, 30.0), 0),
        );

        expected_graph.add_edge(
            smelter_idx,
            plate_prod_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronIngot, 90.0), 0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronOre, 120.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn limited_inputs() {
        let mut recipes = build_recipe_db();
        recipes.retain_mut(|r| !(r.name == "Pure Iron Ingot" || r.name == "Iron Alloy Ingot"));

        let mut input_limits = HashMap::new();
        input_limits.insert(Item::IronOre, 12.5);
        input_limits.insert(Item::CopperOre, 12.0);

        let config = PlanConfig::with_inputs(
            input_limits,
            vec![ItemValuePair::new(Item::Wire, 232.5)],
            recipes.clone(),
        );

        let mut expected_graph = GraphType::new();
        let output_idx =
            expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(Item::Wire, 232.5)));

        let cat_wire_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Caterium Wire", &recipes),
            1.0,
        ));

        let fused_wire_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Fused Wire", &recipes),
            1.0,
        ));

        let iron_wire_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Wire", &recipes),
            1.0,
        ));

        let iron_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Iron Ingot", &recipes),
            12.5 / 30.0,
        ));

        let copper_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Copper Ingot", &recipes),
            0.4,
        ));

        let cat_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Caterium Ingot", &recipes),
            1.2,
        ));

        let iron_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::IronOre,
            12.5,
        )));

        let copper_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::CopperOre,
            12.0,
        )));

        let cat_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Item::CateriumOre,
            54.0,
        )));

        expected_graph.add_edge(
            cat_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Item::Wire, 120.0), 2),
        );

        expected_graph.add_edge(
            fused_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Item::Wire, 90.0), 0),
        );

        expected_graph.add_edge(
            iron_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Item::Wire, 22.5), 0),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            cat_wire_idx,
            NodeEdge::new(ItemValuePair::new(Item::CateriumIngot, 15.0), 2),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            fused_wire_idx,
            NodeEdge::new(ItemValuePair::new(Item::CateriumIngot, 3.0), 2),
        );

        expected_graph.add_edge(
            copper_ingot_idx,
            fused_wire_idx,
            NodeEdge::new(ItemValuePair::new(Item::CopperIngot, 12.0), 2),
        );

        expected_graph.add_edge(
            iron_ingot_idx,
            iron_wire_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronIngot, 12.5), 2),
        );

        expected_graph.add_edge(
            iron_ore_idx,
            iron_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Item::IronOre, 12.5), 2),
        );

        expected_graph.add_edge(
            copper_ore_idx,
            copper_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Item::CopperOre, 12.0), 2),
        );

        expected_graph.add_edge(
            cat_ore_idx,
            cat_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Item::CateriumOre, 54.0), 2),
        );

        let result = solve(&config);
        
        assert!(result.is_ok(), "{:?}", result);
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    fn build_recipe_db() -> Vec<Recipe> {
        vec![
            Recipe {
                name: "Iron Ingot".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::IronOre, 1.0, 30.0)],
                outputs: vec![RecipeIO::new(Item::IronIngot, 1.0, 30.0)],
                power_multiplier: 1.0,
                craft_time: 2,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Copper Ingot".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::CopperOre, 1.0, 30.0)],
                outputs: vec![RecipeIO::new(Item::CopperIngot, 1.0, 30.0)],
                power_multiplier: 1.0,
                craft_time: 2,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Caterium Ingot".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::CateriumOre, 3.0, 45.0)],
                outputs: vec![RecipeIO::new(Item::CateriumIngot, 1.0, 15.0)],
                power_multiplier: 1.0,
                craft_time: 4,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Pure Iron Ingot".into(),
                alternate: true,
                inputs: vec![
                    RecipeIO::new(Item::IronOre, 7.0, 35.0),
                    RecipeIO::new(Item::Water, 5.0, 20.0),
                ],
                outputs: vec![RecipeIO::new(Item::IronIngot, 13.0, 65.0)],
                power_multiplier: 1.0,
                craft_time: 12,
                machine: Machine::Refinery,
            },
            Recipe {
                name: "Iron Alloy Ingot".into(),
                alternate: true,
                inputs: vec![
                    RecipeIO::new(Item::IronOre, 2.0, 20.0),
                    RecipeIO::new(Item::CopperOre, 2.0, 20.0),
                ],
                outputs: vec![RecipeIO::new(Item::IronIngot, 5.0, 50.0)],
                power_multiplier: 1.0,
                craft_time: 6,
                machine: Machine::Foundry,
            },
            Recipe {
                name: "Iron Plate".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::IronIngot, 3.0, 30.0)],
                outputs: vec![RecipeIO::new(Item::IronPlate, 2.0, 20.0)],
                power_multiplier: 1.0,
                craft_time: 6,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Iron Rod".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::IronIngot, 1.0, 15.0)],
                outputs: vec![RecipeIO::new(Item::IronRod, 1.0, 15.0)],
                power_multiplier: 1.0,
                craft_time: 4,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Wire".into(),
                alternate: false,
                inputs: vec![RecipeIO::new(Item::CopperIngot, 1.0, 15.0)],
                outputs: vec![RecipeIO::new(Item::Wire, 2.0, 30.0)],
                power_multiplier: 1.0,
                craft_time: 4,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Iron Wire".into(),
                alternate: true,
                inputs: vec![RecipeIO::new(Item::IronIngot, 5.0, 12.5)],
                outputs: vec![RecipeIO::new(Item::Wire, 9.0, 22.5)],
                power_multiplier: 1.0,
                craft_time: 24,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Caterium Wire".into(),
                alternate: true,
                inputs: vec![RecipeIO::new(Item::CateriumIngot, 1.0, 15.0)],
                outputs: vec![RecipeIO::new(Item::Wire, 8.0, 120.0)],
                power_multiplier: 1.0,
                craft_time: 4,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Fused Wire".into(),
                alternate: true,
                inputs: vec![
                    RecipeIO::new(Item::CateriumIngot, 1.0, 3.0),
                    RecipeIO::new(Item::CopperIngot, 4.0, 12.0),
                ],
                outputs: vec![RecipeIO::new(Item::Wire, 30.0, 90.0)],
                power_multiplier: 1.0,
                craft_time: 20,
                machine: Machine::Assembler,
            },
        ]
    }

    fn find_recipe<'a>(name: &str, recipes: &'a [Recipe]) -> &'a Recipe {
        recipes.iter().find(|r| r.name == name).unwrap()
    }

    fn assert_graphs_equal(actual: GraphType, expected: GraphType) {
        let mut node_mapping: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for i in expected.node_indices() {
            match actual
                .node_indices()
                .find(|j| node_equals(&expected[i], &actual[*j]))
            {
                Some(j) => node_mapping.insert(i, j),
                None => panic!(
                    "Expected node {} was not found in the actual graph {}",
                    format_node(&expected[i]),
                    format_graph_nodes(&actual)
                ),
            };
        }

        for edge in expected.edge_references() {
            let actual_parent = node_mapping.get(&edge.target()).unwrap();
            let actual_child = node_mapping.get(&edge.source()).unwrap();

            let actual_edge = actual
                .find_edge(*actual_child, *actual_parent)
                .unwrap_or_else(|| panic!("Edge connecting {} to {} was not found in actual graph",
                    format_node(&expected[edge.source()]),
                    format_node(&expected[edge.target()])));

            assert!(
                item_value_pair_equals(&actual[actual_edge].value, &edge.weight().value),
                "Mismatched weight for the edge connecting {} to {}. Expected: {}, actual: {}",
                format_node(&expected[edge.source()]),
                format_node(&expected[edge.target()]),
                edge.weight().value,
                actual[actual_edge].value
            );
        }

        assert!(actual.node_count() == expected.node_count());
        assert!(actual.edge_count() == expected.edge_count());
    }

    fn node_equals(a_node: &NodeValue, b_node: &NodeValue) -> bool {
        match (a_node, b_node) {
            (NodeValue::Input(a), NodeValue::Input(b)) => item_value_pair_equals(a, b),
            (NodeValue::Output(a), NodeValue::Output(b)) => item_value_pair_equals(a, b),
            (NodeValue::ByProduct(a), NodeValue::ByProduct(b)) => item_value_pair_equals(a, b),
            (NodeValue::Production(a), NodeValue::Production(b)) => {
                a.recipe == b.recipe && float_equals(a.machine_count, b.machine_count)
            }
            _ => false,
        }
    }

    fn item_value_pair_equals(a: &ItemValuePair, b: &ItemValuePair) -> bool {
        a.item == b.item && float_equals(a.value, b.value)
    }

    fn float_equals(a: f64, b: f64) -> bool {
        f64::abs(a - b) < EPSILON
    }

    fn format_node(node: &NodeValue) -> String {
        match node {
            NodeValue::Input(input) => format!("Input({}:{})", input.item, input.value),
            NodeValue::Output(output) => format!("Output({}:{})", output.item, output.value),
            NodeValue::ByProduct(output) => format!("ByProduct({}:{})", output.item, output.value),
            NodeValue::Production(production) => format!(
                "Production({}, {})",
                production.recipe.name, production.machine_count
            ),
        }
    }

    fn format_graph_nodes(graph: &GraphType) -> String {
        let all_nodes: Vec<String> = graph.node_weights().map(format_node).collect();
        format!("[{}]", all_nodes.join(", "))
    }
}
