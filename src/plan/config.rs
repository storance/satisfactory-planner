use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;

use crate::game::{Item, Recipe};
use crate::plan::PlanError;

static DEFAULT_LIMITS: [(Item, f64); 13] = [
    (Item::Bauxite, 9780.0),
    (Item::CateriumOre, 12040.0),
    (Item::Coal, 30900.0),
    (Item::CopperOre, 28860.0),
    (Item::CrudeOil, 11700.0),
    (Item::IronOre, 70380.0),
    (Item::Limestone, 52860.0),
    (Item::NitrogenGas, 12000.0),
    (Item::RawQuartz, 10500.0),
    (Item::Sulfur, 6840.0),
    (Item::Uranium, 2100.0),
    (Item::Water, 9007199254740991.0),
    (Item::SAMOre, 0.0),
];

#[derive(Debug)]
enum RecipeMatcher {
    IncludeByAlternate(bool),
    IncludeByName(String),
    IncludeByOutputItem(Item),
    ExcludeByName(String),
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
            Self::IncludeByAlternate(is_alt) => recipe.alternate == *is_alt,
            Self::IncludeByName(recipe_name) => recipe.name.eq_ignore_ascii_case(recipe_name),
            Self::IncludeByOutputItem(item) => recipe.has_output_item(*item),
            Self::ExcludeByName(recipe_name) => recipe.name.eq_ignore_ascii_case(recipe_name),
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
        } else {
            Ok(RecipeMatcher::IncludeByName(v.into()))
        }
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        if let Some(field) = map.next_key::<&str>()? {
            if field.eq_ignore_ascii_case("exclude") {
                Ok(RecipeMatcher::IncludeByOutputItem(map.next_value()?))
            } else if field.eq_ignore_ascii_case("output") {
                Ok(RecipeMatcher::ExcludeByName(map.next_value()?))
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
    pub outputs: IndexMap<Item, f64>,
    pub recipes: Vec<Recipe>,
}

impl PlanConfig {
    pub fn from_file(file_path: &str, all_recipes: &[Recipe]) -> Result<PlanConfig, PlanError> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

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
            outputs: config.outputs,
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
