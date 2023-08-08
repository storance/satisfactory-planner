use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

use crate::game::{Fluid, Item};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
pub enum Resource {
    Item(Item),
    Fluid(Fluid),
}

impl<'de> Deserialize<'de> for Resource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ResourceVisitor)
    }
}

struct ResourceVisitor;

impl<'de> Visitor<'de> for ResourceVisitor {
    type Value = Resource;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "an item name or fluid name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if let Some(item) = Item::from_str(v) {
            Ok(Resource::Item(item))
        } else if let Some(fluid) = Fluid::from_str(v) {
            Ok(Resource::Fluid(fluid))
        } else {
            Err(serde::de::Error::custom(&format!(
                "Invalid Item Name: {}",
                v
            )))
        }
    }
}

impl Resource {
    pub fn display_name(&self) -> &str {
        match self {
            Resource::Item(item) => item.display_name(),
            Resource::Fluid(fluid) => fluid.display_name(),
        }
    }

    pub fn is_raw(&self) -> bool {
        match self {
            Resource::Item(item) => item.is_raw(),
            Resource::Fluid(fluid) => fluid.is_raw(),
        }
    }
}

impl fmt::Display for Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl From<Item> for Resource {
    fn from(value: Item) -> Self {
        Self::Item(value)
    }
}

impl From<Fluid> for Resource {
    fn from(value: Fluid) -> Self {
        Self::Fluid(value)
    }
}
