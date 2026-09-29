//! Feature engine: indicators, feature graph, ring buffers, and feature store.

pub mod atr;
pub mod bollinger;
pub mod cross_sectional;
pub mod export;
pub mod fracdiff;
pub mod graph;
pub mod macd;
pub mod mmap_reader;
pub mod moving_avg;
pub mod rsi;
pub mod simd_ops;
pub mod store;
pub mod targets;
pub mod traits;
pub mod volatility;
pub mod window;

pub use atr::*;
pub use bollinger::*;
pub use cross_sectional::*;
pub use export::*;
pub use fracdiff::*;
pub use graph::*;
pub use macd::*;
pub use mmap_reader::*;
pub use moving_avg::*;
pub use rsi::*;
pub use simd_ops::*;
pub use store::*;
pub use targets::*;
pub use traits::*;
pub use volatility::*;
pub use window::*;

/// Version-one training/inference graph. Keep every consumer on the same schema.
pub fn standard_graph(periods_per_year: f64) -> FeatureGraph {
    FeatureGraph::new(vec![
        Box::new(Sma::new(20)),
        Box::new(Ema::new(12)),
        Box::new(Rsi::new(14)),
        Box::new(Macd::standard()),
        Box::new(BollingerBands::standard()),
        Box::new(Atr::new(14)),
        Box::new(RollingVolatility::new(20, periods_per_year.sqrt())),
        Box::new(LogReturn::new(1)),
    ])
}
