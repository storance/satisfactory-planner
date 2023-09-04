use super::{
    full_plan_graph::{FullPlanGraph, PlanNodeWeight},
    NodeWeight,
};
use crate::{
    game::{Building, Item, ItemPerMinute, Recipe},
    utils::{clamp_to_zero, is_zero, round, FloatType},
};
use good_lp::{Solution, Variable};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex, StableDiGraph},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::{collections::HashMap, fmt, rc::Rc};

pub type SolvedGraph = StableDiGraph<SolvedNodeWeight, ItemPerMinute>;

#[derive(Debug, Clone)]
pub enum SolvedNodeWeight {
    Input(ItemPerMinute),
    Output(ItemPerMinute),
    ByProduct(ItemPerMinute),
    Production(Rc<Recipe>, FloatType),
    Producer(Rc<Building>, FloatType),
}

impl SolvedNodeWeight {
    #[inline]
    pub fn new_input(item: Rc<Item>, amount: FloatType) -> Self {
        Self::Input(ItemPerMinute::new(item, amount))
    }

    #[inline]
    pub fn new_output(item: Rc<Item>, amount: FloatType) -> Self {
        Self::Output(ItemPerMinute::new(item, amount))
    }

    #[inline]
    pub fn new_by_product(item: Rc<Item>, amount: FloatType) -> Self {
        Self::ByProduct(ItemPerMinute::new(item, amount))
    }

    #[inline]
    pub fn new_production(recipe: Rc<Recipe>, building_count: FloatType) -> Self {
        Self::Production(recipe, building_count)
    }

    #[inline]
    pub fn new_producer(recipe: Rc<Building>, building_count: FloatType) -> Self {
        Self::Producer(recipe, building_count)
    }
}

impl NodeWeight for SolvedNodeWeight {
    #[inline]
    fn is_input(&self) -> bool {
        matches!(self, Self::Input(..))
    }

    #[inline]
    fn is_input_resource(&self) -> bool {
        matches!(self, Self::Input(input) if input.item.resource)
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

    #[inline]
    fn is_producer(&self) -> bool {
        matches!(self, Self::Producer(..))
    }
}

impl fmt::Display for SolvedNodeWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(input) => {
                write!(f, "{}\n{} / min", input.item, round(input.amount, 3))
            }
            Self::Production(recipe, building_count) => {
                write!(
                    f,
                    "{}\n{}x {}",
                    recipe,
                    round(*building_count, 3),
                    recipe.building
                )
            }
            Self::ByProduct(by_product, ..) => {
                write!(
                    f,
                    "{}\n{} / min",
                    by_product.item,
                    round(by_product.amount, 3)
                )
            }
            Self::Output(output, ..) => {
                write!(f, "{}\n{} / min", output.item, round(output.amount, 3))
            }
            Self::Producer(building, building_count) => {
                write!(f, "{}x {}", round(*building_count, 3), building)
            }
        }
    }
}

pub fn copy_solution<S: Solution>(
    full_graph: &FullPlanGraph,
    solution: S,
    node_variables: HashMap<NodeIndex, Variable>,
    edge_variables: HashMap<EdgeIndex, Variable>,
) -> SolvedGraph {
    let mut node_mapping: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    let mut solved_graph = SolvedGraph::new();

    for i in full_graph.node_indices() {
        let var = *node_variables.get(&i).unwrap();
        let solution = solution.value(var);

        if is_zero(solution) {
            continue;
        }

        let new_idx = match &full_graph[i] {
            PlanNodeWeight::Input(item) => {
                solved_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(item), solution))
            }
            PlanNodeWeight::Output(item) => {
                solved_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(item), solution))
            }
            PlanNodeWeight::ByProduct(item) => {
                solved_graph.add_node(SolvedNodeWeight::new_by_product(Rc::clone(item), solution))
            }
            PlanNodeWeight::Production(recipe, _) => solved_graph.add_node(
                SolvedNodeWeight::new_production(Rc::clone(recipe), solution),
            ),
            PlanNodeWeight::Producer(building) => solved_graph.add_node(
                SolvedNodeWeight::new_producer(Rc::clone(building), solution),
            ),
        };

        node_mapping.insert(i, new_idx);
    }

    for e in full_graph.edge_indices() {
        let var = *edge_variables.get(&e).unwrap();
        let solution = solution.value(var);

        if is_zero(solution) {
            continue;
        }

        let (source, target) = full_graph.edge_endpoints(e).unwrap();
        let new_source = *node_mapping.get(&source).unwrap();
        let new_target = *node_mapping.get(&target).unwrap();

        let weight = ItemPerMinute::new(Rc::clone(&full_graph[e]), solution);
        solved_graph.add_edge(new_source, new_target, weight);
    }

    cleanup_by_product_nodes(&mut solved_graph);
    solved_graph
}

fn cleanup_by_product_nodes(graph: &mut SolvedGraph) {
    let by_product_nodes: Vec<NodeIndex> = graph
        .node_indices()
        .filter(|i| graph[*i].is_by_product())
        .collect();

    by_product_nodes
        .iter()
        .for_each(|i| cleanup_by_product(graph, *i));
}

fn cleanup_by_product(graph: &mut SolvedGraph, node_idx: NodeIndex) {
    let mut parents: Vec<(NodeIndex, ItemPerMinute)> = graph
        .edges_directed(node_idx, Outgoing)
        .map(|e| (e.target(), e.weight().clone()))
        .collect();
    let mut children: Vec<(NodeIndex, ItemPerMinute)> = graph
        .edges_directed(node_idx, Incoming)
        .map(|e| (e.source(), e.weight().clone()))
        .collect();

    parents.sort_unstable_by(|a, b| a.1.cmp(&b.1));
    children.sort_unstable_by(|a, b| a.1.cmp(&b.1).reverse());

    let mut current_child = children.pop().unwrap();
    for parent in parents {
        let mut remaining_output = parent.1;
        loop {
            if remaining_output.is_zero() {
                break;
            }

            if current_child.1.is_zero() {
                delete_edge_between(graph, current_child.0, node_idx);
                current_child = children.pop().unwrap();
            }

            if remaining_output > current_child.1 {
                graph.add_edge(current_child.0, parent.0, current_child.1.clone());
                remaining_output -= current_child.1.amount;
                current_child.1.amount = 0.0;
            } else {
                graph.add_edge(current_child.0, parent.0, remaining_output.clone());
                current_child.1 -= remaining_output.amount;
                remaining_output.amount = 0.0;
                break;
            }
        }
        delete_edge_between(graph, node_idx, parent.0);
    }

    let remaining_output = clamp_to_zero(
        current_child.1.amount + children.iter().map(|c| c.1.amount).sum::<FloatType>(),
    );
    if remaining_output > 0.0 {
        match &mut graph[node_idx] {
            SolvedNodeWeight::ByProduct(by_product) => by_product.amount = remaining_output,
            _ => panic!("Node is not a ByProduct"),
        };

        if !current_child.1.is_zero() {
            let edge_index = graph.find_edge(current_child.0, node_idx).unwrap();
            graph[edge_index] = current_child.1
        }
    } else {
        graph.remove_node(node_idx);
    }
}

fn delete_edge_between(graph: &mut SolvedGraph, a: NodeIndex, b: NodeIndex) -> bool {
    graph
        .find_edge(a, b)
        .map(|e| {
            graph.remove_edge(e);
            true
        })
        .unwrap_or(false)
}
