use crate::game::{Item, ItemValuePair, Machine, MachineIO};
use anyhow;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
struct RecipeDefinition {
    pub name: String,
    #[serde(default)]
    pub alternate: bool,
    #[serde(default)]
    pub ficsmas: bool,
    pub outputs: Vec<ItemValuePair>,
    pub inputs: Vec<ItemValuePair>,
    pub craft_time: u32,
    #[serde(default = "default_power_multiplier")]
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecipesRoot {
    pub recipes: Vec<RecipeDefinition>,
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub name: String,
    pub alternate: bool,
    pub ficsmas: bool,
    pub outputs: Vec<ItemValuePair>,
    pub inputs: Vec<ItemValuePair>,
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Error, Debug, PartialEq)]
pub enum RecipeError {
    #[error("Recipe `{0}`: At least one input is required but none were provided")]
    MissingInputs(String),
    #[error("Recipe `{0}`: Incorrect number of inputs for machine `{1}`. Maximum supported is {} but recipe was defined with {2}.", .1.input_ports())]
    InvalidInputs(String, Machine, MachineIO),
    #[error("Recipe `{0}`: The item `{1}` was specified multiple times in the inputs.")]
    DuplicateInput(String, Item),
    #[error("Recipe `{0}`: At least one output is required but none were provided")]
    MissingOutputs(String),
    #[error("Recipe `{0}`: Incorrect number of outputs for machine `{1}`. Maximum supported is {} but recipe was defined with {2}.", .1.output_ports())]
    InvalidOutputs(String, Machine, MachineIO),
    #[error("Recipe `{0}`: The item `{1}` was specified multiple times in the outputs.")]
    DuplicateOutput(String, Item),
    #[error("Found multiple recipes with the name `{0}`.  Recipe names must be unique.")]
    DuplicateRecipeName(String),
}

const fn default_power_multiplier() -> f32 {
    1.0
}

impl Recipe {
    pub fn load_from_file(file_path: &str) -> anyhow::Result<Vec<Recipe>> {
        let file = File::open(file_path)?;
        let config: RecipesRoot = serde_yaml::from_reader(file)?;

        Ok(Self::convert(config)?)
    }

    fn convert(config: RecipesRoot) -> Result<Vec<Recipe>, RecipeError> {
        let mut converted_recipes: Vec<Recipe> = Vec::with_capacity(config.recipes.len());
        for recipe in config.recipes {
            Self::check_for_duplicate_io(&recipe.inputs, |item| {
                RecipeError::DuplicateInput(recipe.name.clone(), item)
            })?;
            Self::check_for_duplicate_io(&recipe.outputs, |item| {
                RecipeError::DuplicateOutput(recipe.name.clone(), item)
            })?;

            let inputs_count = MachineIO::from(&recipe.inputs);
            let outputs_count = MachineIO::from(&recipe.outputs);

            if inputs_count.is_zero() {
                return Err(RecipeError::MissingInputs(recipe.name));
            }

            if outputs_count.is_zero() {
                return Err(RecipeError::MissingOutputs(recipe.name));
            }

            if inputs_count.is_greater(&recipe.machine.input_ports()) {
                return Err(RecipeError::InvalidInputs(
                    recipe.name,
                    recipe.machine,
                    inputs_count,
                ));
            }

            if outputs_count.is_greater(&recipe.machine.output_ports()) {
                return Err(RecipeError::InvalidOutputs(
                    recipe.name,
                    recipe.machine,
                    outputs_count,
                ));
            }

            if converted_recipes.iter().any(|r| r.name == recipe.name) {
                return Err(RecipeError::DuplicateRecipeName(recipe.name));
            }

            converted_recipes.push(recipe.into());
        }

        Ok(converted_recipes)
    }

