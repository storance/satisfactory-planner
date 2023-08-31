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

RESOURCE_CLASS = "/Script/CoreUObject.Class'/Script/FactoryGame.FGResourceDescriptor'"
ITEM_CLASSES = [
    RESOURCE_CLASS,
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptor'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptorBiomass'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGItemDescriptorNuclearFuel'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeProjectile'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeInstantHit'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGAmmoTypeSpreadshot'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGEquipmentDescriptor'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGConsumableDescriptor'"
]

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
RECIPES_FORCE_ALTERNATE = [
    'Recipe_Alternate_Turbofuel_C', 
]

BUILDING_SIZES = {
    'Desc_SmelterMk1_C': {
        'width_m':  6,
        'length_m': 9,
        'height_m':  9,
    },
    'Desc_ConstructorMk1_C': {
        'width_m':  7.9,
        'length_m': 9.9,
        'height_m':  8,
    },
    'Desc_AssemblerMk1_C': {
        'width_m':  10,
        'length_m': 15,
        'height_m':  11,
    },
    'Desc_FoundryMk1_C': {
        'width_m':  10,
        'length_m': 9,
        'height_m':  9,
    },
    'Desc_ManufacturerMk1_C': {
        'width_m':  18,
        'length_m': 20,
        'height_m':  12,
    },
    'Desc_OilRefinery_C': {
        'width_m':  10,
        'length_m': 20,
        'height_m':  31,
    },
    'Desc_Packager_C': {
        'width_m':  8,
        'length_m': 8,
        'height_m':  12,
    },
    'Desc_Blender_C': {
        'width_m':  18,
        'length_m': 16,
        'height_m':  15,
    },
    'Desc_HadronCollider_C': {
        'width_m':  24,
        'length_m': 38,
        'height_m':  32,
    },
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

def detect_encoding(file: str):
    detector = UniversalDetector()
    with open(file, 'rb') as f:
        for line in open(file, 'rb'):
            detector.feed(line)
            if detector.done:
                break
        detector.close()

        return detector.result['encoding']

def parse_item(native_class:str, definition: dict, game_db: dict):
    match = re.match(r'^.+\'[^.]+\.(.+)\'$', native_class)
    if match is None:
        raise ValueError(f"Could not parse descriptor from {native_class}")
    descriptor = match.group(1)

    fluid = FORM_MAPPING[definition['mForm']] in ('liquid', 'gas')
    # Slight hack to make FICSMAS gifts to be considered a resource
    resource = native_class == RESOURCE_CLASS
    energy = float(definition['mEnergyValue'])
    # for fluids, the energy value is per liter of fluid but we want per cubic meter
    if fluid:
        energy *= 1000
    
    item = {
        'key': definition['ClassName'],
        'name': definition['mDisplayName'],
        'resource': resource,
        'state': FORM_MAPPING[definition['mForm']],
        'sink_points': 0 if fluid else int(definition['mResourceSinkPoints']),
        'energy_mj': energy,
        'bit_mask': None if not resource else next_bit_mask()
    }
    game_db['items'].append(item)
    if descriptor not in game_db['items_by_descriptor']:
        game_db['items_by_descriptor'][descriptor] = []
    game_db['items_by_descriptor'][descriptor].append(item)

resource_number = 0
def next_bit_mask():
    global resource_number
    bit_mask = 1 << resource_number
    resource_number += 1

    return bit_mask

def parse_manufacturer_building(definition: dict, game_db: dict):
    power = {
        'exponent': float(definition['mPowerConsumptionExponent'])
    }
    if 'mEstimatedMininumPowerConsumption' in definition:
        power['type'] = 'variable'
        power['min_mw'] = int(float(definition['mEstimatedMininumPowerConsumption']))
        power['max_mw'] = int(float(definition['mEstimatedMaximumPowerConsumption']))
    else:
        power['type'] = 'fixed'
        power['value_mw'] = int(float(definition['mPowerConsumption']))
    
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    game_db['buildings'].append({
        'type': 'manufacturer',
        'key': building_key,
        'name': definition['mDisplayName'],
        'power_consumption' : power,
        'dimensions' : BUILDING_SIZES.get(building_key)
    })

def parse_power_generator_building(definition: dict, game_db: dict):
    building_key = definition['ClassName'].replace('Build_', 'Desc_')
    supplemental_ratio = float(definition['mSupplementalToPowerRatio'])
    power_production = int(float(definition['mPowerProduction']))
    game_db['buildings'].append({
        'type': 'power_generator',
        'key': building_key,
        'name': definition['mDisplayName'],
        'fuels': parse_fuels(definition['mFuel'], supplemental_ratio, power_production, game_db),
        'power_production_mw': power_production,
        'dimensions' : None
    })

def parse_fuels(fuels, supplemental_ratio: float, power_production: int, game_db: dict) -> list:
    parsed_fuels = []

    for fuel in fuels:
        fuel_item_key = fuel['mFuelClass']
        supplemental_item_key = fuel['mSupplementalResourceClass']
        by_product_item_key = fuel['mByproduct']
        by_product_amount = fuel['mByproductAmount']

        if fuel_item_key in game_db['items_by_descriptor']:
            for fuel_item in game_db['items_by_descriptor'][fuel_item_key]:
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
    if fuel_item['energy_mj'] == 0:
        return None
    
    fuel = {
        'inputs': OrderedDict(),
        'burn_time_secs' : fuel_item['energy_mj'] / power_production
    }
    
    fuel['inputs'][fuel_item['key']] = 1

    if supplemental_item_key not in [None, ""]:
        supplemental_item = find_item(game_db, supplemental_item_key)
        amount = fuel_item['energy_mj'] * supplemental_ratio
        if supplemental_item['state'] != 'solid':
            amount /= 1000

        fuel['inputs'][supplemental_item_key] = amount
    
    if by_product_item_key not in [None, ""] and by_product_amount not in [None, ""]:
        by_product_item = find_item(game_db, by_product_item_key)
        amount = float(by_product_amount)
        if by_product_item['state'] != 'solid':
            amount /= 1000

        fuel['outputs'] = {by_product_item_key: float(by_product_amount)}

    return fuel

def parse_recipe(definition: dict, game_db: dict):
    produces_in = [building for building in parse_produce_in(definition) if building not in FILTER_BUILDINGS]
    if len(produces_in) == 0:
        return
    elif len(produces_in) > 1:
        raise ValueError(f"Recipe {definition['ClassName']} has multiple buildings: {produces_in}")
    
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
        'inputs': parse_item_list(game_db, definition['mIngredients']),
        'outputs': parse_item_list(game_db, definition['mProduct']),
        'craft_time_secs': craft_time_secs,
        'events': parse_events(definition['mRelevantEvents']),
        'building': produces_in[0],
        'power_consumption': {
            'min_mw': power_constant,
            'max_mw': power_constant + power_factor
        }
    })

