use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::{find_production_node, ItemBitSet, NodeValue, PlanConfig};
use crate::utils::EPSILON;

use petgraph::graph::NodeIndex;

use petgraph::stable_graph::EdgeIndex;
use petgraph::visit::EdgeRef;

use petgraph::Incoming;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Index;

use thiserror::Error;

use super::{
    find_by_product_node, find_input_node, find_output_node, GraphType, NodeEdge, PathChain,
    Production, ScoredGraphType, ScoredNodeEdge, DEFAULT_LIMITS,
};

#[derive(Error, Debug)]
#[error("Unsolvable Plan: Unable to craft the desired quantity of `{0}`")]
pub struct SolverError(Item);

pub type SolverResult<T> = Result<T, SolverError>;

pub fn solve(config: &PlanConfig) -> SolverResult<GraphType<'_>> {
    Solver::new(config).solve()
}

struct MergeNode {
    index: NodeIndex,
    chain: PathChain,
    desired_output: ItemValuePair,
}

impl MergeNode {
    fn new(index: NodeIndex, chain: PathChain, desired_output: ItemValuePair) -> Self {
        Self {
            index,
            chain,
            desired_output,
        }
    }

    #[inline]
    pub fn item(&self) -> Item {
        self.desired_output.item
    }

    #[inline]
    pub fn output_value(&self) -> f64 {
        self.desired_output.value
    }
}

struct Solver<'a> {
    config: &'a PlanConfig,
    scored_graph: ScoredGraph<'a>,
    input_limits: HashMap<Item, f64>,
}

impl<'a> Solver<'a> {
    fn new(config: &'a PlanConfig) -> Self {
        let mut scored_graph = ScoredGraph::new(config);
        scored_graph.build();

        let input_limits = config.inputs.clone();

        Self {
            config,
            scored_graph,
            input_limits,
        }
    }

    #[inline]
    fn get_limit(&self, item: Item) -> f64 {
        self.input_limits.get(&item).copied().unwrap_or_default()
    }

    #[inline]
    fn update_limit(&mut self, item: Item, amount: f64) {
        *self.input_limits.entry(item).or_default() += amount
    }

    fn solve(&mut self) -> SolverResult<GraphType<'a>> {
        let mut graph: GraphType<'a> = GraphType::new();

        let outputs: Vec<MergeNode> = self
            .scored_graph
            .output_nodes
            .iter()
            .map(|output| MergeNode::new(output.index, PathChain::empty(), output.output))
            .collect();

        for node in outputs {
            self.merge_optimal_path(node, &mut graph)?;
        }