    fn check_for_duplicate_io<F>(io: &[ItemValuePair], err: F) -> Result<(), RecipeError>
    where
        F: FnOnce(Item) -> RecipeError,
    {
        let mut unique_items: HashSet<Item> = HashSet::new();

        for item_value in io {
            if !unique_items.insert(item_value.item) {
                return Err(err(item_value.item));
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn calc_min_power(&self) -> f32 {
        self.machine.min_power() as f32 * self.power_multiplier
    }

    #[allow(dead_code)]
    pub fn calc_max_power(&self) -> f32 {
        self.machine.max_power() as f32 * self.power_multiplier
    }

    #[allow(dead_code)]
    pub fn calc_avg_power(&self) -> f32 {
        (self.calc_min_power() + self.calc_max_power()) / 2.0
    }

    #[allow(dead_code)]
    pub fn calc_overclocked_avg_power(&self, clock_speed: f32) -> f32 {
        self.calc_avg_power() * (clock_speed / 100.0).powf(1.321928)
    }

    #[allow(dead_code)]
    pub fn find_input_by_item(&self, item: Item) -> Option<&ItemValuePair> {
        self.inputs.iter().find(|output| output.item == item)
    }

    pub fn find_output_by_item(&self, item: Item) -> Option<&ItemValuePair> {
        self.outputs.iter().find(|output| output.item == item)
    }

    pub fn has_output_item(&self, item: Item) -> bool {
        self.outputs.iter().any(|output| output.item == item)
    }

    #[allow(dead_code)]
    pub fn has_input_item(&self, item: Item) -> bool {
        self.inputs.iter().any(|input| input.item == item)
    }
}

impl From<RecipeDefinition> for Recipe {
    fn from(recipe: RecipeDefinition) -> Self {
        let crafts_per_min = 60.0 / recipe.craft_time as f64;

        let inputs = recipe
            .inputs
            .iter()
            .map(|input| *input * crafts_per_min)
            .collect();
        let outputs = recipe
            .outputs
            .iter()
            .map(|output| *output * crafts_per_min)
            .collect();

        Self {
            name: recipe.name,
            alternate: recipe.alternate,
            ficsmas: recipe.ficsmas,
            outputs,
            inputs,
            power_multiplier: recipe.power_multiplier,
            machine: recipe.machine,
        }
    }
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Recipe {}

#[cfg(test)]
mod tests {
    use crate::utils::round_f32;
    use std::vec;

    use super::*;

    #[test]
    fn convert() {
        let recipe_def = RecipeDefinition {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronOre, 1.0)],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 1.0)],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        let expected_recipe = Recipe {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronOre, 15.0)],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 15.0)],
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        assert_eq!(result, Ok(vec![expected_recipe]));
    }

    #[test]
    fn convert_duplicate_inputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Beacon"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::IronPlate, 3.0),
                ItemValuePair::new(Item::IronRod, 1.0),
                ItemValuePair::new(Item::Wire, 15.0),
                ItemValuePair::new(Item::IronPlate, 2.0),
            ],
            outputs: vec![ItemValuePair::new(Item::Beacon, 1.0)],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Manufacturer,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(
            result,
            Err(RecipeError::DuplicateInput(
                "Beacon".into(),
                Item::IronPlate
            ))
        );
    }

    #[test]
    fn convert_duplicate_outputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Encased Uranium Cell"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::Uranium, 10.0),
                ItemValuePair::new(Item::Concrete, 3.0),
                ItemValuePair::new(Item::SulfuricAcid, 8.0),
            ],
            outputs: vec![
                ItemValuePair::new(Item::EncasedUraniumCell, 5.0),
                ItemValuePair::new(Item::EncasedUraniumCell, 2.0),
            ],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Blender,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(
            result,
            Err(RecipeError::DuplicateOutput(
                "Encased Uranium Cell".into(),
                Item::EncasedUraniumCell
            ))
        );
    }

    #[test]
    fn convert_missing_inputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 1.0)],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(result, Err(RecipeError::MissingInputs("Iron Ingot".into())));
    }

    #[test]
    fn convert_missing_outputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronIngot, 1.0)],
            outputs: vec![],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(
            result,
            Err(RecipeError::MissingOutputs("Iron Ingot".into()))
        );
    }

    #[test]
    fn convert_incorrect_inputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Encased Uranium Cell"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::Uranium, 10.0),
                ItemValuePair::new(Item::Concrete, 3.0),
                ItemValuePair::new(Item::IronPlate, 8.0),
            ],
            outputs: vec![ItemValuePair::new(Item::EncasedUraniumCell, 5.0)],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Blender,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(
            result,
            Err(RecipeError::InvalidInputs(
                "Encased Uranium Cell".into(),
                Machine::Blender,
                MachineIO::new(3, 0)
            ))
        );
    }

    #[test]
    fn convert_incorrect_outputs() {
        let recipe_def = RecipeDefinition {
            name: String::from("Encased Uranium Cell"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::Uranium, 10.0),
                ItemValuePair::new(Item::Concrete, 3.0),
                ItemValuePair::new(Item::SulfuricAcid, 8.0),
            ],
            outputs: vec![
                ItemValuePair::new(Item::EncasedUraniumCell, 5.0),
                ItemValuePair::new(Item::UraniumFuelRod, 2.0),
            ],
            craft_time: 4,
            power_multiplier: 1.0,
            machine: Machine::Blender,
        };

        let result = Recipe::convert(RecipesRoot {
            recipes: vec![recipe_def],
        });

        assert_eq!(
            result,
            Err(RecipeError::InvalidOutputs(
                "Encased Uranium Cell".into(),
                Machine::Blender,
                MachineIO::new(2, 0)
            ))
        );
    }

    #[test]
    fn calc_min_power_base_multiplier() {
        let recipe = Recipe {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronOre, 15.0)],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 15.0)],
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        assert_eq!(recipe.calc_min_power(), Machine::Smelter.min_power() as f32);
    }

    #[test]
    fn calc_min_power_with_multiplier() {
        let recipe = Recipe {
            name: String::from("Nuclear Pasta"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::CopperPowder, 100.0),
                ItemValuePair::new(Item::PressureConversionCube, 0.5),
            ],
            outputs: vec![ItemValuePair::new(Item::NuclearPasta, 0.5)],
            power_multiplier: 2.0,
            machine: Machine::ParticleAccelerator,
        };

        assert_eq!(recipe.calc_min_power(), 500.0);
    }

    #[test]
    fn calc_max_power_base_multiplier() {
        let recipe = Recipe {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronOre, 15.0)],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 15.0)],
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        assert_eq!(recipe.calc_max_power(), Machine::Smelter.max_power() as f32);
    }

    #[test]
    fn calc_max_power_with_multiplier() {
        let recipe = Recipe {
            name: String::from("Nuclear Pasta"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::CopperPowder, 100.0),
                ItemValuePair::new(Item::PressureConversionCube, 0.5),
            ],
            outputs: vec![ItemValuePair::new(Item::NuclearPasta, 0.5)],
            power_multiplier: 2.0,
            machine: Machine::ParticleAccelerator,
        };

        assert_eq!(recipe.calc_max_power(), 1500.0);
    }

    #[test]
    fn calc_avg_power_base_multiplier() {
        let recipe = Recipe {
            name: String::from("Plutonium Pellet"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::NonFissileUranium, 100.0),
                ItemValuePair::new(Item::UraniumWaste, 25.0),
            ],
            outputs: vec![ItemValuePair::new(Item::PlutoniumPellet, 30.0)],
            power_multiplier: 1.0,
            machine: Machine::ParticleAccelerator,
        };

        assert_eq!(recipe.calc_avg_power(), 500.0);
    }

    #[test]
    fn calc_avg_power_with_multiplier() {
        let recipe = Recipe {
            name: String::from("Nuclear Pasta"),
            alternate: false,
            ficsmas: false,
            inputs: vec![
                ItemValuePair::new(Item::CopperPowder, 100.0),
                ItemValuePair::new(Item::PressureConversionCube, 0.5),
            ],
            outputs: vec![ItemValuePair::new(Item::NuclearPasta, 0.5)],
            power_multiplier: 2.0,
            machine: Machine::ParticleAccelerator,
        };

        assert_eq!(recipe.calc_avg_power(), 1000.0);
    }

    #[test]
    fn calc_overclocked_avg_power() {
        let recipe = Recipe {
            name: String::from("Iron Ingot"),
            alternate: false,
            ficsmas: false,
            inputs: vec![ItemValuePair::new(Item::IronOre, 15.0)],
            outputs: vec![ItemValuePair::new(Item::IronIngot, 15.0)],
            power_multiplier: 1.0,
            machine: Machine::Smelter,
        };

        assert_eq!(round_f32(recipe.calc_overclocked_avg_power(50.0), 1), 1.6);
        assert_eq!(round_f32(recipe.calc_overclocked_avg_power(100.0), 1), 4.0);
        assert_eq!(round_f32(recipe.calc_overclocked_avg_power(150.0), 1), 6.8);
        assert_eq!(round_f32(recipe.calc_overclocked_avg_power(200.0), 1), 10.0);
        assert_eq!(round_f32(recipe.calc_overclocked_avg_power(250.0), 1), 13.4);
    }
}
