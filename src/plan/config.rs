use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;

use crate::game::{Item, ItemValuePair, Recipe};
use crate::plan::PlanError;

pub static DEFAULT_LIMITS: [(Item, f64); 13] = [
    (Item::Bauxite, 9780.0),
    (Item::CateriumOre, 12040.0),
    (Item::Coal, 30900.0),
    (Item::CopperOre, 28860.0),
    (Item::Oil, 11700.0),
    (Item::IronOre, 70380.0),
    (Item::Limestone, 52860.0),
    (Item::NitrogenGas, 12000.0),
    (Item::RawQuartz, 10500.0),
    (Item::Sulfur, 6840.0),
    (Item::Uranium, 2100.0),
    (Item::Water, 9007199254740991.0),
    (Item::SAMOre, 0.0),
];

#[derive(Debug, Clone, Eq, PartialEq)]
enum RecipeMatcher {
    IncludeByAlternate(bool),
    IncludeByName(String),
    IncludeByOutputItem(Item),
    ExcludeByName(String),
    IncludeFicsmas,
}

impl RecipeMatcher {
    pub fn all_matching(&self, all_recipes: &[Recipe]) -> Result<Vec<Recipe>, PlanError> {
        self.to_result(
            all_recipes
                .iter()
                .cloned()
                .filter(|recipe| self.matches(recipe))
                .collect(),
        )
    }

    pub fn is_include(&self) -> bool {
        match self {
            Self::IncludeByAlternate(..) => true,
            Self::IncludeByName(..) => true,
            Self::IncludeByOutputItem(..) => true,
            Self::IncludeFicsmas => true,
            Self::ExcludeByName(..) => false,
        }
    }

    fn to_result(&self, result: Vec<Recipe>) -> Result<Vec<Recipe>, PlanError> {
        match self {
            Self::IncludeByName(name) => {
                if result.is_empty() {
                    Err(PlanError::InvalidRecipe(name.clone()))
                } else {
                    Ok(result)
                }
            }
            Self::ExcludeByName(name) => {
                if result.is_empty() {
                    Err(PlanError::InvalidRecipe(name.clone()))
                } else {
                    Ok(result)
                }
            }
            _ => Ok(result),
        }
    }

    pub fn matches(&self, recipe: &Recipe) -> bool {
        match self {
            Self::IncludeByAlternate(is_alt) => !recipe.ficsmas && recipe.alternate == *is_alt,
            Self::IncludeByName(recipe_name) => recipe.name.eq_ignore_ascii_case(recipe_name),
            Self::IncludeByOutputItem(item) => recipe.has_output_item(*item),
            Self::ExcludeByName(recipe_name) => recipe.name.eq_ignore_ascii_case(recipe_name),
            Self::IncludeFicsmas => recipe.ficsmas,
        }
    }
}

impl<'de> Deserialize<'de> for RecipeMatcher {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(RecipeMatcherVisitor)
    }
}

struct RecipeMatcherVisitor;

impl<'de> Visitor<'de> for RecipeMatcherVisitor {
    type Value = RecipeMatcher;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "base, alternates, recipe name, output: item name, exclude: recipe name"
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v.eq_ignore_ascii_case("base") {
            Ok(RecipeMatcher::IncludeByAlternate(false))
        } else if v.eq_ignore_ascii_case("alternates") || v.eq_ignore_ascii_case("alts") {
            Ok(RecipeMatcher::IncludeByAlternate(true))
        } else if v.eq_ignore_ascii_case("ficsmas") {
            Ok(RecipeMatcher::IncludeFicsmas)
        } else {
            Ok(RecipeMatcher::IncludeByName(v.into()))
        }
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        if let Some(field) = map.next_key::<String>()? {
            if field.eq_ignore_ascii_case("exclude") {
                Ok(RecipeMatcher::ExcludeByName(map.next_value()?))
            } else if field.eq_ignore_ascii_case("output") {
                Ok(RecipeMatcher::IncludeByOutputItem(map.next_value()?))
            } else {
                Err(serde::de::Error::custom(format!(
                    "Unknown recipe matcher {}",
                    field
                )))
            }
        } else {
            Err(serde::de::Error::custom("Missing matcher and value"))
        }
    }
}