        Ok(graph)
    }

    fn merge_optimal_path(
        &mut self,
        node: MergeNode,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        match self.scored_graph.graph[node.index] {
            NodeValue::Input(input) => {
                assert!(node.item() == input.item);
                self.merge_input_node(node, graph)
            }
            NodeValue::Output(output) => {
                assert!(node.item() == output.item);
                self.merge_output_node(node, graph)
            }
            NodeValue::Production(production) => {
                self.merge_production_node(node, production, graph)
            }
            NodeValue::ByProduct(..) => todo!(),
        }
    }

    fn merge_input_node(
        &mut self,
        node: MergeNode,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let available_input = f64::min(node.output_value(), self.get_limit(node.item()));
        if available_input <= 0.0 {
            return Err(SolverError(node.item()));
        }

        let node_index = match find_input_node(graph, node.item()) {
            Some(existing_index) => {
                *graph[existing_index].as_input_mut() += available_input;
                existing_index
            }
            None => graph.add_node(NodeValue::Input(ItemValuePair::new(
                node.item(),
                available_input,
            ))),
        };

        self.update_limit(node.item(), -available_input);
        Ok((node_index, node.desired_output - available_input))
    }

    fn merge_output_node(
        &mut self,
        node: MergeNode,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let mut remaining_output = node.desired_output;
        let mut new_children: Vec<(NodeIndex, ItemValuePair)> = Vec::new();
        for (e, c) in self.scored_graph.output_children(node.index, &node.chain) {
            if remaining_output.value <= 0.0 {
                break;
            }

            let child_node =
                MergeNode::new(c, self.scored_graph[e].chain.clone(), remaining_output);
            if let Ok((child_index, leftover_output)) = self.merge_optimal_path(child_node, graph) {
                new_children.push((child_index, remaining_output - leftover_output.value));
                remaining_output = leftover_output;
                remaining_output.normalize();
            }
        }

        if remaining_output.value > EPSILON {
            return Err(SolverError(node.item()));
        }

        let node_index = Self::create_or_update_output_node(node.desired_output, graph);
        for (order, (child_index, item_value)) in new_children.iter().copied().enumerate() {
            Self::create_or_update_edge(
                child_index,
                node_index,
                NodeEdge::new(item_value, order as u32),
                graph,
            );
        }

        Ok((node_index, remaining_output))
    }

    fn merge_production_node(
        &mut self,
        node: MergeNode,
        production: Production<'a>,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let recipe_output = *production.recipe.find_output_by_item(node.item()).unwrap();
        let machine_count = node.desired_output / recipe_output;

        let mut min_machine_count = machine_count;
        let mut new_children_by_inputs: Vec<Vec<(NodeIndex, ItemValuePair)>> = Vec::new();
        for (item, children) in self
            .scored_graph
            .production_children(node.index, &node.chain)
        {
            let recipe_input = *production.recipe.find_input_by_item(item).unwrap();

            let (new_children, actual_output) =
                self.merge_production_children(recipe_input * machine_count, children, graph);
            new_children_by_inputs.push(new_children);
            min_machine_count = f64::min(min_machine_count, actual_output / recipe_input);
        }

        let node_index =
            Self::create_or_update_production_node(production.recipe, machine_count, graph);
        for children in new_children_by_inputs {
            for (order, (child_index, item_value)) in children.iter().copied().enumerate() {
                Self::create_or_update_edge(
                    child_index,
                    node_index,
                    NodeEdge::new(item_value, order as u32),
                    graph,
                );
            }
        }

        let reduced_output = recipe_output * (machine_count - min_machine_count);
        self.propagate_reduction(node_index, reduced_output, graph);

        /*for edge in src_graph.edges_directed(node_index, Outgoing) {
            if src_graph[edge.target()].is_by_product() {
                let recipe_output = *production
                    .recipe
                    .find_output_by_item(edge.weight().item)
                    .unwrap();
                merge_by_product_node(
                    recipe_output * min_machine_count,
                    src_graph,
                    dest_graph,
                    input_limits,
                );
            }
        }*/

        Ok((
            node_index,
            node.desired_output - (recipe_output * min_machine_count),
        ))
    }

    fn merge_production_children(
        &mut self,
        desired_output: ItemValuePair,
        children: Vec<(EdgeIndex, NodeIndex)>,
        graph: &mut GraphType<'a>,
    ) -> (Vec<(NodeIndex, ItemValuePair)>, ItemValuePair) {
        let mut new_children: Vec<(NodeIndex, ItemValuePair)> = Vec::new();
        let mut remaining_output = desired_output;

        for (edge_index, child_index) in children {
            if remaining_output.value <= 0.0 {
                break;
            }

            let child_node = MergeNode::new(
                child_index,
                self.scored_graph[edge_index].chain.clone(),
                remaining_output,
            );
            if let Ok((child_index, leftover_output)) = self.merge_optimal_path(child_node, graph) {
                new_children.push((child_index, remaining_output - leftover_output.value));
                remaining_output = leftover_output;
                remaining_output.normalize();
            }
        }

        (new_children, desired_output - remaining_output)
    }

    fn propagate_reduction(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType<'_>,
    ) -> bool {
        if amount.value < EPSILON {
            return false;
        }

        match graph[node_index] {
            NodeValue::Input(input) => {
                self.propagate_reudction_input_node(input, node_index, amount, graph)
            }
            NodeValue::Production(..) => {
                self.propagate_reduction_production_node(node_index, amount, graph)
            }
            _ => {
                panic!("Output and ByProduct nodes can not be reduced");
            }
        }
    }

    fn propagate_reudction_input_node(
        &mut self,
        input: ItemValuePair,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType<'_>,
    ) -> bool {
        assert!(input.item == amount.item);
        let new_value = f64::max(0.0, (input - amount).value);

        self.update_limit(input.item, input.value - new_value);
        if new_value < EPSILON {
            graph.remove_node(node_index);
            true
        } else {
            graph[node_index].as_input_mut().value = new_value;
            false
        }
    }

    fn propagate_reduction_production_node(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType<'_>,
    ) -> bool {
        let production = *graph[node_index].as_production();
        let recipe_output = *production.recipe.find_output_by_item(amount.item).unwrap();
        let new_machine_count = f64::max(0.0, production.machine_count - amount / recipe_output);
        graph[node_index].as_production_mut().machine_count = new_machine_count;

        let mut children_by_items: HashMap<Item, Vec<(EdgeIndex, NodeIndex)>> = HashMap::new();
        for edge in graph.edges_directed(node_index, Incoming) {
            children_by_items
                .entry(edge.weight().item())
                .or_default()
                .push((edge.id(), edge.source()));
        }

        for (item, mut children) in children_by_items {
            children.sort_by(|a, b| graph[a.0].order.cmp(&graph[b.0].order).reverse());

            let recipe_input = *production.recipe.find_input_by_item(item).unwrap();
            self.propagate_reduction_production_children(
                recipe_input * new_machine_count,
                children,
                graph,
            );
        }

        if new_machine_count <= 0.0 {
            graph.remove_node(node_index);
            true
        } else {
            false
        }
    }

    fn propagate_reduction_production_children(
        &mut self,
        desired_input: ItemValuePair,
        children: Vec<(EdgeIndex, NodeIndex)>,
        graph: &mut GraphType,
    ) {
        let total_output: f64 = children.iter().map(|e| graph[e.0].value()).sum();

        let mut total_delta = total_output - desired_input.value;
        for (edge_index, child_index) in children {
            if total_delta < EPSILON {
                break;
            }

            let reduce_amount = total_delta.min(graph[edge_index].value());
            total_delta = f64::max(0.0, total_delta - reduce_amount);

            graph[edge_index].value -= reduce_amount;
            self.propagate_reduction(
                child_index,
                ItemValuePair::new(desired_input.item, reduce_amount),
                graph,
            );
        }
    }

    fn create_or_update_output_node(input: ItemValuePair, graph: &mut GraphType) -> NodeIndex {
        match find_output_node(graph, input.item) {
            Some(existing_index) => {
                graph[existing_index].as_output_mut().value += input.value;
                existing_index
            }
            None => graph.add_node(NodeValue::new_output(input)),
        }
    }

    fn create_or_update_production_node(
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

    fn create_or_update_edge(
        child_index: NodeIndex,
        parent_index: NodeIndex,
        weight: NodeEdge,
        graph: &mut GraphType,
    ) {
        if let Some(edge_index) = graph.find_edge(child_index, parent_index) {
            assert!(graph[edge_index].item() == weight.item());
            graph[edge_index].value += weight.value();
        } else {
            graph.add_edge(child_index, parent_index, weight);
        }
    }
}

