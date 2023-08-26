extern crate anyhow;
extern crate indexmap;
extern crate petgraph;
extern crate serde;
extern crate serde_yaml;
extern crate thiserror;

use crate::game::{Item, Machine, Recipe};
use crate::plan::{PlanConfig};
use game::recipe::RecipeDatabase;
use plan::{print_graph, ScoredGraph, solve};

mod game;
mod plan;
mod utils;

fn main() {
    let recipes = RecipeDatabase::from_file("recipes.yml").unwrap_or_else(|e| {
        panic!("Failed to load recipes: {}", e);
    });

    let plan = PlanConfig::from_file("plan.yml", &recipes).unwrap_or_else(|e| {
        panic!("Failed to load plan: {}", e);
    });

    let graph = solve(&plan).unwrap_or_else(|e| {
        panic!("Failed to solve plan: {}", e);
    });

    print_graph(&graph);
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
