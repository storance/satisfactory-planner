use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

use crate::game::{Item, Recipe, ItemValuePair};
use crate::plan::PlanError;

const DEFAULT_LIMITS: [ItemValuePair<f64>; 12] = [
    ItemValuePair::new(Item::Bauxite, 9780.0),
    ItemValuePair::new(Item::CateriumOre, 12040.0),
    ItemValuePair::new(Item::Coal, 30900.0),
    ItemValuePair::new(Item::CopperOre, 28860.0),
    ItemValuePair::new(Item::CrudeOil, 11700.0),
    ItemValuePair::new(Item::IronOre, 70380.0),
    ItemValuePair::new(Item::Limestone, 52860.0),
    ItemValuePair::new(Item::NitrogenGas, 12000.0),
    ItemValuePair::new(Item::RawQuartz, 10500.0),
    ItemValuePair::new(Item::Sulfur, 6840.0),
    ItemValuePair::new(Item::Uranium, 2100.0),
    ItemValuePair::new(Item::Water, 9007199254740991.0),
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

#[derive(Debug)]
pub struct PlanConfig<'a> {
    pub inputs: HashMap<Item, f64>,
    pub outputs: HashMap<Item, f64>,
    pub recipes: Vec<&'a Recipe>,
    pub input_limits: HashMap<Item, f64>,
}

impl<'a> PlanConfig<'a> {
    pub fn from_file(
        file_path: &str,
        all_recipes: &'a Vec<Recipe>,
    ) -> Result<PlanConfig<'a>, PlanError> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

        let mut input_limits: HashMap<Item, f64> = DEFAULT_LIMITS
            .iter()
            .map(ItemValuePair::to_tuple)
            .collect();

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
                for recipe in all_recipes {
                    if !recipe.alternate && !recipes.contains(&recipe) {
                        recipes.push(recipe);
                    }
                }
            } else if recipe_name.eq_ignore_ascii_case("All Alternate") {
                for recipe in all_recipes {
                    if recipe.alternate && !recipes.contains(&recipe) {
                        recipes.push(recipe);
                    }
                }
            } else {
                let recipe = recipes_by_name.get(recipe_name).map(|r| *r)
                    .ok_or(PlanError::InvalidRecipe(recipe_name.clone()))?;
                if !recipes.contains(&recipe) {
                    recipes.push(recipe);
                }
            }
        }

        Ok(PlanConfig {
            inputs: config.inputs.iter().map(ItemValuePair::to_tuple).collect(),
            outputs: config.outputs.iter().map(ItemValuePair::to_tuple).collect(),
            recipes,
            input_limits,
        })
    }
}
