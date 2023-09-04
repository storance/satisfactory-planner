use std::path::PathBuf;

use crate::{
    game::GameDatabase,
    plan::{print_graph, solve, PlanConfig},
    utils::round,
};
use clap::Parser;
use plan::SolvedNodeWeight;

mod game;
mod plan;
mod utils;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the game database json.  Defaults to game-db.json
    #[arg(short = 'd', long = "game-db")]
    game_db: Option<PathBuf>,

    /// Path to the plan configuration yaml
    #[arg()]
    plan: PathBuf,

    /// Print out the intermediary full plan graph instead
    #[arg(short = 'f', long = "full-plan-graph")]
    full_plan_graph: bool,
}

fn main() {
    let args = Args::parse();

    let game_db_path = args.game_db.unwrap_or(PathBuf::from("game-db.json"));

    let game_db = GameDatabase::from_file(&game_db_path).unwrap_or_else(|e| {
        panic!(
            "Failed to load game database {}: {}",
            game_db_path.display(),
            e
        );
    });

    let plan = PlanConfig::from_file(&args.plan, &game_db).unwrap_or_else(|e| {
        panic!("Failed to load plan {}: {}", args.plan.display(), e);
    });

    if args.full_plan_graph {
        let graph = crate::plan::build_full_plan(&plan).unwrap_or_else(|e| {
            panic!(
                "Failed to build full plan graph {}: {}",
                args.plan.display(),
                e
            );
        });
        print_graph(&graph);
    } else {
        let graph = solve(&plan).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        print_graph(&graph);

        let mut floor_area = 0.0;
        let mut volume = 0.0;
        let mut total_buildings = 0.0;
        let mut power_usage = 0.0;

        for i in graph.node_indices() {
            if let SolvedNodeWeight::Production(recipe, building_count) = &graph[i] {
                floor_area += recipe.building.floor_area() * building_count;
                volume += recipe.building.volume() * building_count;
                total_buildings += building_count;
                power_usage += recipe.average_mw(100.0) * building_count;

                //let last_clock_speed = building_count.fract() * 100.0;
                //power_usage += recipe.average_mw(last_clock_speed);
            }
        }

        println!("Total Buildings: {}", round(total_buildings, 3));
        println!("Floor Area: {} m^2", round(floor_area, 3));
        println!("Volume: {} m^3", round(volume, 3));
        println!("Power Usage: {} MW", round(power_usage, 3));
    }
}
