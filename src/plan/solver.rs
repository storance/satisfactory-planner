use crate::game::{Recipe, Item, ItemValuePair};
use crate::plan::{PlanConfig, PlanGraphNode};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::{Directed, Incoming};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Insufficient or missing input item `{0}`")]
    MissingInput(Item),
    #[error("No recipe found that produces item `{0}`")]
    NoMatchingRecipes(Item),
}

pub type GraphType<'a> = Graph<PlanGraphNode<'a>, ItemValuePair<f64>, Directed>;
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

    pub fn solve(&mut self) -> SolverResult<GraphType<'a>> {
        let mut graph = Graph::new();
        let mut output_nodes: Vec<NodeIndex> = Vec::new();

        self.config.outputs.iter().for_each(|(item, value)| {
            let output_node = PlanGraphNode::new_output(ItemValuePair::new(*item, *value), false);
            output_nodes.push(graph.add_node(output_node));
        });

        for node in &output_nodes {
            self.solve_node(*node, &mut graph)?;
        }

        Ok(graph)
    }

    fn solve_node(&self, node_index: NodeIndex, graph: &mut GraphType<'a>) -> SolverResult<()> {
        let inputs_to_solve = match graph[node_index] {
            PlanGraphNode::OutputNode(resource_value, ..) => vec![resource_value],
            PlanGraphNode::ProductionNode(recipe, machine_count) => recipe
                .inputs
                .iter()
                .map(|input| {
                    ItemValuePair::new(input.item, input.amount_per_minute * machine_count)
                })
                .collect(),
            _ => Vec::new(),
        };

        for input in inputs_to_solve {
            self.solve_for_item(input, node_index, graph)?;
        }

        Ok(())
    }

    fn solve_for_item(
        &self,
        item_value: ItemValuePair<f64>,
        parent_index: NodeIndex,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<()> {
        if item_value.item.is_extractable() {
            let input_limit = *self
                .config
                .input_limits
                .get(&item_value.item)
                .unwrap_or(&0.0);

            self.input_item(item_value, input_limit, parent_index, graph)
        } else if let Some(provided_input) = self.config.inputs.get(&item_value.item)
        {
            self.input_item(item_value, *provided_input, parent_index, graph)
        } else {
            self.produce_item(item_value, parent_index, graph)
        }
    }

    fn input_item(
        &self,
        item_value: ItemValuePair<f64>,
        input_limit: f64,
        parent_index: NodeIndex,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<()> {
        if let Some(existing_node_index) = self.find_input_node(graph, item_value.item) {
            match &mut graph[existing_node_index] {
                PlanGraphNode::InputNode(existing_resource_value) => {
                    if existing_resource_value.value + item_value.value > input_limit {
                        if item_value.item.is_extractable() {
                            return Err(SolverError::MissingInput(item_value.item));
                        } else {
                            existing_resource_value.value = input_limit;
                            let remaining_input = ItemValuePair::new(
                                item_value.item,
                                item_value.value - input_limit,
                            );
                            self.produce_item(remaining_input, parent_index, graph)?;
                        }
                    } else {
                        existing_resource_value.value += item_value.value;
                    }
                }
                _ => {
                    panic!("Unexpected node");
                }
            };
            graph.add_edge(existing_node_index, parent_index, item_value);
        } else {
            let (input_value, remaining_input) = if item_value.value > input_limit {
                (
                    ItemValuePair::new(item_value.item, input_limit),
                    Some(ItemValuePair::new(
                        item_value.item,
                        item_value.value - input_limit,
                    )),
                )
            } else {
                (item_value, None)
            };
            let child_node = PlanGraphNode::new_input(input_value);
            let child_index = graph.add_node(child_node);
            graph.add_edge(child_index, parent_index, item_value);

            remaining_input.map_or(Ok(()), |iv| self.produce_item(iv, parent_index, graph))?;
        }

        Ok(())
    }

    fn produce_item(
        &self,
        resource_value: ItemValuePair<f64>,
        parent_index: NodeIndex,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<()> {
        let recipes = self
            .recipes_by_output
            .get(&resource_value.item)
            .ok_or(SolverError::NoMatchingRecipes(resource_value.item))?;

        let recipe = recipes.get(0).unwrap();

        if let Some(existing_node_index) = self.find_production_node(graph, recipe) {
            graph.add_edge(existing_node_index, parent_index, resource_value);
            self.propagate_production_changes(existing_node_index, resource_value, graph)?;
        } else {
            let output = recipe.find_output_by_item(resource_value.item).unwrap();
            let machine_count = resource_value.value / output.amount_per_minute;

            let child_node = PlanGraphNode::new_production(*recipe, machine_count);
            let child_index = graph.add_node(child_node);
            graph.add_edge(child_index, parent_index, resource_value);

            self.solve_node(child_index, graph)?;
        }

        Ok(())
    }

    fn propagate_production_changes(
        &self,
        node_index: NodeIndex,
        additional_output: ItemValuePair<f64>,
        graph: &mut GraphType<'a>,
    ) -> SolverResult<()> {
        let additional_inputs: Vec<ItemValuePair<f64>> = match &mut graph[node_index] {
            PlanGraphNode::ProductionNode(existing_recipe, existing_machine_count) => {
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
            PlanGraphNode::InputNode(resource_value) => {
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

    fn find_input_node(&self, graph: &GraphType<'a>, item: Item) -> Option<NodeIndex> {
        graph
            .node_indices()
            .find(|i| graph[*i].is_input_for_item(item))
    }

    fn find_production_node(&self, graph: &GraphType<'a>, recipe: &'a Recipe) -> Option<NodeIndex> {
        graph
            .node_indices()
            .find(|i| graph[*i].is_production_for_recipe(recipe))
    }
}
