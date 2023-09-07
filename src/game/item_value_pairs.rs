use serde::{Deserialize, Serialize};

use super::{Item, ItemId};
use crate::utils::{FloatType, EPSILON};
use std::cmp::Ordering;
use std::ops::{AddAssign, SubAssign};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemKeyAmountPair {
    pub item: String,
    pub amount: FloatType,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ItemPerMinute {
    pub item: ItemId,
    pub amount: FloatType,
}

impl ItemKeyAmountPair {
    #[inline]
    pub fn new(item: String, amount: FloatType) -> Self {
        Self { item, amount }
    }

    #[inline]
    pub fn from_item(item: &Item, amount: FloatType) -> Self {
        Self {
            item: item.key.clone(),
            amount,
        }
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.abs() < EPSILON
    }
}

impl Eq for ItemKeyAmountPair {}

impl PartialOrd for ItemKeyAmountPair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.item
                .cmp(&other.item)
                .then_with(|| self.amount.total_cmp(&other.amount)),
        )
    }
}

impl Ord for ItemKeyAmountPair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.item
            .cmp(&other.item)
            .then_with(|| self.amount.total_cmp(&other.amount))
    }
}

impl AddAssign<FloatType> for ItemKeyAmountPair {
    fn add_assign(&mut self, rhs: FloatType) {
        self.amount += rhs
    }
}

impl SubAssign<FloatType> for ItemKeyAmountPair {
    fn sub_assign(&mut self, rhs: FloatType) {
        self.amount -= rhs
    }
}

impl ItemPerMinute {
    #[inline]
    pub fn new(item: ItemId, amount: FloatType) -> Self {
        Self { item, amount }
    }
}

impl Eq for ItemPerMinute {}

impl PartialOrd for ItemPerMinute {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.item
                .cmp(&other.item)
                .then_with(|| self.amount.total_cmp(&other.amount)),
        )
    }
}

impl Ord for ItemPerMinute {
    fn cmp(&self, other: &Self) -> Ordering {
        self.item
            .cmp(&other.item)
            .then_with(|| self.amount.total_cmp(&other.amount))
    }
}
