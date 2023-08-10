pub mod fluid;
pub mod item;
pub mod machine;
mod macros;
pub mod recipe;
pub mod resource;
pub mod resource_value_pair;

use std::fmt;
pub use fluid::Fluid;
pub use item::Item;
pub use machine::{Machine, MachineIO};
pub use recipe::Recipe;
pub use resource::Resource;
pub use resource_value_pair::ResourceValuePair;

pub trait ResourceDefinition {
    fn display_name(&self) -> &str;

    fn is_raw(&self) -> bool;

    fn sink_points(&self) -> Option<u32>;

    fn from_str(value: &str) -> Option<Self> where Self: Sized;
}