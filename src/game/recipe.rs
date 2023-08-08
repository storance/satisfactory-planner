use crate::game::{Machine, MachineIO, ResourceValuePair};
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
    pub production_time_secs: u32,
    #[serde(default = "default_power_multiplier")]
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecipesDefinition {
    pub recipes: Vec<RecipeDefinition>,
}

#[derive(Debug)]
pub struct Recipe {
    pub name: String,
    pub alternate: bool,
    pub outputs_amounts: Vec<ResourceValuePair<u32>>,
    pub outputs_per_min: Vec<ResourceValuePair<f64>>,
    pub inputs_amounts: Vec<ResourceValuePair<u32>>,
    pub inputs_per_min: Vec<ResourceValuePair<f64>>,
    pub production_time_secs: u32,
    pub power_multiplier: f32,
    pub machine: Machine,
}

#[derive(Error, Debug)]
pub enum RecipeError {
    #[error("Machine `{0}` takes {} but recipe contains {1}", .0.input_ports())]
    InvalidInputs(Machine, MachineIO),
    #[error("Machine `{0}` outputs {} but recipe contains {1}", .0.output_ports())]
    InvalidOutputs(Machine, MachineIO),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_yaml::Error),
}

fn default_power_multiplier() -> f32 {
    1.0
}

impl Recipe {
    /*pub fn load_from_file(file_path: &str) -> Result<Vec<Recipe>, RecipeError> {
        let file = File::open(file_path)?;
        let config: RecipesDefinition = serde_yaml::from_reader(file)?;
    }*/

    /*pub fn calc_outputs_per_minute(&self) -> Vec<ItemPerMinute> {
        let crafts_per_min = 60.0 / self.production_time_secs as f64;
        self.outputs.iter().map(|o| o.convert_to_per_minute(crafts_per_min)).collect()
    }

    pub fn calc_inputs_per_minute(&self) -> Vec<ItemPerMinute> {
        let crafts_per_min = 60.0 / self.production_time_secs as f64;
        self.inputs.iter().map(|o| o.convert_to_per_minute(crafts_per_min)).collect()
    }*/

    pub fn calc_min_power(&self) -> f32 {
        self.machine.min_power() as f32 * self.power_multiplier
    }

    pub fn calc_max_power(&self) -> f32 {
        self.machine.max_power() as f32 * self.power_multiplier
    }

    pub fn calc_avg_power(&self) -> f32 {
        (self.calc_min_power() + self.calc_max_power()) / 2.0
    }
}
