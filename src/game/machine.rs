use crate::machine_definition;
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;
use std::{fmt, iter};

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

#[derive(Debug, Copy, Clone)]
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
