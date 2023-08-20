extern crate anyhow;
extern crate petgraph;
extern crate serde;
extern crate serde_yaml;
extern crate thiserror;

use crate::game::{Item, Machine, Recipe};
use crate::plan::{solve, PlanConfig};
use petgraph::dot::Dot;

mod game;
mod plan;
mod utils;

fn main() {
    let recipes = Recipe::load_from_file("recipes.yml").unwrap_or_else(|e| {
        panic!("Failed to load recipes: {}", e);
    });

    let plan = PlanConfig::from_file("plan.yml", &recipes).unwrap_or_else(|e| {
        panic!("Failed to load plan: {}", e);
    });

    let graph = solve(&plan).unwrap_or_else(|e| {
        panic!("Failed to solve plan: {}", e);
    });

    println!("{}", Dot::new(&graph));
}

pub fn print_item(item: Item) {
    println!(
        "{:?}(display_name: {}, is_fluid(): {}. is_extractable: {}, sink_points: {:?})",
        item,
        item.display_name(),
        item.is_fluid(),
        item.is_extractable(),
        item.sink_points()
    )
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
