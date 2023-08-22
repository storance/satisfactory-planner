use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Debug;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign};

use crate::game::Item;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct ItemValuePair {
    pub item: Item,
    pub value: f64,
}

impl ItemValuePair {
    pub fn new(item: Item, value: f64) -> Self {
        Self {
            item,
            value: f64::max(0.0, value),
        }
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

impl AddAssign<f64> for ItemValuePair {
    fn add_assign(&mut self, rhs: f64) {
        self.value += rhs
    }
}

impl Sub<f64> for ItemValuePair {
    type Output = Self;

    fn sub(self, rhs: f64) -> Self::Output {
        Self {
            item: self.item,
            value: self.value - rhs,
        }
    }
}

impl SubAssign<f64> for ItemValuePair {
    fn sub_assign(&mut self, rhs: f64) {
        self.value = f64::max(0.0, self.value - rhs);
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

impl Div<f64> for ItemValuePair {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self {
            item: self.item,
            value: self.value / rhs,
        }
    }
}

impl MulAssign<f64> for ItemValuePair {
    fn mul_assign(&mut self, rhs: f64) {
        self.value *= rhs;
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
