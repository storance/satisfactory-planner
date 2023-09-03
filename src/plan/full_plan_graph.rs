use crate::game::{Item, Recipe};
use anyhow::bail;
use petgraph::{
    stable_graph::{NodeIndex, StableDiGraph},
    Direction::{Incoming, Outgoing},
};
use std::{fmt, rc::Rc};

use super::{PlanConfig, UNSOLVABLE_PLAN_ERROR, NodeWeight, print_graph};

pub type FullPlanGraph = StableDiGraph<PlanNodeWeight, Rc<Item>>;

#[derive(Debug, Clone)]
pub enum PlanNodeWeight {
    Input(Rc<Item>),
    Output(Rc<Item>),
    ByProduct(Rc<Item>, bool),
    Production(Rc<Recipe>),
}

impl PlanNodeWeight {
    #[inline]
    pub fn new_input(item: Rc<Item>) -> Self {
        Self::Input(item)
    }

    #[inline]
    pub fn new_output(item: Rc<Item>) -> Self {
        Self::Output(item)
    }

    #[inline]
    pub fn new_by_product(item: Rc<Item>) -> Self {
        Self::ByProduct(item, true)
    }

    #[inline]
    pub fn new_production(recipe: Rc<Recipe>) -> Self {
        Self::Production(recipe)
    }

    #[inline]
    pub fn is_input_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::Input(i) if i.as_ref() == item)
    }

    #[inline]
    pub fn is_output_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::Output(i) if i.as_ref() == item)
    }

    #[inline]
    pub fn is_by_product_for_item(&self, item: &Item) -> bool {
        matches!(self, Self::ByProduct(i, ..) if i.as_ref() == item)
    }

    #[inline]
    pub fn is_production_for_recipe(&self, recipe: &Recipe) -> bool {
        matches!(self, Self::Production(r) if **r == *recipe)
    }

    pub fn is_partial(&self) -> bool {
        match self {
            Self::ByProduct(_, partial) => *partial,
            _ => panic!("Node is not a ByProduct node"),
        }
    }

    pub fn set_partial(&mut self, new_value: bool) {
        match self {
            Self::ByProduct(_, partial) => *partial = new_value,
            _ => panic!("Node is not a ByProduct node"),
        };
    }
}

impl NodeWeight for PlanNodeWeight {
    #[inline]
    fn is_input(&self) -> bool {
        matches!(self, Self::Input(..))
    }

    #[inline]
    fn is_input_resource(&self) -> bool {
        matches!(self, Self::Input(item) if item.resource)
    }

    #[inline]
    fn is_output(&self) -> bool {
        matches!(self, Self::Output(..))
    }

    #[inline]
    fn is_by_product(&self) -> bool {
        matches!(self, Self::ByProduct(..))
    }

    #[inline]
    fn is_production(&self) -> bool {
        matches!(self, Self::Production(..))
    }
}

impl fmt::Display for PlanNodeWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(item) => {
                write!(f, "{}", item)
            }
            Self::Production(recipe) => {
                write!(f, "{}\n{}", recipe, recipe.building)
            }
            Self::ByProduct(item, ..) => {
                write!(f, "{}", item)
            }
            Self::Output(item) => {
                write!(f, "{}", item)
            }
        }
    }
}

pub fn build_full_plan(config: &PlanConfig) -> Result<FullPlanGraph, anyhow::Error> {
    let mut graph = FullPlanGraph::new();

    config.outputs.iter().for_each(|o| {
        let idx = graph.add_node(PlanNodeWeight::new_output(Rc::clone(&o.item)));
        create_children(config, &mut graph, idx, Rc::clone(&o.item));
    });

    print_graph(&graph);

    for output in &config.outputs {
        let idx = find_output_node(&graph, &output.item).unwrap();
        let mut visited = Vec::new();
        if prune_impossible(config, &mut graph, idx, &mut visited) {
            bail!("{}", UNSOLVABLE_PLAN_ERROR);
        }
    }

    Ok(graph)
}

fn create_children(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    item: Rc<Item>,
) {
    if item.resource {
        create_input_node(graph, parent_idx, item)
    } else {
        create_production_by_product(config, graph, parent_idx, item)
    }
}

fn create_input_node(graph: &mut FullPlanGraph, parent_idx: NodeIndex, item: Rc<Item>) {
    let idx = find_input_node(graph, &item)
        .unwrap_or_else(|| graph.add_node(PlanNodeWeight::new_input(Rc::clone(&item))));
    graph.add_edge(idx, parent_idx, item);
}

