use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use crate::game::{GameDatabase, Item, Recipe};
use crate::utils::FloatType;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("No recipe exists with the name or key `{0}`")]
    UnknownRecipe(String),
    #[error("No item exists with the name or key `{0}`")]
    UnknownItem(String),
    #[error("The item `{0}` is an extractable resource and is not allowed in outputs.")]
    UnexpectedResource(String),
    #[error("The output for item `{0}` must be greater than zero.")]
    InvalidOutputAmount(String),
    #[error("The input for item `{0}` must be greater than or equal to zero.")]
    InvalidInputAmount(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputLimit {
    pub item: String,
    pub amount: FloatType,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProductionAmount {
    Maximize,
    PerMinute(FloatType),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanConfigDefinition {
    #[serde(default)]
    inputs: HashMap<String, FloatType>,
    outputs: HashMap<String, ProductionAmount>,
    recipes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub inputs: HashMap<Arc<Item>, FloatType>,
    pub outputs: HashMap<Arc<Item>, ProductionAmount>,
    pub game_db: Arc<GameDatabase>,
    pub enabled_recipes: Vec<Arc<Recipe>>,
}

#[allow(dead_code)]
impl ProductionAmount {
    pub fn is_maximize(&self) -> bool {
        matches!(self, Self::Maximize)
    }

    pub fn is_per_minute(&self) -> bool {
        matches!(self, Self::PerMinute(..))
    }

    pub fn as_per_minute(&self) -> FloatType {
        match self {
            ProductionAmount::PerMinute(a) => *a,
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
            let item = game_db
                .find_item(&item_name)
                .ok_or_else(|| PlanError::UnknownItem(item_name.clone()))?;
            if item.resource {
                return Err(PlanError::UnexpectedResource(item.name.clone()));
            }

            if let ProductionAmount::PerMinute(v) = value {
                if v <= 0.0 {
                    return Err(PlanError::InvalidOutputAmount(item_name.clone()));
                }
            }

            outputs.insert(item, value);
        }

        let mut inputs: HashMap<Arc<Item>, FloatType> = game_db.resource_limits.clone();
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

        let enabled_recipes = game_db
            .recipes
            .iter()
            .filter(|recipe| {
                config.recipes.contains(&recipe.key) || config.recipes.contains(&recipe.name)
            })
            .cloned()
            .collect();

        Ok(PlanConfig {
            inputs,
            outputs,
            game_db,
            enabled_recipes,
        })
    }

    pub fn find_recipes_by_output(&self, item: &Item) -> Vec<Arc<Recipe>> {
        if self
            .game_db
            .by_product_blacklist
            .iter()
            .any(|i| i.as_ref().eq(item))
        {
            self.enabled_recipes
                .iter()
                .filter(|r| r.is_primary_output(item))
                .cloned()
                .collect()
        } else {
            self.enabled_recipes
                .iter()
                .filter(|r| r.has_output_item(item))
                .cloned()
                .collect()
        }
    }

    pub fn has_input(&self, item: &Arc<Item>) -> bool {
        self.find_input(item) > 0.0
    }

    pub fn find_input(&self, item: &Arc<Item>) -> FloatType {
        self.inputs.get(item).copied().unwrap_or(0.0)
    }

    pub fn find_output(&self, item: &Arc<Item>) -> Option<ProductionAmount> {
        self.outputs.get(item).copied()
    }
}
