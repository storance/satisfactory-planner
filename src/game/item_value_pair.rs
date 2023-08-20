use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Debug;
use std::ops::{Add, Mul};

use crate::game::Item;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct ItemValuePair {
    pub item: Item,
    pub value: f64,
}

impl ItemValuePair {
    pub const fn new(item: Item, value: f64) -> Self {
        Self { item, value }
    }
}

impl Add<f64> for ItemValuePair {
    type Output = Self;

    fn add(self, rhs: f64) -> Self::Output {
        Self {
            item: self.item,
            value: self.value + rhs,
        }
    }
}

impl Mul<f64> for ItemValuePair {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self {
            item: self.item,
            value: self.value * rhs,
        }
    }
}

impl fmt::Display for ItemValuePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.item.display_name(), self.value)
    }
}

impl Serialize for ItemValuePair {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_map(Some(1))?;
        seq.serialize_entry(self.item.display_name(), &self.value)?;
        seq.end()
    }
}

impl<'de> Deserialize<'de> for ItemValuePair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ItemValuePairVisitor)
    }
}

struct ItemValuePairVisitor;

impl<'de> Visitor<'de> for ItemValuePairVisitor {
    type Value = ItemValuePair;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "a map with the key as the item name and value as the amount"
        )
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        if let Some(item) = map.next_key::<Item>()? {
            Ok(ItemValuePair::new(item, map.next_value()?))
        } else {
            Err(serde::de::Error::custom("Missing item and amount pair"))
        }
    }
}
