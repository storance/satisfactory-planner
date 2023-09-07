use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::game::{GameDatabase, ItemId, RecipeId};
use crate::utils::FloatType;

use super::PlanError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputLimit {
    pub item: String,
    pub amount: FloatType,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OutputAmount {
    Maximize,
    PerMinute(FloatType),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanConfigDefinition {
    #[serde(default)]
    inputs: HashMap<String, FloatType>,
    outputs: HashMap<String, OutputAmount>,
    recipes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub inputs: HashMap<ItemId, FloatType>,
    pub outputs: HashMap<ItemId, OutputAmount>,
    pub game_db: Arc<GameDatabase>,
    pub enabled_recipes: Vec<RecipeId>,
}

#[allow(dead_code)]
impl OutputAmount {
    pub fn is_maximize(&self) -> bool {
        matches!(self, Self::Maximize)
    }

    pub fn is_per_minute(&self) -> bool {
        matches!(self, Self::PerMinute(..))
    }

    pub fn as_per_minute(&self) -> FloatType {
        match self {
            OutputAmount::PerMinute(a) => *a,
            _ => panic!("ProductionAmount was not PerMinute"),
        }
    }
}

#[allow(dead_code)]
impl PlanConfig {
    pub fn parse(
        config: PlanConfigDefinition,
        game_db: Arc<GameDatabase>,
    ) -> Result<Self, PlanError> {
        // validate there are no extractable resources in the outputs list
        let mut outputs = HashMap::new();
        for (item_name, value) in config.outputs {
            let item_id = game_db
                .find_item(&item_name)
                .ok_or_else(|| PlanError::UnknownItem(item_name.clone()))?;
            let item = &game_db[item_id];
            if item.resource {
                return Err(PlanError::UnexpectedResourceInOutputs(item.name.clone()));
            }

            if let OutputAmount::PerMinute(v) = value {
                if v <= 0.0 {
                    return Err(PlanError::InvalidOutputAmount(item_name.clone()));
                }
            }

            outputs.insert(item_id, value);
        }

        let mut inputs = game_db.resource_limits.clone();
        for (item_name, value) in config.inputs {
            let item = game_db
                .find_item(&item_name)
                .ok_or_else(|| PlanError::UnknownItem(item_name.clone()))?;

            if value < 0.0 {
                return Err(PlanError::InvalidInputAmount(item_name.clone()));
            }

            inputs.insert(item, value);
        }

        for recipe in &config.recipes {
            if !game_db
                .recipes
                .iter()
                .any(|r| r.key.eq(recipe) || r.name.eq(recipe))
            {
                return Err(PlanError::UnknownRecipe(recipe.clone()));
            }
        }

        let enabled_recipes = game_db.filter_recipes(|r| {
            config.recipes.contains(&r.key) || config.recipes.contains(&r.name)
        });

        Ok(PlanConfig {
            inputs,
            outputs,
            game_db,
            enabled_recipes,
        })
    }

    pub fn find_recipes_by_output(&self, item: ItemId) -> Vec<RecipeId> {
        let is_blacklisted = self.game_db.by_product_blacklist.contains(&item);
        if is_blacklisted {
            self.enabled_recipes
                .iter()
                .filter(|r| self.game_db[**r].is_primary_output(item))
                .copied()
                .collect()
        } else {
            self.enabled_recipes
                .iter()
                .filter(|r| self.game_db[**r].has_output_item(item))
                .copied()
                .collect()
        }
    }

    #[inline]
    pub fn has_input(&self, item: ItemId) -> bool {
        self.find_input(item) > 0.0
    }

    #[inline]
    pub fn find_input(&self, item: ItemId) -> FloatType {
        self.inputs.get(&item).copied().unwrap_or(0.0)
    }

    #[inline]
    pub fn find_output(&self, item: ItemId) -> Option<OutputAmount> {
        self.outputs.get(&item).copied()
    }
}
