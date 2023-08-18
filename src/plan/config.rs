use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;

use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::PlanError;

static DEFAULT_LIMITS: [(Item, f64); 12] = [
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

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct Score {
    pub weighted_items_used: f64,
    pub power_used: f64,
    pub space_used: f64,
    pub buildings_used: f64,
}

#[derive(Debug, Copy, Clone)]
pub struct ScoredRecipe<'a> {
    pub recipe: &'a Recipe,
    pub score: Score,
}

impl<'a> PlanConfig<'a> {
    pub fn from_file(
        file_path: &str,
        all_recipes: &'a Vec<Recipe>,
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
            inputs: config.inputs.iter().map(ItemValuePair::to_tuple).collect(),
            outputs: config.outputs,
            recipes,
            input_limits,
        })
    }
}

pub fn score_recipes<'a>(enabled_recipes: &Vec<&'a Recipe>) -> Vec<ScoredRecipe<'a>> {
    let mut scored_recipes: Vec<ScoredRecipe<'a>> = Vec::new();
    let input_limits: HashMap<Item, f64> = DEFAULT_LIMITS.iter().copied().collect();

    for recipe in enabled_recipes {
        scored_recipes.push(score_recipe(recipe, enabled_recipes, &input_limits));
    }

    scored_recipes
}

fn score_recipe<'a>(
    recipe: &'a Recipe,
    enabled_recipes: &Vec<&'a Recipe>,
    input_limits: &HashMap<Item, f64>,
) -> ScoredRecipe<'a> {
    let output = recipe.outputs.first().unwrap();
    score_recipe_for_output(
        recipe,
        ItemValuePair::new(output.item, 60.0),
        enabled_recipes,
        input_limits,
    )
}

fn score_recipe_for_output<'a>(
    recipe: &'a Recipe,
    desired_output: ItemValuePair<f64>,
    enabled_recipes: &Vec<&'a Recipe>,
    input_limits: &HashMap<Item, f64>,
) -> ScoredRecipe<'a> {
    let mut weighted_items_used = 0.0;
    let mut power_used = 0.0;
    let mut space_used: f64 = 0.0;
    let mut buildings_used: f64 = 0.0;

    let output = recipe.find_output_by_item(desired_output.item).unwrap();
    let machine_count = desired_output.value / output.amount_per_minute;

    for input in &recipe.inputs {
        if input.item.is_extractable() {
            weighted_items_used += (input.amount_per_minute * machine_count
                / input_limits.get(&input.item).unwrap())
                * 10000.0;
        } else {
            let candidate_recipes: Vec<ScoredRecipe<'a>> = enabled_recipes
                .iter()
                .filter(|r| r.outputs.iter().any(|output| output.item == input.item))
                .map(|r| {
                    let desired_output =
                        ItemValuePair::new(input.item, input.amount_per_minute * machine_count);
                    score_recipe_for_output(r, desired_output, enabled_recipes, input_limits)
                })
                .collect();
            if candidate_recipes.is_empty() {
                weighted_items_used = f64::INFINITY;
                power_used = f64::INFINITY;
                space_used = f64::INFINITY;
                buildings_used = f64::INFINITY;
            } else {
                weighted_items_used += candidate_recipes
                    .iter()
                    .map(|r| r.score.weighted_items_used)
                    .min_by(f64::total_cmp)
                    .unwrap();
            }
        }
    }

    ScoredRecipe {
        recipe,
        score: Score {
            weighted_items_used,
            power_used,
            space_used,
            buildings_used,
        },
    }
}
