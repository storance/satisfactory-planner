use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

use crate::game::{Fluid, Item, Recipe, Resource, ResourceDefinition, ResourceValuePair};
use crate::plan::PlanError;

const DEFAULT_LIMITS: [ResourceValuePair<f64>; 12] = [
    ResourceValuePair::for_item(Item::Bauxite, 9780.0),
    ResourceValuePair::for_item(Item::CateriumOre, 12040.0),
    ResourceValuePair::for_item(Item::Coal, 30900.0),
    ResourceValuePair::for_item(Item::CopperOre, 28860.0),
    ResourceValuePair::for_fluid(Fluid::CrudeOil, 11700.0),
    ResourceValuePair::for_item(Item::IronOre, 70380.0),
    ResourceValuePair::for_item(Item::Limestone, 52860.0),
    ResourceValuePair::for_fluid(Fluid::NitrogenGas, 12000.0),
    ResourceValuePair::for_item(Item::RawQuartz, 10500.0),
    ResourceValuePair::for_item(Item::Sulfur, 6840.0),
    ResourceValuePair::for_item(Item::Uranium, 2100.0),
    ResourceValuePair::for_fluid(Fluid::Water, 9007199254740991.0),
];

#[derive(Debug, Serialize, Deserialize)]
struct PlanConfigDefinition {
    #[serde(default)]
    inputs: Vec<ResourceValuePair<f64>>,
    #[serde(default)]
    outputs: Vec<ResourceValuePair<f64>>,
    enabled_recipes: Option<Vec<String>>,
    #[serde(default)]
    override_limits: HashMap<Resource, f64>,
}

#[derive(Debug)]
pub struct PlanConfig<'a> {
    pub inputs: Vec<ResourceValuePair<f64>>,
    pub outputs: Vec<ResourceValuePair<f64>>,
    pub recipes: Vec<&'a Recipe>,
    pub input_limits: HashMap<Resource, f64>,
}

impl<'a> PlanConfig<'a> {
    pub fn from_file(
        file_path: &str,
        all_recipes: &'a Vec<Recipe>,
    ) -> Result<PlanConfig<'a>, PlanError> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

        let mut input_limits: HashMap<Resource, f64> = DEFAULT_LIMITS
            .iter()
            .map(ResourceValuePair::to_tuple)
            .collect();

        for (resource, value) in config.override_limits {
            if !resource.is_raw() {
                return Err(PlanError::InvalidOverrideLimit(String::from(
                    resource.display_name(),
                )));
            } else {
                input_limits.insert(resource, value);
            }
        }

        // validate there are no raw resource in the inputs list
        for input in &config.inputs {
            if input.resource.is_raw() {
                return Err(PlanError::UnexpectedRawInputItem(String::from(
                    input.resource.display_name(),
                )));
            }
        }

        // validate there are no raw resource in the outputs list
        for output in &config.inputs {
            if output.resource.is_raw() {
                return Err(PlanError::UnexpectedRawOutputItem(String::from(
                    output.resource.display_name(),
                )));
            }
        }

        let recipes_by_name: HashMap<String, &'a Recipe> =
            all_recipes.iter().map(|r| (r.name.clone(), r)).collect();

        // lookup the enabled recipes from the set of enabled recipes
        // if enabled_recipes was not specified, default to using all the base recipes
        let recipes: Vec<&Recipe> = if let Some(enabled_recipes) = config.enabled_recipes {
            enabled_recipes
                .iter()
                .map(|recipe_name| {
                    recipes_by_name
                        .get(recipe_name)
                        .map(|r| *r)
                        .ok_or(PlanError::InvalidRecipe(recipe_name.clone()))
                })
                .collect::<Result<Vec<&'a Recipe>, PlanError>>()?
        } else {
            all_recipes.iter().filter(|r| !r.alternate).collect()
        };

        Ok(PlanConfig {
            inputs: config.inputs,
            outputs: config.outputs,
            recipes,
            input_limits,
        })
    }
}
