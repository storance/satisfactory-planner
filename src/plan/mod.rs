use crate::game::Item;
use std::fmt;

mod config;
mod graph;
mod scored_graph;
mod solver;

pub use config::*;
pub use graph::*;
pub use scored_graph::*;
pub use solver::*;

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
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

impl From<&[Item]> for ItemBitSet {
    fn from(value: &[Item]) -> Self {
        let mut bit_set = ItemBitSet::default();
        value.iter().for_each(|i| bit_set.add(*i));

        bit_set
    }
}

impl<const N: usize> From<&[Item; N]> for ItemBitSet {
    fn from(value: &[Item; N]) -> Self {
        Self::from(value.as_slice())
    }
}

impl From<&Vec<Item>> for ItemBitSet {
    fn from(value: &Vec<Item>) -> Self {
        Self::from(value.as_slice())
    }
}

impl fmt::Display for ItemBitSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        let mut first = true;
        for (item, _) in DEFAULT_LIMITS {
            if !self.contains(item) {
                continue;
            }

            if !first {
                write!(f, ", {}", item)?;
            } else {
                write!(f, "{}", item)?;
                first = false;
            }
        }

        write!(f, "]")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_union() {
        let a = ItemBitSet::from(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);
        let b = ItemBitSet::from(&[Item::IronOre, Item::Water, Item::Oil]);

        let merged = a.union(&b);
        assert_eq!(merged.len(), 5);
        assert_eq!(
            merged,
            ItemBitSet::from(&[
                Item::IronOre,
                Item::CopperOre,
                Item::CateriumOre,
                Item::Water,
                Item::Oil
            ])
        )
    }

    #[test]
    fn test_is_subset_of() {
        let bit_set = ItemBitSet::from(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert!(bit_set.is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::IronOre]).is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::CopperOre]).is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::CateriumOre]).is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::IronOre, Item::CateriumOre]).is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::IronOre, Item::CopperOre]).is_subset_of(&bit_set));
        assert!(ItemBitSet::from(&[Item::CopperOre, Item::CateriumOre]).is_subset_of(&bit_set));
    }

    #[test]
    fn test_len() {
        let mut bit_set = ItemBitSet::from(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert_eq!(bit_set.len(), 3);

        bit_set.add(Item::IronOre);
        assert_eq!(bit_set.len(), 3);

        bit_set.add(Item::Water);
        assert_eq!(bit_set.len(), 4);
    }

    #[test]
    fn test_contains() {
        let bit_set = ItemBitSet::from(&[Item::IronOre, Item::CopperOre, Item::CateriumOre]);

        assert!(bit_set.contains(Item::IronOre));
        assert!(bit_set.contains(Item::CopperOre));
        assert!(bit_set.contains(Item::CateriumOre));
        assert!(!bit_set.contains(Item::Water));
        assert!(!bit_set.contains(Item::Oil));
        assert!(!bit_set.contains(Item::NitrogenGas));
        assert!(!bit_set.contains(Item::Coal));
        assert!(!bit_set.contains(Item::Sulfur));
        assert!(!bit_set.contains(Item::Uranium));
        assert!(!bit_set.contains(Item::Bauxite));
        assert!(!bit_set.contains(Item::RawQuartz));
        assert!(!bit_set.contains(Item::Limestone));
    }

    #[test]
    fn test_unique_bitmask() {
        let items: Vec<Item> = DEFAULT_LIMITS
            .iter()
            .map(|(item, _)| item)
            .copied()
            .collect();
        let bit_set = ItemBitSet::from(&items);

        assert_eq!(bit_set.len(), items.len());
        assert_eq!(bit_set.0, (1 << items.len()) - 1);
    }
}
