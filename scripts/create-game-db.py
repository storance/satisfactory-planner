#! /usr/bin/env python3

import argparse
import json
import re
import sys
from collections import OrderedDict
from chardet.universaldetector import UniversalDetector

RECIPE_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGRecipe'"
]

MANUFACTURER_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableManufacturer'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableManufacturerVariablePower'"
]

POWER_GENERATOR_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableGeneratorFuel'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableGeneratorNuclear'",
]

ITEM_PRODUCER_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableFactorySimpleProducer'"
]

RESOURCE_EXTRACTOR_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableResourceExtractor'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableWaterPump'",
]

RESOURCE_WELL_ACTIVATOR_CLASS = "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableFrackingActivator'"
RESOURCE_WELL_EXTRACTOR_CLASS = "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableFrackingExtractor'"

RESOURCE_CLASS = "/Script/CoreUObject.Class'/Script/FactoryGame.FGResourceDescriptor'"
BIOMASS_CLASS = "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptorBiomass'"
ITEM_CLASSES = [
    RESOURCE_CLASS,
    BIOMASS_CLASS,
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptor'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptorNuclearFuel'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeProjectile'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeInstantHit'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeSpreadshot'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGEquipmentDescriptor'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGConsumableDescriptor'"
]

# Buildings we want to filter out from mProduceIn for Recipes
FILTER_BUILDINGS = [
    'BP_WorkshopComponent_C',
    'BP_WorkBenchComponent_C',
    'Build_AutomatedWorkBench_C',
    'FGBuildableAutomatedWorkBench',
    'BP_BuildGun_C',
    'FGBuildGun',
]

FORM_MAPPING = {
    'RF_SOLID': 'solid',
    'RF_LIQUID': 'liquid',
    'RF_GAS': 'gas',
}

EVENT_MAPPING = {
    'EV_Christmas': 'FICSMAS'
}

ALTERNATE_PREFIX = 'Alternate: '
# List of recipes keys that should be considered as an alternate
# even though the name does not start with ALTERNATE_PREFIX
RECIPES_FORCE_ALTERNATE = [
    'Recipe_Alternate_Turbofuel_C',
]

BUILDING_SIZES = {
    'Desc_SmelterMk1_C': {
        'widthM':  6,
        'lengthM': 9,
        'heightM':  9,
    },
    'Desc_ConstructorMk1_C': {
        'widthM':  7.9,
        'lengthM': 9.9,
        'heightM':  8,
    },
    'Desc_AssemblerMk1_C': {
        'widthM':  10,
        'lengthM': 15,
        'heightM':  11,
    },
    'Desc_FoundryMk1_C': {
        'widthM':  10,
        'lengthM': 9,
        'heightM':  9,
    },
    'Desc_ManufacturerMk1_C': {
        'widthM':  18,
        'lengthM': 20,
        'heightM':  12,
    },
    'Desc_OilRefinery_C': {
        'widthM':  10,
        'lengthM': 20,
        'heightM':  31,
    },
    'Desc_Packager_C': {
        'widthM':  8,
        'lengthM': 8,
        'heightM':  12,
    },
    'Desc_Blender_C': {
        'widthM':  18,
        'lengthM': 16,
        'heightM':  15,
    },
    'Desc_HadronCollider_C': {
        'widthM':  24,
        'lengthM': 38,
        'heightM':  32,
    },
    'Desc_TreeGiftProducer_C': {
        'widthM':  8,
        'lengthM': 8,
        'heightM':  18,
    },
    'Desc_GeneratorBiomass_C': {
        'widthM':  8,
        'lengthM': 8,
        'heightM':  7,
    },
    'Desc_GeneratorCoal_C': {
        'widthM':  10,
        'lengthM': 26,
        'heightM': 36,
    },
    'Desc_GeneratorFuel_C': {
        'widthM':  20,
        'lengthM': 20,
        'heightM': 27,
    },
    'Desc_GeneratorNuclear_C': {
        'widthM':  36,
        'lengthM': 43,
        'heightM': 49,
    },
    'Desc_MinerMk1_C': {
        'widthM':  6,
        'lengthM': 14,
        'heightM': 18,
    },
    'Desc_MinerMk2_C': {
        'widthM':  6,
        'lengthM': 14,
        'heightM': 18,
    },
    'Desc_MinerMk3_C': {
        'widthM':  6,
        'lengthM': 14,
        'heightM': 18,
    },
    'Desc_OilPump_C': {
        'widthM':  8,
        'lengthM': 14,
        'heightM': 20,
    },
    'Desc_FrackingSmasher_C': {
        'widthM':  20,
        'lengthM': 20,
        'heightM': 23,
    },
    'Desc_FrackingExtractor_C': {
        'widthM':  4,
        'lengthM': 4,
        'heightM': 5,
    },
    'Desc_WaterPump_C': {
        'widthM':  20,
        'lengthM': 19.5,
        'heightM': 26,
    },
}

