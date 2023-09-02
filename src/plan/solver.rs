use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::{collections::HashMap, rc::Rc};
use thiserror::Error;

use super::{
    find_by_product_node, find_input_node, find_output_node, GraphType, Node, NodeEdge,
    ScoredGraph, ScoredNodeValue,
};
use crate::{
    game::{Item, ItemValuePair, Recipe},
    plan::{find_production_node, NodeValue, PlanConfig},
    utils::{clamp_to_zero, FloatType, EPSILON},
};

#[derive(Error, Debug)]
#[error("Unsolvable Plan: Unable to craft the desired quantity of `{0}`")]
pub struct SolverError(String);

pub type SolverResult<T> = Result<T, SolverError>;

pub fn solve(config: &PlanConfig) -> SolverResult<GraphType> {
    Solver::new(config).solve()
}

struct MergeCandidate {
    index: NodeIndex,
    desired_output: ItemValuePair,
}

impl MergeCandidate {
    #[inline]
    fn new(index: NodeIndex, desired_output: ItemValuePair) -> Self {
        Self {
            index,
            desired_output,
        }
    }
}

struct Solver<'a> {
    scored_graph: ScoredGraph<'a>,
    input_limits: HashMap<Rc<Item>, FloatType>,
}

impl<'a> Solver<'a> {
    fn new(config: &'a PlanConfig) -> Self {
        let mut scored_graph = ScoredGraph::new(config);
        scored_graph.build();

        Self {
            scored_graph,
            input_limits: config.inputs.clone(),
        }
    }

    #[inline]
    fn get_limit(&self, item: &Rc<Item>) -> FloatType {
        self.input_limits.get(item).copied().unwrap_or_default()
    }

    #[inline]
    fn update_limit(&mut self, item: Rc<Item>, amount: FloatType) {
        *self.input_limits.entry(item).or_default() += amount
    }

    fn solve(&mut self) -> SolverResult<GraphType> {
        let mut graph: GraphType = GraphType::new();
        for node in self.scored_graph.output_nodes.clone() {
            let merge_candidate = MergeCandidate::new(node.index, node.value.clone());
            self.copy_optimal_path(merge_candidate, &mut graph)?;
        }
        self.cleanup_by_product_nodes(&mut graph);

        Ok(graph)
    }

    fn copy_optimal_path(
        &mut self,
        node: MergeCandidate,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        match &self.scored_graph.graph[node.index] {
            ScoredNodeValue::Input(item) => {
                assert!(node.desired_output.item == *item);
                self.copy_input(node, graph)
            }
            ScoredNodeValue::Output(item) => {
                assert!(node.desired_output.item == *item);
                self.copy_output(node, graph)
            }
            ScoredNodeValue::Production(recipe) => {
                self.copy_production(node, Rc::clone(recipe), graph)
            }
            ScoredNodeValue::ByProduct(bp) => {
                assert!(node.desired_output.item == bp.item);
                self.copy_by_product(node, graph)
            }
        }
    }

    fn copy_output(
        &mut self,
        candidate: MergeCandidate,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let child_idx = self.scored_graph.output_child(candidate.index).unwrap();
        let child_candidate = MergeCandidate::new(child_idx, candidate.desired_output.clone());
        let (new_child_idx, leftover_output) = self.copy_optimal_path(child_candidate, graph)?;
        if leftover_output.value > EPSILON {
            return Err(SolverError(leftover_output.item.name.clone()));
        }

        let idx = Self::merge_output_node(candidate.desired_output.clone(), graph);
        Self::merge_edge(
            new_child_idx,
            idx,
            NodeEdge::new(candidate.desired_output, 0),
            graph,
        );

        Ok((idx, leftover_output))
    }

