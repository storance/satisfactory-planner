use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::{round, FloatType},
};
use petgraph::stable_graph::{NodeIndex, StableDiGraph};
use petgraph::{dot::Dot, Direction};
use std::{fmt, rc::Rc};

pub type GraphType = StableDiGraph<NodeValue, NodeEdge>;

#[derive(Debug, Clone, PartialEq)]
pub struct Production {
    pub recipe: Rc<Recipe>,
    pub machine_count: FloatType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    Input(ItemValuePair),
    Output(ItemValuePair),
    ByProduct(ItemValuePair),
    Production(Production),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeEdge {
    pub value: ItemValuePair,
    pub order: u32,
}

pub trait Node {
    fn is_input(&self) -> bool;
    fn is_input_resource(&self) -> bool;
    fn is_output(&self) -> bool;
    fn is_by_product(&self) -> bool;
    fn is_production(&self) -> bool;

    fn is_input_for_item(&self, item: &Item) -> bool;
    fn is_output_for_item(&self, item: &Item) -> bool;
    fn is_by_product_for_item(&self, item: &Item) -> bool;
    fn is_production_for_recipe(&self, recipe: &Recipe) -> bool;
}

#[allow(dead_code)]
impl NodeValue {
    #[inline]
    pub fn new_input(input: ItemValuePair) -> Self {
        NodeValue::Input(input)
    }

    #[inline]
    pub fn new_output(output: ItemValuePair) -> Self {
        NodeValue::Output(output)
    }

    #[inline]
    pub fn new_by_product(output: ItemValuePair) -> Self {
        NodeValue::ByProduct(output)
    }

    #[inline]
    pub fn new_production(recipe: Rc<Recipe>, machine_count: FloatType) -> Self {
        NodeValue::Production(Production {
            recipe,
            machine_count,
        })
    }

    #[inline]
    pub fn as_input(&self) -> &ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    #[inline]
    pub fn as_input_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    #[inline]
    pub fn as_output(&self) -> &ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    #[inline]
    pub fn as_output_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    #[inline]
    pub fn as_by_product(&self) -> &ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    #[inline]
    pub fn as_by_product_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    #[inline]
    pub fn as_production(&self) -> &Production {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
    }

    #[inline]
    pub fn as_production_mut(&mut self) -> &mut Production {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
    }
}

impl Node for NodeValue {
    #[inline]
    fn is_input(&self) -> bool {
        matches!(self, NodeValue::Input(..))
    }

    #[inline]
    fn is_input_resource(&self) -> bool {
        matches!(self, NodeValue::Input(i) if i.item.resource)
    }

    #[inline]
    fn is_output(&self) -> bool {
        matches!(self, NodeValue::Output(..))
    }

    #[inline]
    fn is_by_product(&self) -> bool {
        matches!(self, NodeValue::ByProduct(..))
    }

    #[inline]
    fn is_production(&self) -> bool {
        matches!(self, NodeValue::Production(..))
    }

    #[inline]
    fn is_input_for_item(&self, item: &Item) -> bool {
        matches!(self, NodeValue::Input(i) if *i.item == *item)
    }

    #[inline]
    fn is_output_for_item(&self, item: &Item) -> bool {
        matches!(self, NodeValue::Output(i) if *i.item == *item)
    }

    #[inline]
    fn is_by_product_for_item(&self, item: &Item) -> bool {
        matches!(self, NodeValue::ByProduct(i) if *i.item == *item)
    }

    #[inline]
    fn is_production_for_recipe(&self, recipe: &Recipe) -> bool {
        matches!(self, NodeValue::Production(p) if *p.recipe == *recipe)
    }
}

impl fmt::Display for NodeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeValue::Input(item_value) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
            NodeValue::Production(production) => {
                write!(
                    f,
                    "{}\n{}x {}",
                    production.recipe,
                    round(production.machine_count, 3),
                    production.recipe.building
                )
            }
            NodeValue::ByProduct(item_value, ..) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
            NodeValue::Output(item_value, ..) => {
                write!(
                    f,
                    "{}\n{} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
        }
    }
}

impl NodeEdge {
    #[inline]
    pub fn new(value: ItemValuePair, order: u32) -> Self {
        Self { value, order }
    }

    #[inline]
    pub fn item(&self) -> &Item {
        &self.value.item
    }

    #[inline]
    pub fn value(&self) -> FloatType {
        self.value.value
    }
}

impl fmt::Display for NodeEdge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n{} / min",
            self.value.item,
            round(self.value.value, 3)
        )
    }
}

#[inline]
pub fn find_input_node<N: Node, E>(graph: &StableDiGraph<N, E>, item: &Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_input_for_item(item))
}

#[inline]
pub fn find_production_node<N: Node, E>(
    graph: &StableDiGraph<N, E>,
    recipe: &Recipe,
) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_production_for_recipe(recipe))
}

#[inline]
pub fn find_output_node<N: Node, E>(graph: &StableDiGraph<N, E>, item: &Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_output_for_item(item))
}

#[inline]
pub fn find_by_product_node<N: Node, E>(
    graph: &StableDiGraph<N, E>,
    item: &Item,
) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| graph[*i].is_by_product_for_item(item))
}

/// Determines if the target is reachable from the source node by traveling in the given direction.
#[allow(dead_code)]
pub fn is_reachable<N, E>(
    graph: &StableDiGraph<N, E>,
    source: NodeIndex,
    target: NodeIndex,
    dir: Direction,
) -> bool {
    let mut visited = vec![];
    is_reachable_internal(graph, source, target, dir, &mut visited)
}

#[allow(dead_code)]
fn is_reachable_internal<N, E>(
    graph: &StableDiGraph<N, E>,
    source: NodeIndex,
    target: NodeIndex,
    dir: Direction,
    visited: &mut Vec<NodeIndex>,
) -> bool {
    if source == target {
        return true;
    } else if visited.contains(&source) {
        return false;
    }
    visited.push(source);

    for neighbor in graph.neighbors_directed(source, dir) {
        if is_reachable_internal(graph, neighbor, target, dir, visited) {
            return true;
        }
    }

    false
}

pub fn print_graph<N: Node + fmt::Display, E: fmt::Display>(graph: &StableDiGraph<N, E>) {
    println!(
        "{}",
        format!(
            "{}",
            Dot::with_attr_getters(&graph, &[], &|_, _| String::new(), &|_, n| {
                let color = if n.1.is_input_resource() {
                    "lightslategray"
                } else if n.1.is_input() {
                    "peru"
                } else if n.1.is_output() {
                    "mediumseagreen"
                } else if n.1.is_by_product() {
                    "cornflowerblue"
                } else if n.1.is_production() {
                    "darkorange"
                } else {
                    "white"
                };

                format!(
                    "style=\"solid,filled\" shape=\"box\" fontcolor=\"white\" color=\"{}\"",
                    color
                )
            })
        )
        .replace("\\l", "\\n")
    );
}