ITEM_PRODUCER_OUTPUTS = {
    'Desc_TreeGiftProducer_C' : {
        'item': 'Desc_Gift_C',
        'amount': 1
    }
}

RESOURCE_MAP_LIMITS = {
    'Desc_OreBauxite_C': 9780.0,
    'Desc_OreGold_C': 12040.0,
    'Desc_Coal_C': 30900.0,
    'Desc_OreCopper_C': 28860.0,
    'Desc_LiquidOil_C': 11700.0,
    'Desc_OreIron_C': 70380.0,
    'Desc_Stone_C': 52860.0,
    'Desc_NitrogenGas_C': 12000.0,
    'Desc_RawQuartz_C': 10500.0,
    'Desc_Sulfur_C': 6840.0,
    'Desc_OreUranium_C': 2100.0,
    'Desc_Water_C': 9007199254740991.0
}

def detect_encoding(file):
    detector = UniversalDetector()
    with open(file, 'rb') as f:
        for line in open(file, 'rb'):
            detector.feed(line)
            if detector.done:
                break
        detector.close()

        return detector.result['encoding']


def parse_item(native_class, definition, game_db):
    state = FORM_MAPPING[definition['mForm']]
    fluid = state in ('liquid', 'gas')
    resource = native_class == RESOURCE_CLASS
    energy = float(definition['mEnergyValue'])
    # for fluids, the energy value is per liter of fluid but we want per cubic meter
    if fluid:
        energy *= 1000

    item = {
        'key': definition['ClassName'],
        'name': definition['mDisplayName'],
        'resource': resource,
        'state': state,
        'sinkPoints': 0 if fluid else int(definition['mResourceSinkPoints']),
        'energyMJ': energy
    }
    game_db['items'].append(item)

    if native_class == BIOMASS_CLASS and energy > 0 and not fluid:
        game_db['biomass'].append(item)

