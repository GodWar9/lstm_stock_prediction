//! Configuration system: validated serde schemas for the quant platform.

pub mod loader;
pub mod schema;
pub mod validation;

pub use loader::*;
pub use schema::*;
pub use validation::*;
