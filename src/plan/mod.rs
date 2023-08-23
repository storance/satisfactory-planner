use thiserror::Error;

mod config;
mod graph;
mod solver;

use crate::game::Item;
pub use config::*;
pub use graph::*;
pub use solver::*;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum PlanError {
    #[error("No recipe exists with the name `{0}`")]
    InvalidRecipe(String),
    #[error("The raw resource `{0}` is not allowed in outputs.")]
    UnexpectedRawOutputItem(Item)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
#[derive(Default)]
struct ItemBitSet(u16);

#[allow(dead_code)]
impl ItemBitSet {
    pub fn new(item: Item) -> Self {
        Self(Self::to_bit_mask(item))
    }

    #[inline]
    pub fn add(&mut self, item: Item) {
        let bit_mask = Self::to_bit_mask(item);

        self.0 |= bit_mask;
    }

    #[inline]
    pub fn contains(&self, item: Item) -> bool {
        let bit_mask = Self::to_bit_mask(item);
        self.0 & bit_mask == bit_mask
    }

    #[inline]
    pub fn is_subset_of(&self, other: &Self) -> bool {
        other.0 & self.0 == self.0
    }

    #[inline]
    pub fn union(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub fn len(&self) -> usize {
        u16::count_ones(self.0) as usize
    }

    #[inline]
    fn to_bit_mask(item: Item) -> u16 {
        debug_assert!(item.is_extractable());
        1 << (item as u32 % 16)
    }
}



#[cfg(test)]
mod test {
    use super::*;
        
    #[test]
    fn test_union() {
        let a = construct_bit_set(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);
        let b = construct_bit_set(&[Item::IronOre, Item::Water, Item::CrudeOil]);

        let merged = a.union(&b);
        assert_eq!(merged.len(), 5);
        assert_eq!(merged, construct_bit_set(&[Item::IronOre, Item::CopperOre, Item::CateriumOre, Item::Water, Item::CrudeOil]))
    }

    #[test]
    fn test_is_subset_of() {
        let bit_set = construct_bit_set(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert!(bit_set.is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::IronOre]).is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::CopperOre]).is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::CateriumOre]).is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::IronOre, Item::CateriumOre]).is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::IronOre, Item::CopperOre]).is_subset_of(&bit_set));
        assert!(construct_bit_set(&[Item::CopperOre, Item::CateriumOre]).is_subset_of(&bit_set));
    }

    #[test]
    fn test_len() {
        let mut bit_set = construct_bit_set(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert_eq!(bit_set.len(), 3);

        bit_set.add(Item::IronOre);
        assert_eq!(bit_set.len(), 3);

        bit_set.add(Item::Water);
        assert_eq!(bit_set.len(), 4);
    }

    #[test]
    fn test_contains() {
        let bit_set = construct_bit_set(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert!(bit_set.contains(Item::IronOre));
        assert!(bit_set.contains(Item::CopperOre));
        assert!(bit_set.contains(Item::CateriumOre));
        assert!(!bit_set.contains(Item::Water));
        assert!(!bit_set.contains(Item::CrudeOil));
        assert!(!bit_set.contains(Item::NitrogenGas));
        assert!(!bit_set.contains(Item::Coal));
        assert!(!bit_set.contains(Item::Sulfur));
        assert!(!bit_set.contains(Item::Uranium));
        assert!(!bit_set.contains(Item::Bauxite));
        assert!(!bit_set.contains(Item::RawQuartz));
        assert!(!bit_set.contains(Item::Limestone));
    }

    fn construct_bit_set(items: &[Item]) -> ItemBitSet {
        let mut bit_set = ItemBitSet::default();
        items.iter().for_each(|item| bit_set.add(*item));
        bit_set
    }
}