#[derive(Debug, Deserialize)]
struct PlanConfigDefinition {
    #[serde(default)]
    inputs: HashMap<Item, f64>,
    outputs: IndexMap<Item, f64>,
    enabled_recipes: Vec<RecipeMatcher>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub inputs: HashMap<Item, f64>,
    pub outputs: Vec<ItemValuePair>,
    pub recipes: Vec<Recipe>,
}

#[allow(dead_code)]
impl PlanConfig {
    pub fn new(outputs: Vec<ItemValuePair>, recipes: Vec<Recipe>) -> Self {
        PlanConfig {
            inputs: DEFAULT_LIMITS.iter().copied().collect(),
            outputs,
            recipes,
        }
    }

    pub fn with_inputs(
        inputs: HashMap<Item, f64>,
        outputs: Vec<ItemValuePair>,
        recipes: Vec<Recipe>,
    ) -> Self {
        PlanConfig {
            inputs: DEFAULT_LIMITS.iter().copied().chain(inputs).collect(),
            outputs,
            recipes,
        }
    }

    pub fn from_file(file_path: &str, all_recipes: &[Recipe]) -> anyhow::Result<Self> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

        Ok(Self::convert(config, all_recipes)?)
    }

    fn convert(config: PlanConfigDefinition, all_recipes: &[Recipe]) -> Result<Self, PlanError> {
        let mut inputs: HashMap<Item, f64> = DEFAULT_LIMITS.iter().copied().collect();
        inputs.extend(config.inputs);

        // validate there are no extractable resources in the outputs list
        for item in config.outputs.keys() {
            if item.is_extractable() {
                return Err(PlanError::UnexpectedRawOutputItem(*item));
            }
        }

        let mut recipes: Vec<Recipe> = Vec::new();
        let mut recipe_exclusions: Vec<Recipe> = Vec::new();
        for matcher in &config.enabled_recipes {
            let matching_recipes = matcher.all_matching(all_recipes)?;
            if matcher.is_include() {
                recipes.extend(matching_recipes);
            } else {
                recipe_exclusions.extend(matching_recipes);
            }
        }

        recipes.retain(|recipe| !recipe_exclusions.contains(recipe));
        recipes.sort_by(|a, b| a.name.cmp(&b.name));
        recipes.dedup();

        Ok(PlanConfig {
            inputs,
            outputs: config
                .outputs
                .iter()
                .map(|(item, value)| ItemValuePair::new(*item, *value))
                .collect(),
            recipes,
        })
    }

    pub fn find_recipe_by_output(&self, output: Item) -> impl Iterator<Item = &Recipe> {
        self.recipes
            .iter()
            .filter(move |recipe| recipe.has_output_item(output))
    }

    pub fn find_recipe_by_input(&self, input: Item) -> impl Iterator<Item = &Recipe> {
        self.recipes
            .iter()
            .filter(move |recipe| recipe.has_input_item(input))
    }

    pub fn has_input(&self, item: Item) -> bool {
        self.find_input(item) > 0.0
    }

    pub fn find_input(&self, item: Item) -> f64 {
        self.inputs.get(&item).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod test {
    use crate::game::{Machine, RecipeIO};

    use super::*;

    #[test]
    fn recipe_matcher_deserialize() {
        let yaml = "#
            - base
            - alts
            - alternates
            - Pure Iron Ingot
            - exclude: Iron Alloy Ingot
            - output: Copper Ingot
            - ficsmas
        #";

        let result: Result<Vec<RecipeMatcher>, serde_yaml::Error> = serde_yaml::from_str(yaml);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            vec![
                RecipeMatcher::IncludeByAlternate(false),
                RecipeMatcher::IncludeByAlternate(true),
                RecipeMatcher::IncludeByAlternate(true),
                RecipeMatcher::IncludeByName("Pure Iron Ingot".into()),
                RecipeMatcher::ExcludeByName("Iron Alloy Ingot".into()),
                RecipeMatcher::IncludeByOutputItem(Item::CopperIngot),
                RecipeMatcher::IncludeFicsmas
            ]
        );
    }

    #[test]
    fn recipe_matcher_include_by_alternate_false_matches() {
        let base_matcher = RecipeMatcher::IncludeByAlternate(false);
        let pure_iron_ingot = get_pure_iron_ingot_recipe();
        let copper_ingot = get_copper_ingot_recipe();
        let actual_snow = get_actual_snow_recipe();

        assert!(!base_matcher.matches(&actual_snow));
        assert!(!base_matcher.matches(&pure_iron_ingot));
        assert!(base_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_alternate_true_matches() {
        let alts_matcher = RecipeMatcher::IncludeByAlternate(true);
        let pure_iron_ingot = get_pure_iron_ingot_recipe();
        let copper_ingot = get_copper_ingot_recipe();
        let actual_snow = get_actual_snow_recipe();

        assert!(!alts_matcher.matches(&actual_snow));
        assert!(alts_matcher.matches(&pure_iron_ingot));
        assert!(!alts_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_name_matches() {
        let name_matcher = RecipeMatcher::IncludeByName("Pure Iron Ingot".into());
        let name_lc_matcher = RecipeMatcher::IncludeByName("pure iron ingot".into());
        let pure_iron_ingot = get_pure_iron_ingot_recipe();
        let copper_ingot = get_copper_ingot_recipe();

        assert!(name_matcher.matches(&pure_iron_ingot));
        assert!(!name_matcher.matches(&copper_ingot));

        assert!(name_lc_matcher.matches(&pure_iron_ingot));
        assert!(!name_lc_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_exclude_by_name_matches() {
        let exclude_name_matcher = RecipeMatcher::ExcludeByName("Copper Ingot".into());
        let pure_iron_ingot = get_pure_iron_ingot_recipe();
        let copper_ingot = get_copper_ingot_recipe();

        assert!(!exclude_name_matcher.matches(&pure_iron_ingot));
        assert!(exclude_name_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_output_item_matches() {
        let output_item_matcher = RecipeMatcher::IncludeByOutputItem(Item::CopperIngot);
        let pure_iron_ingot = get_pure_iron_ingot_recipe();
        let copper_ingot = get_copper_ingot_recipe();

        assert!(!output_item_matcher.matches(&pure_iron_ingot));
        assert!(output_item_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_ficsmas_matches() {
        let ficsmas_matcher = RecipeMatcher::IncludeFicsmas;
        let actual_snow = get_actual_snow_recipe();
        let copper_ingot = get_copper_ingot_recipe();

        assert!(ficsmas_matcher.matches(&actual_snow));
        assert!(!ficsmas_matcher.matches(&copper_ingot));
    }

    fn get_copper_ingot_recipe() -> Recipe {
        Recipe {
            name: "Copper Ingot".into(),
            alternate: false,
            ficsmas: false,
            inputs: vec![RecipeIO::new(Item::CopperOre, 1.0, 30.0)],
            outputs: vec![RecipeIO::new(Item::CopperIngot, 1.0, 30.0)],
            power_multiplier: 1.0,
            craft_time: 2,
            machine: Machine::Smelter,
        }
    }

    fn get_actual_snow_recipe() -> Recipe {
        Recipe {
            name: "Actual Snow".into(),
            alternate: false,
            ficsmas: true,
            inputs: vec![RecipeIO::new(Item::FicsmasGift, 5.0, 25.0)],
            outputs: vec![RecipeIO::new(Item::ActualSnow, 2.0, 10.0)],
            power_multiplier: 1.0,
            craft_time: 12,
            machine: Machine::Constructor,
        }
    }

    fn get_pure_iron_ingot_recipe() -> Recipe {
        Recipe {
            name: "Pure Iron Ingot".into(),
            alternate: true,
            ficsmas: false,
            inputs: vec![
                RecipeIO::new(Item::IronOre, 7.0, 35.0),
                RecipeIO::new(Item::Water, 5.0, 20.0),
            ],
            outputs: vec![RecipeIO::new(Item::IronIngot, 13.0, 65.0)],
            power_multiplier: 1.0,
            craft_time: 12,
            machine: Machine::Refinery,
        }
    }
}
