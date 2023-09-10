enum ItemState {
    Solid,
    Liquid,
    Gas
}

class ItemPerMinute {
    item: string;
    amount: number;

    constructor(item: string, amount: number) {
        this.item = item;
        this.amount = amount;
    }
}

class Item {
    key: string;
    name: string;
    resource: boolean;
    state: ItemState;
    energyMJ: number;
    sinkPoints: number;

    constructor(key: string, name: string, resource: boolean, state: ItemState, energyMJ: number, sinkPoints: number) {
        this.key = key;
        this.name = name;
        this.resource = resource;
        this.state = state;
        this.energyMJ = energyMJ;
        this.sinkPoints = sinkPoints;
    }
}

enum PowerConsumptionType {
    Fixed,
    Variable
}

class FixedPowerConsumption {
    type = PowerConsumptionType.Fixed as const;
    valueMW: number;
    exponent: number;

    constructor(valueMW: number, exponent: number) {
        this.valueMW = valueMW;
        this.exponent = exponent;
    }
}

class VariablePowerConsumption {
    type = PowerConsumptionType.Variable as const;
    minMW: number;
    maxMW: number;
    exponent: number;

    constructor(minMW: number, maxMW: number, exponent: number) {
        this.minMW = minMW;
        this.maxMW = maxMW;
        this.exponent = exponent;
    }
}

type PowerConsumption = FixedPowerConsumption | VariablePowerConsumption;

class Dimensions {
    lengthM: number;
    widthM: number;
    heightM: number;

    constructor(lengthM: number, widthM: number, heightM: number) {
        this.lengthM = lengthM;
        this.widthM = widthM;
        this.heightM = heightM;

    }
}

class Manufacturer {
    type = BuildingType.Manufacturer as const;
    key: string;
    name: string;
    powerConsumption: PowerConsumption;
    dimensions: Dimensions

    constructor(key: string, name: string, powerConsumption: PowerConsumption, dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerConsumption = powerConsumption;
        this.dimensions = dimensions;
    }
}

class Fuel {
    fuel: ItemPerMinute;
    supplemental: ItemPerMinute;
    byProduct: ItemPerMinute;
    burnTimeSecs: number;

    constructor(fuel: ItemPerMinute, supplemental: ItemPerMinute, byProduct: ItemPerMinute, burnTimeSecs: number) {
        this.fuel = fuel;
        this.supplemental = supplemental;
        this.byProduct = byProduct;
        this.burnTimeSecs = burnTimeSecs;
    }
}

class PowerGenerator {
    type = BuildingType.PowerGenerator as const;
    key: string;
    name: string;
    powerProductionMW: number;
    fuels: [Fuel];
    dimensions: Dimensions

    constructor(key: string, name: string, powerProductionMW: number, fuels: [Fuel], dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerProductionMW = powerProductionMW;
        this.fuels = fuels;
        this.dimensions = dimensions;
    }
}

class ItemProducer {
    type = BuildingType.ItemProducer as const;
    key: string;
    name: string;
    powerConsumption: PowerConsumption;
    craftTimeSecs: number;
    output: ItemPerMinute;
    dimensions: Dimensions;

    constructor(key: string,
        name: string,
        powerConsumption: PowerConsumption,
        craftTimeSecs: number,
        output: ItemPerMinute,
        dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerConsumption = powerConsumption;
        this.craftTimeSecs = craftTimeSecs;
        this.output = output;
        this.dimensions = dimensions;
    }
}

class ResourceExtractor {
    type = BuildingType.ResourceExtractor as const;
    key: string;
    name: string;
    powerConsumption: PowerConsumption;
    extractionRate: number;
    allowedResources: [string];
    extractorType: string;
    dimensions: Dimensions;

    constructor(key: string,
        name: string,
        powerConsumption: PowerConsumption,
        extractionRate: number,
        allowedResources: [string],
        extractorType: string,
        dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerConsumption = powerConsumption;
        this.extractionRate = extractionRate;
        this.allowedResources = allowedResources;
        this.extractorType = extractorType;
        this.dimensions = dimensions;
    }
}

class ResourceWellExtractor {
    key: string;
    name: string;
    powerConsumption: PowerConsumption;
    extractionRate: number;
    dimensions: Dimensions;

    constructor(key: string,
        name: string,
        powerConsumption: PowerConsumption,
        extractionRate: number,
        dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerConsumption = powerConsumption;
        this.extractionRate = extractionRate;
        this.dimensions = dimensions;
    }
}

class ResourceWell {
    type = BuildingType.ResourceWell as const;
    key: string;
    name: string;
    powerConsumption: PowerConsumption;
    allowedResources: [string];
    satelliteBuildings: [ResourceWellExtractor]
    extractorType: string;
    dimensions: Dimensions;

    constructor(key: string,
        name: string,
        powerConsumption: PowerConsumption,
        allowedResources: [string],
        satelliteBuildings: [ResourceWellExtractor],
        extractorType: string,
        dimensions: Dimensions) {
        this.key = key;
        this.name = name;
        this.powerConsumption = powerConsumption;
        this.allowedResources = allowedResources;
        this.satelliteBuildings = satelliteBuildings;
        this.extractorType = extractorType;
        this.dimensions = dimensions;
    }
}

type Building = Manufacturer | PowerGenerator | ItemProducer | ResourceExtractor | ResourceWell;

enum BuildingType {
    Manufacturer,
    PowerGenerator,
    ResourceExtractor,
    ResourceWell,
    ItemProducer
}

class RecipePower {
    minMW: number;
    maxMW: number;

    constructor(minMW: number, maxMW: number) {
        this.minMW = minMW;
        this.maxMW = maxMW;
    }
}

class Recipe {
    key: string;
    name: string;
    alternate: boolean;
    inputs: [ItemPerMinute];
    outputs: [ItemPerMinute];
    craftTimeSecs: number;
    building: Manufacturer;
    events: [string];
    power: RecipePower;

    constructor(key: string,
        name: string,
        alternate: boolean,
        inputs: [ItemPerMinute],
        outputs: [ItemPerMinute],
        craftTimeSecs: number,
        building: Manufacturer,
        events: [string],
        power: RecipePower) {
        this.key = key;
        this.name = name;
        this.alternate = alternate;
        this.inputs = inputs;
        this.outputs = outputs;
        this.craftTimeSecs = craftTimeSecs;
        this.building = building;
        this.events = events;
        this.power = power;
    }
}

class GameDatabase {
    items: Map<string, Item>;
    buildings: Map<string, Building>;
    recipes: Map<string, Recipe>;
    resourceLimits: Map<String, number>;

    constructor(items: Map<string, Item>, buildings: Map<string, Building>, recipes: Map<string, Recipe>, resourceLimits: Map<String, number>) {
        this.items = items;
        this.buildings = buildings;
        this.recipes = recipes;
        this.resourceLimits = resourceLimits;
    }
}