    fn copy_by_product(
        &mut self,
        node: MergeCandidate,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let mut remaining_output =
            Self::calc_by_product_required_output(&node.desired_output, graph);
        if remaining_output.is_zero() {
            return Ok((
                find_by_product_node(graph, &node.desired_output.item).unwrap(),
                remaining_output,
            ));
        }

        let mut new_children = Vec::new();
        for child in self.scored_graph.by_product_children(node.index) {
            if remaining_output.is_zero() {
                break;
            }

            let child_candidate = MergeCandidate::new(child.1, remaining_output.clone());
            if let Ok((new_idx, leftover)) = self.copy_optimal_path(child_candidate, graph) {
                let used_output = remaining_output - leftover.value;
                new_children.push((new_idx, used_output));
                remaining_output = leftover;
            }
        }

        let actual_output = &node.desired_output - &remaining_output;
        if actual_output.is_zero() {
            return Err(SolverError(node.desired_output.item.name.clone()));
        }

        let idx = Self::merge_by_product_node(actual_output, graph);
        for (order, child) in new_children.iter().enumerate() {
            Self::merge_edge(
                child.0,
                idx,
                NodeEdge::new(child.1.clone(), order as u32),
                graph,
            );
        }

        Ok((idx, remaining_output))
    }

    fn copy_production(
        &mut self,
        candidate: MergeCandidate,
        recipe: Rc<Recipe>,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let recipe_output = recipe
            .find_output_by_item(&candidate.desired_output.item)
            .unwrap();
        let desired_building_count = candidate.desired_output.ratio(recipe_output);

        let mut actual_building_count = desired_building_count;
        let mut new_children: Vec<(NodeIndex, ItemValuePair, FloatType)> = Vec::new();
        for (edge, node) in self.scored_graph.production_children(candidate.index) {
            let item = &self.scored_graph[edge].item;
            let recipe_input = recipe.find_input_by_item(item).unwrap();
            let desired_output = recipe_input.mul(actual_building_count);

            let child_candidate = MergeCandidate::new(node, desired_output.clone());
            match self.copy_optimal_path(child_candidate, graph) {
                Ok((child_index, leftover_output)) => {
                    let used_output = desired_output - leftover_output;
                    let building_count = used_output.ratio(recipe_input);
                    actual_building_count = actual_building_count.min(building_count);
                    new_children.push((child_index, used_output, building_count));
                }
                Err(..) => {
                    actual_building_count = 0.0;
                }
            }

            // floating points errors might give us very small positive or negative numbers,
            // so just clamp those to 0
            actual_building_count = clamp_to_zero(actual_building_count);
            if actual_building_count == 0.0 {
                break;
            }
        }

        let result = if actual_building_count > 0.0 {
            let node_idx =
                Self::merge_production_node(Rc::clone(&recipe), actual_building_count, graph);

            Self::merge_by_products_for_other_outputs(
                node_idx,
                recipe
                    .outputs
                    .iter()
                    .filter(|o| o.item != candidate.desired_output.item),
                actual_building_count,
                graph,
            );

            for (order, child) in new_children.iter().enumerate() {
                let output_value = child.1.mul(actual_building_count / child.2);

                Self::merge_edge(
                    child.0,
                    node_idx,
                    NodeEdge::new(output_value, order as u32),
                    graph,
                );
            }

            Ok((
                node_idx,
                candidate
                    .desired_output
                    .mul(1.0 - actual_building_count / desired_building_count),
            ))
        } else {
            Err(SolverError(candidate.desired_output.item.name.clone()))
        };

        // before returning, we need to propagate any difference between our initial calculated
        // building count and the actual building count based on what is possible from our inputs
        if actual_building_count <= desired_building_count {
            for child in new_children {
                let reduced_value = child.1.mul(1.0 - actual_building_count / child.2);
                self.propagate_reduction(child.0, reduced_value, graph);
            }
        }

        result
    }

