use super::{item_value_pairs::ItemKeyAmountPair, BuildingId, GameDatabase, ItemId, ItemPerMinute};
use crate::utils::FloatType;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Default, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecipePower {
    pub min_mw: FloatType,
    pub max_mw: FloatType,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct RecipeDefinition {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub alternate: bool,
    pub outputs: Vec<ItemKeyAmountPair>,
    pub inputs: Vec<ItemKeyAmountPair>,
    pub craft_time_secs: FloatType,
    #[serde(default)]
    pub events: Vec<String>,
    pub building: String,
    #[serde(default)]
    pub power: RecipePower,
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub key: String,
    pub name: String,
    pub alternate: bool,
    pub outputs: Vec<ItemPerMinute>,
    pub inputs: Vec<ItemPerMinute>,
    pub craft_time_secs: FloatType,
    pub events: Vec<String>,
    pub building: BuildingId,
    pub power: RecipePower,
}

impl Recipe {
    #[inline]
    pub fn average_mw(&self, game_db: &GameDatabase, clock_speed: FloatType) -> FloatType {
        game_db[self.building]
            .as_manufacturer()
            .power_consumption
            .average_mw_overclocked(self, clock_speed)
    }

    #[inline]
    pub fn find_input_by_item(&self, item: ItemId) -> Option<&ItemPerMinute> {
        self.inputs.iter().find(|output| output.item == item)
    }

    #[inline]
    pub fn find_output_by_item(&self, item: ItemId) -> Option<&ItemPerMinute> {
        self.outputs.iter().find(|output| output.item == item)
    }

    #[inline]
    pub fn has_output_item(&self, item: ItemId) -> bool {
        self.outputs.iter().any(|output| output.item == item)
    }

    #[inline]
    pub fn is_primary_output(&self, item: ItemId) -> bool {
        self.outputs
            .first()
            .map(|o| o.item == item)
            .unwrap_or(false)
    }

    #[inline]
    pub fn has_input_item(&self, item: ItemId) -> bool {
        self.inputs.iter().any(|input| input.item == item)
    }
}

impl Hash for Recipe {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Recipe {}