def parse_produce_in(definition):
    building_regexp = r'"[^.]+\.(.+?)"'
    return re.findall(building_regexp, definition['mProducedIn'])

def parse_events(events: str):
    if len(events) == 0:
        return []
    
    return [EVENT_MAPPING[event] for event in events.strip('()').split(",")]

def parse_item_list(game_db: dict, value: str):
    items = OrderedDict()

    line_start = 0
    for match in re.findall(r'(ItemClass=[^,]+\'"[^.]+\.(?P<item>[^,]+)"\',Amount=(?P<amount>\d+))', value):
        item = match[1]
        amount = int(match[2])

        # convert liquids and gases from liters to meters cubed
        if find_item(game_db, item)['state'] != 'solid':
            amount = amount / 1000.0

        items[item] = amount

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
                used_items.update(fuel['inputs'].keys())
                if 'outputs' in fuel:
                    used_items.update(fuel['outputs'].keys())

    for recipe in game_db['recipes']:
        used_items.update(recipe['inputs'].keys())
        used_items.update(recipe['outputs'].keys())

    game_db['items'] = [item for item in game_db['items'] if item['key'] in used_items]

def main():
    parser = argparse.ArgumentParser('create-game-db.py', description='Reads the Docs.json from Satisfactory and ' +
                                     'produces the json format expected by satisfactory-planner.')
    parser.add_argument('-d', '--docs-file', default='Docs.json', help='Path to Satisfactory\'s Docs.json')
    parser.add_argument('-p', '--pretty-print', action='store_true', help='Pretty print the outputted json.')
    parser.add_argument('-f', '--output-file', default='game-db.json', help='Output file for the game database json.  Use - for stdout.')
    
    args = parser.parse_args()
    game_db = {
        'by_product_blacklist': [
            'Desc_FluidCanister_C',
            'Desc_GasTank_C'
        ],
        'items': [],
        'items_by_descriptor': {},
        'buildings': [],
        'recipes': [],
        'resource_limits': RESOURCE_MAP_LIMITS
    }
    encoding = detect_encoding(args.docs_file)
    with open(args.docs_file, encoding=encoding) as df:
        native_classes = json.load(df)

        # First pass parse items
        for native_class in native_classes:
            if native_class['NativeClass'] in ITEM_CLASSES:
                for definition in native_class['Classes']:
                    parse_item(native_class['NativeClass'], definition, game_db)
        
        # second pass parse buildings
        for native_class in native_classes:
            if native_class['NativeClass'] in MANUFACTURER_CLASSES:
                for definition in native_class['Classes']:
                    parse_manufacturer_building(definition, game_db)
            elif native_class['NativeClass'] in POWER_GENERATOR_CLASSES:
                for definition in native_class['Classes']:
                    parse_power_generator_building(definition, game_db)

        # third pass parse recipes
        for native_class in native_classes:
            parser = None
            if native_class['NativeClass'] in RECIPE_CLASSES:
                for definition in native_class['Classes']:
                    parse_recipe(definition, game_db)
    
    prune_unused_items(game_db)

    game_db['items'].sort(key=lambda item: (not item['resource'], item['name'].lower()))
    game_db['buildings'].sort(key=lambda machine: machine['name'].lower())
    game_db['recipes'].sort(key=lambda recipe: recipe['name'].lower())
    del game_db['items_by_descriptor']

    output_handle = None
    close_handle = False
    if args.output_file == '-':
        output_handle = sys.stdout
    else:
        output_handle = open(args.output_file, 'w')
        close_handle = True

    try:
        json.dump(game_db, output_handle, indent=4 if args.pretty_print else None)
    finally:
        if close_handle:
            output_handle.close()

if __name__ == '__main__':
    main()
