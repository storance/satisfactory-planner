use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Mul};

use crate::game::{Item, RecipeIO};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ItemValuePair<V: Debug + Copy + Clone + PartialEq> {
    pub item: Item,
    pub value: V,
}

impl From<RecipeIO> for ItemValuePair<f64> {
    fn from(value: RecipeIO) -> Self {
        Self {
            item: value.item,
            value: value.amount_per_minute,
        }
    }
}

impl From<RecipeIO> for ItemValuePair<u32> {
    fn from(value: RecipeIO) -> Self {
        Self {
            item: value.item,
            value: value.amount,
        }
    }
}

impl<V: Debug + Copy + Clone + PartialEq> ItemValuePair<V> {
    pub const fn new(item: Item, value: V) -> Self {
        Self { item, value }
    }

    pub fn to_tuple(&self) -> (Item, V) {
        (self.item, self.value)
    }
}

impl<V: Debug + Copy + Clone + PartialEq + Add<Output = V>> Add<V> for ItemValuePair<V> {
    type Output = Self;

    fn add(self, rhs: V) -> Self::Output {
        Self {
            item: self.item,
            value: self.value + rhs,
        }
    }
}

impl<V: Debug + Copy + Clone + PartialEq + Mul<Output = V>> Mul<V> for ItemValuePair<V> {
    type Output = Self;

    fn mul(self, rhs: V) -> Self::Output {
        Self {
            item: self.item,
            value: self.value * rhs,
        }
    }
}


impl<V: fmt::Display + Debug + Copy + Clone + PartialEq> fmt::Display for ItemValuePair<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.item.display_name(), self.value)
    }
}

impl<V: Serialize + Debug + Copy + Clone + PartialEq> Serialize for ItemValuePair<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_map(Some(1))?;
        seq.serialize_entry(self.item.display_name(), &self.value)?;
        seq.end()
    }
}

impl<'de, V: Deserialize<'de> + Debug + Copy + Clone + PartialEq> Deserialize<'de>
    for ItemValuePair<V>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ItemValuePairVisitor {
            phantom: PhantomData,
        })
    }
}

struct ItemValuePairVisitor<V: Debug + Copy + Clone + PartialEq> {
    phantom: PhantomData<V>,
}

impl<'de, V: Deserialize<'de> + Debug + Copy + Clone + PartialEq> Visitor<'de>
    for ItemValuePairVisitor<V>
{
    type Value = ItemValuePair<V>;

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

impl<V: Debug + Copy + Clone + PartialEq> From<(Item, V)> for ItemValuePair<V> {
    fn from(value: (Item, V)) -> Self {
        Self {
            item: value.0,
            value: value.1,
        }
    }
}
