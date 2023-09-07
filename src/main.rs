use std::sync::Arc;
use actix_files::{NamedFile, Files};
use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{HttpServer, Result, App, web, HttpResponse, Responder, get, post, HttpRequest};
use serde::Serialize;

use crate::game::GameDatabase;
use crate::plan::{solve, PlanConfig, PlanConfigDefinition, SolvedGraph};

mod game;
mod plan;
mod utils;

#[derive(Debug, Clone)]
pub struct State {
    pub game_db: Arc<GameDatabase>
}

#[derive(Serialize)]
pub struct GraphResponse {
    #[serde(flatten)]
    graph: SolvedGraph
}

impl GraphResponse {
    pub fn new(graph: SolvedGraph) -> Self {
        Self { graph }
    }
}

impl Responder for GraphResponse {
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        let body = serde_json::to_string(&self).unwrap();

        // Create response and set content type
        HttpResponse::Ok()
            .content_type(ContentType::json())
            .body(body)
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let game_db = Arc::new(GameDatabase::from_file("game-db.json").unwrap_or_else(|e| {
        panic!("Failed to load game database game-db.json: {}", e);
    }));
    let state = web::Data::new(State { game_db });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(index)
            .service(Files::new("/assets", "./assets"))
            .service(get_database)
            .service(create_plan)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

#[get("/")]
async fn index() -> Result<NamedFile> {
    Ok(NamedFile::open("./assets/index.html")?)
}

#[get("/api/1/database")]
async fn get_database() -> Result<NamedFile> {
    Ok(NamedFile::open("./game-db.json")?)
}

#[post("/api/1/plan")]
async fn create_plan(
    state: web::Data<State>,
    config: web::Json<PlanConfigDefinition>,
) -> impl Responder {
    let config = PlanConfig::parse(config.0, Arc::clone(&state.game_db)).expect("Whoops! you broke it");

    let graph = solve(&config).expect("Whoops! you broke it");
    GraphResponse::new(graph)
}
