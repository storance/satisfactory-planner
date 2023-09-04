use good_lp::{minilp, variable, variables, Expression, SolverModel, Variable};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::collections::HashMap;

use crate::utils::FloatType;

use super::{
    full_plan_graph::{build_full_plan, PlanNodeWeight},
    solved_graph::{copy_solution, SolvedGraph},
    PlanConfig,
};

const RESOURCE_WEIGHT: FloatType = 10_000.0;

pub fn solve(config: &PlanConfig) -> Result<SolvedGraph, anyhow::Error> {
    let full_graph = build_full_plan(config)?;

    let mut node_variables: HashMap<NodeIndex, Variable> = HashMap::new();
    let mut edge_variables: HashMap<EdgeIndex, Variable> = HashMap::new();
    let mut by_product_variables: HashMap<NodeIndex, Variable> = HashMap::new();

    let mut vars = variables!();
    let mut resource_expr: Expression = 0.into();
    let mut complexity_expr: Expression = 0.into();

    for i in full_graph.node_indices() {
        match &full_graph[i] {
            PlanNodeWeight::Input(item) => {
                let var = vars.add(variable().min(0.0));
                if item.resource {
                    let limit = config.game_db.get_resource_limit(item);
                    resource_expr += var * 10_000.0 / limit;
                }

                node_variables.insert(i, var);
            }
            PlanNodeWeight::ByProduct(..) => {
                let var = vars.add(variable().min(0.0));
                let excess_var = vars.add(variable().min(0.0));

                node_variables.insert(i, var);
                by_product_variables.insert(i, excess_var);
            }
            PlanNodeWeight::Production(_, complexity) => {
                let var = vars.add(variable().min(0.0));
                complexity_expr += var * *complexity;
                node_variables.insert(i, var);
            }
            _ => {
                node_variables.insert(i, vars.add(variable().min(0.0)));
            }
        }
    }

    for e in full_graph.edge_indices() {
        edge_variables.insert(e, vars.add(variable().min(0.0)));
    }

    let mut problem = vars
        .minimise((RESOURCE_WEIGHT * resource_expr) + complexity_expr)
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

                let desired_output = config.find_output(item);
                problem = problem
                    .with(Expression::from(var).eq(desired_output))
                    .with(edge_sum.eq(var));
            }
            PlanNodeWeight::Input(item) => {
                let mut edge_sum: Expression = 0.into();
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    edge_sum += edge_var;
                }

                let limit = config.find_input(item);
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
            PlanNodeWeight::Production(recipe, ..) => {
                for edge in full_graph.edges_directed(i, Outgoing) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    let recipe_output = recipe.find_output_by_item(edge.weight()).unwrap();

                    problem = problem.with((var * recipe_output.amount).eq(edge_var));
                }

                for edge in full_graph.edges_directed(i, Incoming) {
                    let edge_var = edge_variables.get(&edge.id()).unwrap();
                    let recipe_input = recipe.find_input_by_item(edge.weight()).unwrap();

                    problem = problem.with((var * recipe_input.amount).eq(edge_var));
                }
            }
        }
    }

    let solution = problem.solve()?;
    Ok(copy_solution(
        &full_graph,
        solution,
        node_variables,
        edge_variables,
    ))
}

#[cfg(test)]
mod tests {
    use petgraph::visit::IntoEdgeReferences;
    use std::rc::Rc;

    use super::*;
    use crate::{
        game::{
            test::{get_game_db_with_base_recipes_plus, get_test_game_db_with_recipes},
            ItemPerMinute,
        },
        plan::solved_graph::SolvedNodeWeight,
        utils::{round, FloatType, EPSILON},
    };

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
                        let item = $game_db.find_item($item).unwrap_or_else(||
                            panic!("Item {} does not exist", $item));

