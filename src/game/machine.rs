use crate::machine_definition;
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use std::{fmt, iter};

use super::ItemValuePair;

machine_definition!(
    Machine {
        Smelter(name: "Smelter", power: 4, inputs: [Item], outputs: [Item]),
        Foundry(name: "Foundry", power: 16, inputs: [Item, Item], outputs: [Item]),
        Constructor(name: "Constructor", power: 4, inputs: [Item], outputs: [Item]),
        Assembler(name: "Assembler", power: 15, inputs: [Item, Item], outputs: [Item]),
        Manufacturer(name: "Manufacturer", power: 55, inputs: [Item, Item, Item, Item], outputs: [Item]),
        Refinery(name: "Refinery", power: 30, inputs: [Item, Fluid], outputs: [Item, Fluid]),
        Packager(name: "Packager", power: 10, inputs: [Item, Fluid], outputs: [Item, Fluid]),
        Blender(name: "Blender", power: 75, inputs: [Item, Item, Fluid, Fluid], outputs: [Item, Fluid]),
        ParticleAccelerator(name: "Particle Accelerator", min_power: 250, max_power: 750, inputs: [Item, Item, Fluid], outputs: [Item])
    }
);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MachineIO {
    pub items: u8,
    pub fluids: u8,
}

impl MachineIO {
    pub fn new(items: u8, fluids: u8) -> Self {
        Self { items, fluids }
    }

    pub fn zero() -> Self {
        Self::new(0, 0)
    }

    pub fn total(&self) -> u8 {
        self.fluids + self.items
    }

    pub fn is_zero(&self) -> bool {
        self.total() == 0
    }

    pub fn is_greater(&self, other: &Self) -> bool {
        self.items > other.items || self.fluids > other.fluids
    }
}

impl From<&[ItemValuePair]> for MachineIO {
    fn from(value: &[ItemValuePair]) -> MachineIO {
        value.iter().fold(MachineIO::zero(), |mut acc, item_value| {
            acc.fluids += item_value.item.is_fluid() as u8;
            acc.items += (!item_value.item.is_fluid()) as u8;
            acc
        })
    }
}

impl From<&Vec<ItemValuePair>> for MachineIO {
    fn from(value: &Vec<ItemValuePair>) -> MachineIO {
        Self::from(value.as_slice())
    }
}

impl fmt::Display for MachineIO {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let item_part = iter::repeat("Item").take(self.items as usize);
        let fluid_part = iter::repeat("Fluid").take(self.fluids as usize);

        let formatted_str = item_part.chain(fluid_part).fold(String::new(), |a, b| {
            if a.is_empty() {
                a + b
            } else {
                a + ", " + b
            }
        });

        write!(f, "[{}]", formatted_str)
    }
}

#[cfg(test)]
mod tests {
    use crate::game::Item;

    use super::*;

    #[test]
    fn machineio_display() {
        assert_eq!(
            format!("{}", MachineIO::new(2, 2)),
            "[Item, Item, Fluid, Fluid]"
        );
        assert_eq!(format!("{}", MachineIO::new(1, 0)), "[Item]");
        assert_eq!(format!("{}", MachineIO::new(0, 3)), "[Fluid, Fluid, Fluid]");
    }

    #[test]
    fn machineio_from() {
        assert_eq!(
            MachineIO::from(&vec![
                ItemValuePair::new(Item::IronPlate, 2.0),
                ItemValuePair::new(Item::IronRod, 3.0),
                ItemValuePair::new(Item::Water, 4.0),
                ItemValuePair::new(Item::SulfuricAcid, 4.0),
            ]),
            MachineIO::new(2, 2)
        );

        assert_eq!(
            MachineIO::from(&vec![
                ItemValuePair::new(Item::IronPlate, 2.0),
                ItemValuePair::new(Item::IronRod, 3.0)
            ]),
            MachineIO::new(2, 0)
        );

        assert_eq!(
            MachineIO::from(&vec![
                ItemValuePair::new(Item::Water, 4.0),
                ItemValuePair::new(Item::SulfuricAcid, 4.0),
            ]),
            MachineIO::new(0, 2)
        );
    }

    #[test]
    fn machineio_zero() {
        assert_eq!(MachineIO::new(0, 0), MachineIO::zero());
    }

    #[test]
    fn machineio_total() {
        assert_eq!(MachineIO::new(2, 1).total(), 3);
        assert_eq!(MachineIO::new(2, 0).total(), 2);
        assert_eq!(MachineIO::new(0, 3).total(), 3);
        assert_eq!(MachineIO::new(0, 0).total(), 0);
    }

    #[test]
    fn machineio_is_empty() {
        assert!(!MachineIO::new(2, 1).is_zero());
        assert!(!MachineIO::new(2, 0).is_zero());
        assert!(!MachineIO::new(3, 0).is_zero());
        assert!(MachineIO::new(0, 0).is_zero());
    }

    #[test]
    fn machineio_is_greater() {
        assert!(!MachineIO::new(2, 0).is_greater(&MachineIO::new(2, 2)));
        assert!(!MachineIO::new(2, 1).is_greater(&MachineIO::new(2, 2)));
        assert!(!MachineIO::new(2, 2).is_greater(&MachineIO::new(2, 2)));
        assert!(MachineIO::new(2, 3).is_greater(&MachineIO::new(2, 2)));

        assert!(!MachineIO::new(0, 2).is_greater(&MachineIO::new(2, 2)));
        assert!(!MachineIO::new(1, 2).is_greater(&MachineIO::new(2, 2)));
        assert!(!MachineIO::new(2, 2).is_greater(&MachineIO::new(2, 2)));
        assert!(MachineIO::new(3, 2).is_greater(&MachineIO::new(2, 2)));

        assert!(!MachineIO::new(1, 0).is_greater(&MachineIO::new(1, 0)));
        assert!(MachineIO::new(2, 0).is_greater(&MachineIO::new(1, 0)));

        assert!(!MachineIO::new(0, 1).is_greater(&MachineIO::new(0, 1)));
        assert!(MachineIO::new(0, 2).is_greater(&MachineIO::new(0, 1)));
    }
}
