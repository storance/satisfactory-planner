extern crate anyhow;
extern crate indexmap;
extern crate petgraph;
extern crate serde;
extern crate serde_json;
extern crate serde_yaml;
extern crate thiserror;

use crate::{
    game::GameDatabase,
    plan::{print_graph, solve, PlanConfig},
};

mod game;
mod plan;
mod utils;

fn main() {
    let game_db = GameDatabase::from_file("game-db.json").unwrap_or_else(|e| {
        panic!("Failed to load recipes: {}", e);
    });

    let plan = PlanConfig::from_file("plan.yml", &game_db).unwrap_or_else(|e| {
        panic!("Failed to load plan: {}", e);
    });

    let graph = solve(&plan).unwrap_or_else(|e| {
        panic!("Failed to solve plan: {}", e);
    });
    print_graph(&graph);

    /*let mut graph = crate::plan::ScoredGraph::new(&plan);
    graph.build();

    print_graph(&graph.graph);*/
}
