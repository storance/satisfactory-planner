#! /usr/bin/env python3

import argparse
import json
import re
from chardet.universaldetector import UniversalDetector

RECIPE_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGRecipe'"
]

MANUFACTURER_CLASSES = [
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableManufacturer'",
    "/Script/CoreUObject.Class'/Script/FactoryGame.FGBuildableManufacturerVariablePower'"
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
    fluid = FORM_MAPPING[definition['mForm']] in ('liquid', 'gas')
    item = {
        'key': definition['ClassName'],
        'name': definition['mDisplayName'],
        # Slight hack to make FICSMAS gifts to be considered a resource
        'resource': native_class == RESOURCE_CLASS or definition['ClassName'] == 'Desc_Gift_C',
        'state': FORM_MAPPING[definition['mForm']],
        'sink_points' : 0 if fluid else int(definition['mResourceSinkPoints'])
    }
    game_db['items'].append(item)

def parse_machine(definition: dict, game_db: dict):
    power = {
        'exponent': float(definition['mPowerConsumptionExponent'])
    }
    if 'mEstimatedMininumPowerConsumption' in definition:
        power['type'] = 'variable'
        power['min_mw'] = float(definition['mEstimatedMininumPowerConsumption'])
        power['max_mw'] = float(definition['mEstimatedMaximumPowerConsumption'])
    else:
        power['type'] = 'fixed'
        power['value_mw'] = float(definition['mPowerConsumption'])
    
    game_db['buildings'].append({
        'key': definition['ClassName'],
        'name': definition['mDisplayName'],
        'power_consumption' : power
    })

def parse_recipe(definition: dict, game_db: dict):
    produces_in = [building for building in parse_produce_in(definition) if building not in FILTER_BUILDINGS]
    if len(produces_in) == 0:
        return
    elif len(produces_in) > 1:
        raise ValueError(f"Recipe {definition['ClassName']} has multiple buildings: {produces_in}")
    
    check_machine_exists(game_db, produces_in[0])

    

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
    items = []

    line_start = 0
    for match in re.findall(r'(ItemClass=[^,]+\'"[^.]+\.(?P<item>[^,]+)"\',Amount=(?P<amount>\d+))', value):
        item = match[1]
        amount = int(match[2])

        # convert liquids and gases from liters to meters cubed
        if find_item(game_db, item)['state'] != 'solid':
            amount = amount / 1000.0

        items.append({
            item: amount
        })

    return items

def find_item(game_db: dict, item: str):
    for entry in game_db['items']:
        if entry['key'] == item:
            return entry
        
    raise ValueError(f'Item {item} is not in the game database.')

def check_machine_exists(game_db: dict, building: str):
    for entry in game_db['buildings']:
        if entry['key'] == building:
            return
        
    raise ValueError(f'Building {building} is not in the game database.')

def main():
    parser = argparse.ArgumentParser('create-game-db.py', description='Reads the Docs.json from Satisfactory and ' +
                                     'produces the db format expected by satisfactory-planner.')
    parser.add_argument('-d', '--docs-file', default='Docs.json', help='Path to Satisfactory\'s Docs.json')
    parser.add_argument('-f', '--output-file', default='game-db.json', help='Output file for the game database.  Use - for stdout.')
    
    args = parser.parse_args()
    game_db = {
        'by_product_blacklist': [
            'Desc_FluidCanister_C',
            'Desc_GasTank_C'
        ],
        'items': [],
        'buildings': [],
        'recipes': [],
        'resource_limits': {
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
            'Desc_Water_C': 9007199254740991.0,
            'Desc_Gift_C': 3.402823466e38
        }
    }
    encoding = detect_encoding(args.docs_file)
    with open(args.docs_file, encoding=encoding) as df:
        native_classes = json.load(df)

        for native_class in native_classes:
            if native_class['NativeClass'] in MANUFACTURER_CLASSES:
                for definition in native_class['Classes']:
                    parse_machine(definition, game_db)
            elif native_class['NativeClass'] in ITEM_CLASSES:
                for definition in native_class['Classes']:
                    parse_item(native_class['NativeClass'], definition, game_db)

        for native_class in native_classes:
            parser = None
            if native_class['NativeClass'] in RECIPE_CLASSES:
                for definition in native_class['Classes']:
                    parse_recipe(definition, game_db)

    game_db['items'].sort(key=lambda item: (not item['resource'], item['name'].lower()))
    game_db['buildings'].sort(key=lambda machine: machine['name'].lower())
    with open(args.output_file, 'w') as of:
        json.dump(game_db, of, indent=4)

if __name__ == '__main__':
    main()
