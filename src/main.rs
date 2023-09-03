use std::path::PathBuf;

use crate::{
    game::GameDatabase,
    plan::{print_graph, solve, PlanConfig},
};
use clap::Parser;

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
            panic!("Failed to build full plan graph {}: {}", args.plan.display(), e);
        });
        print_graph(&graph);
    } else {
        let graph = solve(&plan).unwrap_or_else(|e| {
            panic!("Failed to solve plan: {}", e);
        });
        print_graph(&graph);
    }
}
