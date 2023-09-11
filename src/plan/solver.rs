use super::{
    full_plan_graph::{build_full_plan, PlanNodeWeight},
    solved_graph::{copy_solution, SolvedGraph},
    PlanConfig, PlanError,
};
use crate::game::Building;
use good_lp::{minilp, variable, variables, Expression, SolverModel, Variable};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::collections::HashMap;

pub fn solve(config: &PlanConfig) -> Result<SolvedGraph, PlanError> {
    let full_graph = build_full_plan(config)?;

    let mut node_variables: HashMap<NodeIndex, Variable> = HashMap::new();
    let mut edge_variables: HashMap<EdgeIndex, Variable> = HashMap::new();
    let mut by_product_variables: HashMap<NodeIndex, Variable> = HashMap::new();

    let mut vars = variables!();
    let mut maximize_output_expr: Expression = 0.into();
    let mut minimize_expr: Expression = 0.into();
    let mut should_maximize = false;

    for i in full_graph.node_indices() {
        match &full_graph[i] {
            PlanNodeWeight::Input(item_id) => {
                let var = vars.add(variable().min(0.0));
                let item = &config.game_db[*item_id];
                if item.resource {
                    let limit = config.game_db.get_resource_limit(*item_id);
                    minimize_expr += var * 10_000.0 / limit;
                }

                node_variables.insert(i, var);
            }
            PlanNodeWeight::ByProduct(..) => {
                let var = vars.add(variable().min(0.0));
                let excess_var = vars.add(variable().min(0.0));

                node_variables.insert(i, var);
                by_product_variables.insert(i, excess_var);
            }
            PlanNodeWeight::Production(..) => {
                let var = vars.add(variable().min(0.0));
                node_variables.insert(i, var);
            }
            PlanNodeWeight::Output(item) => {
                let var = vars.add(variable().min(0.0));
                if config.find_output(*item).unwrap().is_maximize() {
                    maximize_output_expr += var;
                    should_maximize = true;
                }
                node_variables.insert(i, var);
            }
            PlanNodeWeight::Producer(..) => {
                node_variables.insert(i, vars.add(variable().min(0.0)));
            }
        }
    }

    for e in full_graph.edge_indices() {
        edge_variables.insert(e, vars.add(variable().min(0.0)));
    }

    let mut problem = if should_maximize {
        vars.maximise(maximize_output_expr)
    } else {
        vars.minimise(minimize_expr)
    }
    .using(minilp);

    for i in full_graph.node_indices() {
        let var = *node_variables.get(&i).unwrap();

        match &full_graph[i] {
            PlanNodeWeight::Output(item) => {
                let mut edge_sum: Expression = 0.into();
                for edge in full_graph.edges_directed(i, Incoming) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    edge_sum += edge_var;
                }

                problem = problem.with(edge_sum.eq(var));
                let desired_output = config.find_output(*item).unwrap();
                if desired_output.is_per_minute() {
                    problem =
                        problem.with(Expression::from(var).eq(desired_output.as_per_minute()));
                }
            }
            PlanNodeWeight::Input(item) => {
                let mut edge_sum: Expression = 0.into();
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    edge_sum += edge_var;
                }

                let limit = config.find_input(*item);
                problem = problem
                    .with(Expression::from(var).leq(limit))
                    .with(edge_sum.eq(var));
            }
            PlanNodeWeight::ByProduct(..) => {
                let excess_var = *by_product_variables.get(&i).unwrap();

                let mut incoming_sum: Expression = 0.into();
                for edge in full_graph.edges_directed(i, Incoming) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    incoming_sum += edge_var;
                }

                let mut outgoing_sum: Expression = excess_var.into();
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    outgoing_sum += edge_var;
                }

                problem = problem
                    .with(incoming_sum.eq(var))
                    .with(outgoing_sum.eq(var));
            }
            PlanNodeWeight::Production(recipe_id, ..) => {
                let recipe = &config.game_db[*recipe_id];
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    let recipe_output = recipe.find_output_by_item(*edge.weight()).unwrap();

                    problem = problem.with((var * recipe_output.amount).eq(edge_var));
                }

                for edge in full_graph.edges_directed(i, Incoming) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    let recipe_input = recipe.find_input_by_item(*edge.weight()).unwrap();

                    problem = problem.with((var * recipe_input.amount).eq(edge_var));
                }
            }
            PlanNodeWeight::Producer(building) => {
                let mut edge_sum: Expression = 0.into();
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    edge_sum += edge_var;
                }

                if let Building::ItemProducer(ip) = &config.game_db[*building] {
                    problem = problem.with(edge_sum.eq(var * ip.output.amount));
                }
            }
        }
    }

    let solution = problem.solve().map_err(|_| PlanError::UnsolvablePlan)?;
    Ok(copy_solution(
        config,
        &full_graph,
        solution,
        node_variables,
        edge_variables,
    ))
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;
    use std::sync::Arc;

    use super::*;
    use crate::{
        game::{test::get_test_game_db, ItemId, ItemKeyAmountPair, RecipeId},
        plan::{solved_graph::SolvedNodeWeight, OutputAmount},
        utils::{FloatType, EPSILON},
    };

    pub fn round(value: FloatType, decimals: u8) -> FloatType {
        let base: FloatType = 10.0;
        let multiplier = base.powi(decimals as i32);

        (value * multiplier).round() / multiplier
    }

    macro_rules! graph_builder {
        (
            Graph($game_db:ident) {
                nodes: [
                    $($id:literal [$node_type:ident($($node_args:tt)*)]),*
                ],
                edges: [
                    $($source:literal -> $target:literal [$item:literal, $amount:expr]),*
                ]
            }
        ) => {
            {
                let mut temp_graph = SolvedGraph::new();
                let mut node_mappings: HashMap<u32, NodeIndex> = HashMap::new();

                $(
                    {
                        let weight = graph_builder!(@node($game_db) $node_type($($node_args)*));
                        if !node_mappings.insert($id, temp_graph.add_node(weight)).is_none() {
                            panic!("Node id {} was used multiple times", $id);
                        }
                    }
                )*

                $(
                    {
                        let src_id = *node_mappings.get(&$source).unwrap_or_else(||
                            panic!("Source Node {} does not exist", $source));
                        let target_id = *node_mappings.get(&$target).unwrap_or_else(||
                            panic!("Target Node {} does not exist", $target));
                        temp_graph.add_edge(src_id, target_id, ItemKeyAmountPair::new($item.into(), $amount));
                    }
                )*

                temp_graph
            }
        };
        (
            @node($game_db:ident) Production($recipe: literal, $building_count:expr)
        ) => {
            SolvedNodeWeight::Production {
                recipe: $recipe.into(),
                building_count: $building_count
            }
        };
        (
            @node($game_db:ident) Input($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::Input {
                input: ItemKeyAmountPair::new($item.into(), $amount)
            }
        };
        (
            @node($game_db:ident) Output($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::Output {
                output: ItemKeyAmountPair::new($item.into(), $amount)
            }
        };
        (
            @node($game_db:ident) ByProduct($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::ByProduct {
                by_product: ItemKeyAmountPair::new($item.into(), $amount)
            }
        };
        (
            @node($game_db:ident) Producer($building: literal, $building_count:expr)
        ) => {
            SolvedNodeWeight::Producer {
                building: $building.into(),
                count: $building_count
            }
        };

    }

    macro_rules! inputs {
        ($game_db:ident {}) => {
            $game_db.resource_limits.clone()
        };
        (
            $game_db:ident {$($item:literal : $amount:literal),+}
        ) => {
            {
                let mut inputs = $game_db.resource_limits.clone();
                $(
                    inputs.insert(
                        $game_db.find_item($item).unwrap_or_else(||
                            panic!("Item {} does not exist", $item)),
                        $amount);
                )+

                inputs
            }
        }
    }
    macro_rules! outputs {
        (
            $game_db:ident {$($item:literal : $amount:literal),+}
        ) => {
            {
                let mut outputs: HashMap<ItemId, OutputAmount> = HashMap::new();
                $(outputs.insert(
                    $game_db.find_item($item).unwrap_or_else(||
                        panic!("Item {} does not exist", $item)),
                        outputs!(@amount $amount));
                )+

                outputs
            }
        };
        (
            @amount Maximize
        ) => {
            OutputAmount::Maximize
        };
        (
            @amount $amount:literal
        ) => {
            OutputAmount::PerMinute($amount)
        };
    }

    #[test]
    fn test_iron_ingot_base_recipes() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> =
            game_db.filter_recipes(|r| r.key == "Recipe_IngotIron_C");

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_IronIngot_C", 30.0)],
                    1 [Production("Recipe_IngotIron_C", 1.0)],
                    2 [Input("Desc_OreIron_C", 30.0)]
                ],
                edges: [
                    2 -> 1 ["Desc_OreIron_C", 30.0],
                    1 -> 0 ["Desc_IronIngot_C", 30.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_IronIngot_C": 30.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_iron_ingot_with_pure_ingot_recipe() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            r.key == "Recipe_IngotIron_C" || r.key == "Recipe_Alternate_PureIronIngot_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_IronIngot_C", 65.0)],
                    1 [Production("Recipe_Alternate_PureIronIngot_C", 1.0)],
                    2 [Input("Desc_OreIron_C", 35.0)],
                    3 [Input("Desc_Water_C", 20.0)]
                ],
                edges: [
                    3 -> 1 ["Desc_Water_C", 20.0],
                    2 -> 1 ["Desc_OreIron_C", 35.0],
                    1 -> 0 ["Desc_IronIngot_C", 65.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_IronIngot_C": 65.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_iron_rods_and_plates() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| !r.alternate);

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_IronPlate_C", 60.0)],
                    1 [Output("Desc_IronRod_C", 30.0)],
                    2 [Production("Recipe_IronPlate_C", 3.0)],
                    3 [Production("Recipe_IronRod_C", 2.0)],
                    4 [Production("Recipe_IngotIron_C", 4.0)],
                    5 [Input("Desc_OreIron_C", 120.0)]
                ],
                edges: [
                    5 -> 4 ["Desc_OreIron_C", 120.0],
                    4 -> 3 ["Desc_IronIngot_C", 30.0],
                    4 -> 2 ["Desc_IronIngot_C", 90.0],
                    3 -> 1 ["Desc_IronRod_C", 30.0],
                    2 -> 0 ["Desc_IronPlate_C", 60.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_IronPlate_C": 60.0,
            "Desc_IronRod_C": 30.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_reinforced_iron_plates_with_provided_inputs() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| !r.alternate);

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_IronPlateReinforced_C", 7.5)],
                    1 [Production("Recipe_IronPlateReinforced_C", 1.5)],
                    2 [Production("Recipe_Screw_C", 0.75)],
                    3 [Production("Recipe_IronPlate_C", 2.25)],
                    4 [Production("Recipe_IronRod_C", 0.5)],
                    5 [Production("Recipe_IngotIron_C", 2.5)],
                    6 [Input("Desc_OreIron_C", 75.0)],
                    7 [Input("Desc_IronScrew_C", 60.0)]
                ],
                edges: [
                    6 -> 5 ["Desc_OreIron_C", 75.0],
                    5 -> 3 ["Desc_IronIngot_C", 67.5],
                    5 -> 4 ["Desc_IronIngot_C", 7.5],
                    4 -> 2 ["Desc_IronRod_C", 7.5],
                    7 -> 1 ["Desc_IronScrew_C", 60.0],
                    3 -> 1 ["Desc_IronPlate_C", 45.0],
                    2 -> 1 ["Desc_IronScrew_C", 30.0],
                    1 -> 0 ["Desc_IronPlateReinforced_C", 7.5]
                ]
            }
        );

        let inputs = inputs!(game_db {
            "Desc_IronScrew_C": 60.0
        });
        let outputs = outputs!(game_db {
            "Desc_IronPlateReinforced_C": 7.5
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_wire_with_input_limits() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            !r.alternate
                || r.key == "Recipe_Alternate_FusedWire_C"
                || r.key == "Recipe_Alternate_Wire_1_C"
                || r.key == "Recipe_Alternate_Wire_2_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_Wire_C", 232.5)],
                    1 [Production("Recipe_Alternate_Wire_1_C", 1.0)],
                    2 [Production("Recipe_Alternate_FusedWire_C", 1.0)],
                    3 [Production("Recipe_Alternate_Wire_2_C", 1.0)],
                    4 [Production("Recipe_IngotIron_C", 12.5 / 30.0)],
                    5 [Production("Recipe_IngotCopper_C", 0.4)],
                    6 [Production("Recipe_IngotCaterium_C", 1.2)],
                    7 [Input("Desc_OreIron_C", 12.5)],
                    8 [Input("Desc_OreCopper_C", 12.0)],
                    9 [Input("Desc_OreGold_C", 54.0)]
                ],
                edges: [
                    9 -> 6 ["Desc_OreGold_C", 54.0],
                    8 -> 5 ["Desc_OreCopper_C", 12.0],
                    7 -> 4 ["Desc_OreIron_C", 12.5],
                    6 -> 3 ["Desc_GoldIngot_C", 15.0],
                    6 -> 2 ["Desc_GoldIngot_C", 3.0],
                    5 -> 2 ["Desc_CopperIngot_C", 12.0],
                    4 -> 1 ["Desc_IronIngot_C", 12.5],
                    3 -> 0 ["Desc_Wire_C", 120.0],
                    2 -> 0 ["Desc_Wire_C", 90.0],
                    1 -> 0 ["Desc_Wire_C", 22.5]
                ]
            }
        );

        let inputs = inputs!(game_db {
            "Desc_OreIron_C": 12.5,
            "Desc_OreCopper_C": 12.0
        });
        let outputs = outputs!(game_db {
            "Desc_Wire_C": 232.5
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_fuel_and_plastic() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            r.key == "Recipe_Alternate_HeavyOilResidue_C"
                || r.key == "Recipe_ResidualFuel_C"
                || r.key == "Recipe_ResidualPlastic_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_LiquidFuel_C", 180.0)],
                    1 [Output("Desc_Plastic_C", 30.0)],
                    2 [Production("Recipe_ResidualFuel_C", 4.5)],
                    3 [Production("Recipe_ResidualPlastic_C", 1.5)],
                    4 [Production("Recipe_Alternate_HeavyOilResidue_C", 6.75)],
                    5 [ByProduct("Desc_PolymerResin_C", 45.0)],
                    6 [Input("Desc_LiquidOil_C", 202.5)],
                    7 [Input("Desc_Water_C", 30.0)]
                ],
                edges: [
                    6 -> 4 ["Desc_LiquidOil_C", 202.5],
                    4 -> 5 ["Desc_PolymerResin_C", 45.0],
                    4 -> 2 ["Desc_HeavyOilResidue_C", 270.0],
                    7 -> 3 ["Desc_Water_C", 30.0],
                    4 -> 3 ["Desc_PolymerResin_C", 90.0],
                    3 -> 1 ["Desc_Plastic_C", 30.0],
                    2 -> 0 ["Desc_LiquidFuel_C", 180.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_LiquidFuel_C": 180.0,
            "Desc_Plastic_C": 30.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    pub fn test_diluted_packaged_fuel() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            !r.alternate
                || r.key == "Recipe_Alternate_HeavyOilResidue_C"
                || r.key == "Recipe_Alternate_DilutedPackagedFuel_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_LiquidFuel_C", 120.0)],
                    1 [Output("Desc_Fuel_C", 20.0)],
                    2 [Production("Recipe_UnpackageFuel_C", 2.0)],
                    3 [Production("Recipe_Alternate_DilutedPackagedFuel_C", 7.0 / 3.0)],
                    4 [Production("Recipe_PackagedWater_C", 7.0 / 3.0)],
                    5 [Production("Recipe_ResidualPlastic_C", 0.5)],
                    6 [Production("Recipe_FluidCanister_C", 1.0 / 3.0)],
                    7 [Production("Recipe_Alternate_HeavyOilResidue_C", 1.75)],
                    8 [ByProduct("Desc_PolymerResin_C", 5.0)],
                    9 [Input("Desc_LiquidOil_C", 52.5)],
                    10 [Input("Desc_Water_C", 150.0)]
                ],
                edges: [
                    9 -> 7 ["Desc_LiquidOil_C", 52.5],
                    10 -> 5 ["Desc_Water_C", 10.0],
                    10 -> 4 ["Desc_Water_C", 140.0],
                    7 -> 8 ["Desc_PolymerResin_C", 5.0],
                    7 -> 5 ["Desc_PolymerResin_C", 30.0],
                    5 -> 6 ["Desc_Plastic_C", 10.0],
                    6 -> 4 ["Desc_FluidCanister_C", 20.0],
                    2 -> 4 ["Desc_FluidCanister_C", 120.0],
                    7 -> 3 ["Desc_HeavyOilResidue_C", 70.0],
                    4 -> 3 ["Desc_PackagedWater_C", 140.0],
                    3 -> 2 ["Desc_Fuel_C", 120.0],
                    3 -> 1 ["Desc_Fuel_C", 20.0],
                    2 -> 0 ["Desc_LiquidFuel_C", 120.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_LiquidFuel_C": 120.0,
                "Desc_Fuel_C": 20.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_recycled_rubber_plastic_loop() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            !r.alternate
                || r.key == "Recipe_Alternate_HeavyOilResidue_C"
                || r.key == "Recipe_Alternate_DilutedFuel_C"
                || r.key == "Recipe_Alternate_Plastic_1_C"
                || r.key == "Recipe_Alternate_RecycledRubber_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_Plastic_C", 300.0)],
                    1 [Output("Desc_Rubber_C", 300.0)],
                    2 [Production("Recipe_Alternate_RecycledRubber_C", 8.518521)],
                    3 [Production("Recipe_Alternate_Plastic_1_C", 9.25926)],
                    4 [Production("Recipe_Alternate_DilutedFuel_C", 16.0 / 3.0)],
                    5 [Production("Recipe_ResidualRubber_C", 10.0 / 3.0)],
                    6 [Production("Recipe_Alternate_HeavyOilResidue_C", 20.0 / 3.0)],
                    7 [Input("Desc_LiquidOil_C", 200.0)],
                    8 [Input("Desc_Water_C", 2000.0 / 3.0)]
                ],
                edges: [
                    7 -> 6 ["Desc_LiquidOil_C", 200.0],
                    8 -> 5 ["Desc_Water_C", 400.0 / 3.0],
                    8 -> 4 ["Desc_Water_C", 1600.0 / 3.0],
                    6 -> 5 ["Desc_PolymerResin_C", 400.0 / 3.0],
                    6 -> 4 ["Desc_HeavyOilResidue_C", 800.0 / 3.0],
                    4 -> 2 ["Desc_LiquidFuel_C", 2300.0 / 9.0],
                    4 -> 3 ["Desc_LiquidFuel_C", 2500.0 / 9.0],
                    5 -> 3 ["Desc_Rubber_C", 200.0 / 3.0],
                    3 -> 2 ["Desc_Plastic_C", 2300.0 / 9.0],
                    2 -> 3 ["Desc_Rubber_C", 1900.0 / 9.0],
                    3 -> 0 ["Desc_Plastic_C", 300.0],
                    2 -> 1 ["Desc_Rubber_C", 300.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_Plastic_C": 300.0,
                "Desc_Rubber_C": 300.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_ficsmas() {
        let game_db = Arc::new(get_test_game_db());
        let enabled_recipes: Vec<RecipeId> = game_db.filter_recipes(|r| {
            !r.alternate
                || r.key == "Recipe_XmasBall1_C"
                || r.key == "Recipe_XmasBall2_C"
                || r.key == "Recipe_XmasBall3_C"
                || r.key == "Recipe_XmasBall4_C"
        });

        let expected_graph = graph_builder!(
            Graph(game_db) {
                nodes: [
                    0 [Output("Desc_XmasBall3_C", 10.0)],
                    1 [Output("Desc_XmasBall4_C", 10.0)],
                    2 [Production("Recipe_XmasBall3_C", 2.0)],
                    3 [Production("Recipe_XmasBall4_C", 2.0)],
                    4 [Production("Recipe_XmasBall1_C", 4.0)],
                    5 [Production("Recipe_XmasBall2_C", 3.0)],
                    6 [Production("Recipe_IngotIron_C", 1.0)],
                    7 [Production("Recipe_IngotCopper_C", 2.0 / 3.0)],
                    8 [Producer("Desc_TreeGiftProducer_C", 7.0 / 3.0)],
                    9 [Input("Desc_OreIron_C", 30.0)],
                    10 [Input("Desc_OreCopper_C", 20.0)]
                ],
                edges: [
                    8 -> 4 ["Desc_Gift_C", 20.0],
                    8 -> 5 ["Desc_Gift_C", 15.0],
                    9 -> 6 ["Desc_OreIron_C", 30.0],
                    10 -> 7 ["Desc_OreCopper_C", 20.0],
                    6 -> 3 ["Desc_IronIngot_C", 30.0],
                    7 -> 2 ["Desc_CopperIngot_C", 20.0],
                    5 -> 3 ["Desc_XmasBall2_C", 30.0],
                    4 -> 2 ["Desc_XmasBall1_C", 20.0],
                    3 -> 1 ["Desc_XmasBall4_C", 10.0],
                    2 -> 0 ["Desc_XmasBall3_C", 10.0]
                ]
            }
        );

        let inputs = inputs!(game_db {});
        let outputs = outputs!(game_db {
            "Desc_XmasBall3_C": 10.0,
                "Desc_XmasBall4_C": 10.0
        });
        let config = PlanConfig {
            game_db,
            inputs,
            outputs,
            enabled_recipes,
        };
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    fn assert_graphs_equal(actual: SolvedGraph, expected: SolvedGraph) {
        let mut node_mapping: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for i in expected.node_indices() {
            match actual
                .node_indices()
                .find(|j| node_equals(&expected[i], &actual[*j]))
            {
                Some(j) => node_mapping.insert(i, j),
                None => panic!(
                    "Expected node {:?} was not found in the actual graph {}",
                    expected[i],
                    format_nodes(&actual)
                ),
            };
        }

        for edge in expected.edge_references() {
            let actual_parent = node_mapping.get(&edge.target()).unwrap();
            let actual_child = node_mapping.get(&edge.source()).unwrap();

            let actual_edge = actual
                .find_edge(*actual_child, *actual_parent)
                .unwrap_or_else(|| {
                    panic!(
                        "Edge connecting {:?} to {:?} was not found in actual graph",
                        expected[edge.source()],
                        expected[edge.target()]
                    )
                });

            assert!(
                item_amount_equals(&actual[actual_edge], edge.weight()),
                "Mismatched weight for the edge connecting {:?} to {:?}. Expected: {:?}, actual: {:?}",
                expected[edge.source()],
                expected[edge.target()],
                edge.weight(),
                actual[actual_edge]
            );
        }

        assert!(actual.node_count() == expected.node_count());
        assert!(actual.edge_count() == expected.edge_count());
    }

    fn node_equals(a_node: &SolvedNodeWeight, b_node: &SolvedNodeWeight) -> bool {
        match (a_node, b_node) {
            (SolvedNodeWeight::Input { input: a }, SolvedNodeWeight::Input { input: b }) => {
                item_amount_equals(a, b)
            }
            (SolvedNodeWeight::Output { output: a }, SolvedNodeWeight::Output { output: b }) => {
                item_amount_equals(a, b)
            }
            (
                SolvedNodeWeight::ByProduct { by_product: a },
                SolvedNodeWeight::ByProduct { by_product: b },
            ) => item_amount_equals(a, b),
            (
                SolvedNodeWeight::Production {
                    recipe: a_recipe,
                    building_count: a_building_count,
                },
                SolvedNodeWeight::Production {
                    recipe: b_recipe,
                    building_count: b_building_count,
                },
            ) => a_recipe == b_recipe && float_equals(*a_building_count, *b_building_count),
            (
                SolvedNodeWeight::Producer {
                    building: a_building,
                    count: a_count,
                },
                SolvedNodeWeight::Producer {
                    building: b_building,
                    count: b_count,
                },
            ) => a_building == b_building && float_equals(*a_count, *b_count),
            _ => false,
        }
    }

    fn item_amount_equals(a: &ItemKeyAmountPair, b: &ItemKeyAmountPair) -> bool {
        a.item == b.item && float_equals(a.amount, b.amount)
    }

    fn float_equals(a: FloatType, b: FloatType) -> bool {
        round(FloatType::abs(a - b), 3) < EPSILON
    }

    fn format_nodes(graph: &SolvedGraph) -> String {
        format!(
            "[{}]",
            graph
                .node_weights()
                .map(|n| format!("{:?}", n))
                .collect::<Vec<String>>()
                .join(", ")
        )
    }
}
