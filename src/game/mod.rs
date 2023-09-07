pub mod building;
pub mod item;
pub mod item_value_pairs;
pub mod recipe;

use recipe::RecipeDefinition;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs::File, ops::Index, path::Path};
use thiserror::Error;

use self::building::BuildingDefinition;
pub use building::{
    Building, Dimensions, Fuel, ItemProducer, PowerConsumption, PowerGenerator, ResourceExtractor,
    ResourceWell,
};
pub use item::{Item, ItemState};
pub use item_value_pairs::{ItemKeyAmountPair, ItemPerMinute};
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
    #[error("Recipe `{0}: Building `{1}` is not a manufacturer.")]
    NotAManufacturer(String, String),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ItemId(usize);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RecipeId(usize);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BuildingId(usize);

#[derive(Debug, Serialize, Deserialize)]
pub struct GameDatabaseDefinition {
    by_product_blacklist: Vec<String>,
    items: Vec<Item>,
    buildings: Vec<BuildingDefinition>,
    recipes: Vec<RecipeDefinition>,
    resource_limits: HashMap<String, FloatType>,
}

#[derive(Debug, Clone)]
pub struct GameDatabase {
    pub by_product_blacklist: Vec<ItemId>,
    pub items: Vec<Item>,
    pub buildings: Vec<Building>,
    pub recipes: Vec<Recipe>,
    pub resource_limits: HashMap<ItemId, FloatType>,
}

#[allow(dead_code)]
impl GameDatabase {
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> Result<GameDatabase, anyhow::Error> {
        let file = File::open(file_path)?;
        let config: GameDatabaseDefinition = serde_json::from_reader(file)?;

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
            let item_id = Self::find_item_by_key(item_key, &definition.items)?;
            let item = &definition.items[item_id.0];

            if !item.resource {
                return Err(GameDatabaseError::ItemNotAResource(item.key.clone()));
            }

            resource_limits.insert(item_id, *limit);
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
            if recipes.iter().any(|r: &Recipe| r.key == recipe.key) {
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
        items: &[Item],
    ) -> Result<Building, GameDatabaseError> {
        Ok(match building {
            BuildingDefinition::Manufacturer(m) => Building::Manufacturer(m),
            BuildingDefinition::PowerGenerator(pg) => {
                let mut fuels = Vec::new();
                for fuel in pg.fuels {
                    let cycles_per_min = 60.0 / fuel.burn_time_secs;
                    fuels.push(Fuel {
                        fuel: Self::convert_item_amount(&fuel.fuel, cycles_per_min, items)?,
                        supplemental: fuel
                            .supplemental
                            .map(|i| Self::convert_item_amount(&i, cycles_per_min, items))
                            .transpose()?,
                        by_product: fuel
                            .by_product
                            .map(|i| Self::convert_item_amount(&i, cycles_per_min, items))
                            .transpose()?,
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
                    extractor_type: re.extractor_type,
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
                    output: Self::convert_item_amount(&ip.output, crafts_per_min, items)?,
                    power_consumption: ip.power_consumption,
                    dimensions: ip.dimensions,
                })
            }
            BuildingDefinition::ResourceWell(rw) => {
                let mut allowed_resources = Vec::new();
                for allowed_resource in rw.allowed_resources {
                    allowed_resources.push(Self::find_item_by_key(&allowed_resource, items)?);
                }
                Building::ResourceWell(ResourceWell {
                    key: rw.key,
                    name: rw.name,
                    allowed_resources,
                    satellite_buildings: rw.satellite_buildings,
                    extractor_type: rw.extractor_type,
                    power_consumption: rw.power_consumption,
                    dimensions: rw.dimensions,
                })
            }
        })
    }

    fn convert_recipe(
        recipe: RecipeDefinition,
        buildings: &[Building],
        items: &[Item],
    ) -> Result<Recipe, GameDatabaseError> {
        let building_id = Self::find_building_by_key(&recipe.building, buildings)?;
        let building = &buildings[building_id.0];

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
        let inputs = recipe
            .inputs
            .iter()
            .map(|i| Self::convert_item_amount(i, crafts_per_min, items))
            .collect::<Result<Vec<ItemPerMinute>, GameDatabaseError>>()?;

        let outputs = recipe
            .outputs
            .iter()
            .map(|o| Self::convert_item_amount(o, crafts_per_min, items))
            .collect::<Result<Vec<ItemPerMinute>, GameDatabaseError>>()?;

        Ok(Recipe {
            key: recipe.key,
            name: recipe.name,
            alternate: recipe.alternate,
            inputs,
            outputs,
            craft_time_secs: recipe.craft_time_secs,
            events: recipe.events,
            building: building_id,
            power: recipe.power,
        })
    }

    pub fn convert_item_amount(
        item_amount: &ItemKeyAmountPair,
        cycles_per_min: FloatType,
        items: &[Item],
    ) -> Result<ItemPerMinute, GameDatabaseError> {
        Ok(ItemPerMinute::new(
            Self::find_item_by_key(&item_amount.item, items)?,
            item_amount.amount * cycles_per_min,
        ))
    }

    #[inline]
    fn find_item_by_key(item_key: &str, items: &[Item]) -> Result<ItemId, GameDatabaseError> {
        items
            .iter()
            .position(|i| i.key == item_key)
            .map(ItemId)
            .ok_or(GameDatabaseError::UnknownItemKey(item_key.into()))
    }

    #[inline]
    fn find_building_by_key(
        building_key: &str,
        buildings: &[Building],
    ) -> Result<BuildingId, GameDatabaseError> {
        buildings
            .iter()
            .position(|b| b.key() == building_key)
            .map(BuildingId)
            .ok_or(GameDatabaseError::UnknownBuildingKey(building_key.into()))
    }

    #[inline]
    pub fn find_recipe(&self, name_or_key: &str) -> Option<&Recipe> {
        self.recipes
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name_or_key) || r.key == name_or_key)
    }

    #[inline]
    pub fn find_item(&self, name_or_key: &str) -> Option<ItemId> {
        self.items
            .iter()
            .position(|i| i.name.eq_ignore_ascii_case(name_or_key) || i.key == name_or_key)
            .map(ItemId)
    }

    #[inline]
    pub fn find_item_producers(&self, item: ItemId) -> Vec<BuildingId> {
        self.buildings
            .iter()
            .enumerate()
            .filter(|(_, b)| b.is_item_producer() && b.as_item_producer().output.item == item)
            .map(|(i, _)| BuildingId(i))
            .collect()
    }

    pub fn filter_recipes<F>(&self, predicate: F) -> Vec<RecipeId>
    where
        F: Fn(&&Recipe) -> bool,
    {
        self.recipes
            .iter()
            .enumerate()
            .filter(|(_, r)| predicate(r))
            .map(|(i, _)| RecipeId(i))
            .collect()
    }

    #[inline]
    pub fn get_resource_limit(&self, item: ItemId) -> FloatType {
        self.resource_limits.get(&item).copied().unwrap_or(0.0)
    }
}

impl Index<ItemId> for GameDatabase {
    type Output = Item;

    fn index(&self, index: ItemId) -> &Self::Output {
        &self.items[index.0]
    }
}

impl Index<BuildingId> for GameDatabase {
    type Output = Building;

    fn index(&self, index: BuildingId) -> &Self::Output {
        &self.buildings[index.0]
    }
}

impl Index<RecipeId> for GameDatabase {
    type Output = Recipe;

    fn index(&self, index: RecipeId) -> &Self::Output {
        &self.recipes[index.0]
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
}
