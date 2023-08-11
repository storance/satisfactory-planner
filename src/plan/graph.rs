use std::fmt;
use crate::game::{Recipe, Resource, ResourceValuePair};

#[derive(Debug)]
pub enum PlanGraphNode<'a> {
    InputNode (ResourceValuePair<f64>),
    OutputNode(ResourceValuePair<f64>, bool),
    ProductionNode(&'a Recipe, f64),
}

impl<'a> PlanGraphNode<'a> {
    pub fn new_input(resource_value: ResourceValuePair<f64>) -> Self {
        PlanGraphNode::InputNode(resource_value)
    }

    pub fn new_output(resource_value: ResourceValuePair<f64>, by_product: bool) -> Self {
        PlanGraphNode::OutputNode(resource_value, by_product)
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

    pub fn is_input_for_item(&self, resource: Resource) -> bool {
        match self {
            PlanGraphNode::InputNode(resource_value) => resource_value.resource == resource,
            _ => false,
        }
    }

    pub fn is_output(&self) -> bool {
        match self {
            PlanGraphNode::OutputNode(..) => true,
            _ => false,
        }
    }

    pub fn is_output_for_item(&self, resource: Resource) -> bool {
        match self {
            PlanGraphNode::OutputNode (resource_value, ..) => resource_value.resource == resource,
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
            PlanGraphNode::ProductionNode(node_recipe, .. ) => *node_recipe == recipe,
            _ => false,
        }
    }
}

impl <'a> fmt::Display for PlanGraphNode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanGraphNode::InputNode (resource_value) => {
                write!(f, "{} {} / min", resource_value.resource, round(resource_value.value, 3))
            },
            PlanGraphNode::ProductionNode(recipe, machine_count, ..) => {
                write!(f, "{} {:.1}x {}", recipe.name, round(*machine_count, 3), recipe.machine)
            },
            PlanGraphNode::OutputNode (resource_value, ..) => {
                write!(f, "{} {} / min", resource_value.resource, round(resource_value.value, 3))
            }
        }
    }
}

fn round(value: f64, places: u8) -> f64 {
    let base: f64 = 10.0;
    let multiplier = base.powi(places as i32);

    (value * multiplier).round() / multiplier
}
