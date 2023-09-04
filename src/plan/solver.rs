use good_lp::{minilp, variable, variables, Expression, SolverModel, Variable};
use petgraph::{
    stable_graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction::{Incoming, Outgoing},
};
use std::collections::HashMap;

use super::{
    full_plan_graph::{build_full_plan, PlanNodeWeight},
    solved_graph::{copy_solution, SolvedGraph},
    PlanConfig,
};

pub fn solve(config: &PlanConfig) -> Result<SolvedGraph, anyhow::Error> {
    let full_graph = build_full_plan(config)?;

    let mut node_variables: HashMap<NodeIndex, Variable> = HashMap::new();
    let mut edge_variables: HashMap<EdgeIndex, Variable> = HashMap::new();
    let mut by_product_variables: HashMap<NodeIndex, Variable> = HashMap::new();

    let mut vars = variables!();
    let mut min_expr: Expression = 0.into();

    for i in full_graph.node_indices() {
        match &full_graph[i] {
            PlanNodeWeight::Input(item) => {
                let var = vars.add(variable().min(0.0));
                if item.resource {
                    let limit = config.game_db.get_resource_limit(item);
                    min_expr += var / limit;
                }

                node_variables.insert(i, var);
            }
            PlanNodeWeight::ByProduct(..) => {
                let var = vars.add(variable().min(0.0));
                let excess_var = vars.add(variable().min(0.0));

                // TODO: do we want to try to minimize by products? Maybe make it an option?
                //min_expr += excess_var;
                node_variables.insert(i, var);
                by_product_variables.insert(i, excess_var);
            }
            _ => {
                node_variables.insert(i, vars.add(variable().min(0.0)));
            }
        }
    }

    for e in full_graph.edge_indices() {
        edge_variables.insert(e, vars.add(variable().min(0.0)));
    }

    let mut problem = vars.minimise(min_expr).using(minilp);

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
            PlanNodeWeight::Production(recipe) => {
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

    use crate::{
        game::{test::get_test_game_db_with_recipes, ItemPerMinute},
        plan::solved_graph::SolvedNodeWeight,
        utils::{FloatType, EPSILON},
    };

    use super::*;

    #[test]
    fn test_iron_ingot_base_recipes() {
        let game_db = get_test_game_db_with_recipes(&["Recipe_IngotIron_C"]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();

        let config = PlanConfig::new(
            vec![ItemPerMinute::new(Rc::clone(&iron_ingot), 30.0)],
            game_db.clone(),
        );

        let mut expected_graph = SolvedGraph::new();
        let output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&iron_ingot), 30.0));
        let smelter_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_ingot_recipe),
            1.0,
        ));
        let input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&iron_ore), 30.0));

        expected_graph.add_edge(
            smelter_idx,
            output_idx,
            ItemPerMinute::new(Rc::clone(&iron_ingot), 30.0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            ItemPerMinute::new(Rc::clone(&iron_ore), 30.0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_iron_ingot_with_pure_ingot_recipe() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_Alternate_PureIronIngot_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();

        let iron_ingot_recipe = game_db
            .find_recipe("Recipe_Alternate_PureIronIngot_C")
            .unwrap();

        let config = PlanConfig::new(
            vec![ItemPerMinute::new(Rc::clone(&iron_ingot), 65.0)],
            game_db,
        );

        let mut expected_graph = SolvedGraph::new();
        let output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&iron_ingot), 65.0));
        let refinery_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_ingot_recipe),
            1.0,
        ));
        let ore_input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&iron_ore), 35.0));

        let water_input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&water), 20.0));

        expected_graph.add_edge(
            refinery_idx,
            output_idx,
            ItemPerMinute::new(Rc::clone(&iron_ingot), 65.0),
        );
        expected_graph.add_edge(
            ore_input_idx,
            refinery_idx,
            ItemPerMinute::new(Rc::clone(&iron_ore), 35.0),
        );
        expected_graph.add_edge(
            water_input_idx,
            refinery_idx,
            ItemPerMinute::new(Rc::clone(&water), 20.0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_iron_rods_and_plates() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IronPlate_C",
            "Recipe_IronRod_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let iron_plate = game_db.find_item("Desc_IronPlate_C").unwrap();
        let iron_rod = game_db.find_item("Desc_IronRod_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();
        let iron_plate_recipe = game_db.find_recipe("Recipe_IronPlate_C").unwrap();
        let iron_rod_recipe = game_db.find_recipe("Recipe_IronRod_C").unwrap();

        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(Rc::clone(&iron_rod), 30.0),
                ItemPerMinute::new(Rc::clone(&iron_plate), 60.0),
            ],
            game_db,
        );

        let mut expected_graph = SolvedGraph::new();
        let plate_output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&iron_plate), 60.0));
        let rod_output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&iron_rod), 30.0));

        let plate_prod_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_plate_recipe),
            3.0,
        ));
        let rod_prod_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_rod_recipe),
            2.0,
        ));
        let smelter_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_ingot_recipe),
            4.0,
        ));
        let input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&iron_ore), 120.0));

        expected_graph.add_edge(
            plate_prod_idx,
            plate_output_idx,
            ItemPerMinute::new(Rc::clone(&iron_plate), 60.0),
        );

        expected_graph.add_edge(
            rod_prod_idx,
            rod_output_idx,
            ItemPerMinute::new(Rc::clone(&iron_rod), 30.0),
        );

        expected_graph.add_edge(
            smelter_idx,
            rod_prod_idx,
            ItemPerMinute::new(Rc::clone(&iron_ingot), 30.0),
        );

        expected_graph.add_edge(
            smelter_idx,
            plate_prod_idx,
            ItemPerMinute::new(Rc::clone(&iron_ingot), 90.0),
        );
        expected_graph.add_edge(
            input_idx,
            smelter_idx,
            ItemPerMinute::new(Rc::clone(&iron_ore), 120.0),
        );
        let result = solve(&config);

        assert!(result.is_ok());
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_wire_with_input_limits() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_IngotIron_C",
            "Recipe_IngotCopper_C",
            "Recipe_IngotCaterium_C",
            "Recipe_Wire_C",
            "Recipe_Alternate_FusedWire_C",
            "Recipe_Alternate_Wire_1_C",
            "Recipe_Alternate_Wire_2_C",
        ]);

        let iron_ore = game_db.find_item("Desc_OreIron_C").unwrap();
        let copper_ore = game_db.find_item("Desc_OreCopper_C").unwrap();
        let caterium_ore = game_db.find_item("Desc_OreGold_C").unwrap();

        let iron_ingot = game_db.find_item("Desc_IronIngot_C").unwrap();
        let copper_ingot = game_db.find_item("Desc_CopperIngot_C").unwrap();
        let caterium_ingot = game_db.find_item("Desc_GoldIngot_C").unwrap();

        let wire = game_db.find_item("Desc_Wire_C").unwrap();

        let iron_ingot_recipe = game_db.find_recipe("Recipe_IngotIron_C").unwrap();
        let copper_ingot_recipe = game_db.find_recipe("Recipe_IngotCopper_C").unwrap();
        let caterium_ingot_recipe = game_db.find_recipe("Recipe_IngotCaterium_C").unwrap();

        let iron_wire_recipe = game_db.find_recipe("Recipe_Alternate_Wire_1_C").unwrap();
        let fused_wire_recipe = game_db.find_recipe("Recipe_Alternate_FusedWire_C").unwrap();
        let caterium_wire_recipe = game_db.find_recipe("Recipe_Alternate_Wire_2_C").unwrap();

        let mut input_limits = HashMap::new();
        input_limits.insert(Rc::clone(&iron_ore), 12.5);
        input_limits.insert(Rc::clone(&copper_ore), 12.0);

        let config = PlanConfig::with_inputs(
            input_limits,
            vec![ItemPerMinute::new(Rc::clone(&wire), 232.5)],
            game_db,
        );

        let mut expected_graph = SolvedGraph::new();
        let output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&wire), 232.5));

        let cat_wire_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&caterium_wire_recipe),
            1.0,
        ));

        let fused_wire_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&fused_wire_recipe),
            1.0,
        ));

        let iron_wire_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_wire_recipe),
            1.0,
        ));

        let iron_ingot_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&iron_ingot_recipe),
            12.5 / 30.0,
        ));

        let copper_ingot_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&copper_ingot_recipe),
            0.4,
        ));

        let cat_ingot_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&caterium_ingot_recipe),
            1.2,
        ));

        let iron_ore_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&iron_ore), 12.5));

        let copper_ore_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&copper_ore), 12.0));

        let cat_ore_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&caterium_ore), 54.0));

        expected_graph.add_edge(
            cat_wire_idx,
            output_idx,
            ItemPerMinute::new(Rc::clone(&wire), 120.0),
        );

        expected_graph.add_edge(
            fused_wire_idx,
            output_idx,
            ItemPerMinute::new(Rc::clone(&wire), 90.0),
        );

        expected_graph.add_edge(
            iron_wire_idx,
            output_idx,
            ItemPerMinute::new(Rc::clone(&wire), 22.5),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            cat_wire_idx,
            ItemPerMinute::new(Rc::clone(&caterium_ingot), 15.0),
        );

        expected_graph.add_edge(
            cat_ingot_idx,
            fused_wire_idx,
            ItemPerMinute::new(Rc::clone(&caterium_ingot), 3.0),
        );

        expected_graph.add_edge(
            copper_ingot_idx,
            fused_wire_idx,
            ItemPerMinute::new(Rc::clone(&copper_ingot), 12.0),
        );

        expected_graph.add_edge(
            iron_ingot_idx,
            iron_wire_idx,
            ItemPerMinute::new(Rc::clone(&iron_ingot), 12.5),
        );

        expected_graph.add_edge(
            iron_ore_idx,
            iron_ingot_idx,
            ItemPerMinute::new(Rc::clone(&iron_ore), 12.5),
        );

        expected_graph.add_edge(
            copper_ore_idx,
            copper_ingot_idx,
            ItemPerMinute::new(Rc::clone(&copper_ore), 12.0),
        );

        expected_graph.add_edge(
            cat_ore_idx,
            cat_ingot_idx,
            ItemPerMinute::new(Rc::clone(&caterium_ore), 54.0),
        );

        let result = solve(&config);

        assert!(result.is_ok(), "{:?}", result);
        assert_graphs_equal(result.unwrap(), expected_graph);
    }

    #[test]
    fn test_fuel_and_plastic() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_Alternate_HeavyOilResidue_C",
            "Recipe_ResidualFuel_C",
            "Recipe_ResidualPlastic_C",
        ]);

        let oil = game_db.find_item("Desc_LiquidOil_C").unwrap();
        let fuel = game_db.find_item("Desc_LiquidFuel_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let heavy_oil_residue = game_db.find_item("Desc_HeavyOilResidue_C").unwrap();
        let plastic = game_db.find_item("Desc_Plastic_C").unwrap();
        let polymer_resin = game_db.find_item("Desc_PolymerResin_C").unwrap();

        let hor_recipe = game_db
            .find_recipe("Recipe_Alternate_HeavyOilResidue_C")
            .unwrap();
        let residual_fuel_recipe = game_db.find_recipe("Recipe_ResidualFuel_C").unwrap();
        let residual_plastic_recipe = game_db.find_recipe("Recipe_ResidualPlastic_C").unwrap();

        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(Rc::clone(&fuel), 180.0),
                ItemPerMinute::new(Rc::clone(&plastic), 30.0),
            ],
            game_db,
        );

        let mut expected_graph = SolvedGraph::new();
        let fuel_output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&fuel), 180.0));
        let plastic_output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&plastic), 30.0));

        let resin_by_prod_idx = expected_graph.add_node(SolvedNodeWeight::new_by_product(
            Rc::clone(&polymer_resin),
            45.0,
        ));

        let hor_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&hor_recipe),
            6.75,
        ));

        let plastic_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&residual_plastic_recipe),
            1.5,
        ));

        let fuel_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&residual_fuel_recipe),
            4.5,
        ));

        let oil_input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&oil), 202.5));

        let water_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&water), 30.0));

        expected_graph.add_edge(
            fuel_idx,
            fuel_output_idx,
            ItemPerMinute::new(Rc::clone(&fuel), 180.0),
        );

        expected_graph.add_edge(
            hor_idx,
            fuel_idx,
            ItemPerMinute::new(Rc::clone(&heavy_oil_residue), 270.0),
        );

        expected_graph.add_edge(
            hor_idx,
            resin_by_prod_idx,
            ItemPerMinute::new(Rc::clone(&polymer_resin), 45.0),
        );

        expected_graph.add_edge(
            hor_idx,
            plastic_idx,
            ItemPerMinute::new(Rc::clone(&polymer_resin), 90.0),
        );

        expected_graph.add_edge(
            water_idx,
            plastic_idx,
            ItemPerMinute::new(Rc::clone(&water), 30.0),
        );

        expected_graph.add_edge(
            plastic_idx,
            plastic_output_idx,
            ItemPerMinute::new(Rc::clone(&plastic), 30.0),
        );

        expected_graph.add_edge(
            oil_input_idx,
            hor_idx,
            ItemPerMinute::new(Rc::clone(&oil), 202.5),
        );

        let result = solve(&config).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        assert_graphs_equal(result, expected_graph);
    }

    #[test]
    pub fn test_diluted_packaged_fuel() {
        let game_db = get_test_game_db_with_recipes(&[
            "Recipe_Alternate_HeavyOilResidue_C",
            "Recipe_ResidualFuel_C",
            "Recipe_Plastic_C",
            "Recipe_ResidualPlastic_C",
            "Recipe_PackagedWater_C",
            "Recipe_UnpackageFuel_C",
            "Recipe_FluidCanister_C",
            "Recipe_Alternate_DilutedPackagedFuel_C",
        ]);

        let oil = game_db.find_item("Desc_LiquidOil_C").unwrap();
        let fuel = game_db.find_item("Desc_LiquidFuel_C").unwrap();
        let water = game_db.find_item("Desc_Water_C").unwrap();
        let heavy_oil_residue = game_db.find_item("Desc_HeavyOilResidue_C").unwrap();
        let plastic = game_db.find_item("Desc_Plastic_C").unwrap();
        let polymer_resin = game_db.find_item("Desc_PolymerResin_C").unwrap();
        let empty_canister = game_db.find_item("Desc_FluidCanister_C").unwrap();
        let packaged_water = game_db.find_item("Desc_PackagedWater_C").unwrap();
        let packaged_fuel = game_db.find_item("Desc_Fuel_C").unwrap();

        let hor_recipe = game_db
            .find_recipe("Recipe_Alternate_HeavyOilResidue_C")
            .unwrap();
        let empty_canister_recipe = game_db.find_recipe("Recipe_FluidCanister_C").unwrap();
        let residual_plastic_recipe = game_db.find_recipe("Recipe_ResidualPlastic_C").unwrap();
        let unpackage_fuel_recipe = game_db.find_recipe("Recipe_UnpackageFuel_C").unwrap();
        let packaged_water_recipe = game_db.find_recipe("Recipe_PackagedWater_C").unwrap();
        let diluted_fuel_recipe = game_db
            .find_recipe("Recipe_Alternate_DilutedPackagedFuel_C")
            .unwrap();

        let config = PlanConfig::new(
            vec![
                ItemPerMinute::new(Rc::clone(&fuel), 120.0),
                ItemPerMinute::new(Rc::clone(&packaged_fuel), 20.0),
            ],
            game_db,
        );

        let mut expected_graph = SolvedGraph::new();
        let fuel_output_idx =
            expected_graph.add_node(SolvedNodeWeight::new_output(Rc::clone(&fuel), 120.0));
        let packaged_fuel_output_idx = expected_graph.add_node(SolvedNodeWeight::new_output(
            Rc::clone(&packaged_fuel),
            20.0,
        ));

        let resin_by_prod_idx = expected_graph.add_node(SolvedNodeWeight::new_by_product(
            Rc::clone(&polymer_resin),
            5.0,
        ));

        let hor_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&hor_recipe),
            1.75,
        ));

        let plastic_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&residual_plastic_recipe),
            0.5,
        ));

        let empty_canister_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&empty_canister_recipe),
            1.0 / 3.0,
        ));

        let packaged_water_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&packaged_water_recipe),
            7.0 / 3.0,
        ));

        let diluted_fuel_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&diluted_fuel_recipe),
            7.0 / 3.0,
        ));

        let unpackage_fuel_idx = expected_graph.add_node(SolvedNodeWeight::new_production(
            Rc::clone(&unpackage_fuel_recipe),
            2.0,
        ));

        let oil_input_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&oil), 52.5));

        let water_idx =
            expected_graph.add_node(SolvedNodeWeight::new_input(Rc::clone(&water), 150.0));

        expected_graph.add_edge(
            unpackage_fuel_idx,
            fuel_output_idx,
            ItemPerMinute::new(Rc::clone(&fuel), 120.0),
        );

        expected_graph.add_edge(
            diluted_fuel_idx,
            packaged_fuel_output_idx,
            ItemPerMinute::new(Rc::clone(&packaged_fuel), 20.0),
        );

        expected_graph.add_edge(
            diluted_fuel_idx,
            unpackage_fuel_idx,
            ItemPerMinute::new(Rc::clone(&packaged_fuel), 120.0),
        );

        expected_graph.add_edge(
            unpackage_fuel_idx,
            packaged_water_idx,
            ItemPerMinute::new(Rc::clone(&empty_canister), 120.0),
        );

        expected_graph.add_edge(
            packaged_water_idx,
            diluted_fuel_idx,
            ItemPerMinute::new(Rc::clone(&packaged_water), 140.0),
        );

        expected_graph.add_edge(
            hor_idx,
            diluted_fuel_idx,
            ItemPerMinute::new(Rc::clone(&heavy_oil_residue), 70.0),
        );

        expected_graph.add_edge(
            empty_canister_idx,
            packaged_water_idx,
            ItemPerMinute::new(Rc::clone(&empty_canister), 20.0),
        );

        expected_graph.add_edge(
            water_idx,
            packaged_water_idx,
            ItemPerMinute::new(Rc::clone(&water), 140.0),
        );

        expected_graph.add_edge(
            plastic_idx,
            empty_canister_idx,
            ItemPerMinute::new(Rc::clone(&plastic), 10.0),
        );

        expected_graph.add_edge(
            hor_idx,
            plastic_idx,
            ItemPerMinute::new(Rc::clone(&polymer_resin), 30.0),
        );

        expected_graph.add_edge(
            water_idx,
            plastic_idx,
            ItemPerMinute::new(Rc::clone(&water), 10.0),
        );

        expected_graph.add_edge(
            hor_idx,
            resin_by_prod_idx,
            ItemPerMinute::new(Rc::clone(&polymer_resin), 5.0),
        );

        expected_graph.add_edge(
            oil_input_idx,
            hor_idx,
            ItemPerMinute::new(Rc::clone(&oil), 52.5),
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
                    "Expected node {} was not found in the actual graph {}",
                    format_node(&expected[i]),
                    format_graph_nodes(&actual)
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
                        "Edge connecting {} to {} was not found in actual graph",
                        format_node(&expected[edge.source()]),
                        format_node(&expected[edge.target()])
                    )
                });

            assert!(
                item_value_pair_equals(&actual[actual_edge], &edge.weight()),
                "Mismatched weight for the edge connecting {} to {}. Expected: {}, actual: {}",
                format_node(&expected[edge.source()]),
                format_node(&expected[edge.target()]),
                edge.weight().amount,
                actual[actual_edge].amount
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
        FloatType::abs(a - b) < EPSILON
    }

    fn format_node(node: &SolvedNodeWeight) -> String {
        match node {
            SolvedNodeWeight::Input(input) => format!("Input({}:{})", input.item, input.amount),
            SolvedNodeWeight::Output(output) => format!("Output({}:{})", output.item, output.amount),
            SolvedNodeWeight::ByProduct(output) => {
                format!("ByProduct({}:{})", output.item, output.amount)
            }
            SolvedNodeWeight::Production(recipe, building_count) => {
                format!("Production({}, {})", recipe.name, building_count)
            }
        }
    }

    fn format_graph_nodes(graph: &SolvedGraph) -> String {
        let all_nodes: Vec<String> = graph.node_weights().map(format_node).collect();
        format!("[{}]", all_nodes.join(", "))
    }
}
