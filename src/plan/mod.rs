use actix_web::{ResponseError, http::header::ContentType, HttpResponse};
use serde::{Serialize, Deserialize};
use thiserror::Error;

mod config;
mod full_plan_graph;
mod solved_graph;
mod solver;

pub use config::*;
pub use full_plan_graph::*;
pub use solved_graph::*;
pub use solver::*;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("No recipe exists with the name or key `{0}`")]
    UnknownRecipe(String),
    #[error("No item exists with the name or key `{0}`")]
    UnknownItem(String),
    #[error("The item `{0}` is an extractable resource and is not allowed in outputs.")]
    UnexpectedResourceInOutputs(String),
    #[error("The output for item `{0}` must be greater than zero.")]
    InvalidOutputAmount(String),
    #[error("The input for item `{0}` must be greater than or equal to zero.")]
    InvalidInputAmount(String),
    #[error("Unable to solve the given factory plan.  This can be caused by missing inputs, insufficient resources, or disabled recipes.")]
    UnsolvablePlan
}

impl PlanError {
    pub fn error_code(&self) -> String {
        match self {
            PlanError::UnknownRecipe(_) => "UnknownRecipe",
            PlanError::UnknownItem(_) => "UnknownItem",
            PlanError::UnexpectedResourceInOutputs(_) => "UnexpectedResourceInOutputs",
            PlanError::InvalidOutputAmount(_) => "InvalidOutputAmount",
            PlanError::InvalidInputAmount(_) => "InvalidInputAmount",
            PlanError::UnsolvablePlan => "UnsolvablePlan",
        }.into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error_code: String,
    pub message: String
}

impl ResponseError for PlanError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        actix_web::http::StatusCode::BAD_REQUEST
    }

    fn error_response(&self) -> HttpResponse {
        let error_response = ErrorResponse {
            error_code: self.error_code(),
            message: self.to_string()
        };

        HttpResponse::build(self.status_code())
            .insert_header(ContentType::html())
            .body(serde_json::to_string(&error_response).unwrap())
    }
}