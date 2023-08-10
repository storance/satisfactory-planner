use serde::de::{Error, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Debug;
use std::marker::PhantomData;

use crate::game::{Fluid, Item, Resource, ResourceDefinition};

#[derive(Debug, Copy, Clone)]
pub struct ResourceValuePair<V: Debug + Copy + Clone> {
    pub resource: Resource,
    pub value: V,
}

impl<V: Debug + Copy + Clone> ResourceValuePair<V> {
    pub const fn new(resource: Resource, value: V) -> Self {
        Self { resource, value }
    }

    pub const fn for_item(item: Item, value: V) -> Self {
        Self {
            resource: Resource::Item(item),
            value,
        }
    }

    pub const fn for_fluid(fluid: Fluid, value: V) -> Self {
        Self {
            resource: Resource::Fluid(fluid),
            value,
        }
    }

    pub fn to_tuple(&self) -> (Resource, V) {
        (self.resource, self.value)
    }
}

impl<V: fmt::Display + Debug + Copy + Clone> fmt::Display for ResourceValuePair<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.resource.display_name(), self.value)
    }
}

impl<V: Serialize + Debug + Copy + Clone> Serialize for ResourceValuePair<V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_map(Some(1))?;
        seq.serialize_entry(self.resource.display_name(), &self.value)?;
        seq.end()
    }
}

impl<'de, V: Deserialize<'de> + Debug + Copy + Clone> Deserialize<'de> for ResourceValuePair<V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ItemValuePairVisitor {
            phantom: PhantomData,
        })
    }
}

struct ItemValuePairVisitor<V: Debug + Copy + Clone> {
    phantom: PhantomData<V>,
}

impl<'de, V: Deserialize<'de> + Debug + Copy + Clone> Visitor<'de> for ItemValuePairVisitor<V> {
    type Value = ResourceValuePair<V>;

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
        if let Some(resource) = map.next_key::<Resource>()? {
            Ok(ResourceValuePair::new(resource, map.next_value()?))
        } else {
            Err(serde::de::Error::custom("Missing item and amount pair"))
        }
    }
}

impl<V: Debug + Copy + Clone> From<(Resource, V)> for ResourceValuePair<V> {
    fn from(value: (Resource, V)) -> Self {
        Self {
            resource: value.0,
            value: value.1,
        }
    }
}
