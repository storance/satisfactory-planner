
use crate::game::GameDatabase;
use axum::{Router, routing::get};

mod game;
mod plan;
mod utils;

#[tokio::main]
async fn main() {
    let game_db = GameDatabase::from_file("game-db.json").unwrap_or_else(|e| {
        panic!(
            "Failed to load game database game-db.json: {}",
            e
        );
    });

    let app = Router::new().route("/", get(|| async { "Hello, world!" }));

    axum::Server::bind(&"0.0.0.0:8100".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