    fn copy_input(
        &mut self,
        node: MergeCandidate,
        graph: &mut GraphType,
    ) -> SolverResult<(NodeIndex, ItemValuePair)> {
        let available_input = FloatType::min(
            node.desired_output.value,
            self.get_limit(&node.desired_output.item),
        );
        if available_input <= 0.0 {
            return Err(SolverError(node.desired_output.item.name.clone()));
        }

        let node_index = match find_input_node(graph, &node.desired_output.item) {
            Some(existing_index) => {
                *graph[existing_index].as_input_mut() += available_input;
                existing_index
            }
            None => graph.add_node(NodeValue::Input(
                node.desired_output.with_value(available_input),
            )),
        };

        self.update_limit(Rc::clone(&node.desired_output.item), -available_input);
        Ok((node_index, node.desired_output - available_input))
    }

    fn calc_by_product_required_output(
        desired_output: &ItemValuePair,
        graph: &mut GraphType,
    ) -> ItemValuePair {
        match find_by_product_node(graph, &desired_output.item) {
            Some(node_idx) => {
                let mut used_output = 0.0;
                for edge in graph.edges_directed(node_idx, Outgoing) {
                    used_output += edge.weight().value.value;
                }

                let remaining_output = graph[node_idx].as_by_product().clone() - used_output;
                if remaining_output > *desired_output {
                    desired_output.with_value(0.0)
                } else {
                    desired_output - remaining_output
                }
            }
            None => desired_output.clone(),
        }
    }

    fn propagate_reduction(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType,
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
        graph: &mut GraphType,
    ) -> bool {
        let input = graph[node_index].as_input();
        let new_value = FloatType::max(0.0, input.value - amount.value);

        self.update_limit(Rc::clone(&input.item), input.value - new_value);
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
        idx: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType,
    ) -> bool {
        if amount.value < EPSILON {
            return false;
        }

        let production = graph[idx].as_production().clone();
        let recipe_output = production.recipe.find_output_by_item(&amount.item).unwrap();
        let new_building_count =
            FloatType::max(0.0, production.machine_count - amount.ratio(recipe_output));
        graph[idx].as_production_mut().machine_count = new_building_count;

        let mut children: Vec<(EdgeIndex, NodeIndex)> = graph
            .edges_directed(idx, Incoming)
            .map(|e| (e.id(), e.source()))
            .collect();
        children.sort_unstable_by(|a, b| graph[a.0].order.cmp(&graph[b.0].order).reverse());

        for (edge_idx, child_idx) in children {
            let recipe_input = production
                .recipe
                .find_input_by_item(&graph[edge_idx].value.item)
                .unwrap();
            let reduce_amount =
                graph[edge_idx].value.clone() - recipe_input.mul(new_building_count).value;

            graph[edge_idx].value -= reduce_amount.value;
            self.propagate_reduction(child_idx, reduce_amount, graph);
        }

        let mut parent_walker = graph.neighbors_directed(idx, Outgoing).detach();
        while let Some((edge_idx, parent_idx)) = parent_walker.next(graph) {
            let item = &graph[parent_idx].as_by_product().item;
            if *item == amount.item {
                continue;
            }

            let recipe_output = production.recipe.find_output_by_item(item).unwrap();
            let new_amount = recipe_output.mul(new_building_count).value;
            graph[edge_idx].value.value = new_amount;
            graph[parent_idx].as_by_product_mut().value = new_amount;
        }

        if new_building_count <= 0.0 {
            graph.remove_node(idx);
            true
        } else {
            false
        }
    }

