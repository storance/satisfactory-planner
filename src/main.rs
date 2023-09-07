use actix_files::{Files, NamedFile};
use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result};
use petgraph::visit::NodeIndexable;
use serde::Serialize;
use std::sync::Arc;

use crate::game::{GameDatabase, ItemKeyAmountPair};
use crate::plan::{
    solve, PlanConfig, PlanConfigDefinition, PlanError, SolvedGraph, SolvedNodeWeight,
};

mod game;
mod plan;
mod utils;

#[derive(Debug, Clone)]
pub struct State {
    pub game_db: Arc<GameDatabase>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SolvedEdge {
    pub from: usize,
    pub to: usize,
    pub weight: ItemKeyAmountPair,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphResponse {
    nodes: Vec<SolvedNodeWeight>,
    edges: Vec<SolvedEdge>,
}

impl From<SolvedGraph> for GraphResponse {
    fn from(value: SolvedGraph) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let mut node_mapping = vec![usize::MAX; value.node_bound()];
        for i in value.node_indices() {
            nodes.push(value[i].clone());
            node_mapping[i.index()] = nodes.len() - 1;
        }

        for e in value.edge_indices() {
            let (src, target) = value.edge_endpoints(e).unwrap();
            let from = node_mapping[src.index()];
            let to = node_mapping[target.index()];

            edges.push(SolvedEdge {
                from,
                to,
                weight: value[e].clone(),
            });
        }

        Self { nodes, edges }
    }
}

impl Responder for GraphResponse {
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        let body = serde_json::to_string(&self).unwrap();
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
) -> std::result::Result<GraphResponse, PlanError> {
    let config = PlanConfig::parse(config.0, Arc::clone(&state.game_db))?;
    let graph = solve(&config)?;
    Ok(graph.into())
}
