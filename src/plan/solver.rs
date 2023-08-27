use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::collections::HashMap;
use thiserror::Error;

use super::{
    find_by_product_child, find_by_product_node, find_input_node, find_output_node, GraphType,
    NodeEdge, PathChain, Production, ScoredGraph,
};
use crate::{
    game::{Item, ItemValuePair, Recipe},
    plan::{find_production_node, NodeValue, PlanConfig},
    utils::EPSILON,
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
    scored_graph: ScoredGraph<'a>,
    input_limits: HashMap<Item, f64>,
}

impl<'a> Solver<'a> {
    fn new(config: &'a PlanConfig) -> Self {
        let mut scored_graph = ScoredGraph::new(config);
        scored_graph.build();

        let input_limits = config.inputs.clone();

        Self {
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
        self.cleanup_by_product_nodes(&mut graph);

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
            NodeValue::ByProduct(..) => self.merge_by_product_node(node, graph),
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
            let desired_output = recipe_input * machine_count;
            let mut new_children = Vec::new();
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
                if let Ok((child_index, leftover_output)) =
                    self.merge_optimal_path(child_node, graph)
                {
                    new_children.push((child_index, remaining_output - leftover_output.value));
                    remaining_output = leftover_output;
                }
            }

            new_children_by_inputs.push(new_children);
            min_machine_count = f64::min(
                min_machine_count,
                (desired_output - remaining_output) / recipe_input,
            );
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

        Ok((
            node_index,
            node.desired_output - (recipe_output * min_machine_count),
        ))
    }

