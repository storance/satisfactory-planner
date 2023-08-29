pub mod building;
pub mod item;
pub mod item_value_pair;
pub mod recipe;

use recipe::RecipeDefinition;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, path::Path, rc::Rc};
use thiserror::Error;

pub use building::{Building, Dimensions, PowerConsumption};
pub use item::{Item, ItemState};
pub use item_value_pair::ItemValuePair;
pub use recipe::Recipe;

use crate::utils::FloatType;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum GameDatabaseError {
    #[error("Recipe `{0}`: At least one input is required but none were provided")]
    MissingRecipeInputs(String),
    #[error("Recipe `{0}`: At least one output is required but none were provided")]
    MissingRecipeOutputs(String),
    #[error("Recipe `{0}`: Multiple recipes with the same key found.")]
    DuplicateRecipeKey(String),
    #[error("Item `{0}` is not a resource and can't appear in resource_limits.")]
    ItemNotAResource(String),
    #[error("Item `{0}`: No such item exists.")]
    UnknownItemKey(String),
    #[error("Building `{0}`: No such building exists.")]
    UnknownBuildingKey(String),
}

#[derive(Debug, Serialize, Deserialize)]
struct GameDatabaseDefinition {
    by_product_blacklist: Vec<String>,
    items: Vec<Rc<Item>>,
    buildings: Vec<Rc<Building>>,
    recipes: Vec<RecipeDefinition>,
    resource_limits: HashMap<String, f32>,
}

#[derive(Debug, Clone)]
pub struct GameDatabase {
    pub by_product_blacklist: Vec<Rc<Item>>,
    pub items: Vec<Rc<Item>>,
    pub buildings: Vec<Rc<Building>>,
    pub recipes: Vec<Rc<Recipe>>,
    pub resource_limits: HashMap<Rc<Item>, f32>,
}

#[allow(dead_code)]
impl GameDatabase {
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> Result<GameDatabase, anyhow::Error> {
        let file = File::open(file_path)?;
        let config: GameDatabaseDefinition = serde_yaml::from_reader(file)?;

        Ok(Self::convert(config)?)
    }

    fn convert(definition: GameDatabaseDefinition) -> Result<GameDatabase, GameDatabaseError> {
        // validate the items in by_product_blacklist
        let mut by_product_blacklist = Vec::new();
        for item_key in &definition.by_product_blacklist {
            let item = Self::find_item_by_key(item_key, &definition.items)?;
            by_product_blacklist.push(item);
        }

        // validate the items in resource_limits
        let mut resource_limits = HashMap::new();
        for (item_key, limit) in &definition.resource_limits {
            let item = Self::find_item_by_key(item_key, &definition.items)?;
            if !item.resource {
                return Err(GameDatabaseError::ItemNotAResource(item.key.clone()));
            }

            resource_limits.insert(item, *limit);
        }

        let mut recipes = Vec::with_capacity(definition.recipes.len());
        for recipe_definition in definition.recipes {
            let building =
                Self::find_building_in_slice(&recipe_definition.building, &definition.buildings)?;

            if recipe_definition.inputs.is_empty() {
                return Err(GameDatabaseError::MissingRecipeInputs(
                    recipe_definition.key.clone(),
                ));
            }

            if recipe_definition.outputs.is_empty() {
                return Err(GameDatabaseError::MissingRecipeOutputs(
                    recipe_definition.key.clone(),
                ));
            }

            if recipes
                .iter()
                .any(|r: &Rc<Recipe>| r.key == recipe_definition.key)
            {
                return Err(GameDatabaseError::DuplicateRecipeKey(
                    recipe_definition.key.clone(),
                ));
            }

            let crafts_per_minutes = 60.0 / recipe_definition.craft_time_secs;
            let mut inputs = Vec::new();
            for (item_key, amount) in &recipe_definition.inputs {
                inputs.push(ItemValuePair::new(
                    Self::find_item_by_key(item_key, &definition.items)?,
                    *amount * crafts_per_minutes,
                ));
            }

            let mut outputs = Vec::new();
            for (item_key, amount) in &recipe_definition.outputs {
                outputs.push(ItemValuePair::new(
                    Self::find_item_by_key(item_key, &definition.items)?,
                    *amount * crafts_per_minutes,
                ));
            }

            recipes.push(Rc::new(Recipe {
                key: recipe_definition.key,
                name: recipe_definition.name,
                alternate: recipe_definition.alternate,
                outputs,
                inputs,
                craft_time_secs: recipe_definition.craft_time_secs,
                events: recipe_definition.events,
                building,
                power: recipe_definition.power,
            }));
        }

        Ok(Self {
            by_product_blacklist,
            items: definition.items,
            buildings: definition.buildings,
            recipes,
            resource_limits,
        })
    }

    pub fn filter<F>(&self, predicate: F) -> Self
    where
        F: Fn(&Recipe) -> bool,
    {
        Self {
            by_product_blacklist: self.by_product_blacklist.clone(),
            items: self.items.clone(),
            buildings: self.buildings.clone(),
            recipes: self
                .recipes
                .iter()
                .filter(|r| predicate(r.as_ref()))
                .cloned()
                .collect(),
            resource_limits: self.resource_limits.clone(),
        }
    }

    pub fn find_recipe(&self, name_or_key: &str) -> Option<Rc<Recipe>> {
        self.recipes
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name_or_key) || r.key == name_or_key)
            .cloned()
    }

    pub fn find_item(&self, name_or_key: &str) -> Option<Rc<Item>> {
        self.items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name_or_key) || i.key == name_or_key)
            .cloned()
    }

    #[inline]
    fn find_item_by_key(item_key: &str, items: &[Rc<Item>]) -> Result<Rc<Item>, GameDatabaseError> {
        items
            .iter()
            .find(|i| i.key == item_key)
            .cloned()
            .ok_or(GameDatabaseError::UnknownItemKey(item_key.into()))
    }

    #[inline]
    fn find_building_in_slice(
        building_key: &str,
        buildings: &[Rc<Building>],
    ) -> Result<Rc<Building>, GameDatabaseError> {
        buildings
            .iter()
            .find(|b| b.key == building_key)
            .cloned()
            .ok_or(GameDatabaseError::UnknownBuildingKey(building_key.into()))
    }

    #[inline]
    pub fn find_recipes_by_output(&self, item: &Item) -> Vec<Rc<Recipe>> {
        self.recipes
            .iter()
            .filter(|r| r.has_output_item(item))
            .cloned()
            .collect()
    }

    pub fn get_resource_limit(&self, item: &Rc<Item>) -> FloatType {
        self.resource_limits.get(item).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
pub mod test {
    use std::path::PathBuf;

    use super::GameDatabase;

    pub fn get_test_game_db() -> GameDatabase {
        let mut game_db_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        game_db_path.push("game-db.json");

        GameDatabase::from_file(game_db_path.as_path()).expect("Failed to load game-db.json")
    }

    pub fn get_test_game_db_with_recipes(recipe_keys: &[&str]) -> GameDatabase {
        get_test_game_db().filter(|r| recipe_keys.contains(&r.key.as_str()))
    }
}
