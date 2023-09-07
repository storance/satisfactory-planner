use super::{PlanConfig, PlanError};
use crate::game::{BuildingId, ItemId, RecipeId};
use petgraph::{
    stable_graph::{NodeIndex, StableDiGraph},
    Direction::{Incoming, Outgoing},
};

pub type FullPlanGraph = StableDiGraph<PlanNodeWeight, ItemId>;

#[derive(Debug, Copy, Clone)]
pub enum PlanNodeWeight {
    Input(ItemId),
    Output(ItemId),
    ByProduct(ItemId),
    Production(RecipeId),
    Producer(BuildingId),
}

impl PlanNodeWeight {
    #[inline]
    pub fn new_input(item: ItemId) -> Self {
        Self::Input(item)
    }

    #[inline]
    pub fn new_output(item: ItemId) -> Self {
        Self::Output(item)
    }

    #[inline]
    pub fn new_by_product(item: ItemId) -> Self {
        Self::ByProduct(item)
    }

    #[inline]
    pub fn new_production(recipe: RecipeId) -> Self {
        Self::Production(recipe)
    }

    #[inline]
    pub fn new_producer(building: BuildingId) -> Self {
        Self::Producer(building)
    }

    #[inline]
    pub fn is_input_for_item(&self, item: ItemId) -> bool {
        matches!(self, Self::Input(i) if *i == item)
    }

    #[inline]
    pub fn is_output_for_item(&self, item: ItemId) -> bool {
        matches!(self, Self::Output(i) if *i == item)
    }

    #[inline]
    pub fn is_by_product_for_item(&self, item: ItemId) -> bool {
        matches!(self, Self::ByProduct(i) if *i == item)
    }

    #[inline]
    pub fn is_producer_for_building(&self, building: BuildingId) -> bool {
        matches!(self, Self::Producer(b) if *b == building)
    }

    #[inline]
    pub fn is_production_for_recipe(&self, recipe: RecipeId) -> bool {
        matches!(self, Self::Production(r, ..) if *r == recipe)
    }
}

pub fn build_full_plan(config: &PlanConfig) -> Result<FullPlanGraph, PlanError> {
    let mut graph = FullPlanGraph::new();

    config.outputs.iter().for_each(|(item, _)| {
        let idx = graph.add_node(PlanNodeWeight::new_output(*item));
        create_children(config, &mut graph, idx, *item);
    });

    for item in config.outputs.keys() {
        let idx = find_output_node(&graph, *item).unwrap();
        let mut visited = Vec::new();
        if prune_impossible(config, &mut graph, idx, &mut visited) {
            return Err(PlanError::UnsolvablePlan);
        }
    }

    Ok(graph)
}

fn create_children(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    item_id: ItemId,
) {
    let item = &config.game_db[item_id];
    if item.resource {
        create_input_node(graph, parent_idx, item_id)
    } else {
        create_production_by_product(config, graph, parent_idx, item_id)
    }
}

fn create_input_node(graph: &mut FullPlanGraph, parent_idx: NodeIndex, item: ItemId) {
    let idx = find_input_node(graph, item)
        .unwrap_or_else(|| graph.add_node(PlanNodeWeight::new_input(item)));
    graph.add_edge(idx, parent_idx, item);
}

pub fn create_production_by_product(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    item: ItemId,
) {
    let idx = match find_by_product_node(graph, item) {
        Some(idx) => idx,
        None => graph.add_node(PlanNodeWeight::new_by_product(item)),
    };

    for recipe in config.find_recipes_by_output(item) {
        create_production_node(config, graph, idx, recipe, item);
    }

    for building in config.game_db.find_item_producers(item) {
        create_producer_node(config, graph, parent_idx, building, item);
    }

    if config.has_input(item) {
        create_input_node(graph, idx, item);
    }

    graph.update_edge(idx, parent_idx, item);
}

fn create_producer_node(
    _config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    building: BuildingId,
    item: ItemId,
) -> u32 {
    let idx = find_producer_node(graph, building)
        .unwrap_or_else(|| graph.add_node(PlanNodeWeight::new_producer(building)));
    graph.add_edge(idx, parent_idx, item);
    1
}

fn create_production_node(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    parent_idx: NodeIndex,
    recipe_id: RecipeId,
    item_id: ItemId,
) {
    if find_production_node(graph, recipe_id).is_none() {
        let idx = graph.add_node(PlanNodeWeight::new_production(recipe_id));

        let recipe = &config.game_db[recipe_id];
        for output in &recipe.outputs {
            if output.item != item_id {
                create_partial_by_product_node(graph, idx, output.item);
            }
        }

        for input in &recipe.inputs {
            create_children(config, graph, idx, input.item);
        }
        graph.add_edge(idx, parent_idx, item_id);
    }
}

fn create_partial_by_product_node(
    graph: &mut FullPlanGraph,
    child_idx: NodeIndex,
    item: ItemId,
) -> NodeIndex {
    let idx = match find_by_product_node(graph, item) {
        Some(idx) => idx,
        None => graph.add_node(PlanNodeWeight::new_by_product(item)),
    };
    graph.update_edge(child_idx, idx, item);
    idx
}

fn prune_impossible(
    config: &PlanConfig,
    graph: &mut FullPlanGraph,
    idx: NodeIndex,
    visited: &mut Vec<NodeIndex>,
) -> bool {
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
        PlanNodeWeight::Production(recipe_id, ..) => {
            let recipe = &config.game_db[*recipe_id];
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
            if config.find_input(*item) == 0.0 {
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
        PlanNodeWeight::Producer(..) => false,
    }
}

fn prune(graph: &mut FullPlanGraph, idx: NodeIndex) {
    if let PlanNodeWeight::Production(..) = graph[idx] {
        let mut parent_walker = graph.neighbors_directed(idx, Outgoing).detach();
        while let Some(parent_idx) = parent_walker.next_node(graph) {
            // if our parent only has a single child, then that is us and it should be deleted
            if graph.neighbors_undirected(parent_idx).count() == 1 {
                graph.remove_node(parent_idx);
            }
        }
    }

    let mut child_walker = graph.neighbors_directed(idx, Incoming).detach();
    while let Some(child_idx) = child_walker.next_node(graph) {
        prune(graph, child_idx);
    }

    graph.remove_node(idx);
}

#[inline]
fn find_output_node(graph: &FullPlanGraph, item: ItemId) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_output_for_item(item))
}

#[inline]
fn find_input_node(graph: &FullPlanGraph, item: ItemId) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_input_for_item(item))
}

#[inline]
fn find_production_node(graph: &FullPlanGraph, recipe: RecipeId) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_production_for_recipe(recipe))
}

#[inline]
fn find_producer_node(graph: &FullPlanGraph, building: BuildingId) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_producer_for_building(building))
}

#[inline]
fn find_by_product_node(graph: &FullPlanGraph, item: ItemId) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_by_product_for_item(item))
}