def parse_manufacturer_building(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    game_db['buildings'].append({
        'type': 'manufacturer',
        'key': building_key,
        'name': definition['mDisplayName'],
        'powerConsumption': parse_power_consumption(definition),
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_power_consumption(definition):
    if 'mEstimatedMininumPowerConsumption' in definition:
        return {
            'type':  'variable',
            'minMW':  int(float(definition['mEstimatedMininumPowerConsumption'])),
            'maxMW':  int(float(definition['mEstimatedMaximumPowerConsumption'])),
            'exponent': float(definition['mPowerConsumptionExponent'])
        }
    else:
        return {
            'type': 'fixed',
            'valueMW':  int(float(definition['mPowerConsumption'])),
            'exponent': float(definition['mPowerConsumptionExponent'])
        }

def parse_power_generator(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    supplemental_ratio = float(definition['mSupplementalToPowerRatio'])
    power_production = int(float(definition['mPowerProduction']))
    game_db['buildings'].append({
        'type': 'power_generator',
        'key': building_key,
        'name': definition['mDisplayName'],
        'fuels': parse_fuels(definition['mFuel'], supplemental_ratio, power_production, game_db),
        'powerProductionMW': power_production,
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_item_producer(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    game_db['buildings'].append({
        'type': 'item_producer',
        'key': building_key,
        'name': definition['mDisplayName'],
        'craftTimeSecs': float(definition['mTimeToProduceItem']),
        'output': ITEM_PRODUCER_OUTPUTS[building_key],
        'powerConsumption': parse_power_consumption(definition),
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_resource_extractor(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    extraction_cycle_time = float(definition['mExtractCycleTime'])
    amount = int(definition['mItemsPerCycle'])

    allowed_forms = parse_forms(definition['mAllowedResourceForms'])
    if 'liquid' in allowed_forms or 'gas' in allowed_forms:
        amount /= 1000.0

    if definition['mOnlyAllowCertainResources'] == 'True':
        allowed_resources = parse_class_list(definition['mAllowedResources'])
    else:
        allowed_resources = [item['key'] for item in game_db['items'] 
                             if item['resource'] and item['state'] in allowed_forms]
        
    extractor_type = definition['mExtractorTypeName']
    if extractor_type == 'None':
        extractor_type = None

    game_db['buildings'].append({
        'type': 'resource_extractor',
        'key': building_key,
        'name': definition['mDisplayName'],
        'extractionRate': amount * 60.0 / extraction_cycle_time,
        'extractorType': extractor_type,
        'allowedResources': allowed_resources,
        'powerConsumption': parse_power_consumption(definition),
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_resource_well(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')

    allowed_forms = parse_forms(definition['mAllowedResourceForms'])
    if definition['mOnlyAllowCertainResources'] == 'True':
        allowed_resources = parse_class_list(definition['mAllowedResources'])
    else:
        allowed_resources = [item['key'] for item in game_db['items'] 
                             if item['resource'] and item['state'] in allowed_forms]
        
    extractor_type = definition['mExtractorTypeName']
    if extractor_type == 'None':
        extractor_type = None

    game_db['buildings'].append({
        'type': 'resource_well',
        'key': building_key,
        'name': definition['mDisplayName'],
        'allowedResources': allowed_resources,
        'extractorType': extractor_type,
        'satelliteBuildings': [],
        'powerConsumption': parse_power_consumption(definition),
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_resource_well_extractor(definition, game_db):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    extraction_cycle_time = float(definition['mExtractCycleTime'])
    amount = int(definition['mItemsPerCycle'])

    allowed_forms = parse_forms(definition['mAllowedResourceForms'])
    if 'liquid' in allowed_forms or 'gas' in allowed_forms:
        amount /= 1000.0

    parent = next((b for b in game_db['buildings'] if b['key'] == 'Desc_FrackingSmasher_C'), None)
    if parent is None:
        raise ValueError('Could not find parent building Desc_FrackingSmasher_C')
    parent['satelliteBuildings'].append({
        'key': building_key,
        'name': definition['mDisplayName'],
        'extractionRate': amount * 60.0 / extraction_cycle_time,
        'powerConsumption': parse_power_consumption(definition),
        'dimensions': BUILDING_SIZES.get(building_key)
    })

def parse_forms(forms):
    if len(forms) == 0:
        return []

    return [FORM_MAPPING[form] for form in forms.strip('()').split(",") if form in FORM_MAPPING]

def parse_fuels(fuels, supplemental_ratio, power_production, game_db):
    parsed_fuels = []

    for fuel in fuels:
        fuel_item_key = fuel['mFuelClass']
        supplemental_item_key = fuel['mSupplementalResourceClass']
        by_product_item_key = fuel['mByproduct']
        by_product_amount = fuel['mByproductAmount']

        if fuel_item_key == 'FGItemDescriptorBiomass':
            for fuel_item in game_db['biomass']:
                parsed_fuel = build_fuel(fuel_item,
                                         supplemental_item_key,
                                         by_product_item_key,
                                         by_product_amount,
                                         supplemental_ratio,
                                         power_production,
                                         game_db)
                if parsed_fuel is not None:
                    parsed_fuels.append(parsed_fuel)
        else:
            parsed_fuel = build_fuel(find_item(game_db, fuel_item_key),
                                     supplemental_item_key,
                                     by_product_item_key,
                                     by_product_amount,
                                     supplemental_ratio,
                                     power_production,
                                     game_db)
            if parsed_fuel is not None:
                parsed_fuels.append(parsed_fuel)

    return parsed_fuels


def build_fuel(fuel_item,
               supplemental_item_key,
               by_product_item_key,
               by_product_amount,
               supplemental_ratio,
               power_production,
               game_db):
    if fuel_item['energyMJ'] == 0:
        return None

    fuel = {
        'fuel': {
            'item': fuel_item['key'],
            'amount': 1
        },
        'burnTimeSecs': fuel_item['energyMJ'] / power_production
    }

    if supplemental_item_key not in [None, ""]:
        supplemental_item = find_item(game_db, supplemental_item_key)
        amount = fuel_item['energyMJ'] * supplemental_ratio
        if supplemental_item['state'] != 'solid':
            amount /= 1000.0

        fuel['supplemental'] = {
            'item': supplemental_item_key,
            'amount': amount
        }

    if by_product_item_key not in [None, ""] and by_product_amount not in [None, ""]:
        by_product_item = find_item(game_db, by_product_item_key)
        amount = float(by_product_amount)
        if by_product_item['state'] != 'solid':
            amount /= 1000.0

        fuel['byProduct'] = {
            'item': by_product_item_key,
            'amount': float(by_product_amount)
        }

    return fuel


def parse_recipe(definition, game_db):
    produces_in = [building for building in parse_class_list(definition['mProducedIn']) 
                   if building not in FILTER_BUILDINGS]
    if len(produces_in) == 0:
        return
    elif len(produces_in) > 1:
        raise ValueError(
            f"Recipe {definition['ClassName']} has multiple buildings: {produces_in}")

    produces_in = [building.replace('Build_', 'Desc_') for building in produces_in]
    check_building_exists(game_db, produces_in[0])

    recipe_key = definition['ClassName']
    display_name = definition['mDisplayName']
    alternate = recipe_key in RECIPES_FORCE_ALTERNATE
    craft_time_secs = float(definition['mManufactoringDuration'])
    power_constant = float(definition['mVariablePowerConsumptionConstant'])
    power_factor = float(definition['mVariablePowerConsumptionFactor'])
    if display_name.startswith(ALTERNATE_PREFIX):
        display_name = display_name[len(ALTERNATE_PREFIX):]
        alternate = True

    game_db['recipes'].append({
        'key': recipe_key,
        'name': display_name,
        'alternate': alternate,
        'inputs': parse_item_list(definition['mIngredients'], game_db),
        'outputs': parse_item_list(definition['mProduct'], game_db),
        'craftTimeSecs': craft_time_secs,
        'events': parse_events(definition['mRelevantEvents']),
        'building': produces_in[0],
        'powerConsumption': {
            'minMW': power_constant,
            'maxMW': power_constant + power_factor
        }
    })


def parse_class_list(class_list):
    building_regexp = r'"[^.]+\.(.+?)"'
    return re.findall(building_regexp, class_list)

def parse_events(events):
    if len(events) == 0:
        return []

    return [EVENT_MAPPING[event] for event in events.strip('()').split(",")]

def parse_item_list(value, game_db):
    items = []

    line_start = 0
    for match in re.findall(r'(ItemClass=[^,]+\'"[^.]+\.(?P<item>[^,]+)"\',Amount=(?P<amount>\d+))', value):
        item = match[1]
        amount = int(match[2])

        # convert liquids and gases from liters to meters cubed
        if find_item(game_db, item)['state'] != 'solid':
            amount = amount / 1000.0

        items.append({
            'item': item,
            'amount': amount
        })

    return items


def find_item(game_db: dict, item: str):
    for entry in game_db['items']:
        if entry['key'] == item:
            return entry

    raise ValueError(f'Item `{item}` is not in the game database.')


def check_building_exists(game_db: dict, building: str):
    for entry in game_db['buildings']:
        if entry['key'] == building:
            return

    raise ValueError(f'Building {building} is not in the game database.')


def prune_unused_items(game_db):
    used_items = set()
    for building in game_db['buildings']:
        if 'fuels' in building:
            for fuel in building['fuels']:
                used_items.add(fuel['fuel']['item'])
                if 'supplemental' in fuel:
                    used_items.add(fuel['supplemental']['item'])
                if 'byProduct' in fuel:
                    used_items.add(fuel['byProduct']['item'])

    for recipe in game_db['recipes']:
        used_items.update([input['item'] for input in recipe['inputs']])
        used_items.update([output['item'] for output in recipe['outputs']])

    game_db['items'] = [item for item in game_db['items'] if item['key'] in used_items]


def main():
    parser = argparse.ArgumentParser('create-game-db.py', description='Reads the Docs.json from Satisfactory and ' +
                                     'produces the json format expected by satisfactory-planner.')
    parser.add_argument('-d', '--docs-file', default='Docs.json',
                        help='Path to Satisfactory\'s Docs.json')
    parser.add_argument('-p', '--pretty-print', action='store_true',
                        help='Pretty print the outputted json.')
    parser.add_argument('-f', '--output-file', default='game-db.json',
                        help='Output file for the game database json.  Use - for stdout.')

    args = parser.parse_args()
    game_db = {
        'byProductBlacklist': [
            'Desc_FluidCanister_C',
            'Desc_GasTank_C'
        ],
        'items': [],
        'biomass': [],
        'buildings': [],
        'recipes': [],
        'resourceLimits': RESOURCE_MAP_LIMITS
    }
    encoding = detect_encoding(args.docs_file)
    with open(args.docs_file, encoding=encoding) as df:
        native_classes = json.load(df)

        # First pass parse items
        for native_class in native_classes:
            if native_class['NativeClass'] in ITEM_CLASSES:
                for definition in native_class['Classes']:
                    parse_item(
                        native_class['NativeClass'], definition, game_db)

        # second pass parse buildings
        for native_class in native_classes:
            if native_class['NativeClass'] in MANUFACTURER_CLASSES:
                for definition in native_class['Classes']:
                    parse_manufacturer_building(definition, game_db)
            elif native_class['NativeClass'] in POWER_GENERATOR_CLASSES:
                for definition in native_class['Classes']:
                    parse_power_generator(definition, game_db)
            elif native_class['NativeClass'] in ITEM_PRODUCER_CLASSES:
                for definition in native_class['Classes']:
                    parse_item_producer(definition, game_db)
            elif native_class['NativeClass'] in RESOURCE_EXTRACTOR_CLASSES:
                for definition in native_class['Classes']:
                    parse_resource_extractor(definition, game_db)
            elif native_class['NativeClass']  == RESOURCE_WELL_ACTIVATOR_CLASS:
                for definition in native_class['Classes']:
                    parse_resource_well(definition, game_db)

        # third pass parse resource well satellite buildings
        for native_class in native_classes:
            if native_class['NativeClass']  == RESOURCE_WELL_EXTRACTOR_CLASS:
                for definition in native_class['Classes']:
                    parse_resource_well_extractor(definition, game_db)

        # fourth pass parse recipes
        for native_class in native_classes:
            parser = None
            if native_class['NativeClass'] in RECIPE_CLASSES:
                for definition in native_class['Classes']:
                    parse_recipe(definition, game_db)

    prune_unused_items(game_db)

    game_db['items'].sort(key=lambda item: (
        not item['resource'], item['name'].lower()))
    game_db['buildings'].sort(key=lambda machine: machine['name'].lower())
    game_db['recipes'].sort(key=lambda recipe: recipe['name'].lower())
    del game_db['biomass']

    output_handle = None
    close_handle = False
    if args.output_file == '-':
        output_handle = sys.stdout
    else:
        output_handle = open(args.output_file, 'w')
        close_handle = True

    try:
        json.dump(game_db, output_handle,
                  indent=4 if args.pretty_print else None)
    finally:
        if close_handle:
            output_handle.close()


if __name__ == '__main__':
    main()
