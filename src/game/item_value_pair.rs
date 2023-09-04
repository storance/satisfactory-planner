use crate::utils::{clamp_to_zero, round, FloatType, EPSILON};
use ::serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::rc::Rc;

use super::Item;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAmount {
    pub item: String,
    pub amount: FloatType,
}

#[derive(Clone, PartialEq)]
pub struct ItemValuePair {
    pub item: Rc<Item>,
    pub value: FloatType,
}

impl ItemValuePair {
    #[inline]
    pub fn new(item: Rc<Item>, value: FloatType) -> Self {
        Self { item, value }
    }

    pub fn is_zero(&self) -> bool {
        self.value.abs() < EPSILON
    }

    pub fn with_value(&self, value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            value,
        }
    }

    pub fn clamp(&self, min_value: FloatType, max_value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            value: self.value.min(max_value).max(min_value),
        }
    }

    pub fn mul(&self, value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            value: clamp_to_zero(self.value * value),
        }
    }

    pub fn ratio(&self, other: &Self) -> FloatType {
        assert!(self.item == other.item);
        clamp_to_zero(self.value / other.value)
    }
}

impl Eq for ItemValuePair {}

impl PartialOrd for ItemValuePair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.item
                .cmp(&other.item)
                .then_with(|| self.value.total_cmp(&other.value)),
        )
    }
}

impl Ord for ItemValuePair {
    fn cmp(&self, other: &Self) -> Ordering {
        self.item
            .cmp(&other.item)
            .then_with(|| self.value.total_cmp(&other.value))
    }
}

impl Neg for ItemValuePair {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            item: Rc::clone(&self.item),
            value: -self.value,
        }
    }
}

impl Add<FloatType> for ItemValuePair {
    type Output = Self;

    fn add(self, rhs: FloatType) -> Self::Output {
        Self {
            item: self.item,
            value: self.value + rhs,
        }
    }
}

impl Add<FloatType> for &ItemValuePair {
    type Output = ItemValuePair;

    fn add(self, rhs: FloatType) -> Self::Output {
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value + rhs,
        }
    }
}

impl Add<ItemValuePair> for ItemValuePair {
    type Output = Self;

    fn add(self, rhs: ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            value: self.value + rhs.value,
        }
    }
}

impl Add<ItemValuePair> for &ItemValuePair {
    type Output = ItemValuePair;

    fn add(self, rhs: ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value + rhs.value,
        }
    }
}

impl Add<&ItemValuePair> for ItemValuePair {
    type Output = Self;

    fn add(self, rhs: &ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            value: self.value + rhs.value,
        }
    }
}

impl Add<&ItemValuePair> for &ItemValuePair {
    type Output = ItemValuePair;

    fn add(self, rhs: &ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value + rhs.value,
        }
    }
}

impl AddAssign<FloatType> for ItemValuePair {
    fn add_assign(&mut self, rhs: FloatType) {
        self.value += rhs
    }
}

impl Sub<FloatType> for ItemValuePair {
    type Output = Self;

    fn sub(self, rhs: FloatType) -> Self::Output {
        Self {
            item: self.item,
            value: self.value - rhs,
        }
    }
}

impl Sub<FloatType> for &ItemValuePair {
    type Output = ItemValuePair;

    fn sub(self, rhs: FloatType) -> Self::Output {
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value - rhs,
        }
    }
}

impl Sub<ItemValuePair> for ItemValuePair {
    type Output = Self;

    fn sub(self, rhs: ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            value: self.value - rhs.value,
        }
    }
}

impl Sub<ItemValuePair> for &ItemValuePair {
    type Output = ItemValuePair;

    fn sub(self, rhs: ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value - rhs.value,
        }
    }
}

impl Sub<&ItemValuePair> for ItemValuePair {
    type Output = Self;

    fn sub(self, rhs: &ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            value: self.value - rhs.value,
        }
    }
}

impl Sub<&ItemValuePair> for &ItemValuePair {
    type Output = ItemValuePair;

    fn sub(self, rhs: &ItemValuePair) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemValuePair {
            item: Rc::clone(&self.item),
            value: self.value - rhs.value,
        }
    }
}

impl SubAssign<FloatType> for ItemValuePair {
    fn sub_assign(&mut self, rhs: FloatType) {
        self.value -= rhs;
    }
}

impl fmt::Debug for ItemValuePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemValuePair")
            .field("item", &self.item.name)
            .field("value", &self.value)
            .finish()
    }
}

impl fmt::Display for ItemValuePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{} / min", self.item, round(self.value, 3))
    }
}
