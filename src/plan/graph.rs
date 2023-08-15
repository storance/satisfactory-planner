use crate::game::{Recipe, Item, ItemValuePair};
use std::fmt;

#[derive(Debug, Clone)]
pub enum NodeValue<'a> {
    InputNode(ItemValuePair<f64>),
    OutputNode(ItemValuePair<f64>, bool),
    ProductionNode(&'a Recipe, f64),
}

pub struct ScoredNodeValue<'a> {
    pub node: NodeValue<'a>,
    pub score: Option<f64>
}

impl<'a> NodeValue<'a> {
    pub fn new_input(item_value: ItemValuePair<f64>) -> Self {
        NodeValue::InputNode(item_value)
    }

    pub fn new_output(item_value: ItemValuePair<f64>, by_product: bool) -> Self {
        NodeValue::OutputNode(item_value, by_product)
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        NodeValue::ProductionNode(recipe, machine_count)
    }

    pub fn is_input(&self) -> bool {
        match self {
            NodeValue::InputNode(..) => true,
            _ => false,
        }
    }

    pub fn is_input_for_item(&self, item: Item) -> bool {
        match self {
            NodeValue::InputNode(item_value) => item_value.item == item,
            _ => false,
        }
    }

    pub fn is_output(&self) -> bool {
        match self {
            NodeValue::OutputNode(..) => true,
            _ => false,
        }
    }

    pub fn is_output_for_item(&self, item: Item) -> bool {
        match self {
            NodeValue::OutputNode(item_value, ..) => item_value.item == item,
            _ => false,
        }
    }

    pub fn is_production(&self) -> bool {
        match self {
            NodeValue::ProductionNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_production_for_recipe(&self, recipe: &Recipe) -> bool {
        match self {
            NodeValue::ProductionNode(node_recipe, ..) => *node_recipe == recipe,
            _ => false,
        }
    }
}

impl<'a> fmt::Display for NodeValue<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeValue::InputNode(item_value) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
            NodeValue::ProductionNode(recipe, machine_count) => {
                write!(
                    f,
                    "{} {}x {}",
                    recipe.name,
                    round(*machine_count, 3),
                    recipe.machine
                )
            }
            NodeValue::OutputNode(item_value, ..) => {
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

impl <'a> From<NodeValue<'a>> for ScoredNodeValue<'a> {
    fn from(value: NodeValue<'a>) -> Self {
        Self {
            node: value,
            score: None
        }
    }
}

fn round(value: f64, places: u8) -> f64 {
    let base: f64 = 10.0;
    let multiplier = base.powi(places as i32);

    (value * multiplier).round() / multiplier
}
