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
pub use item_value_pair::ItemPerMinute;
pub use recipe::Recipe;

use crate::utils::FloatType;

use self::{
    building::{
        BuildingDefinition, Fuel, ItemProducer, PowerGenerator, ResourceExtractor, ResourceWell,
    },
    item_value_pair::ItemAmountDefinition,
};

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
        }))
    }

    fn convert_recipe(
        recipe: RecipeDefinition,
        buildings: &[Rc<Building>],
        items: &[Rc<Item>],
    ) -> Result<Rc<Recipe>, GameDatabaseError> {
        let building = Self::find_building_by_key(&recipe.building, buildings)?;

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

        Ok(Rc::new(Recipe {
            key: recipe.key,
            name: recipe.name,
            alternate: recipe.alternate,
            inputs,
            outputs,
            craft_time_secs: recipe.craft_time_secs,
            events: recipe.events,
            building,
            power: recipe.power,
        }))
    }

    pub fn convert_item_amount(
        item_amount: &ItemAmountDefinition,
        cycles_per_min: FloatType,
        items: &[Rc<Item>],
    ) -> Result<ItemPerMinute, GameDatabaseError> {
        Ok(ItemPerMinute::new(
            Self::find_item_by_key(&item_amount.item, items)?,
            item_amount.amount * cycles_per_min,
        ))
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
    fn find_building_by_key(
        building_key: &str,
        buildings: &[Rc<Building>],
    ) -> Result<Rc<Building>, GameDatabaseError> {
        buildings
            .iter()
            .find(|b| b.key() == building_key)
            .cloned()
            .ok_or(GameDatabaseError::UnknownBuildingKey(building_key.into()))
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

    #[inline]
    pub fn find_recipe(&self, name_or_key: &str) -> Option<Rc<Recipe>> {
        self.recipes
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name_or_key) || r.key == name_or_key)
            .cloned()
    }

    #[inline]
    pub fn find_item(&self, name_or_key: &str) -> Option<Rc<Item>> {
        self.items
            .iter()
            .find(|i| i.name.eq_ignore_ascii_case(name_or_key) || i.key == name_or_key)
            .cloned()
    }

    #[inline]
    pub fn find_building(&self, name_or_key: &str) -> Option<Rc<Building>> {
        self.buildings
            .iter()
            .find(|b| b.name().eq_ignore_ascii_case(name_or_key) || b.key() == name_or_key)
            .cloned()
    }

    pub fn find_recipes_by_output(&self, item: &Item) -> Vec<Rc<Recipe>> {
        if self
            .by_product_blacklist
            .iter()
            .any(|i| i.as_ref().eq(item))
        {
            self.recipes
                .iter()
                .filter(|r| r.is_primary_output(item))
                .cloned()
                .collect()
        } else {
            self.recipes
                .iter()
                .filter(|r| r.has_output_item(item))
                .cloned()
                .collect()
        }
    }

    #[inline]
    pub fn find_item_producers(&self, item: &Item) -> Vec<Rc<Building>> {
        self.buildings
            .iter()
            .filter(|b| b.is_item_producer() && *b.as_item_producer().output.item == *item)
            .cloned()
            .collect()
    }

    #[inline]
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

    pub fn get_game_db_with_base_recipes_plus(recipe_keys: &[&str]) -> GameDatabase {
        get_test_game_db().filter(|r| !r.alternate || recipe_keys.contains(&r.key.as_str()))
    }
}