                        temp_graph.add_edge(src_id, target_id, ItemPerMinute::new(item, $amount));
                    }
                )*

                temp_graph
            }
        };
        (
            @node($game_db:ident) Production($recipe: literal, $building_count:expr)
        ) => {
            SolvedNodeWeight::new_production(
                $game_db.find_recipe($recipe).unwrap_or_else(||
                    panic!("Recipe {} does not exist", $recipe)),
                $building_count
            )
        };
        (
            @node($game_db:ident) Input($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::new_input(
                $game_db.find_item($item).unwrap_or_else(||
                    panic!("Item {} does not exist", $item)),
                $amount
            )
        };
        (
            @node($game_db:ident) Output($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::new_output(
                $game_db.find_item($item).unwrap_or_else(||
                    panic!("Item {} does not exist", $item)),
                $amount
            )
        };
        (
            @node($game_db:ident) ByProduct($item:literal, $amount:expr)
        ) => {
            SolvedNodeWeight::new_by_product(
                $game_db.find_item($item).unwrap_or_else(||
                    panic!("Item {} does not exist", $item)),
                $amount
            )
        };
    }

    #[test]
    fn test_iron_ingot_base_recipes() {
        let game_db = get_test_game_db_with_recipes(&["Recipe_IngotIron_C"]);

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

        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let config = PlanConfig::new(vec![ItemPerMinute::new(iron_ingot, 30.0)], game_db);

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_iron_ingot_with_pure_ingot_recipe() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_Alternate_PureIronIngot_C",
        ]);

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

        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let config = PlanConfig::new(vec![ItemPerMinute::new(iron_ingot, 65.0)], game_db);

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_iron_rods_and_plates() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IronPlate_C",
            "Recipe_IronRod_C",
        ]);

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

        let iron_plate = game_db.find_item("Desc_IronPlate_C").unwrap();
        let iron_rod = game_db.find_item("Desc_IronRod_C").unwrap();
        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(iron_rod, 30.0),
                ItemPerMinute::new(iron_plate, 60.0),
            ],
            game_db,
        );
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_wire_with_input_limits() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IngotCopper_C",
            "Recipe_IngotCaterium_C",
            "Recipe_Wire_C",
            "Recipe_Alternate_FusedWire_C",
            "Recipe_Alternate_Wire_1_C", // Iron Wire
            "Recipe_Alternate_Wire_2_C", // Caterium Wire
        ]);

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

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let wire = game_db.find_item("Desc_Wire_C").unwrap();

        let mut input_limits = HashMap::new();
        input_limits.insert(iron_ore, 12.5);
        input_limits.insert(copper_ore, 12.0);

        let config =
            PlanConfig::with_inputs(input_limits, vec![ItemPerMinute::new(wire, 232.5)], game_db);

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    pub fn test_diluted_packaged_fuel() {
        let game_db = get_game_db_with_base_recipes_plus(&[
            "Recipe_Alternate_HeavyOilResidue_C",
            "Recipe_Alternate_DilutedPackagedFuel_C",
        ]);

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

        let fuel = game_db.find_item("Desc_LiquidFuel_C").unwrap();
        let packaged_fuel = game_db.find_item("Desc_Fuel_C").unwrap();
        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(Rc::clone(&fuel), 120.0),
                ItemPerMinute::new(Rc::clone(&packaged_fuel), 20.0),
            ],
            game_db,
        );
        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    fn test_recycled_rubber_plastic_loop() {
        let game_db = get_game_db_with_base_recipes_plus(&[
            "Recipe_Alternate_HeavyOilResidue_C",
            "Recipe_Alternate_DilutedFuel_C",
            "Recipe_Alternate_Plastic_1_C",
            "Recipe_Alternate_RecycledRubber_C",
        ]);

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

        let plastic = game_db.find_item("Desc_Plastic_C").unwrap();
        let rubber = game_db.find_item("Desc_Rubber_C").unwrap();
        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(Rc::clone(&rubber), 300.0),
                ItemPerMinute::new(Rc::clone(&plastic), 300.0),
            ],
            game_db,
        );

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
                    "Expected node {:?} was not found in the actual graph {:?}",
                    expected[i], actual
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
                item_value_pair_equals(&actual[actual_edge], &edge.weight()),
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
            (SolvedNodeWeight::Input(a), SolvedNodeWeight::Input(b)) => {
                item_value_pair_equals(a, b)
            }
            (SolvedNodeWeight::Output(a), SolvedNodeWeight::Output(b)) => {
                item_value_pair_equals(a, b)
            }
            (SolvedNodeWeight::ByProduct(a), SolvedNodeWeight::ByProduct(b)) => {
                item_value_pair_equals(a, b)
            }
            (
                SolvedNodeWeight::Production(a_recipe, a_building_count),
                SolvedNodeWeight::Production(b_recipe, b_building_count),
            ) => a_recipe == b_recipe && float_equals(*a_building_count, *b_building_count),
            _ => false,
        }
    }

    fn item_value_pair_equals(a: &ItemPerMinute, b: &ItemPerMinute) -> bool {
        a.item == b.item && float_equals(a.amount, b.amount)
    }

    fn float_equals(a: FloatType, b: FloatType) -> bool {
        round(FloatType::abs(a - b), 3) < EPSILON
    }
}
