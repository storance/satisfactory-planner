use crate::game::{Recipe, Item, ItemValuePair};
use crate::plan::{PlanConfig, NodeValue};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::{Directed, Incoming};
use std::collections::HashMap;
use thiserror::Error;

use super::ScoredNodeValue;

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

struct Solver<'a, 'b> {
    config: &'b PlanConfig<'a>,
    recipes_by_output: HashMap<Item, Vec<&'a Recipe>>,
    recipes_by_input: HashMap<Item, Vec<&'a Recipe>>,
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

        // get order outputs

        // find optimal path
        todo!()
    }

    pub fn build_graph(&self) -> ScoredGraphType<'a> {
        let mut graph = Graph::new();

        let mut nodes: Vec<NodeIndex> = Vec::new();

        self.config.outputs.iter().for_each(|(item, value)| {
            let output_node = ScoredNodeValue::from(NodeValue::new_output(ItemValuePair::new(*item, *value), false));
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
