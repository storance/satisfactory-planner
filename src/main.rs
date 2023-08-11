extern crate serde;
extern crate serde_yaml;
extern crate thiserror;
extern crate petgraph;

use crate::game::{Item, Fluid, Machine, Recipe, ResourceDefinition};
use crate::plan::{PlanConfig, solve};
use petgraph::dot::Dot;

mod game;
mod plan;

fn main() {
    let recipes = Recipe::load_from_file("recipes.yml").unwrap_or_else(|e| {
        panic!("Failed to load recipes: {}", e);
    });

    // recipes.iter().for_each(|r| println!("{:?}", r));

    let plan = PlanConfig::from_file("plan.yml", &recipes).unwrap_or_else(|e| {
        panic!("Failed to load plan: {}", e);
    });



    let graph = solve(&plan).unwrap_or_else(|e| {
        panic!("Failed to solve plan: {}", e);
    });

    println!("{}", Dot::new(&graph));
}

pub fn print_item(item: Item) {
    println!("{:?}(display_name: {}, is_raw: {}, sink_points: {:?})",
             item,
             item.display_name(),
             item.is_raw(),
             item.sink_points())
}

pub fn print_fluid(item: Fluid) {
    println!("{:?}(display_name: {}, is_raw: {}, sink_points: {:?})",
             item,
             item.display_name(),
             item.is_raw(),
             item.sink_points())
}

pub fn print_machine(machine: Machine) {
    println!(
        "{}{{{}-{}}}{} => {}",
        machine.display_name(),
        machine.min_power(),
        machine.max_power(),
        machine.input_ports(),
        machine.output_ports()
    );
}
