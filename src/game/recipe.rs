use super::{item_value_pair::ItemAmountDefinition, Building, Item, ItemPerMinute};
use crate::utils::FloatType;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

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
    pub outputs: Vec<ItemAmountDefinition>,
    pub inputs: Vec<ItemAmountDefinition>,
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
    pub building: Rc<Building>,
    pub power: RecipePower,
}

#[allow(dead_code)]
impl Recipe {
    pub fn average_mw(&self, clock_speed: FloatType) -> FloatType {
        self.building
            .as_manufacturer()
            .power_consumption
            .average_mw_overclocked(self, clock_speed)
    }

    #[inline]
    pub fn find_input_by_item(&self, item: &Item) -> Option<&ItemPerMinute> {
        self.inputs.iter().find(|output| *output.item == *item)
    }

    #[inline]
    pub fn find_output_by_item(&self, item: &Item) -> Option<&ItemPerMinute> {
        self.outputs.iter().find(|output| *output.item == *item)
    }

    #[inline]
    pub fn has_output_item(&self, item: &Item) -> bool {
        self.outputs.iter().any(|output| *output.item == *item)
    }

    pub fn is_primary_output(&self, item: &Item) -> bool {
        self.outputs
            .first()
            .map(|o| *o.item == *item)
            .unwrap_or(false)
    }

    #[inline]
    pub fn has_input_item(&self, item: &Item) -> bool {
        self.inputs.iter().any(|input| *input.item == *item)
    }

    #[inline]
    pub fn calc_buildings_for_output(&self, output: &ItemPerMinute) -> Option<FloatType> {
        self.find_output_by_item(&output.item)
            .map(|ro| output.ratio(ro))
    }

    #[inline]
    pub fn calc_buildings_for_input(&self, input: &ItemPerMinute) -> Option<FloatType> {
        self.find_input_by_item(&input.item)
            .map(|ri| input.ratio(ri))
    }
}

impl fmt::Display for Recipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.alternate {
            write!(f, "Alternate: {}", self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl Hash for Recipe {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl PartialEq for Recipe {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Recipe {}
