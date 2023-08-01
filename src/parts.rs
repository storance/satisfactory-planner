enum Item {
    IronOre,
    CopperOre,
    Coal,
    RawQuartz,
    CateriumOre,
    Limestone,
    Bauxite,
    Uranium,
    Sulfur,
}

enum Fluid {
    Water,
    CrudeOil,
    Nitrogen
}

enum Machine {
    Smelter,
    Foundry,
    Constructor,
    Assembler,
    Manufacturer,
    Refinery,
    Packager,
    Blender,
    ParticleAccelerator
}

struct ItemPort {
    item: Item,
    amount: u32
}

struct FluidPort {
    item: Fluid,
    amount_m3: u32
}

struct Recipe {
    output_items: Vec<ItemPort>,
    output_fluids: Vec<FluidPort>,
    input_items: Vec<ItemPort>,
    input_fluids: Vec<FluidPort>,
    production_time_secs: u32,
    machine: Machine
}

trait ItemDefinition {

}

trait MachineDefinition {

}