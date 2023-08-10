use crate::item_definition;
use crate::game::ResourceDefinition;
use serde::{Deserialize, Serialize};
use std::fmt;

item_definition!(
    Fluid {
        Water(name: "Water", raw),
        CrudeOil(name: "Crude Oil", raw),
        NitrogenGas(name: "Nitrogen Gas", raw),
        LiquidBiofuel(name: "Liquid Biofuel"),
        HeavyOilResidue(name: "Heavy Oil Residue"),
        Fuel(name: "Fuel"),
        Turbofuel(name: "Turbofuel"),
        AluminaSolution(name: "Alumina Solution"),
        SulfuricAcid(name: "Sulfuric Acid"),
        NitricAcid(name: "Nitric Acid")
    }
);
