//! CLI subcommands and option definitions for quantctl.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "quantctl", author, version, about = "Quantitative Trading & Research Platform CLI", long_about = None)]
pub struct Cli {
    /// Path to the configuration YAML file
    #[arg(short, long, default_value = "configs/default.yaml", global = true)]
    pub config: PathBuf,

    /// Output log format: "json" or "pretty"
    #[arg(long, default_value = "pretty", global = true)]
    pub log_format: String,

    /// Verbose logging level (trace / debug)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Check staged model predictions against saved PyTorch reference windows
    VerifyModel {
        #[arg(long)]
        artifact_dir: PathBuf,
    },
    /// Serve the local read-only research inspector
    Serve {
        #[arg(long, default_value_t = 8787)]
        port: u16,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Print the generated OpenAPI contract
    Openapi,

    /// Configuration management and validation
    Config(ConfigArgs),

    /// Market data ingestion and verification
    Data(DataArgs),

    /// Feature calculation and dataset preparation
    Features(FeaturesArgs),

    /// Model training invocation (PyTorch ML subprocess)
    Train(TrainArgs),

    /// Export PyTorch model to versioned ONNX artifact
    ExportModel(ExportModelArgs),

    /// Run real-time/offline inference using Rust ONNX runtime
    Predict(PredictArgs),

    /// Execute deterministic event-driven backtest
    Backtest(BacktestArgs),

    /// Run Monte Carlo, stress test, or parameter sensitivity simulation
    Simulate(SimulateArgs),

    /// Generate summary reports from backtest and simulation runs
    Report(ReportArgs),

    /// Performance benchmarks across features, inference, and backtest loops
    Benchmark(BenchmarkArgs),

    /// Display platform environment, git commit, and system build metadata
    Env,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum ConfigSubcommands {
    /// Validate configuration schema and constraints
    Validate,
    /// Print active resolved configuration
    Show,
}

#[derive(Args, Debug)]
pub struct DataArgs {
    #[command(subcommand)]
    pub command: DataSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum DataSubcommands {
    /// Ingest historical OHLCV data via configured adapter
    Ingest {
        #[arg(long)]
        symbol: Option<String>,
    },
    /// Validate point-in-time correctness, timestamp monotonicity, and absence of duplicate bars
    Validate {
        #[arg(long)]
        dataset_version: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct FeaturesArgs {
    #[command(subcommand)]
    pub command: FeaturesSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum FeaturesSubcommands {
    /// Compute features and persist to Parquet FeatureStore
    Build {
        #[arg(long, default_value = "baseline_v1")]
        feature_set: String,
    },
}

#[derive(Args, Debug)]
pub struct TrainArgs {
    /// Optional override path for training config
    #[arg(long)]
    pub train_config: Option<PathBuf>,

    /// Optional Rust-generated Arrow training dataset
    #[arg(long)]
    pub dataset: Option<PathBuf>,

    /// Optional manifest for the Arrow training dataset
    #[arg(long)]
    pub manifest: Option<PathBuf>,

    /// Use deterministic synthetic data for development smoke tests
    #[arg(long)]
    pub synthetic: bool,

    /// Enable rolling walk-forward cross-validation with per-fold models
    #[arg(long)]
    pub walk_forward: bool,

    /// Number of folds for rolling walk-forward CV
    #[arg(long, default_value = "5")]
    pub folds: usize,
}

#[derive(Args, Debug)]
pub struct ExportModelArgs {
    /// Model version identifier to export (e.g. lstm_v1)
    #[arg(long)]
    pub model_version: String,
    /// New artifact ID; the source checkpoint is never overwritten
    #[arg(long)]
    pub output_model: String,
}

#[derive(Args, Debug)]
pub struct PredictArgs {
    /// Model version artifact to load
    #[arg(long)]
    pub model: String,

    /// Target equity symbol
    #[arg(long)]
    pub symbol: String,
}

#[derive(Args, Debug)]
pub struct BacktestArgs {
    #[command(subcommand)]
    pub command: BacktestSubcommands,
}

#[derive(Subcommand, Debug)]
pub enum BacktestSubcommands {
    /// Run backtest simulation
    Run {
        #[arg(long)]
        model: String,

        #[arg(long, default_value = "test")]
        split: String,

        /// Allow reusing the test split multiple times
        #[arg(long, default_value_t = false)]
        allow_reuse: bool,
    },
}

#[derive(Args, Debug)]
pub struct SimulateArgs {
    /// Path to backtest report JSON
    #[arg(long)]
    pub report: PathBuf,

    /// Number of simulated paths
    #[arg(long, default_value_t = 1000)]
    pub paths: usize,
}

#[derive(Args, Debug)]
pub struct ReportArgs {
    /// Path to backtest report JSON
    #[arg(long)]
    pub backtest: PathBuf,
}

#[derive(Args, Debug)]
pub struct BenchmarkArgs {
    /// Benchmark suite name (e.g. inference, features, backtest)
    #[arg(long, default_value = "inference")]
    pub suite: String,
    /// Model to measure for inference or backtest
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, default_value_t = 20, value_parser = clap::value_parser!(u32).range(1..=1000))]
    pub iterations: u32,
}
