export enum ItemState {
    Solid = "solid",
    Liquid = "liquid",
    Gas = "gas"
}

export class ItemPerMinute {
    item: string;
    amount: number;

    constructor(item: string, amount: number) {
        this.item = item;
        this.amount = amount;
    }
}

export class Item {
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

export enum PowerConsumptionType {
    Fixed = "fixed",
    Variable = "variable"
}

export class FixedPowerConsumption {
    type = PowerConsumptionType.Fixed as const;
    valueMW: number;
    exponent: number;

    constructor(valueMW: number, exponent: number) {
        this.valueMW = valueMW;
        this.exponent = exponent;
    }
}

export class VariablePowerConsumption {
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

export type PowerConsumption = FixedPowerConsumption | VariablePowerConsumption;

export class Dimensions {
    lengthM: number;
    widthM: number;
    heightM: number;

    constructor(lengthM: number, widthM: number, heightM: number) {
        this.lengthM = lengthM;
        this.widthM = widthM;
        this.heightM = heightM;

    }
}

export class Manufacturer {
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

export class Fuel {
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

export class PowerGenerator {
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

export class ItemProducer {
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

export class ResourceExtractor {
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

export class ResourceWellExtractor {
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

export class ResourceWell {
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

export type Building = Manufacturer | PowerGenerator | ItemProducer | ResourceExtractor | ResourceWell;

export enum BuildingType {
    Manufacturer = "manufacturer",
    PowerGenerator = "power_generator",
    ResourceExtractor = "resource_extractor",
    ResourceWell = "resource_well",
    ItemProducer = "item_producer"
}

export class RecipePower {
    minMW: number;
    maxMW: number;

    constructor(minMW: number, maxMW: number) {
        this.minMW = minMW;
        this.maxMW = maxMW;
    }
}

export class Recipe {
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

export class GameDatabase {
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

export function parse_game_db(json: {items: Item[], buildings: Building[], recipes: Recipe[], resourceLimits: Map<string, number>}): GameDatabase {
    let items = new Map();
    let buildings = new Map();
    let recipes = new Map();

    for (var item of json['items']) {
        items.set(item.key, item);
    }

    for (var building of json['buildings']) {
        buildings.set(building.key, building);
    }

    for (var recipe of json['recipes']) {
        recipes.set(recipe.key, recipe);
    }

    return new GameDatabase(items, buildings, recipes, json.resourceLimits);
}