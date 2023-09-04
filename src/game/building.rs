use core::panic;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    hash::{Hash, Hasher},
    rc::Rc,
};

use crate::utils::FloatType;

use super::{item_value_pair::ItemAmountDefinition, Item, ItemPerMinute, Recipe};

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PowerConsumption {
    #[serde(rename = "fixed")]
    Fixed { value_mw: u32, exponent: FloatType },
    #[serde(rename = "variable")]
    Variable {
        min_mw: u32,
        max_mw: u32,
        exponent: FloatType,
    },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dimensions {
    pub length_m: FloatType,
    pub width_m: FloatType,
    pub height_m: FloatType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manufacturer {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    #[serde(default)]
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct FuelDefinition {
    pub fuel: ItemAmountDefinition,
    #[serde(default)]
    pub supplemental: Option<ItemAmountDefinition>,
    #[serde(default)]
    pub by_product: Option<ItemAmountDefinition>,
    pub burn_time_secs: FloatType,
}

#[derive(Debug, Clone)]
pub struct Fuel {
    pub fuel: ItemPerMinute,
    pub supplemental: Option<ItemPerMinute>,
    pub by_product: Option<ItemPerMinute>,
    pub burn_time_secs: FloatType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct PowerGeneratorDefinition {
    pub key: String,
    pub name: String,
    pub power_production_mw: u32,
    pub fuels: Vec<FuelDefinition>,
    #[serde(default)]
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone)]
pub struct PowerGenerator {
    pub key: String,
    pub name: String,
    pub power_production_mw: u32,
    pub fuels: Vec<Fuel>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ResourceExtractorDefinition {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub extraction_rate: FloatType,
    pub allowed_resources: Vec<String>,
    pub extractor_type: Option<String>,
    #[serde(default)]
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone)]
pub struct ResourceExtractor {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub extraction_rate: FloatType,
    pub allowed_resources: Vec<Rc<Item>>,
    pub extractor_type: Option<String>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceWellExtractor {
    pub key: String,
    pub name: String,
    pub extraction_rate: FloatType,
    pub power_consumption: PowerConsumption,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceWellDefinition {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub allowed_resources: Vec<String>,
    pub satellite_buildings: Vec<ResourceWellExtractor>,
    pub extractor_type: Option<String>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone)]
pub struct ResourceWell {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub allowed_resources: Vec<Rc<Item>>,
    pub satellite_buildings: Vec<ResourceWellExtractor>,
    pub extractor_type: Option<String>,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProducerDefinition {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub craft_time_secs: FloatType,
    pub output: ItemAmountDefinition,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone)]
pub struct ItemProducer {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    pub craft_time_secs: FloatType,
    pub output: ItemPerMinute,
    pub dimensions: Option<Dimensions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(super) enum BuildingDefinition {
    #[serde(rename = "manufacturer")]
    Manufacturer(Manufacturer),
    #[serde(rename = "power_generator")]
    PowerGenerator(PowerGeneratorDefinition),
    #[serde(rename = "resource_extractor")]
    ResourceExtractor(ResourceExtractorDefinition),
    #[serde(rename = "resource_well")]
    ResourceWell(ResourceWellDefinition),
    #[serde(rename = "item_producer")]
    ItemProducer(ItemProducerDefinition),
}

#[derive(Debug, Clone)]
pub enum Building {
    Manufacturer(Manufacturer),
    PowerGenerator(PowerGenerator),
    ResourceExtractor(ResourceExtractor),
    ItemProducer(ItemProducer),
    ResourceWell(ResourceWell),
}

#[allow(dead_code)]
impl Building {
    pub fn key(&self) -> &str {
        match self {
            Self::Manufacturer(m) => &m.key,
            Self::PowerGenerator(pg) => &pg.key,
            Self::ResourceExtractor(re) => &re.key,
            Self::ItemProducer(ip) => &ip.key,
            Self::ResourceWell(rw) => &rw.key,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Manufacturer(m) => &m.name,
            Self::PowerGenerator(pg) => &pg.name,
            Self::ResourceExtractor(re) => &re.name,
            Self::ItemProducer(ip) => &ip.name,
            Self::ResourceWell(rw) => &rw.name,
        }
    }

    pub fn dimensions(&self) -> Option<&Dimensions> {
        match self {
            Self::Manufacturer(m) => m.dimensions.as_ref(),
            Self::PowerGenerator(pg) => pg.dimensions.as_ref(),
            Self::ResourceExtractor(re) => re.dimensions.as_ref(),
            Self::ItemProducer(ip) => ip.dimensions.as_ref(),
            Self::ResourceWell(rw) => rw.dimensions.as_ref(),
        }
    }

    pub fn volume(&self) -> FloatType {
        self.dimensions().map(|d| d.volume()).unwrap_or(0.0)
    }

    pub fn floor_area(&self) -> FloatType {
        self.dimensions().map(|d| d.floor_area()).unwrap_or(0.0)
    }

    pub fn is_manufacturer(&self) -> bool {
        matches!(self, Self::Manufacturer(..))
    }

    pub fn as_manufacturer(&self) -> &Manufacturer {
        match self {
            Self::Manufacturer(m) => m,
            _ => {
                panic!("Building is not a Manufacturer")
            }
        }
    }

    pub fn is_power_generator(&self) -> bool {
        matches!(self, Self::Manufacturer(..))
    }

    pub fn as_power_generator(&self) -> &PowerGenerator {
        match self {
            Self::PowerGenerator(pg) => pg,
            _ => {
                panic!("Building is not a PowerGenerator")
            }
        }
    }

    pub fn is_resource_extractor(&self) -> bool {
        matches!(self, Self::Manufacturer(..))
    }

    pub fn as_resource_extractor(&self) -> &ResourceExtractor {
        match self {
            Self::ResourceExtractor(re) => re,
            _ => {
                panic!("Building is not a ResourceExtractor")
            }
        }
    }

    pub fn is_item_producer(&self) -> bool {
        matches!(self, Self::ItemProducer(..))
    }

    pub fn as_item_producer(&self) -> &ItemProducer {
        match self {
            Self::ItemProducer(ip) => ip,
            _ => {
                panic!("Building is not a ItemProducer")
            }
        }
    }

    pub fn is_resource_well(&self) -> bool {
        matches!(self, Self::ResourceWell(..))
    }

    pub fn as_resource_well(&self) -> &ResourceWell {
        match self {
            Self::ResourceWell(rw) => rw,
            _ => {
                panic!("Building is not a ResourceWell")
            }
        }
    }
}

impl fmt::Display for Building {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl Hash for Building {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

impl PartialEq for Building {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for Building {}

#[allow(dead_code)]
impl Dimensions {
    pub fn volume(&self) -> FloatType {
        self.length_m * self.width_m * self.height_m
    }

    pub fn floor_area(&self) -> FloatType {
        self.length_m * self.width_m
    }
}

impl fmt::Display for Dimensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}m x {}m x {}m",
            self.length_m, self.width_m, self.height_m
        )
    }
}

#[allow(dead_code)]
impl PowerConsumption {
    pub fn average_mw_overclocked(&self, recipe: &Recipe, clock_speed: FloatType) -> FloatType {
        match self {
            Self::Fixed { value_mw, exponent } => {
                *value_mw as FloatType * (clock_speed / 100.0).powf(*exponent)
            }
            Self::Variable { exponent, .. } => {
                let avg_power = (recipe.power.max_mw - recipe.power.min_mw) / 2.0;
                avg_power * (clock_speed / 100.0).powf(*exponent)
            }
        }
    }

    #[inline]
    pub fn average_mw(&self, recipe: &Recipe) -> FloatType {
        self.average_mw_overclocked(recipe, 100.0)
    }
}

impl fmt::Display for PowerConsumption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed { value_mw, .. } => write!(f, "{} MW", value_mw),
            Self::Variable { min_mw, max_mw, .. } => write!(f, "{} - {} MW)", min_mw, max_mw),
        }
    }
}
