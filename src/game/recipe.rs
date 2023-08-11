use crate::game::{Machine, MachineIO, Resource, ResourceValuePair};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
struct RecipeDefinition {
    pub name: String,
    #[serde(default)]
    pub alternate: bool,
    pub outputs: Vec<ResourceValuePair<u32>>,
    pub inputs: Vec<ResourceValuePair<u32>>,
    pub craft_time: u32,
    #[serde(default = "default_power_multiplier")]
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecipesDefinition {
    pub recipes: Vec<RecipeDefinition>,
}

#[derive(Debug, Copy, Clone)]
pub struct RecipeResource {
    pub resource: Resource,
    pub amount: u32,
    pub amount_per_minute: f64
}

impl RecipeResource {
    pub fn new(resource: Resource, amount: u32, amount_per_minute: f64) -> Self {
        Self {
            resource,
            amount,
            amount_per_minute
        }
    }

    pub fn from(rv: &ResourceValuePair<u32>, crafts_per_minute: f64) -> Self {
        Self {
            resource: rv.resource,
            amount: rv.value,
            amount_per_minute: rv.value as f64 * crafts_per_minute
        }
    }
}

#[derive(Debug)]
pub struct Recipe {
    pub name: String,
    pub alternate: bool,
    pub outputs: Vec<RecipeResource>,
    pub inputs: Vec<RecipeResource>,
    pub craft_time: u32,
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Error, Debug)]
pub enum RecipeError {
    #[error("Recipe `{0}`: At least one input is required but none were provided")]
    MissingInputs(String),
    #[error("Recipe `{0}`: Incorrect number of inputs for machine `{1}`. Maximum supported is {} but recipe was defined with {2}.", .1.input_ports())]
    InvalidInputs(String, Machine, MachineIO),
    #[error("Recipe `{0}`: At least one output is required but none were provided")]
    MissingOutputs(String),
    #[error("Recipe `{0}`: Incorrect number of outputs for machine `{1}`. Maximum supported is {} but recipe was defined with {2}.", .1.output_ports())]
    InvalidOutputs(String, Machine, MachineIO),
    #[error("Found multiple recipes with the name `{0}`.  Recipe names must be unique.")]
    DuplicateRecipeName(String),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_yaml::Error),
}

const fn default_power_multiplier() -> f32 {
    1.0
}

fn is_max_ports_exceeded(max_inputs: MachineIO, actual_inputs: MachineIO) -> bool {
    actual_inputs.items > max_inputs.items || actual_inputs.fluids > max_inputs.fluids
}

impl Recipe {
    pub fn load_from_file(file_path: &str) -> Result<Vec<Recipe>, RecipeError> {
        let file = File::open(file_path)?;
        let config: RecipesDefinition = serde_yaml::from_reader(file)?;

        let mut recipes : Vec<Recipe> = Vec::with_capacity(config.recipes.len());
        for recipe in config.recipes {
            let inputs_count = recipe.inputs.iter().fold(MachineIO::zero(), |mut acc, rv| {
                match rv.resource {
                    Resource::Item(..) => acc.items += 1,
                    Resource::Fluid(..) => acc.fluids += 1,
                }

                acc
            });

            let outputs_count = recipe.outputs.iter().fold(MachineIO::zero(), |mut acc, rv| {
                match rv.resource {
                    Resource::Item(..) => acc.items += 1,
                    Resource::Fluid(..) => acc.fluids += 1,
                }

                acc
            });

            if inputs_count.items + inputs_count.fluids == 0 {
                return Err(RecipeError::MissingInputs(recipe.name))
            }

            if outputs_count.items + outputs_count.fluids == 0 {
                return Err(RecipeError::MissingOutputs(recipe.name))
            }

            if is_max_ports_exceeded(recipe.machine.input_ports(), inputs_count) {
                return Err(RecipeError::InvalidInputs(recipe.name, recipe.machine, inputs_count))
            }

            if is_max_ports_exceeded(recipe.machine.output_ports(), outputs_count) {
                return Err(RecipeError::InvalidOutputs(recipe.name,recipe.machine, outputs_count))
            }

            if let Some(..) = recipes.iter().find(|r| r.name == recipe.name) {
               return Err(RecipeError::DuplicateRecipeName(recipe.name));
            }

            recipes.push(Self::from(recipe));
        }

        Ok(recipes)
    }

    pub fn calc_min_power(&self) -> f32 {
        self.machine.min_power() as f32 * self.power_multiplier
    }

    pub fn calc_max_power(&self) -> f32 {
        self.machine.max_power() as f32 * self.power_multiplier
    }

    pub fn calc_avg_power(&self) -> f32 {
        (self.calc_min_power() + self.calc_max_power()) / 2.0
    }

    pub fn find_input_by_item(&self, resource: Resource) -> Option<&RecipeResource> {
        self.inputs.iter().find(|output| output.resource == resource)
    }

    pub fn find_output_by_item(&self, resource: Resource) -> Option<&RecipeResource> {
        self.outputs.iter().find(|output| output.resource == resource)
    }
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl From<RecipeDefinition> for Recipe {
    fn from(recipe: RecipeDefinition) -> Self {
        let crafts_per_min = 60.0 / recipe.craft_time as f64;

        let inputs = recipe.inputs.iter()
            .map(|rv| RecipeResource::from(rv, crafts_per_min))
            .collect();
        let outputs = recipe.outputs.iter()
            .map(|rv| RecipeResource::from(rv, crafts_per_min))
            .collect();

        Self {
            name: recipe.name,
            alternate: recipe.alternate,
            outputs,
            inputs,
            craft_time: recipe.craft_time,
            power_multiplier: recipe.power_multiplier,
            machine: recipe.machine
        }
    }
}
