//! Backtest engine: deterministic event-driven simulation and performance analysis.

pub mod engine;
pub mod report;
pub mod stream;

pub use engine::*;
pub use report::*;
pub use stream::*;
pub mod multi_asset;
pub use multi_asset::{AssetBars, AssetPosition, MultiAssetReport};
