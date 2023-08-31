pub mod building;
pub mod item;
pub mod item_value_pair;
pub mod recipe;

use indexmap::IndexMap;
use recipe::RecipeDefinition;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, path::Path, rc::Rc};
use thiserror::Error;

pub use building::{Building, Dimensions, PowerConsumption};
pub use item::{Item, ItemState};
pub use item_value_pair::ItemValuePair;
pub use recipe::Recipe;

use crate::utils::FloatType;

use self::building::{BuildingDefinition, Fuel, ItemProducer, PowerGenerator, ResourceExtractor};

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
    #[error("Recipe `{0}: Building `{1}` is not a manufacturer.")]
    NotAManufacturer(String, String),
}

#[derive(Debug, Serialize, Deserialize)]
struct GameDatabaseDefinition {
    by_product_blacklist: Vec<String>,
    items: Vec<Rc<Item>>,
    buildings: Vec<BuildingDefinition>,
    recipes: Vec<RecipeDefinition>,
    resource_limits: HashMap<String, FloatType>,
}

#[derive(Debug, Clone)]
pub struct GameDatabase {
    pub by_product_blacklist: Vec<Rc<Item>>,
    pub items: Vec<Rc<Item>>,
    pub buildings: Vec<Rc<Building>>,
    pub recipes: Vec<Rc<Recipe>>,
    pub resource_limits: HashMap<Rc<Item>, FloatType>,
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

        let mut buildings = Vec::new();
        for building_definition in definition.buildings {
            buildings.push(Self::convert_building(
                building_definition,
                &definition.items,
            )?);
        }

        let mut recipes = Vec::with_capacity(definition.recipes.len());
        for recipe in definition.recipes {
            if recipes.iter().any(|r: &Rc<Recipe>| r.key == recipe.key) {
                return Err(GameDatabaseError::DuplicateRecipeKey(recipe.key.clone()));
            }

            recipes.push(Self::convert_recipe(recipe, &buildings, &definition.items)?);
        }

        Ok(Self {
            by_product_blacklist,
            items: definition.items,
            buildings,
            recipes,
            resource_limits,
        })
    }

    fn convert_building(
        building: BuildingDefinition,
        items: &[Rc<Item>],
    ) -> Result<Rc<Building>, GameDatabaseError> {
        Ok(Rc::new(match building {
            BuildingDefinition::Manufacturer(m) => Building::Manufacturer(m),
            BuildingDefinition::PowerGenerator(pg) => {
                let mut fuels = Vec::new();
                for fuel in pg.fuels {
                    let cycles_per_min = 60.0 / fuel.burn_time_secs;
                    fuels.push(Fuel {
                        inputs: Self::convert_item_amounts(fuel.inputs, cycles_per_min, items)?,
                        outputs: Self::convert_item_amounts(fuel.outputs, cycles_per_min, items)?,
                        burn_time_secs: fuel.burn_time_secs,
                    });
                }

                Building::PowerGenerator(PowerGenerator {
                    key: pg.key,
                    name: pg.name,
                    power_production_mw: pg.power_production_mw,
                    fuels,
                    dimensions: pg.dimensions,
                })
            }
            BuildingDefinition::ResourceExtractor(re) => {
                let mut allowed_resources = Vec::new();
                for allowed_resource in re.allowed_resources {
                    allowed_resources.push(Self::find_item_by_key(&allowed_resource, items)?);
                }

                Building::ResourceExtractor(ResourceExtractor {
                    key: re.key,
                    name: re.name,
                    extraction_rate: re.extraction_rate,
                    allowed_resources,
                    power_consumption: re.power_consumption,
                    dimensions: re.dimensions,
                })
            }
            BuildingDefinition::ItemProducer(ip) => {
                let crafts_per_min = 60.0 / ip.craft_time_secs;
                Building::ItemProducer(ItemProducer {
                    key: ip.key,
                    name: ip.name,
                    craft_time_secs: ip.craft_time_secs,
                    outputs: Self::convert_item_amounts(ip.outputs, crafts_per_min, items)?,
                    power_consumption: ip.power_consumption,
                    dimensions: ip.dimensions,
                })
            }
        }))
    }

    fn convert_recipe(
        recipe: RecipeDefinition,
        buildings: &[Rc<Building>],
        items: &[Rc<Item>],
    ) -> Result<Rc<Recipe>, GameDatabaseError> {
        let building = Self::find_building_in_slice(&recipe.building, buildings)?;

        if !building.is_manufacturer() {
            return Err(GameDatabaseError::NotAManufacturer(
                recipe.name.clone(),
                recipe.building.clone(),
            ));
        }

        if recipe.inputs.is_empty() {
            return Err(GameDatabaseError::MissingRecipeInputs(recipe.key.clone()));
        }

        if recipe.outputs.is_empty() {
            return Err(GameDatabaseError::MissingRecipeOutputs(recipe.key.clone()));
        }

        let crafts_per_min = 60.0 / recipe.craft_time_secs;
        Ok(Rc::new(Recipe {
            key: recipe.key,
            name: recipe.name,
            alternate: recipe.alternate,
            inputs: Self::convert_item_amounts(recipe.inputs, crafts_per_min, items)?,
            outputs: Self::convert_item_amounts(recipe.outputs, crafts_per_min, items)?,
            craft_time_secs: recipe.craft_time_secs,
            events: recipe.events,
            building,
            power: recipe.power,
        }))
    }

    pub fn convert_item_amounts(
        item_amounts: IndexMap<String, FloatType>,
        cycles_per_min: FloatType,
        items: &[Rc<Item>],
    ) -> Result<Vec<ItemValuePair>, GameDatabaseError> {
        let mut item_value_pairs = Vec::new();
        for (item_key, amount) in item_amounts {
            item_value_pairs.push(ItemValuePair::new(
                Self::find_item_by_key(&item_key, items)?,
                amount * cycles_per_min,
            ));
        }

        Ok(item_value_pairs)
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
            .find(|b| b.key() == building_key)
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