fn merge_by_product_node<'a>(
    desired_output: ItemValuePair,
    _src_graph: &ScoredGraphType<'a>,
    dest_graph: &mut GraphType<'a>,
    _input_limits: &mut HashMap<Item, f64>,
) {
    // TODO: follow outgoing path

    // TODO: only create if there are no outgoing paths
    match find_by_product_node(dest_graph, desired_output.item) {
        Some(existing_index) => *dest_graph[existing_index].as_by_product_mut() += desired_output,
        None => {
            dest_graph.add_node(NodeValue::new_by_product(desired_output));
        }
    };
}

#[derive(Debug, Copy, Clone)]
struct OutputNodeScore {
    output: ItemValuePair,
    index: NodeIndex,
    score: f64,
    unique_inputs: u8,
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
    config: &'a PlanConfig,
    pub graph: ScoredGraphType<'a>,
    unique_inputs_by_item: HashMap<Item, u8>,
    output_nodes: Vec<OutputNodeScore>,
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

    fn output_children(
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

    fn production_children(
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

    combinations
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::game::Machine;

    use super::*;

    #[test]
    fn single_production_node() {
        let mut recipes: Vec<Recipe> = build_recipe_db();
        recipes.retain_mut(|r| !r.alternate);

        let config = PlanConfig::new(
            vec![ItemValuePair::new(Item::IronIngot, 30.0)],
            recipes.clone().into(),
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
            recipes.clone().into(),
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
            recipes.clone().into(),
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
            recipes.clone().into(),
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
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::IronOre, 30.0)],
                outputs: vec![ItemValuePair::new(Item::IronIngot, 30.0)],
                power_multiplier: 1.0,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Copper Ingot".into(),
                alternate: false,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::CopperOre, 30.0)],
                outputs: vec![ItemValuePair::new(Item::CopperIngot, 30.0)],
                power_multiplier: 1.0,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Caterium Ingot".into(),
                alternate: false,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::CateriumOre, 45.0)],
                outputs: vec![ItemValuePair::new(Item::CateriumIngot, 15.0)],
                power_multiplier: 1.0,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Pure Iron Ingot".into(),
                alternate: true,
                ficsmas: false,
                inputs: vec![
                    ItemValuePair::new(Item::IronOre, 35.0),
                    ItemValuePair::new(Item::Water, 20.0),
                ],
                outputs: vec![ItemValuePair::new(Item::IronIngot, 65.0)],
                power_multiplier: 1.0,
                machine: Machine::Refinery,
            },
            Recipe {
                name: "Iron Alloy Ingot".into(),
                alternate: true,
                ficsmas: false,
                inputs: vec![
                    ItemValuePair::new(Item::IronOre, 20.0),
                    ItemValuePair::new(Item::CopperOre, 20.0),
                ],
                outputs: vec![ItemValuePair::new(Item::IronIngot, 50.0)],
                power_multiplier: 1.0,
                machine: Machine::Foundry,
            },
            Recipe {
                name: "Iron Plate".into(),
                alternate: false,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::IronIngot, 30.0)],
                outputs: vec![ItemValuePair::new(Item::IronPlate, 20.0)],
                power_multiplier: 1.0,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Iron Rod".into(),
                alternate: false,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::IronIngot, 15.0)],
                outputs: vec![ItemValuePair::new(Item::IronRod, 15.0)],
                power_multiplier: 1.0,
                machine: Machine::Smelter,
            },
            Recipe {
                name: "Wire".into(),
                alternate: false,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::CopperIngot, 15.0)],
                outputs: vec![ItemValuePair::new(Item::Wire, 30.0)],
                power_multiplier: 1.0,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Iron Wire".into(),
                alternate: true,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::IronIngot, 12.5)],
                outputs: vec![ItemValuePair::new(Item::Wire, 22.5)],
                power_multiplier: 1.0,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Caterium Wire".into(),
                alternate: true,
                ficsmas: false,
                inputs: vec![ItemValuePair::new(Item::CateriumIngot, 15.0)],
                outputs: vec![ItemValuePair::new(Item::Wire, 120.0)],
                power_multiplier: 1.0,
                machine: Machine::Constructor,
            },
            Recipe {
                name: "Fused Wire".into(),
                alternate: true,
                ficsmas: false,
                inputs: vec![
                    ItemValuePair::new(Item::CateriumIngot, 3.0),
                    ItemValuePair::new(Item::CopperIngot, 12.0),
                ],
                outputs: vec![ItemValuePair::new(Item::Wire, 90.0)],
                power_multiplier: 1.0,
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
                .unwrap_or_else(|| {
                    panic!(
                        "Edge connecting {} to {} was not found in actual graph",
                        format_node(&expected[edge.source()]),
                        format_node(&expected[edge.target()])
                    )
                });

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
