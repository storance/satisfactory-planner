use actix_cors::Cors;
use actix_files::{Files, NamedFile};
use actix_web::body::BoxBody;
use actix_web::http::header::ContentType;
use actix_web::{
    get, middleware::Logger, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
    Result,
};
use clap::Parser;
use log::info;
use petgraph::visit::NodeIndexable;
use serde::Serialize;
use std::path::PathBuf;
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

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the game database json.  Defaults to game-db.json
    #[arg(short = 'd', long = "game-db")]
    game_db: Option<PathBuf>,

    /// Enable a permissive CORS header for local testing.
    #[arg(short = 'c', long = "permissive-cors")]
    permissive_cors: bool,

    /// Port number to listen on
    #[arg(short = 'p', long = "listen-port", default_value_t = 8080)]
    listen_port: u16,

    // IP Address to listen on
    #[arg(short = 'a', long = "listen-address", default_value = "127.0.0.1")]
    listen_address: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    let args = Args::parse();

    let game_db_path = args.game_db.unwrap_or(PathBuf::from("game-db.json"));
    let game_db = Arc::new(GameDatabase::from_file(game_db_path).unwrap_or_else(|e| {
        panic!("Failed to load game database game-db.json: {}", e);
    }));
    let state = web::Data::new(State { game_db });

    let listen_address = (args.listen_address, args.listen_port);
    info!("Listening on {}:{}", listen_address.0, listen_address.1);
    HttpServer::new(move || {
        let cors = if args.permissive_cors {
            Cors::permissive()
        } else {
            Cors::default()
        };

        App::new()
            .app_data(state.clone())
            .service(index)
            .service(Files::new("/assets", "./assets"))
            .service(get_database)
            .service(create_plan)
            .wrap(cors)
            .wrap(Logger::new("%a \"%r\" %s - %T"))
    })
    .bind(listen_address)?
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
