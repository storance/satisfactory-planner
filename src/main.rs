extern crate serde;
extern crate serde_yaml;
extern crate thiserror;

use crate::game::{Item, Fluid, Machine, MachineIO, Recipe, ResourceDefinition};

mod game;
mod plan;

fn main() {
    print_item(Item::IronOre);
    print_item(Item::NuclearPasta);
    print_fluid(Fluid::Water);

    let recipes = Recipe::load_from_file("recipes.yml").unwrap_or_else(|e| {
        panic!("Failed to load recipes: {}", e);
    });
    println!("{:?}", recipes);
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
