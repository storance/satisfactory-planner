extern crate serde;
extern crate serde_yaml;
extern crate thiserror;

use crate::game::{Machine, MachineIO};

mod game;
mod plan;

fn main() {
    print_machine(Machine::Smelter);
    print_machine(Machine::Constructor);
    print_machine(Machine::Foundry);
    print_machine(Machine::Assembler);
    print_machine(Machine::Manufacturer);
    print_machine(Machine::Refinery);
    print_machine(Machine::Packager);
    print_machine(Machine::Blender);
    print_machine(Machine::ParticleAccelerator);

    /*let yaml = "
        inputs:
            - Iron Ingot: 30
            - Copper Ingot: 60
        outputs:
            - Iron Plate: 20
            - Wire: 120
        override_limits:
            - Iron Ore: 600
        enabled_recipes:
            - Iron Plate
            - Iron Ingot
    ";

    let recipes_yaml = "
        recipes:
            - name: Iron Plate
              inputs:
                - Iron Ore: 3
              outputs:
                - Iron Plate: 2
              production_time_secs: 6
              machine: Constructor
            - name: Iron Ingot
              inputs:
                - Iron Ore: 1
              outputs:
                - Iron Ingot: 1
              production_time_secs: 2
              machine: Smelter
    ";

    let factory: plan::PlanConfig = serde_yaml::from_str(yaml).expect("Failed to parse plan yaml");
    println!("PlanConfig: {:?}", factory);

    let recipe_db: recipes::RecipeDatabase = serde_yaml::from_str(recipes_yaml).expect("Failed to parse plan yaml");
    println!("RecipeDatabase: {:?}", recipe_db);

    let plan_settings = plan::PlanSettings::from_config(factory, &recipe_db.recipes).expect("Failed to convert plan settings");
    println!("PlanSettings: {:?}", plan_settings);*/
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