    fn propagate_reduction_by_product_node(
        &mut self,
        node_index: NodeIndex,
        amount: ItemValuePair,
        graph: &mut GraphType,
    ) -> bool {
        if amount.value < EPSILON {
            return false;
        }

        *graph[node_index].as_by_product_mut() -= amount.value;

        let mut children: Vec<(EdgeIndex, NodeIndex)> = graph
            .edges_directed(node_index, Incoming)
            .map(|e| (e.id(), e.source()))
            .collect();
        children.sort_unstable_by(|a, b| graph[a.0].order.cmp(&graph[b.0].order).reverse());

        let mut all_deleted = false;
        let mut remaining_amount = amount;
        for child in children {
            if remaining_amount.is_zero() {
                break;
            }

            let reduce_amount = if remaining_amount > graph[child.0].value {
                graph[child.0].value.clone()
            } else {
                remaining_amount.clone()
            };
            remaining_amount -= reduce_amount.value;

            graph[child.0].value -= reduce_amount.value;
            all_deleted &= self.propagate_reduction(child.1, reduce_amount, graph);
        }

        if all_deleted {
            graph.remove_node(node_index);
        }

        all_deleted
    }

    fn merge_output_node(output: ItemValuePair, graph: &mut GraphType) -> NodeIndex {
        match find_output_node(graph, &output.item) {
            Some(existing_index) => {
                graph[existing_index].as_output_mut().value += output.value;
                existing_index
            }
            None => graph.add_node(NodeValue::new_output(output)),
        }
    }

    fn merge_production_node(
        recipe: Rc<Recipe>,
        machine_count: FloatType,
        graph: &mut GraphType,
    ) -> NodeIndex {
        match find_production_node(graph, &recipe) {
            Some(existing_index) => {
                graph[existing_index].as_production_mut().machine_count += machine_count;
                existing_index
            }
            None => graph.add_node(NodeValue::new_production(recipe, machine_count)),
        }
    }

    fn merge_by_product_node(output: ItemValuePair, graph: &mut GraphType) -> NodeIndex {
        match find_by_product_node(graph, &output.item) {
            Some(existing_index) => {
                *graph[existing_index].as_by_product_mut() += output.value;
                existing_index
            }
            None => graph.add_node(NodeValue::new_by_product(output)),
        }
    }