pub fn create_production_by_product(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    item: Rc<Item>,
) {
    let idx = match find_by_product_node(graph, &item) {
        Some(idx) => {
            if !graph[idx].is_partial() {
                graph.add_edge(idx, parent_idx, item);
                return;
            }

            idx
        }
        None => graph.add_node(PlanNodeWeight::new_by_product(Rc::clone(&item))),
    };

    for recipe in config.game_db.find_recipes_by_output(&item) {
        create_production_node(config, graph, idx, recipe, Rc::clone(&item));
    }

    if config.has_input(&item) {
        create_input_node(graph, idx, Rc::clone(&item));
    }

    graph.add_edge(idx, parent_idx, item);
}

fn create_production_node(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    recipe: Rc<Recipe>,
    item: Rc<Item>,
) {
    if find_production_node(graph, &recipe).is_none() {
        let idx = graph.add_node(PlanNodeWeight::new_production(Rc::clone(&recipe)));

        let by_product_nodes: Vec<NodeIndex> = recipe
            .outputs
            .iter()
            .filter(|o| o.item != item)
            .map(|o| create_partial_by_product_node(graph, idx, Rc::clone(&o.item)))
            .collect();

        for input in &recipe.inputs {
            create_children(config, graph, idx, Rc::clone(&input.item));
        }

        by_product_nodes
            .iter()
            .for_each(|i| graph[*i].set_partial(false));
        graph.add_edge(idx, parent_idx, item);
    }
}

fn create_partial_by_product_node(
    graph: &mut FullPlanGraph,
    child_idx: NodeIndex,
    item: Rc<Item>,
) -> NodeIndex {
    let idx = graph.add_node(PlanNodeWeight::new_by_product(Rc::clone(&item)));

    graph.add_edge(child_idx, idx, item);

    idx
}

fn prune_impossible(config: &PlanConfig, graph: &mut FullPlanGraph, idx: NodeIndex, visited: &mut Vec<NodeIndex>) -> bool {
    if visited.contains(&idx) {
        return false;
    }
    visited.push(idx);

    match &graph[idx] {
        PlanNodeWeight::ByProduct(..) => {
            let mut child_walker = graph.neighbors_directed(idx, Incoming).detach();
            let mut all_deleted = true;
            while let Some(child_idx) = child_walker.next_node(graph) {
                all_deleted &= prune_impossible(config, graph, child_idx, visited);
            }

            if all_deleted {
                graph.remove_node(idx);
            }
            all_deleted
        }
        PlanNodeWeight::Production(recipe, ..) => {
            let total_inputs = recipe.inputs.len();
            let mut child_walker = graph.neighbors_directed(idx, Incoming).detach();
            let mut total_children = 0;
            while let Some(child_idx) = child_walker.next_node(graph) {
                if !prune_impossible(config, graph, child_idx, visited) {
                    total_children += 1;
                }
            }

            if total_children != total_inputs {
                prune(graph, idx);
                true
            } else {
                false
            }
        }
        PlanNodeWeight::Input(item) => {
            if config.find_input(&item) == 0.0 {
                graph.remove_node(idx);
                true
            } else {
                false
            }
        }
        PlanNodeWeight::Output(..) => {
            if let Some(child_idx) = graph.neighbors_directed(idx, Incoming).next() {
                if prune_impossible(config, graph, child_idx, visited) {
                    graph.remove_node(idx);
                    true
                } else {
                    false
                }
            } else {
                graph.remove_node(idx);
                true
            }
        }
    }
}

fn prune(graph: &mut FullPlanGraph, idx: NodeIndex) {
    match &graph[idx] {
        PlanNodeWeight::Production(..) => {
            let mut parent_walker = graph.neighbors_directed(idx, Outgoing).detach();
            while let Some(parent_idx) = parent_walker.next_node(graph) {
                // if our parent only has a single child, then that is us and it should be deleted
                if graph.neighbors_undirected(parent_idx).count() == 1 {
                    graph.remove_node(parent_idx);
                }
            }
        }
        _ => {}
    };

    let mut child_walker = graph.neighbors_directed(idx, Incoming).detach();
    while let Some(child_idx) = child_walker.next_node(graph) {
        prune(graph, child_idx);
    }

    graph.remove_node(idx);
}

#[inline]
fn find_output_node(graph: &FullPlanGraph, item: &Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_output_for_item(item))
}

#[inline]
fn find_input_node(graph: &FullPlanGraph, item: &Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_input_for_item(item))
}

#[inline]
fn find_production_node(graph: &FullPlanGraph, recipe: &Recipe) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_production_for_recipe(recipe))
}

#[inline]
fn find_by_product_node(graph: &FullPlanGraph, item: &Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_by_product_for_item(item))
}