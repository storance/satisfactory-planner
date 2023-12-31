use crate::utils::{clamp_to_zero, round, FloatType, EPSILON};
use ::serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::rc::Rc;

use super::Item;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAmountDefinition {
    pub item: String,
    pub amount: FloatType,
}

#[derive(Clone, PartialEq)]
pub struct ItemPerMinute {
    pub item: Rc<Item>,
    pub amount: FloatType,
}

impl ItemPerMinute {
    #[inline]
    pub fn new(item: Rc<Item>, amount: FloatType) -> Self {
        Self { item, amount }
    }

    pub fn is_zero(&self) -> bool {
        self.amount.abs() < EPSILON
    }

    pub fn with_value(&self, amount: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            amount,
        }
    }

    pub fn clamp(&self, min_value: FloatType, max_value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            amount: self.amount.min(max_value).max(min_value),
        }
    }

    pub fn mul(&self, value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            amount: clamp_to_zero(self.amount * value),
        }
    }

    pub fn ratio(&self, other: &Self) -> FloatType {
        assert!(self.item == other.item);
        clamp_to_zero(self.amount / other.amount)
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
            item: Rc::clone(&self.item),
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

impl Add<FloatType> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn add(self, rhs: FloatType) -> Self::Output {
        ItemPerMinute {
            item: Rc::clone(&self.item),
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
            item: Rc::clone(&self.item),
            amount: self.amount + rhs.amount,
        }
    }
}

impl Add<&ItemPerMinute> for ItemPerMinute {
    type Output = Self;

    fn add(self, rhs: &ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            amount: self.amount + rhs.amount,
        }
    }
}

impl Add<&ItemPerMinute> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn add(self, rhs: &ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemPerMinute {
            item: Rc::clone(&self.item),
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
            item: Rc::clone(&self.item),
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
            item: Rc::clone(&self.item),
            amount: self.amount - rhs.amount,
        }
    }
}

impl Sub<&ItemPerMinute> for ItemPerMinute {
    type Output = Self;

    fn sub(self, rhs: &ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        Self {
            item: self.item,
            amount: self.amount - rhs.amount,
        }
    }
}

impl Sub<&ItemPerMinute> for &ItemPerMinute {
    type Output = ItemPerMinute;

    fn sub(self, rhs: &ItemPerMinute) -> Self::Output {
        assert!(self.item == rhs.item);
        ItemPerMinute {
            item: Rc::clone(&self.item),
            amount: self.amount - rhs.amount,
        }
    }
}

impl SubAssign<FloatType> for ItemPerMinute {
    fn sub_assign(&mut self, rhs: FloatType) {
        self.amount -= rhs;
    }
}

impl fmt::Debug for ItemPerMinute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemPerMinute")
            .field("item", &self.item.name)
            .field("value", &self.amount)
            .finish()
    }
}

impl fmt::Display for ItemPerMinute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{} / min", self.item, round(self.amount, 3))
    }
}
