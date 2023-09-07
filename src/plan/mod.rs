use std::fmt;
use thiserror::Error;

mod config;
mod full_plan_graph;
mod solved_graph;
mod solver;

pub use config::*;
pub use full_plan_graph::*;
use good_lp::ResolutionError;
pub use solved_graph::*;
pub use solver::*;



#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Unable to solve the given factory factory plan due to missing inputs or recipes.")]
    MissingInputsOrRecipes,
    #[error("Unable to solve the given factory factory plan due to insufficient resources.")]
    InsufficientResources(#[from] ResolutionError)
}

pub trait NodeWeight
where
    Self: fmt::Display,
{
    fn is_input(&self) -> bool;
    fn is_input_resource(&self) -> bool;
    fn is_output(&self) -> bool;
    fn is_by_product(&self) -> bool;
    fn is_production(&self) -> bool;
    fn is_producer(&self) -> bool;
}
