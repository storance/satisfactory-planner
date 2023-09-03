use petgraph::dot::Dot;
use petgraph::stable_graph::StableDiGraph;
use std::fmt;

mod config;
mod full_plan_graph;
mod solved_graph;
mod solver;

pub use config::*;
pub use full_plan_graph::*;
pub use solved_graph::*;
pub use solver::*;

pub const UNSOLVABLE_PLAN_ERROR: &str = "Unable to solve the given factory plan.";

pub trait NodeWeight
where
    Self: fmt::Display,
{
    fn is_input(&self) -> bool;
    fn is_input_resource(&self) -> bool;
    fn is_output(&self) -> bool;
    fn is_by_product(&self) -> bool;
    fn is_production(&self) -> bool;

    /*fn is_input_for_item(&self, item: &Item) -> bool;
    fn is_output_for_item(&self, item: &Item) -> bool;
    fn is_by_product_for_item(&self, item: &Item) -> bool;
    fn is_production_for_recipe(&self, recipe: &Recipe) -> bool;*/
}

pub fn print_graph<N: NodeWeight, E: fmt::Display>(graph: &StableDiGraph<N, E>) {
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
