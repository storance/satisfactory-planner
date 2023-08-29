use std::{
    fmt,
    hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::utils::FloatType;

use super::Recipe;

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
pub struct Building {
    pub key: String,
    pub name: String,
    pub power_consumption: PowerConsumption,
    #[serde(default)]
    pub dimensions: Option<Dimensions>,
}

#[allow(dead_code)]
impl Building {
    pub fn volume(&self) -> FloatType {
        self.dimensions.map(|d| d.volume()).unwrap_or(0.0)
    }

    pub fn floor_area(&self) -> FloatType {
        self.dimensions.map(|d| d.floor_area()).unwrap_or(0.0)
    }
}

impl fmt::Display for Building {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Hash for Building {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialEq for Building {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
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
    pub fn average_mw(&self, recipe: &Recipe, clock_speed: FloatType) -> FloatType {
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
}

impl fmt::Display for PowerConsumption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fixed { value_mw, .. } => write!(f, "{} MW", value_mw),
            Self::Variable { min_mw, max_mw, .. } => write!(f, "{} - {} MW)", min_mw, max_mw),
        }
    }
}
