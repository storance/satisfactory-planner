use petgraph::{Graph, Directed};
use petgraph::graph::NodeIndex;

use crate::game::{Item, ItemValuePair, Recipe};
use std::fmt;

#[derive(Debug, Copy, Clone)]
pub enum NodeValue<'a> {
    Input(ItemValuePair<f64>),
    Output(ItemValuePair<f64>, bool),
    Production(&'a Recipe, f64),
}

#[derive(Debug, Copy, Clone)]
pub struct ScoredNodeValue<'a> {
    pub node: NodeValue<'a>,
    pub score: Option<f64>,
}

pub type GraphType<'a> = Graph<NodeValue<'a>, ItemValuePair<f64>, Directed>;
pub type ScoredGraphType<'a> = Graph<ScoredNodeValue<'a>, ItemValuePair<f64>, Directed>;

impl<'a> NodeValue<'a> {
    pub fn new_input(input: ItemValuePair<f64>) -> Self {
        NodeValue::Input(input)
    }

    pub fn new_output(output: ItemValuePair<f64>) -> Self {
        NodeValue::Output(output, false)
    }

    pub fn new_by_product(output: ItemValuePair<f64>) -> Self {
        NodeValue::Output(output, true)
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        NodeValue::Production(recipe, machine_count)
    }

    pub fn is_input(&self) -> bool {
        matches!(self, NodeValue::Input { .. })
    }

    pub fn is_output(&self) -> bool {
        matches!(self, NodeValue::Output { .. })
    }

    pub fn is_production(&self) -> bool {
        matches!(self, NodeValue::Production { .. })
    }

    pub fn as_input_mut(&mut self) -> &mut ItemValuePair<f64> {
        match self {
            NodeValue::Input( input) => input,
            _ => panic!("NodeValue is not an InputNode")
        }
    }

    pub fn as_output_mut(&mut self) -> (&mut ItemValuePair<f64>, &mut bool) {
        match self {
            NodeValue::Output( output, by_product) => (output, by_product),
            _ => panic!("NodeValue is not an OutputNode")
        }
    }

    pub fn as_production_mut(&mut self) -> (&mut &'a Recipe, &mut f64) {
        match self {
            NodeValue::Production( recipe, machine_count) => (recipe, machine_count),
            _ => panic!("NodeValue is not an ProductionNode")
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
                    round(item_value.value, 3)
                )
            }
            NodeValue::Production(recipe, machine_count) => {
                write!(
                    f,
                    "{} {}x {}",
                    recipe.name,
                    round(*machine_count, 3),
                    recipe.machine
                )
            }
            NodeValue::Output(item_value, ..) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
        }
    }
}

impl <'a> From<ScoredNodeValue<'a>> for NodeValue<'a> {
    fn from(value: ScoredNodeValue<'a>) -> Self {
        value.node
    }
}

impl<'a> fmt::Display for ScoredNodeValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(score) = self.score {
            write!(f, "{}\nScore: {}\n", self.node, score)
        } else {
            write!(f, "{}\nScore: None\n", self.node)
        }
    }
}

impl<'a> From<NodeValue<'a>> for ScoredNodeValue<'a> {
    fn from(node: NodeValue<'a>) -> Self {
        Self { node, score: None }
    }
}

fn round(value: f64, places: u8) -> f64 {
    let base: f64 = 10.0;
    let multiplier = base.powi(places as i32);

    (value * multiplier).round() / multiplier
}

pub fn find_input_node(graph: &GraphType<'_>, item: Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| match graph[*i] {
            NodeValue::Input(input) => item == input.item,
            _ => false
        })
}

pub fn find_production_node(graph: &GraphType<'_>, recipe: &Recipe) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| match graph[*i] {
            NodeValue::Production(node_recipe, _) => recipe == node_recipe,
            _ => false
        })
}

pub fn find_output_node(graph: &GraphType<'_>, item: Item) -> Option<NodeIndex> {
    graph
        .node_indices()
        .find(|i| match graph[*i] {
            NodeValue::Output(output, _) => item == output.item,
            _ => false
        })
}
