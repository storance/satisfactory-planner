use std::io;
use thiserror::Error;

mod config;
mod graph;
mod solver;

use crate::game::Item;
pub use config::*;
pub use graph::*;
pub use solver::*;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("No recipe exists with the name `{0}`")]
    InvalidRecipe(String),
    #[error("The raw resource `{0}` is not allowed in inputs.")]
    UnexpectedRawInputItem(Item),
    #[error("The raw resource `{0}` is not allowed in outputs.")]
    UnexpectedRawOutputItem(Item),
    #[error("Item `{0}` in override_limits is not a raw resource.")]
    InvalidOverrideLimit(Item),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_yaml::Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
struct ItemBitSet(u16);

impl ItemBitSet {
    pub fn new(item: Item) -> Self {
        Self(Self::item_to_u16(item))
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0 & self.0 == self.0
    }

    pub fn union(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    fn item_to_u16(item: Item) -> u16 {
        match item {
            Item::Bauxite => 1,
            Item::CateriumOre => 2,
            Item::Coal => 4,
            Item::CopperOre => 8,
            Item::CrudeOil => 16,
            Item::IronOre => 32,
            Item::Limestone => 64,
            Item::NitrogenGas => 128,
            Item::RawQuartz => 256,
            Item::Sulfur => 512,
            Item::Uranium => 1024,
            Item::Water => 2048,
            Item::SAMOre => 4096,
            _ => {
                panic!("Item `{}` not supported in ItemBitSet", item)
            }
        }
    }
}