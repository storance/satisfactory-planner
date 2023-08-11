use std::io;
use thiserror::Error;

mod config;
mod graph;
mod solver;

pub use config::*;
pub use graph::*;
pub use solver::*;

#[derive(Error, Debug)]
pub enum PlanError {
    #[error("No recipe exists with the name `{0}`")]
    InvalidRecipe(String),
    #[error("The raw resource `{0}` is not allowed in inputs.")]
    UnexpectedRawInputItem(String),
    #[error("The raw resource `{0}` is not allowed in outputs.")]
    UnexpectedRawOutputItem(String),
    #[error("Item `{0}` in override_limits is not a raw resource.")]
    InvalidOverrideLimit(String),
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    SerdeError(#[from] serde_yaml::Error),
}