    fn merge_by_product_node(
        &mut self,
        node: MergeNode,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let (_, prod_node_index) = find_by_product_child(node.index, &self.scored_graph.graph);
        let recipe = self.scored_graph[prod_node_index].as_production().recipe;
        let recipe_output = recipe.find_output_by_item(node.item()).unwrap();

        let desired_output = if let Some(index) = find_production_node(graph, recipe) {
            let machine_count = graph[index].as_production().machine_count;
            let current_output = *recipe_output * machine_count;
            if current_output >= node.desired_output {
                ItemValuePair::new(node.item(), 0.0)
            } else {
                node.desired_output - current_output
            }
        } else {
            node.desired_output
        };

        let production_node = MergeNode::new(prod_node_index, node.chain, desired_output);
        let (new_prod_index, leftover_output) = self.merge_optimal_path(production_node, graph)?;
        let machine_count = graph[new_prod_index].as_production().machine_count;

        let mut by_product_index = None;
        for output in &recipe.outputs {
            let output_value = *output * machine_count;

            let node_index = match find_by_product_node(graph, output.item) {
                Some(existing_index) => {
                    *graph[existing_index].as_by_product_mut() = output_value;
                    existing_index
                }
                None => graph.add_node(NodeValue::new_by_product(output_value)),
            };

            graph.update_edge(new_prod_index, node_index, NodeEdge::new(output_value, 0));
            if output.item == desired_output.item {
                by_product_index = Some(node_index);
            }
        }

        Ok((by_product_index.unwrap(), leftover_output))
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
            NodeValue::Input(..) => self.propagate_reduction_input_node(node_index, amount, graph),
            NodeValue::Production(..) => {
                self.propagate_reduction_production_node(node_index, amount, graph)
            }
            NodeValue::ByProduct(..) => {
                self.propagate_reduction_by_product_node(node_index, amount, graph)
            }
            _ => {
                panic!("Output nodes can not be reduced");
            }
        }
    }

    fn propagate_reduction_input_node(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType<'_>,
    ) -> bool {
        let input = graph[node_index].as_input();
        let new_value = f64::max(0.0, (*input - amount).value);

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
        if amount.value < EPSILON {
            return false;
        }

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
            children.sort_unstable_by(|a, b| graph[a.0].order.cmp(&graph[b.0].order));

            let recipe_input = *production.recipe.find_input_by_item(item).unwrap();
            let mut required_input = recipe_input * new_machine_count;
            for (edge_index, child_index) in children {
                required_input -= graph[edge_index].value;
                if required_input.value < 0.0 {
                    let reduce_amount = -required_input;
                    required_input.value = 0.0;

                    graph[edge_index].value -= reduce_amount;
                    self.propagate_reduction(child_index, reduce_amount, graph);
                }
            }
        }

        if new_machine_count <= 0.0 {
            graph.remove_node(node_index);
            true
        } else {
            false
        }
    }

    #[allow(dead_code)]
    fn propagate_reduction_by_product_node(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType<'_>,
    ) -> bool {
        if amount.value < EPSILON {
            return false;
        }

        let (_, production_idx) = find_by_product_child(node_index, graph);
        let recipe = graph[production_idx].as_production().recipe;
        let machine_count = graph[production_idx].as_production().machine_count;

        let mut by_product_nodes = Vec::new();
        let mut required_machine_count: f64 = 0.0;
        for edge in graph.edges_directed(production_idx, Outgoing) {
            let item = graph[edge.target()].as_by_product().item;
            let recipe_output = recipe.find_output_by_item(item).unwrap();

            by_product_nodes.push((edge.id(), edge.target(), recipe_output));
            if edge.target() == node_index {
                continue;
            }

            let used_output: f64 = graph
                .edges_directed(edge.target(), Outgoing)
                .map(|e| e.weight().value())
                .sum();
            required_machine_count = required_machine_count.min(used_output / recipe_output.value);
        }

        let recipe_output = recipe.find_output_by_item(amount.item).unwrap();
        let new_machine_count =
            required_machine_count.max(machine_count - (amount / *recipe_output));
        let new_amount = *recipe_output * (machine_count - new_machine_count);

        if self.propagate_reduction_production_node(production_idx, new_amount, graph) {
            for (_, idx, _) in by_product_nodes {
                graph.remove_node(idx);
            }

            true
        } else {
            for (e_idx, n_idx, recipe_output) in by_product_nodes {
                let new_amount = *recipe_output * new_machine_count;
                graph[e_idx].value = new_amount;
                *graph[n_idx].as_by_product_mut() = new_amount;
            }

            false
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

    fn cleanup_by_product_nodes(&self, graph: &mut GraphType<'a>) {
        let by_product_nodes: Vec<NodeIndex> = graph
            .node_indices()
            .filter(|i| graph[*i].is_by_product())
            .collect();

        by_product_nodes
            .iter()
            .for_each(|i| self.cleanup_by_product(*i, graph));
    }

    fn cleanup_by_product(&self, node_index: NodeIndex, graph: &mut GraphType<'a>) {
        let (_, prod_index) = find_by_product_child(node_index, graph);

        let mut used_output: f64 = 0.0;
        let mut walker = graph.neighbors_directed(node_index, Outgoing).detach();

        while let Some((e, i)) = walker.next(graph) {
            let weight = graph[e];
            used_output += weight.value();

            // move the edge to go directly from the parent to the production node
            graph.add_edge(prod_index, i, weight);
            graph.remove_edge(e);
        }

        *graph[node_index].as_by_product_mut() -= used_output;
        let unused_output = graph[node_index].as_by_product();
        if unused_output.value.abs() < EPSILON {
            graph.remove_node(node_index);
        } else {
            graph.update_edge(prod_index, node_index, NodeEdge::new(*unused_output, 0));
        }
    }
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;

    use crate::game::Machine;

    use super::*;

    #[test]
    fn test_single_production_node() {
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
    fn test_single_production_node_optimizes_resources() {
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
    fn test_multiple_outputs() {
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
    fn test_input_limits() {
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

    #[test]
    fn test_multiple_output_recipes() {
        let recipes = build_recipe_db();

        let config = PlanConfig::new(
            vec![
                ItemValuePair::new(Item::Fuel, 180.0),
                ItemValuePair::new(Item::Plastic, 30.0),
            ],
            recipes.clone().into(),
        );

        let mut expected_graph = GraphType::new();
        let fuel_output_idx =
            expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(Item::Fuel, 180.0)));
        let plastic_output_idx = expected_graph.add_node(NodeValue::new_output(
            ItemValuePair::new(Item::Plastic, 30.0),
        ));

        let resin_by_prod_idx = expected_graph.add_node(NodeValue::new_by_product(
            ItemValuePair::new(Item::PolymerResin, 45.0),
        ));

        let hor_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Heavy Oil Residue", &recipes),
            6.75,
        ));

        let plastic_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Residual Plastic", &recipes),
            1.5,
        ));

        let fuel_idx = expected_graph.add_node(NodeValue::new_production(
            find_recipe("Residual Fuel", &recipes),
            4.5,
        ));

        let oil_input_idx =
            expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(Item::Oil, 202.5)));

        let water_idx =
            expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(Item::Water, 30.0)));

        expected_graph.add_edge(
            fuel_idx,
            fuel_output_idx,
            NodeEdge::new(ItemValuePair::new(Item::Fuel, 180.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            fuel_idx,
            NodeEdge::new(ItemValuePair::new(Item::HeavyOilResidue, 270.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            resin_by_prod_idx,
            NodeEdge::new(ItemValuePair::new(Item::PolymerResin, 45.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            plastic_idx,
            NodeEdge::new(ItemValuePair::new(Item::PolymerResin, 90.0), 0),
        );

        expected_graph.add_edge(
            water_idx,
            plastic_idx,
            NodeEdge::new(ItemValuePair::new(Item::Water, 30.0), 0),
        );

        expected_graph.add_edge(
            plastic_idx,
            plastic_output_idx,
            NodeEdge::new(ItemValuePair::new(Item::Plastic, 30.0), 0),
        );

        expected_graph.add_edge(
            oil_input_idx,
            hor_idx,
            NodeEdge::new(ItemValuePair::new(Item::Oil, 202.5), 0),
        );

        let result = solve(&config);

        assert!(result.is_ok(), "{:?}", result);
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    fn build_recipe_db() -> Vec<Recipe> {
        vec![
            Recipe::new_base(
                "Iron Ingot".into(),
                vec![ItemValuePair::new(Item::IronOre, 30.0)],
                vec![ItemValuePair::new(Item::IronIngot, 30.0)],
                Machine::Smelter,
            ),
            Recipe::new_base(
                "Copper Ingot".into(),
                vec![ItemValuePair::new(Item::CopperOre, 30.0)],
                vec![ItemValuePair::new(Item::CopperIngot, 30.0)],
                Machine::Smelter,
            ),
            Recipe::new_base(
                "Caterium Ingot".into(),
                vec![ItemValuePair::new(Item::CateriumOre, 45.0)],
                vec![ItemValuePair::new(Item::CateriumIngot, 15.0)],
                Machine::Smelter,
            ),
            Recipe::new_alt(
                "Pure Iron Ingot".into(),
                vec![
                    ItemValuePair::new(Item::IronOre, 35.0),
                    ItemValuePair::new(Item::Water, 20.0),
                ],
                vec![ItemValuePair::new(Item::IronIngot, 65.0)],
                Machine::Refinery,
            ),
            Recipe::new_alt(
                "Iron Alloy Ingot".into(),
                vec![
                    ItemValuePair::new(Item::IronOre, 20.0),
                    ItemValuePair::new(Item::CopperOre, 20.0),
                ],
                vec![ItemValuePair::new(Item::IronIngot, 50.0)],
                Machine::Foundry,
            ),
            Recipe::new_base(
                "Iron Plate".into(),
                vec![ItemValuePair::new(Item::IronIngot, 30.0)],
                vec![ItemValuePair::new(Item::IronPlate, 20.0)],
                Machine::Constructor,
            ),
            Recipe::new_base(
                "Iron Rod".into(),
                vec![ItemValuePair::new(Item::IronIngot, 15.0)],
                vec![ItemValuePair::new(Item::IronRod, 15.0)],
                Machine::Smelter,
            ),
            Recipe::new_base(
                "Wire".into(),
                vec![ItemValuePair::new(Item::CopperIngot, 15.0)],
                vec![ItemValuePair::new(Item::Wire, 30.0)],
                Machine::Constructor,
            ),
            Recipe::new_alt(
                "Iron Wire".into(),
                vec![ItemValuePair::new(Item::IronIngot, 12.5)],
                vec![ItemValuePair::new(Item::Wire, 22.5)],
                Machine::Constructor,
            ),
            Recipe::new_alt(
                "Caterium Wire".into(),
                vec![ItemValuePair::new(Item::CateriumIngot, 15.0)],
                vec![ItemValuePair::new(Item::Wire, 120.0)],
                Machine::Constructor,
            ),
            Recipe::new_alt(
                "Fused Wire".into(),
                vec![
                    ItemValuePair::new(Item::CateriumIngot, 3.0),
                    ItemValuePair::new(Item::CopperIngot, 12.0),
                ],
                vec![ItemValuePair::new(Item::Wire, 90.0)],
                Machine::Assembler,
            ),
            Recipe::new_alt(
                "Heavy Oil Residue".into(),
                vec![ItemValuePair::new(Item::Oil, 30.0)],
                vec![
                    ItemValuePair::new(Item::HeavyOilResidue, 40.0),
                    ItemValuePair::new(Item::PolymerResin, 20.0),
                ],
                Machine::Refinery,
            ),
            Recipe::new_base(
                "Residual Plastic".into(),
                vec![
                    ItemValuePair::new(Item::PolymerResin, 60.0),
                    ItemValuePair::new(Item::Water, 20.0),
                ],
                vec![ItemValuePair::new(Item::Plastic, 20.0)],
                Machine::Refinery,
            ),
            Recipe::new_base(
                "Residual Rubber".into(),
                vec![
                    ItemValuePair::new(Item::PolymerResin, 40.0),
                    ItemValuePair::new(Item::Water, 20.0),
                ],
                vec![ItemValuePair::new(Item::Rubber, 20.0)],
                Machine::Refinery,
            ),
            Recipe::new_base(
                "Residual Fuel".into(),
                vec![ItemValuePair::new(Item::HeavyOilResidue, 60.0)],
                vec![ItemValuePair::new(Item::Fuel, 40.0)],
                Machine::Refinery,
            ),
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
