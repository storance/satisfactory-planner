use crate::game::Item;

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
pub struct ItemBitSet(u16);

#[allow(dead_code)]
impl ItemBitSet {
    pub fn new(item: &Item) -> Self {
        Self(item.bit_mask.unwrap())
    }

    #[inline]
    pub fn add(&mut self, item: &Item) {
        let bit_mask = item.bit_mask.unwrap();

        self.0 |= bit_mask;
    }

    #[inline]
    pub fn contains(&self, item: &Item) -> bool {
        let bit_mask = item.bit_mask.unwrap();
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
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::game::test::get_test_game_db;
    use std::rc::Rc;

    pub fn create_bit_set(items: &[&Rc<Item>]) -> ItemBitSet {
        let mut bit_set = ItemBitSet::default();
        items.iter().for_each(|i| bit_set.add(i));

        bit_set
    }

    #[test]
    fn test_union() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let oil = game_db.find_item("Desc_LiquidOil_C").unwrap();

        let a = create_bit_set(&[&iron_ore, &copper_ore, &caterium_ore]);
        let b = create_bit_set(&[&iron_ore, &water, &oil]);

        let merged = a.union(&b);
        assert_eq!(merged.len(), 5);
        assert_eq!(
            merged,
            create_bit_set(&[&iron_ore, &copper_ore, &caterium_ore, &water, &oil])
        )
    }

    #[test]
    fn test_is_subset_of() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        let bit_set = create_bit_set(&[&iron_ore, &copper_ore, &caterium_ore]);

        assert!(bit_set.is_subset_of(&bit_set));
        assert!(create_bit_set(&[&iron_ore]).is_subset_of(&bit_set));
        assert!(create_bit_set(&[&copper_ore]).is_subset_of(&bit_set));
        assert!(create_bit_set(&[&caterium_ore]).is_subset_of(&bit_set));
        assert!(create_bit_set(&[&iron_ore, &caterium_ore]).is_subset_of(&bit_set));
        assert!(create_bit_set(&[&iron_ore, &copper_ore]).is_subset_of(&bit_set));
        assert!(create_bit_set(&[&copper_ore, &caterium_ore]).is_subset_of(&bit_set));
        assert!(!create_bit_set(&[&water]).is_subset_of(&bit_set));
        assert!(!create_bit_set(&[&water, &iron_ore]).is_subset_of(&bit_set));
        assert!(!create_bit_set(&[&water, &iron_ore, &copper_ore]).is_subset_of(&bit_set));
    }

    #[test]
    fn test_len() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        let mut bit_set = create_bit_set(&[&iron_ore, &copper_ore, &caterium_ore]);

        assert_eq!(bit_set.len(), 3);

        bit_set.add(&iron_ore);
        assert_eq!(bit_set.len(), 3);

        bit_set.add(&water);
        assert_eq!(bit_set.len(), 4);
    }

    #[test]
    fn test_contains() {
        let game_db = get_test_game_db();

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let oil = game_db.find_item("Desc_LiquidOil_C").unwrap();
        let limestone = game_db.find_item("Desc_Stone_C").unwrap();
        let raw_quartz = game_db.find_item("Desc_RawQuartz_C").unwrap();
        let nitrogen_gas = game_db.find_item("Desc_NitrogenGas_C").unwrap();
        let sulfur = game_db.find_item("Desc_Sulfur_C").unwrap();
        let uranium = game_db.find_item("Desc_OreUranium_C").unwrap();
        let coal = game_db.find_item("Desc_Coal_C").unwrap();
        let bauxite = game_db.find_item("Desc_OreBauxite_C").unwrap();

        let bit_set = create_bit_set(&[&iron_ore, &copper_ore, &caterium_ore]);

        assert!(bit_set.contains(&iron_ore));
        assert!(bit_set.contains(&copper_ore));
        assert!(bit_set.contains(&caterium_ore));
        assert!(!bit_set.contains(&water));
        assert!(!bit_set.contains(&oil));
        assert!(!bit_set.contains(&nitrogen_gas));
        assert!(!bit_set.contains(&coal));
        assert!(!bit_set.contains(&sulfur));
        assert!(!bit_set.contains(&uranium));
        assert!(!bit_set.contains(&bauxite));
        assert!(!bit_set.contains(&raw_quartz));
        assert!(!bit_set.contains(&limestone));
    }
}