    fn merge_by_products_for_other_outputs<'b>(
        node_idx: NodeIndex,
        recipe_outputs: impl Iterator<Item = &'b ItemValuePair>,
        building_count: f32,
        graph: &mut GraphType,
    ) {
        for recipe_output in recipe_outputs {
            let extra_output = recipe_output.mul(building_count);
            let parent_idx = Self::merge_by_product_node(extra_output.clone(), graph);

            Self::merge_edge(node_idx, parent_idx, NodeEdge::new(extra_output, 0), graph);
        }
    }

    fn merge_edge(
        child_index: NodeIndex,
        parent_index: NodeIndex,
        weight: NodeEdge,
        graph: &mut GraphType,
    ) {
        if let Some(edge_index) = graph.find_edge(child_index, parent_index) {
            assert!(graph[edge_index].item() == weight.item());
            graph[edge_index].value += weight.value();
            graph[edge_index].order = graph[edge_index].order.max(weight.order);
        } else {
            graph.add_edge(child_index, parent_index, weight);
        }
    }

    fn cleanup_by_product_nodes(&self, graph: &mut GraphType) {
        let by_product_nodes: Vec<NodeIndex> = graph
            .node_indices()
            .filter(|i| graph[*i].is_by_product())
            .collect();

        by_product_nodes
            .iter()
            .for_each(|i| self.cleanup_by_product(*i, graph));
    }

    fn cleanup_by_product(&self, node_idx: NodeIndex, graph: &mut GraphType) {
        let mut parents: Vec<(NodeIndex, ItemValuePair)> = graph
            .edges_directed(node_idx, Outgoing)
            .map(|e| (e.target(), e.weight().value.clone()))
            .collect();
        let mut children: Vec<(NodeIndex, ItemValuePair)> = graph
            .edges_directed(node_idx, Incoming)
            .map(|e| (e.source(), e.weight().value.clone()))
            .collect();

        parents.sort_unstable_by(|a, b| a.1.cmp(&b.1));
        children.sort_unstable_by(|a, b| a.1.cmp(&b.1).reverse());

        let mut current_child = children.pop().unwrap();
        for parent in parents {
            let mut remaining_output = parent.1;
            loop {
                if current_child.1.is_zero() {
                    Self::delete_edge_between(graph, current_child.0, node_idx);
                    current_child = children.pop().unwrap();
                }

                if remaining_output > current_child.1 {
                    graph.add_edge(
                        current_child.0,
                        parent.0,
                        NodeEdge::new(current_child.1.clone(), 0),
                    );
                    remaining_output -= current_child.1.value;
                    current_child.1.value = 0.0;
                } else {
                    graph.add_edge(
                        current_child.0,
                        parent.0,
                        NodeEdge::new(remaining_output.clone(), 0),
                    );
                    current_child.1 -= remaining_output.value;
                    remaining_output.value = 0.0;
                    break;
                }
            }
            Self::delete_edge_between(graph, node_idx, parent.0);
        }

        let remaining_output = clamp_to_zero(
            current_child.1.value + children.iter().map(|c| c.1.value).sum::<FloatType>(),
        );
        if remaining_output > 0.0 {
            graph[node_idx].as_by_product_mut().value = remaining_output;

            if !current_child.1.is_zero() {
                let edge_index = graph.find_edge(current_child.0, node_idx).unwrap();
                graph[edge_index].value = current_child.1
            }
        } else {
            graph.remove_node(node_idx);
        }
    }

    fn delete_edge_between(graph: &mut GraphType, a: NodeIndex, b: NodeIndex) -> bool {
        graph
            .find_edge(a, b)
            .map(|e| {
                graph.remove_edge(e);
                true
            })
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use crate::game::test::get_test_game_db_with_recipes;

    use super::*;
    use petgraph::visit::IntoEdgeReferences;

    #[test]
    fn test_single_production_node() {
        let game_db = get_test_game_db_with_recipes(&["Recipe_IngotIron_C"]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();

        let config = PlanConfig::new(
            vec![ItemValuePair::new(Rc::clone(&iron_ingot), 30.0)],
            game_db.clone(),
        );

        let mut expected_graph = GraphType::new();
        let output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&iron_ingot),
            30.0,
        )));
        let smelter_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&iron_ingot_recipe),
            1.0,
        ));
        let input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&iron_ore),
            30.0,
        )));

        expected_graph.add_edge(
            smelter_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ingot), 30.0), 0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ore), 30.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_single_production_node_optimizes_resources() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_Alternate_PureIronIngot_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        let iron_ingot_recipe = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();

        let config = PlanConfig::new(
            vec![ItemValuePair::new(Rc::clone(&iron_ingot), 65.0)],
            game_db,
        );

        let mut expected_graph = GraphType::new();
        let output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&iron_ingot),
            65.0,
        )));
        let refinery_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&iron_ingot_recipe),
            1.0,
        ));
        let ore_input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&iron_ore),
            35.0,
        )));

        let water_input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&water),
            20.0,
        )));

        expected_graph.add_edge(
            refinery_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ingot), 65.0), 0),
        );
        expected_graph.add_edge(
            ore_input_idx,
            refinery_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ore), 35.0), 0),
        );
        expected_graph.add_edge(
            water_input_idx,
            refinery_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&water), 20.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_multiple_outputs() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IronPlate_C",
            "Recipe_IronRod_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let iron_plate = game_db.find_item("Desc_IronPlate_C").unwrap();
        let iron_rod = game_db.find_item("Desc_IronRod_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();
        let iron_plate_recipe = game_db.find_recipe("Recipe_IronPlate_C").unwrap();
        let iron_rod_recipe = game_db.find_recipe("Recipe_IronRod_C").unwrap();

        let config = PlanConfig::new(
            vec![
                ItemValuePair::new(Rc::clone(&iron_rod), 30.0),
                ItemValuePair::new(Rc::clone(&iron_plate), 60.0),
            ],
            game_db,
        );

        let mut expected_graph = GraphType::new();
        let plate_output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&iron_plate),
            60.0,
        )));
        let rod_output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&iron_rod),
            30.0,
        )));

        let plate_prod_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&iron_plate_recipe),
            3.0,
        ));
        let rod_prod_idx =
            expected_graph.add_node(NodeValue::new_production(Rc::clone(&iron_rod_recipe), 2.0));
        let smelter_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&iron_ingot_recipe),
            4.0,
        ));
        let input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&iron_ore),
            120.0,
        )));

        expected_graph.add_edge(
            plate_prod_idx,
            plate_output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_plate), 60.0), 0),
        );

        expected_graph.add_edge(
            rod_prod_idx,
            rod_output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_rod), 30.0), 0),
        );

        expected_graph.add_edge(
            smelter_idx,
            rod_prod_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ingot), 30.0), 0),
        );

        expected_graph.add_edge(
            smelter_idx,
            plate_prod_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ingot), 90.0), 0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ore), 120.0), 0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_input_limits() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IngotCopper_C",
            "Recipe_IngotCaterium_C",
            "Recipe_Wire_C",
            "Recipe_Alternate_FusedWire_C",
            "Recipe_Alternate_Wire_1_C",
            "Recipe_Alternate_Wire_2_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();

        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let copper_ingot = game_db.find_item("Desc_CopperIngot_C").unwrap();
        let caterium_ingot = game_db.find_item("Desc_GoldIngot_C").unwrap();

        let wire = game_db.find_item("Desc_Wire_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();
        let copper_ingot_recipe = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();
        let caterium_ingot_recipe = game_db.find_recipe("Recipe_IngotCaterium_C").unwrap();

        let iron_wire_recipe = game_db.find_recipe("Recipe_Alternate_Wire_1_C").unwrap();
        let fused_wire_recipe = game_db.find_recipe("Recipe_Alternate_FusedWire_C").unwrap();
        let caterium_wire_recipe = game_db.find_recipe("Recipe_Alternate_Wire_2_C").unwrap();

        let mut input_limits = HashMap::new();
        input_limits.insert(Rc::clone(&iron_ore), 12.5);
        input_limits.insert(Rc::clone(&copper_ore), 12.0);

        let config = PlanConfig::with_inputs(
            input_limits,
            vec![ItemValuePair::new(Rc::clone(&wire), 232.5)],
            game_db,
        );

        let mut expected_graph = GraphType::new();
        let output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&wire),
            232.5,
        )));

        let cat_wire_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&caterium_wire_recipe),
            1.0,
        ));

        let fused_wire_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&fused_wire_recipe),
            1.0,
        ));

        let iron_wire_idx =
            expected_graph.add_node(NodeValue::new_production(Rc::clone(&iron_wire_recipe), 1.0));

        let iron_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&iron_ingot_recipe),
            12.5 / 30.0,
        ));

        let copper_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&copper_ingot_recipe),
            0.4,
        ));

        let cat_ingot_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&caterium_ingot_recipe),
            1.2,
        ));

        let iron_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&iron_ore),
            12.5,
        )));

        let copper_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&copper_ore),
            12.0,
        )));

        let cat_ore_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&caterium_ore),
            54.0,
        )));

        expected_graph.add_edge(
            cat_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&wire), 120.0), 2),
        );

        expected_graph.add_edge(
            fused_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&wire), 90.0), 0),
        );

        expected_graph.add_edge(
            iron_wire_idx,
            output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&wire), 22.5), 0),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            cat_wire_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&caterium_ingot), 15.0), 2),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            fused_wire_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&caterium_ingot), 3.0), 2),
        );

        expected_graph.add_edge(
            copper_ingot_idx,
            fused_wire_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&copper_ingot), 12.0), 2),
        );

        expected_graph.add_edge(
            iron_ingot_idx,
            iron_wire_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ingot), 12.5), 2),
        );

        expected_graph.add_edge(
            iron_ore_idx,
            iron_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&iron_ore), 12.5), 2),
        );

        expected_graph.add_edge(
            copper_ore_idx,
            copper_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&copper_ore), 12.0), 2),
        );

        expected_graph.add_edge(
            cat_ore_idx,
            cat_ingot_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&caterium_ore), 54.0), 2),
        );

        let result = solve(&config);

        assert!(result.is_ok(), "{:?}", result);
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_multiple_output_recipes() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_Alternate_HeavyOilResidue_C",
            "Recipe_ResidualFuel_C",
            "Recipe_ResidualPlastic_C",
        ]);

        let oil = game_db.find_item("Desc_LiquidOil_C").unwrap();
        let fuel = game_db.find_item("Desc_LiquidFuel_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let heavy_oil_residue = game_db.find_item("Desc_HeavyOilResidue_C").unwrap();
        let plastic = game_db.find_item("Desc_Plastic_C").unwrap();
        let polymer_resin = game_db.find_item("Desc_PolymerResin_C").unwrap();

        let hor_recipe = game_db
            .find_recipe("Recipe_Alternate_HeavyOilResidue_C")
            .unwrap();
        let residual_fuel_recipe = game_db.find_recipe("Recipe_ResidualFuel_C").unwrap();
        let residual_plastic_recipe = game_db.find_recipe("Recipe_ResidualPlastic_C").unwrap();

        let config = PlanConfig::new(
            vec![
                ItemValuePair::new(Rc::clone(&fuel), 180.0),
                ItemValuePair::new(Rc::clone(&plastic), 30.0),
            ],
            game_db,
        );

        let mut expected_graph = GraphType::new();
        let fuel_output_idx = expected_graph.add_node(NodeValue::new_output(ItemValuePair::new(
            Rc::clone(&fuel),
            180.0,
        )));
        let plastic_output_idx = expected_graph.add_node(NodeValue::new_output(
            ItemValuePair::new(Rc::clone(&plastic), 30.0),
        ));

        let resin_by_prod_idx = expected_graph.add_node(NodeValue::new_by_product(
            ItemValuePair::new(Rc::clone(&polymer_resin), 45.0),
        ));

        let hor_idx =
            expected_graph.add_node(NodeValue::new_production(Rc::clone(&hor_recipe), 6.75));

        let plastic_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&residual_plastic_recipe),
            1.5,
        ));

        let fuel_idx = expected_graph.add_node(NodeValue::new_production(
            Rc::clone(&residual_fuel_recipe),
            4.5,
        ));

        let oil_input_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&oil),
            202.5,
        )));

        let water_idx = expected_graph.add_node(NodeValue::new_input(ItemValuePair::new(
            Rc::clone(&water),
            30.0,
        )));

        expected_graph.add_edge(
            fuel_idx,
            fuel_output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&fuel), 180.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            fuel_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&heavy_oil_residue), 270.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            resin_by_prod_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&polymer_resin), 45.0), 0),
        );

        expected_graph.add_edge(
            hor_idx,
            plastic_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&polymer_resin), 90.0), 0),
        );

        expected_graph.add_edge(
            water_idx,
            plastic_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&water), 30.0), 0),
        );

        expected_graph.add_edge(
            plastic_idx,
            plastic_output_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&plastic), 30.0), 0),
        );

        expected_graph.add_edge(
            oil_input_idx,
            hor_idx,
            NodeEdge::new(ItemValuePair::new(Rc::clone(&oil), 202.5), 0),
        );

        let result = solve(&config);

        assert!(result.is_ok(), "{:?}", result);
        assert_graphs_equal(result.unwrap(), expected_graph);
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

    fn float_equals(a: FloatType, b: FloatType) -> bool {
        FloatType::abs(a - b) < EPSILON
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
