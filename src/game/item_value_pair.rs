use ::serde::{Serialize, Deserialize};

use crate::utils::{FloatType, EPSILON};
use std::fmt;
use std::fmt::Debug;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::rc::Rc;

use super::Item;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemAmount {
    pub item: String,
    pub amount: FloatType
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ItemValuePair {
    pub item: Rc<Item>,
    pub value: FloatType,
}

#[allow(dead_code)]
impl ItemValuePair {
    #[inline]
    pub fn new(item: Rc<Item>, value: FloatType) -> Self {
        Self { item, value }
    }

    pub fn is_zero(&self) -> bool {
        self.value.abs() < EPSILON
    }

    pub fn mul(&self, value: FloatType) -> Self {
        Self {
            item: Rc::clone(&self.item),
            value: self.value * value,
        }
    }

    pub fn ratio(&self, other: &Self) -> FloatType {
        assert!(self.item == other.item);
        self.value / other.value
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

impl AddAssign<FloatType> for ItemValuePair {
    fn add_assign(&mut self, rhs: FloatType) {
        self.value += rhs
    }
}

impl AddAssign<ItemValuePair> for ItemValuePair {
    fn add_assign(&mut self, rhs: ItemValuePair) {
        assert!(self.item == rhs.item);
        self.value += rhs.value
    }
}

impl AddAssign<&ItemValuePair> for ItemValuePair {
    fn add_assign(&mut self, rhs: &ItemValuePair) {
        assert!(self.item == rhs.item);
        self.value += rhs.value
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

impl SubAssign<FloatType> for ItemValuePair {
    fn sub_assign(&mut self, rhs: FloatType) {
        self.value -= rhs;
    }
}

impl SubAssign<ItemValuePair> for ItemValuePair {
    fn sub_assign(&mut self, rhs: ItemValuePair) {
        assert!(self.item == rhs.item);
        self.value -= rhs.value;
    }
}

impl SubAssign<&ItemValuePair> for ItemValuePair {
    fn sub_assign(&mut self, rhs: &ItemValuePair) {
        assert!(self.item == rhs.item);
        self.value -= rhs.value;
    }
}

impl fmt::Display for ItemValuePair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.item, self.value)
    }
}
