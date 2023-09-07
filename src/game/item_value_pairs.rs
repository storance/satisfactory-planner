use serde::{Deserialize, Serialize};

use super::{Item, ItemId};
use crate::utils::{FloatType, EPSILON};
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

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

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.amount.abs() < EPSILON
    }

    #[inline]
    pub fn with_value(&self, amount: FloatType) -> Self {
        Self {
            item: self.item,
            amount,
        }
    }

    #[inline]
    pub fn clamp(&self, min_value: FloatType, max_value: FloatType) -> Self {
        Self {
            item: self.item,
            amount: self.amount.min(max_value).max(min_value),
        }
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

impl Neg for ItemPerMinute {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            item: self.item,
            amount: -self.amount,
        }
    }
}

impl Add<FloatType> for ItemPerMinute {
    type Output = Self;

    fn add(self, rhs: FloatType) -> Self::Output {
        Self {
            item: self.item,
            amount: self.amount + rhs,
        }
    }
}

impl Add<ItemPerMinute> for ItemPerMinute {
    type Output = Self;

    fn add(self, rhs: ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            amount: self.amount + rhs.amount,
        }
    }
}

impl Add<ItemPerMinute> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn add(self, rhs: ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemPerMinute {
            item: self.item,
            amount: self.amount + rhs.amount,
        }
    }
}

impl AddAssign<FloatType> for ItemPerMinute {
    fn add_assign(&mut self, rhs: FloatType) {
        self.amount += rhs
    }
}

impl Sub<FloatType> for ItemPerMinute {
    type Output = Self;

    fn sub(self, rhs: FloatType) -> Self::Output {
        Self {
            item: self.item,
            amount: self.amount - rhs,
        }
    }
}

impl Sub<FloatType> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn sub(self, rhs: FloatType) -> Self::Output {
        ItemPerMinute {
            item: self.item,
            amount: self.amount - rhs,
        }
    }
}

impl Sub<ItemPerMinute> for ItemPerMinute {
    type Output = Self;

    fn sub(self, rhs: ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            amount: self.amount - rhs.amount,
        }
    }
}

impl Sub<ItemPerMinute> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn sub(self, rhs: ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemPerMinute {
            item: self.item,
            amount: self.amount - rhs.amount,
        }
    }
}

impl SubAssign<FloatType> for ItemPerMinute {
    fn sub_assign(&mut self, rhs: FloatType) {
        self.amount -= rhs;
    }
}
