pub mod artifacts;
pub mod contract;
mod server;
pub use server::{openapi, router, serve};
