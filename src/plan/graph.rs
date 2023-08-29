use crate::{
    game::{Item, ItemValuePair, Recipe},
    utils::{round, FloatType},
};
use petgraph::{dot::Dot, graph::NodeIndex};
use petgraph::{
    stable_graph::{EdgeIndex, StableDiGraph},
    visit::EdgeRef,
    Direction::Incoming,
};
use std::{fmt, rc::Rc};

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

pub type GraphType = StableDiGraph<NodeValue, NodeEdge>;

#[allow(dead_code)]
impl NodeValue {
    pub fn new_input(input: ItemValuePair) -> Self {
        NodeValue::Input(input)
    }

    pub fn new_output(output: ItemValuePair) -> Self {
        NodeValue::Output(output)
    }

    pub fn new_by_product(output: ItemValuePair) -> Self {
        NodeValue::ByProduct(output)
    }

    pub fn new_production(recipe: Rc<Recipe>, machine_count: FloatType) -> Self {
        NodeValue::Production(Production {
            recipe,
            machine_count,
        })
    }

    pub fn is_input(&self) -> bool {
        matches!(self, NodeValue::Input(..))
    }

    pub fn is_output(&self) -> bool {
        matches!(self, NodeValue::Output(..))
    }

    pub fn is_by_product(&self) -> bool {
        matches!(self, NodeValue::ByProduct(..))
    }

    pub fn is_production(&self) -> bool {
        matches!(self, NodeValue::Production(..))
    }

    pub fn as_input(&self) -> &ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    pub fn as_input_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not Input"),
        }
    }

    pub fn as_output(&self) -> &ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    pub fn as_output_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not Output"),
        }
    }

    pub fn as_by_product(&self) -> &ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    pub fn as_by_product_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not ByProduct"),
        }
    }

    pub fn as_production(&self) -> &Production {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
    }

    pub fn as_production_mut(&mut self) -> &mut Production {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not Production"),
        }
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

#[derive(Debug, Clone, PartialEq)]
pub struct NodeEdge {
    pub value: ItemValuePair,
    pub order: u32,
}

impl NodeEdge {
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

pub fn find_input_node<E>(graph: &StableDiGraph<NodeValue, E>, item: &Item) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match &graph[*i] {
        NodeValue::Input(input) => *item == *input.item,
        _ => false,
    })
}

pub fn find_production_node<E>(
    graph: &StableDiGraph<NodeValue, E>,
    recipe: &Recipe,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match &graph[*i] {
        NodeValue::Production(production) => *production.recipe == *recipe,
        _ => false,
    })
}

pub fn find_output_node<E>(graph: &StableDiGraph<NodeValue, E>, item: &Item) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match &graph[*i] {
        NodeValue::Output(output) => *item == *output.item,
        _ => false,
    })
}

#[allow(dead_code)]
pub fn find_by_product_node<E>(
    graph: &StableDiGraph<NodeValue, E>,
    item: &Item,
) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match &graph[*i] {
        NodeValue::ByProduct(output) => *item == *output.item,
        _ => false,
    })
}

pub fn find_by_product_child<E>(
    node_index: NodeIndex,
    graph: &StableDiGraph<NodeValue, E>,
) -> (EdgeIndex, NodeIndex) {
    assert!(graph[node_index].is_by_product());

    let edge = graph
        .edges_directed(node_index, Incoming)
        .next()
        .unwrap_or_else(|| {
            panic!(
                "ByProduct node {:?} is missing it's Production node child",
                graph[node_index]
            )
        });
    (edge.id(), edge.source())
}

pub fn print_graph<E: fmt::Display>(graph: &StableDiGraph<NodeValue, E>) {
    println!(
        "{}",
        format!(
            "{}",
            Dot::with_attr_getters(&graph, &[], &|_, _| String::new(), &|_, n| {
                let color = match n.1 {
                    NodeValue::Input(input) => {
                        if input.item.resource {
                            "lightslategray"
                        } else {
                            "peru"
                        }
                    }
                    NodeValue::Output(..) => "mediumseagreen",
                    NodeValue::ByProduct(..) => "cornflowerblue",
                    NodeValue::Production(..) => "darkorange",
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
