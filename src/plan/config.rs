use indexmap::IndexMap;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use thiserror::Error;

use crate::game::{GameDatabase, Item, ItemPerMinute, Recipe};
use crate::utils::FloatType;

#[derive(Error, Debug, Eq, PartialEq)]
pub enum PlanError {
    #[error("No recipe exists with the name or key `{0}`")]
    UnknownRecipe(String),
    #[error("No item exists with the name or key `{0}`")]
    UnknownItem(String),
    #[error("The resource `{0}` is not allowed in outputs.")]
    UnexpectedResource(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum RecipeMatcher {
    IncludeBase,
    IncludeAlternate,
    IncludeByNameOrKey(String),
    IncludeByOutputItem(String),
    ExcludeByNameOrKey(String),
    IncludeByEvent(String),
}

impl RecipeMatcher {
    pub fn is_include(&self) -> bool {
        match self {
            Self::IncludeBase => true,
            Self::IncludeAlternate => true,
            Self::IncludeByNameOrKey(..) => true,
            Self::IncludeByOutputItem(..) => true,
            Self::IncludeByEvent(..) => true,
            Self::ExcludeByNameOrKey(..) => false,
        }
    }

    pub fn validate(&self, game_db: &GameDatabase) -> Result<(), PlanError> {
        match self {
            Self::IncludeByNameOrKey(name) => {
                if game_db
                    .recipes
                    .iter()
                    .any(|r| r.name.eq_ignore_ascii_case(name) || r.key.eq(name))
                {
                    Ok(())
                } else {
                    Err(PlanError::UnknownRecipe(name.clone()))
                }
            }
            Self::ExcludeByNameOrKey(name) => {
                if game_db
                    .recipes
                    .iter()
                    .any(|r| r.name.eq_ignore_ascii_case(name) || r.key.eq(name))
                {
                    Ok(())
                } else {
                    Err(PlanError::UnknownRecipe(name.clone()))
                }
            }
            Self::IncludeByOutputItem(item) => {
                if game_db
                    .items
                    .iter()
                    .any(|i| i.name.eq_ignore_ascii_case(item) || i.key.eq(item))
                {
                    Ok(())
                } else {
                    Err(PlanError::UnknownItem(item.clone()))
                }
            }
            _ => Ok(()),
        }
    }

    pub fn matches(&self, recipe: &Recipe) -> bool {
        match self {
            Self::IncludeBase => recipe.events.is_empty() && !recipe.alternate,
            Self::IncludeAlternate => recipe.alternate,
            Self::IncludeByNameOrKey(name) => {
                recipe.name.eq_ignore_ascii_case(name) || recipe.key.eq(name)
            }
            Self::IncludeByOutputItem(item) => recipe
                .outputs
                .iter()
                .any(|o| o.item.name.eq_ignore_ascii_case(item) || o.item.key.eq(item)),
            Self::ExcludeByNameOrKey(name) => {
                recipe.name.eq_ignore_ascii_case(name) || recipe.key.eq(name)
            }
            Self::IncludeByEvent(event) => {
                recipe.events.iter().any(|e| e.eq_ignore_ascii_case(event))
            }
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
            Ok(RecipeMatcher::IncludeBase)
        } else if v.eq_ignore_ascii_case("alternates") || v.eq_ignore_ascii_case("alts") {
            Ok(RecipeMatcher::IncludeAlternate)
        } else {
            Ok(RecipeMatcher::IncludeByNameOrKey(v.into()))
        }
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        if let Some(field) = map.next_key::<String>()? {
            if field.eq_ignore_ascii_case("exclude") {
                Ok(RecipeMatcher::ExcludeByNameOrKey(map.next_value()?))
            } else if field.eq_ignore_ascii_case("output") {
                Ok(RecipeMatcher::IncludeByOutputItem(map.next_value()?))
            } else if field.eq_ignore_ascii_case("event") {
                Ok(RecipeMatcher::IncludeByEvent(map.next_value()?))
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
    inputs: HashMap<String, FloatType>,
    outputs: IndexMap<String, FloatType>,
    enabled_recipes: Vec<RecipeMatcher>,
}

#[derive(Debug, Clone)]
pub struct PlanConfig {
    pub inputs: HashMap<Rc<Item>, FloatType>,
    pub outputs: Vec<ItemPerMinute>,
    pub game_db: GameDatabase,
}

#[allow(dead_code)]
impl PlanConfig {
    pub fn new(outputs: Vec<ItemPerMinute>, game_db: GameDatabase) -> Self {
        PlanConfig {
            inputs: game_db.resource_limits.clone(),
            outputs,
            game_db,
        }
    }

    pub fn with_inputs(
        inputs: HashMap<Rc<Item>, FloatType>,
        outputs: Vec<ItemPerMinute>,
        game_db: GameDatabase,
    ) -> Self {
        let mut all_inputs = game_db.resource_limits.clone();
        all_inputs.extend(inputs);

        PlanConfig {
            inputs: all_inputs,
            outputs,
            game_db,
        }
    }

    pub fn from_file<P: AsRef<Path>>(file_path: P, game_db: &GameDatabase) -> anyhow::Result<Self> {
        let file = File::open(file_path)?;
        let config: PlanConfigDefinition = serde_yaml::from_reader(file)?;

        Ok(Self::convert(config, game_db)?)
    }

    fn convert(config: PlanConfigDefinition, game_db: &GameDatabase) -> Result<Self, PlanError> {
        // validate there are no extractable resources in the outputs list
        let mut outputs = Vec::new();
        for (item_name, value) in config.outputs {
            let item = game_db
                .find_item(&item_name)
                .ok_or(PlanError::UnknownItem(item_name))?;
            if item.resource {
                return Err(PlanError::UnexpectedResource(item.name.clone()));
            }

            outputs.push(ItemPerMinute::new(item, value))
        }

        let mut inputs: HashMap<Rc<Item>, FloatType> = game_db.resource_limits.clone();
        for (item_name, value) in config.inputs {
            let item = game_db
                .find_item(&item_name)
                .ok_or(PlanError::UnknownItem(item_name))?;

            inputs.insert(item, value);
        }

        for matcher in &config.enabled_recipes {
            matcher.validate(game_db)?;
        }

        let (include_matchers, exclude_matchers): (Vec<_>, Vec<_>) =
            config.enabled_recipes.iter().partition(|m| m.is_include());

        Ok(PlanConfig {
            inputs,
            outputs,
            game_db: game_db.filter(|recipe| {
                include_matchers.iter().any(|m| m.matches(recipe))
                    && !exclude_matchers.iter().any(|m| m.matches(recipe))
            }),
        })
    }

    pub fn has_input(&self, item: &Rc<Item>) -> bool {
        self.find_input(item) > 0.0
    }

    pub fn find_input(&self, item: &Rc<Item>) -> FloatType {
        self.inputs.get(item).copied().unwrap_or(0.0)
    }

    pub fn find_output(&self, item: &Item) -> FloatType {
        self.outputs
            .iter()
            .find(|o| o.item.as_ref() == item)
            .map(|o| o.amount)
            .unwrap_or(0.0)
    }
}

#[cfg(test)]
mod test {
    use crate::game::test::get_test_game_db;

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
            - event: FICSMAS
        #";

        let result: Result<Vec<RecipeMatcher>, serde_yaml::Error> = serde_yaml::from_str(yaml);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            vec![
                RecipeMatcher::IncludeBase,
                RecipeMatcher::IncludeAlternate,
                RecipeMatcher::IncludeAlternate,
                RecipeMatcher::IncludeByNameOrKey("Pure Iron Ingot".into()),
                RecipeMatcher::ExcludeByNameOrKey("Iron Alloy Ingot".into()),
                RecipeMatcher::IncludeByOutputItem("Copper Ingot".into()),
                RecipeMatcher::IncludeByEvent("FICSMAS".into())
            ]
        );
    }

    #[test]
    fn recipe_matcher_include_base_matches() {
        let game_db = get_test_game_db();

        let base_matcher = RecipeMatcher::IncludeBase;
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();
        let actual_snow = game_db.find_recipe("Recipe_Snow_C").unwrap();

        assert!(!base_matcher.matches(&actual_snow));
        assert!(!base_matcher.matches(&pure_iron_ingot));
        assert!(base_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_alternate_matches() {
        let game_db = get_test_game_db();

        let alts_matcher = RecipeMatcher::IncludeAlternate;
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();
        let actual_snow = game_db.find_recipe("Recipe_Snow_C").unwrap();

        assert!(!alts_matcher.matches(&actual_snow));
        assert!(alts_matcher.matches(&pure_iron_ingot));
        assert!(!alts_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_name_or_key_matches_name() {
        let game_db = get_test_game_db();

        let name_matcher = RecipeMatcher::IncludeByNameOrKey("Pure Iron Ingot".into());
        let name_lc_matcher = RecipeMatcher::IncludeByNameOrKey("pure iron ingot".into());
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(name_matcher.matches(&pure_iron_ingot));
        assert!(!name_matcher.matches(&copper_ingot));

        assert!(name_lc_matcher.matches(&pure_iron_ingot));
        assert!(!name_lc_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_name_or_key_matches_key() {
        let game_db = get_test_game_db();

        let name_matcher =
            RecipeMatcher::IncludeByNameOrKey("Recipe_Alternate_PureIronIngot_C".into());
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(name_matcher.matches(&pure_iron_ingot));
        assert!(!name_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_exclude_by_name_or_key_matches_name() {
        let game_db = get_test_game_db();

        let exclude_name_matcher = RecipeMatcher::ExcludeByNameOrKey("Copper Ingot".into());
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(!exclude_name_matcher.matches(&pure_iron_ingot));
        assert!(exclude_name_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_exclude_by_name_or_key_matches_key() {
        let game_db = get_test_game_db();

        let exclude_name_matcher = RecipeMatcher::ExcludeByNameOrKey("Recipe_IngotCopper_C".into());
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(!exclude_name_matcher.matches(&pure_iron_ingot));
        assert!(exclude_name_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_output_item_matches() {
        let game_db = get_test_game_db();

        let output_item_matcher = RecipeMatcher::IncludeByOutputItem("Copper Ingot".into());
        let pure_iron_ingot = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(!output_item_matcher.matches(&pure_iron_ingot));
        assert!(output_item_matcher.matches(&copper_ingot));
    }

    #[test]
    fn recipe_matcher_include_by_event_matches() {
        let game_db = get_test_game_db();

        let ficsmas_matcher = RecipeMatcher::IncludeByEvent("FICSMAS".into());
        let actual_snow = game_db.find_recipe("Recipe_Snow_C").unwrap();
        let copper_ingot = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();

        assert!(ficsmas_matcher.matches(&actual_snow));
        assert!(!ficsmas_matcher.matches(&copper_ingot));
    }
}
