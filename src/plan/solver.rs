use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::{find_production_node, NodeValue, PlanConfig};
use crate::utils::EPSILON;
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::Incoming,
};
use std::collections::HashMap;
use thiserror::Error;

use super::{
    find_by_product_node, find_input_node, find_output_node, GraphType, NodeEdge, PathChain,
    Production, ScoredGraph,
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

        for (order, (edge_index, by_product_index)) in self
            .scored_graph
            .production_by_products(node.index, &node.chain)
            .iter()
            .enumerate()
        {
            if self.scored_graph.graph[*by_product_index].is_by_product() {
                let edge_weight = &self.scored_graph[*edge_index];

                let recipe_output = *production
                    .recipe
                    .find_output_by_item(edge_weight.item())
                    .unwrap();
                let child_node = MergeNode::new(
                    *by_product_index,
                    edge_weight.chain.clone(),
                    recipe_output * min_machine_count,
                );
                let new_index = self.merge_by_product_node(child_node, graph);
                Self::create_or_update_edge(
                    node_index,
                    new_index,
                    NodeEdge::new(recipe_output * min_machine_count, order as u32),
                    graph,
                );
            }
        }

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

    fn merge_by_product_node(&mut self, node: MergeNode, graph: &mut GraphType<'a>) -> NodeIndex {
        match find_by_product_node(graph, node.item()) {
            Some(existing_index) => {
                *graph[existing_index].as_by_product_mut() += node.desired_output;
                existing_index
            }
            None => graph.add_node(NodeValue::new_by_product(node.desired_output)),
        }
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
                self.propagate_reduction_input_node(input, node_index, amount, graph)
            }
            NodeValue::Production(..) => {
                self.propagate_reduction_production_node(node_index, amount, graph)
            }
            _ => {
                panic!("Output and ByProduct nodes can not be reduced");
            }
        }
    }

    fn propagate_reduction_input_node(
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
