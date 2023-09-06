use serde::{Deserialize,  Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use thiserror::Error;

use crate::game::{GameDatabase, Item};
use crate::utils::FloatType;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum PlanError {
    #[error("No recipe exists with the name or key `{0}`")]
    UnknownRecipe(String),
    #[error("No item exists with the name or key `{0}`")]
    UnknownItem(String),
    #[error("The resource `{0}` is not allowed in outputs.")]
    UnexpectedResource(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputLimit{
    pub item: String,
    pub amount: FloatType,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ProductionAmount {
    Maximize,
    PerMinute(FloatType)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlanConfigDefinition {
    #[serde(default)]
    inputs: HashMap<String, FloatType>,
    production: HashMap<String, ProductionAmount>,
    recipes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub inputs: HashMap<Rc<Item>, FloatType>,
    pub production: HashMap<Rc<Item>, ProductionAmount>,
    pub game_db: GameDatabase,
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
            _ => panic!("ProductionAmount was not PerMinute")
        }
    }
}

#[allow(dead_code)]
impl PlanConfig {
    pub fn new(production: HashMap<Rc<Item>, ProductionAmount>, game_db: GameDatabase) -> Self {
        PlanConfig {
            inputs: game_db.resource_limits.clone(),
            production,
            game_db,
        }
    }

    pub fn with_inputs(
        inputs: HashMap<Rc<Item>, FloatType>,
        production: HashMap<Rc<Item>, ProductionAmount>,
        game_db: GameDatabase,
    ) -> Self {
        let mut all_inputs = game_db.resource_limits.clone();
        all_inputs.extend(inputs);

        PlanConfig {
            inputs: all_inputs,
            production,
            game_db,
        }
    }

    pub fn from(config: PlanConfigDefinition, game_db: &GameDatabase) -> Result<Self, PlanError> {
        // validate there are no extractable resources in the outputs list
        let mut production = HashMap::new();
        for (item_name, value) in config.production {
            let item = game_db
                .find_item(&item_name)
                .ok_or(PlanError::UnknownItem(item_name))?;
            if item.resource {
                return Err(PlanError::UnexpectedResource(item.name.clone()));
            }

            production.insert(item, value);
        }

        let mut inputs: HashMap<Rc<Item>, FloatType> = game_db.resource_limits.clone();
        for (item_name, value) in config.inputs {
            let item = game_db
                .find_item(&item_name)
                .ok_or(PlanError::UnknownItem(item_name))?;

            inputs.insert(item, value);
        }

        for recipe in &config.recipes {
            if !game_db.recipes.iter().any(|r| r.key.eq(recipe) || r.name.eq(recipe)) {
                return Err(PlanError::UnknownRecipe(recipe.clone()));
            }
        }

        Ok(PlanConfig {
            inputs,
            production,
            game_db: game_db.filter(|recipe| {
                config.recipes.contains(&recipe.key) || config.recipes.contains(&recipe.name)
            }),
        })
    }

    pub fn has_input(&self, item: &Rc<Item>) -> bool {
        self.find_input(item) > 0.0
    }

    pub fn find_input(&self, item: &Rc<Item>) -> FloatType {
        self.inputs.get(item).copied().unwrap_or(0.0)
    }

    pub fn find_output(&self, item: &Rc<Item>) -> Option<ProductionAmount> {
        self.production
            .get(item)
            .copied()
    }
}