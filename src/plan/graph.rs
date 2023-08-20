use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;

use crate::{game::{Item, ItemValuePair, Recipe}, utils::round_f64};
use std::fmt;

#[derive(Debug, Copy, Clone)]
pub struct Production<'a> {
    pub recipe: &'a Recipe,
    pub machine_count: f64,
}

#[derive(Debug, Copy, Clone)]
pub enum NodeValue<'a> {
    Input(ItemValuePair),
    Output(ItemValuePair),
    ByProduct(ItemValuePair),
    Production(Production<'a>),
}

#[derive(Debug, Clone)]
pub struct ScoredNodeValue<'a> {
    pub node: NodeValue<'a>,
    pub score: f64,
}

pub type GraphType<'a> = StableDiGraph<NodeValue<'a>, ItemValuePair>;
pub type ScoredGraphType<'a> = StableDiGraph<ScoredNodeValue<'a>, ItemValuePair>;

impl<'a> NodeValue<'a> {
    pub fn new_input(input: ItemValuePair) -> Self {
        NodeValue::Input(input)
    }

    pub fn new_output(output: ItemValuePair) -> Self {
        NodeValue::Output(output)
    }

    pub fn new_by_product(output: ItemValuePair) -> Self {
        NodeValue::ByProduct(output)
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        NodeValue::Production(Production { recipe, machine_count })
    }

    pub fn is_input(&self) -> bool {
        matches!(self, NodeValue::Input(..))
    }

    #[allow(dead_code)]
    pub fn is_output(&self) -> bool {
        matches!(self, NodeValue::Output(..))
    }

    #[allow(dead_code)]
    pub fn is_by_product(&self) -> bool {
        matches!(self, NodeValue::ByProduct(..))
    }

    #[allow(dead_code)]
    pub fn is_production(&self) -> bool {
        matches!(self, NodeValue::Production(..))
    }

    pub fn as_input_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Input(input) => input,
            _ => panic!("NodeValue is not an Input"),
        }
    }

    pub fn as_output_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::Output(output) => output,
            _ => panic!("NodeValue is not an Output"),
        }
    }

    #[allow(dead_code)]
    pub fn as_by_product_mut(&mut self) -> &mut ItemValuePair {
        match self {
            NodeValue::ByProduct(output) => output,
            _ => panic!("NodeValue is not an ByProduct"),
        }
    }

    pub fn as_production_mut(&mut self) -> &mut Production<'a> {
        match self {
            NodeValue::Production(production) => production,
            _ => panic!("NodeValue is not an Production"),
        }
    }
}

impl<'a> fmt::Display for NodeValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeValue::Input(item_value) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            },
            NodeValue::Production(production) => {
                write!(
                    f,
                    "{} {}x {}",
                    production.recipe.name,
                    round_f64(production.machine_count, 3),
                    production.recipe.machine
                )
            },
            NodeValue::ByProduct(item_value, ..) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            },
            NodeValue::Output(item_value, ..) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round_f64(item_value.value, 3)
                )
            }
        }
    }
}

impl<'a> ScoredNodeValue<'a> {
    pub fn new_input(input: ItemValuePair) -> Self {
        Self::from(NodeValue::new_input(input))
    }

    pub fn new_output(output: ItemValuePair) -> Self {
        Self::from(NodeValue::new_output(output))
    }

    #[allow(dead_code)]
    pub fn new_by_product(output: ItemValuePair) -> Self {
        Self::from(NodeValue::new_by_product(output))
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        Self::from(NodeValue::new_production(recipe, machine_count))
    }

    #[allow(dead_code)]
    pub fn is_input(&self) -> bool {
        self.node.is_input()
    }

    #[allow(dead_code)]
    pub fn is_output(&self) -> bool {
        self.node.is_output()
    }

    #[allow(dead_code)]
    pub fn is_production(&self) -> bool {
        self.node.is_production()
    }
}

impl<'a> From<ScoredNodeValue<'a>> for NodeValue<'a> {
    fn from(value: ScoredNodeValue<'a>) -> Self {
        value.node
    }
}

impl<'a> fmt::Display for ScoredNodeValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\nScore: {}\n", self.node, round_f64(self.score, 3))
    }
}

impl<'a> From<NodeValue<'a>> for ScoredNodeValue<'a> {
    fn from(node: NodeValue<'a>) -> Self {
        Self { node, score: f64::INFINITY }
    }
}

pub fn find_input_node(graph: &GraphType<'_>, item: Item) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Input(input) => item == input.item,
        _ => false,
    })
}

pub fn find_production_node(graph: &GraphType<'_>, recipe: &Recipe) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Production(production) => production.recipe == recipe,
        _ => false,
    })
}

pub fn find_output_node(graph: &GraphType<'_>, item: Item) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::Output(output) => item == output.item,
        _ => false,
    })
}

pub fn find_by_product_node(graph: &GraphType<'_>, item: Item) -> Option<NodeIndex> {
    graph.node_indices().find(|i| match graph[*i] {
        NodeValue::ByProduct(output) => item == output.item,
        _ => false,
    })
}
