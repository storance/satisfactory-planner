use crate::plan::{PlanConfig, PlanGraphNode};
use crate::game::{Resource, ResourceDefinition, Recipe, ResourceValuePair};
use std::collections::HashMap;
use thiserror::Error;
use petgraph::{Directed, Incoming};
use petgraph::graph::{Graph, NodeIndex};

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Missing item `{0}` which is required to produce `{1}`")]
    MissingInput(Resource, Resource),
    #[error("No recipe found that produces item `{0}`")]
    NoMatchingRecipes(Resource)
}

pub type GraphType<'a> = Graph<PlanGraphNode<'a>, ResourceValuePair<f64>, Directed>;
pub type SolverResult<T> = Result<T, SolverError>;

pub fn solve<'a>(config: &PlanConfig<'a>) -> SolverResult<GraphType<'a>> {
    Solver::new(config).solve()
}

struct Solver<'a, 'b> {
    config: &'b PlanConfig<'a>,
    recipes_by_output: HashMap<Resource, Vec<&'a Recipe>>,
    recipes_by_input: HashMap<Resource, Vec<&'a Recipe>>,
}

impl <'a, 'b> Solver<'a, 'b> {
    pub fn new(config: &'b PlanConfig<'a>) -> Self {
        let mut recipes_by_output: HashMap<Resource, Vec<&Recipe>> = HashMap::new();
        let mut recipes_by_input: HashMap<Resource, Vec<&Recipe>> = HashMap::new();
        for recipe in &config.recipes {
            for output in &recipe.outputs {
                recipes_by_output.entry(output.resource)
                    .and_modify(|recipes| recipes.push(*recipe))
                    .or_insert_with(|| vec![*recipe]);
            }

            for input in &recipe.inputs {
                recipes_by_input.entry(input.resource)
                    .and_modify(|recipes| recipes.push(*recipe))
                    .or_insert_with(|| vec![*recipe]);
            }
        }

        Self {
            config,
            recipes_by_output,
            recipes_by_input
        }
    }

    pub fn solve(&mut self) -> SolverResult<GraphType<'a>> {
        let mut graph = Graph::new();
        let mut output_nodes: Vec<NodeIndex>  = Vec::new();

        self.config.outputs.iter().for_each(|output| {
            let output_node = PlanGraphNode::new_output(*output, false);
            output_nodes.push(graph.add_node(output_node));
        });

        for node in &output_nodes {
            self.solve_node(*node, &mut graph)?;
        }

        Ok(graph)
    }

    fn solve_node(&self, node_index: NodeIndex, graph: &mut GraphType<'a>) -> SolverResult<()> {
        let inputs_to_solve = match graph[node_index] {
            PlanGraphNode::OutputNode (resource_value, ..) => vec![resource_value],
            PlanGraphNode::ProductionNode (recipe, machine_count) =>
                recipe.inputs.iter().
                    map(|input| ResourceValuePair::new(input.resource, input.amount_per_minute * machine_count))
                    .collect()
            ,
            _ => Vec::new()
        };

        for input in inputs_to_solve {
            self.solve_for_item(input, node_index, graph)?;
        }

        Ok(())
    }

    fn solve_for_item(&self, resource_value: ResourceValuePair<f64>, parent_index: NodeIndex, graph: &mut GraphType<'a>) -> SolverResult<()> {
        if resource_value.resource.is_raw() {
            if let Some(existing_node_index) = self.find_input_node(graph, resource_value.resource) {
                match &mut graph[existing_node_index] {
                    PlanGraphNode::InputNode (existing_resource_value) => {
                        existing_resource_value.value += resource_value.value;
                    },
                    _ => {
                        panic!("Unexpected node");
                    }
                };
                graph.add_edge(existing_node_index, parent_index, resource_value);
            } else {
                let child_node = PlanGraphNode::new_input(resource_value);
                let child_index = graph.add_node(child_node);
                graph.add_edge(child_index, parent_index, resource_value);
            }
        } else {
            let recipes = self.recipes_by_output.get(&resource_value.resource)
                .ok_or(SolverError::NoMatchingRecipes(resource_value.resource))?;

            let recipe = recipes.get(0).unwrap();

            if let Some(existing_node_index) = self.find_production_node(graph, recipe) {
                graph.add_edge(existing_node_index, parent_index, resource_value);
                self.propagate_production_changes(existing_node_index, resource_value, graph)?;
            } else {
                let output = recipe.find_output_by_item(resource_value.resource).unwrap();
                let machine_count = resource_value.value / output.amount_per_minute;

                let child_node = PlanGraphNode::new_production(*recipe, machine_count);
                let child_index = graph.add_node(child_node);
                graph.add_edge(child_index, parent_index, resource_value);

                self.solve_node(child_index, graph)?;
            }
        }

        Ok(())
    }

    fn propagate_production_changes(&self, node_index: NodeIndex, additional_output: ResourceValuePair<f64>, graph: &mut GraphType<'a>) -> SolverResult<()> {
        let additional_inputs: Vec<ResourceValuePair<f64>> = match &mut graph[node_index] {
            PlanGraphNode::ProductionNode (existing_recipe, existing_machine_count) => {
                let output = existing_recipe.find_output_by_item(additional_output.resource).unwrap();
                let machine_count = additional_output.value / output.amount_per_minute;

                *existing_machine_count += machine_count;

                existing_recipe.inputs.iter().map(|input| {
                    ResourceValuePair::new(input.resource, input.amount_per_minute * machine_count)
                }).collect()
            },
            PlanGraphNode::InputNode( resource_value) => {
                resource_value.value += additional_output.value;
                Vec::new()
            },
            _ => {
                panic!("Unexpected node");
            }
        };

        let mut walker = graph.neighbors_directed(node_index, Incoming).detach();
        while let Some((edge_index, source_node_index)) = walker.next(graph) {
            let resource = graph[edge_index].resource;
            let input = additional_inputs.iter().find(|input| input.resource == resource).unwrap();

            graph[edge_index].value += input.value;
            self.propagate_production_changes(source_node_index, *input, graph)?;
        }

        Ok(())
    }

    fn find_input_node(&self, graph: &GraphType<'a>, resource: Resource) -> Option<NodeIndex> {
        graph.node_indices().find(|i| graph[*i].is_input_for_item(resource))
    }

    fn find_production_node(&self, graph: &GraphType<'a>, recipe: &'a Recipe) -> Option<NodeIndex> {
        graph.node_indices().find(|i| graph[*i].is_production_for_recipe(recipe))
    }
}