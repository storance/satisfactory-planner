use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::PlanError;

static DEFAULT_LIMITS: [(Item, f64); 13] = [
    (Item::Bauxite, 9780.0),
    (Item::CateriumOre, 12040.0),
    (Item::Coal, 30900.0),
    (Item::CopperOre, 28860.0),
    (Item::CrudeOil, 11700.0),
    (Item::IronOre, 70380.0),
    (Item::Limestone, 52860.0),
    (Item::NitrogenGas, 12000.0),
    (Item::RawQuartz, 10500.0),
    (Item::Sulfur, 6840.0),
    (Item::Uranium, 2100.0),
    (Item::Water, 9007199254740991.0),
    (Item::SAMOre, 0.0),
];

#[derive(Debug, Serialize, Deserialize)]
struct PlanConfigDefinition {
    #[serde(default)]
    inputs: Vec<ItemValuePair<f64>>,
    #[serde(default)]
    outputs: Vec<ItemValuePair<f64>>,
    enabled_recipes: Vec<String>,
    #[serde(default)]
    override_limits: HashMap<Item, f64>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig<'a> {
    pub inputs: HashMap<Item, f64>,
    pub outputs: Vec<ItemValuePair<f64>>,
    pub recipes: Vec<&'a Recipe>,
    pub input_limits: HashMap<Item, f64>,
}

impl<'a> PlanConfig<'a> {
    pub fn from_file(
        file_path: &str,
        all_recipes: &'a [Recipe],
    ) -> Result<PlanConfig<'a>, PlanError> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

        let mut input_limits: HashMap<Item, f64> =
            DEFAULT_LIMITS.iter().copied().collect();

        for (item, value) in config.override_limits {
            if !item.is_extractable() {
                return Err(PlanError::InvalidOverrideLimit(item));
            } else {
                input_limits.insert(item, value);
            }
        }

        // validate there are no raw resource in the inputs list
        for input in &config.inputs {
            if input.item.is_extractable() {
                return Err(PlanError::UnexpectedRawInputItem(input.item));
            }
        }

        // validate there are no raw resource in the outputs list
        for output in &config.outputs {
            if output.item.is_extractable() {
                return Err(PlanError::UnexpectedRawOutputItem(output.item));
            }
        }

        let recipes_by_name: HashMap<String, &'a Recipe> =
            all_recipes.iter().map(|r| (r.name.clone(), r)).collect();

        // lookup the enabled recipes from the list of all recipes
        let mut recipes: Vec<&Recipe> = Vec::new();
        for recipe_name in &config.enabled_recipes {
            if recipe_name.eq_ignore_ascii_case("All Base") {
                recipes.extend(all_recipes.iter().filter(|r| !r.alternate));
            } else if recipe_name.eq_ignore_ascii_case("All Alternate") {
                recipes.extend(all_recipes.iter().filter(|r| r.alternate));
            } else {
                let recipe = recipes_by_name
                    .get(recipe_name).copied()
                    .ok_or(PlanError::InvalidRecipe(recipe_name.clone()))?;
                if !recipes.contains(&recipe) {
                    recipes.push(recipe);
                }
            }
        }

        recipes.sort_by_key(|r| &r.name);
        recipes.dedup();

        Ok(PlanConfig {
            inputs: config.inputs.iter().copied().map(ItemValuePair::to_tuple).collect(),
            outputs: config.outputs,
            recipes,
            input_limits,
        })
    }
}
