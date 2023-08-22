extern crate anyhow;
extern crate indexmap;
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

    println!("{}", format!("{}", Dot::with_attr_getters(&graph,
        &[],
        &|_, _| String::new(),
        &|_, n| {
            let color = match n.1 {
                plan::NodeValue::Input(input) => if input.item.is_extractable() { "lightslategray"} else { "peru" },
                plan::NodeValue::Output(..) => "mediumseagreen",
                plan::NodeValue::ByProduct(..) => "cornflowerblue",
                plan::NodeValue::Production(..) => "darkorange"
            };

            format!("style=\"solid,filled\" shape=\"box\" fontcolor=\"white\" color=\"{}\"", color)
        })).replace("\\l", "\\n"));
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
