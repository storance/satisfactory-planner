use crate::game::{Recipe, Item, ItemValuePair};
use std::fmt;

#[derive(Debug)]
pub enum PlanGraphNode<'a> {
    InputNode(ItemValuePair<f64>),
    OutputNode(ItemValuePair<f64>, bool),
    ProductionNode(&'a Recipe, f64),
}

impl<'a> PlanGraphNode<'a> {
    pub fn new_input(item_value: ItemValuePair<f64>) -> Self {
        PlanGraphNode::InputNode(item_value)
    }

    pub fn new_output(item_value: ItemValuePair<f64>, by_product: bool) -> Self {
        PlanGraphNode::OutputNode(item_value, by_product)
    }

    pub fn new_production(recipe: &'a Recipe, machine_count: f64) -> Self {
        PlanGraphNode::ProductionNode(recipe, machine_count)
    }

    pub fn is_input(&self) -> bool {
        match self {
            PlanGraphNode::InputNode(..) => true,
            _ => false,
        }
    }

    pub fn is_input_for_item(&self, item: Item) -> bool {
        match self {
            PlanGraphNode::InputNode(item_value) => item_value.item == item,
            _ => false,
        }
    }

    pub fn is_output(&self) -> bool {
        match self {
            PlanGraphNode::OutputNode(..) => true,
            _ => false,
        }
    }

    pub fn is_output_for_item(&self, item: Item) -> bool {
        match self {
            PlanGraphNode::OutputNode(item_value, ..) => item_value.item == item,
            _ => false,
        }
    }

    pub fn is_production(&self) -> bool {
        match self {
            PlanGraphNode::ProductionNode { .. } => true,
            _ => false,
        }
    }

    pub fn is_production_for_recipe(&self, recipe: &Recipe) -> bool {
        match self {
            PlanGraphNode::ProductionNode(node_recipe, ..) => *node_recipe == recipe,
            _ => false,
        }
    }
}

impl<'a> fmt::Display for PlanGraphNode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanGraphNode::InputNode(item_value) => {
                write!(
                    f,
                    "{} {} / min",
                    item_value.item,
                    round(item_value.value, 3)
                )
            }
            PlanGraphNode::ProductionNode(recipe, machine_count) => {
                write!(
                    f,
                    "{} {}x {}",
                    recipe.name,
                    round(*machine_count, 3),
                    recipe.machine
                )
            }
            PlanGraphNode::OutputNode(item_value, ..) => {
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

fn round(value: f64, places: u8) -> f64 {
    let base: f64 = 10.0;
    let multiplier = base.powi(places as i32);

    (value * multiplier).round() / multiplier
}
