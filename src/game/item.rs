use serde::{Deserialize, Serialize};
use std::{
    fmt,
    hash::{Hash, Hasher},
};

#[derive(Debug, Copy, Clone, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ItemState {
    #[serde(rename = "solid")]
    Solid,
    #[serde(rename = "liquid")]
    Liquid,
    #[serde(rename = "gas")]
    Gas,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub name: String,
    pub resource: bool,
    pub state: ItemState,
    pub energy_mj: u32,
    pub sink_points: u32,
    pub bit_mask: Option<u16>,
}

#[allow(dead_code)]
impl ItemState {
    pub fn is_fluid(&self) -> bool {
        !matches!(self, Self::Solid)
    }

    pub fn is_solid(&self) -> bool {
        matches!(self, Self::Solid)
    }

    pub fn is_liquid(&self) -> bool {
        matches!(self, Self::Liquid)
    }

    pub fn is_gas(&self) -> bool {
        matches!(self, Self::Gas)
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Hash for Item {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for Item {}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.key.partial_cmp(&other.key)
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}
