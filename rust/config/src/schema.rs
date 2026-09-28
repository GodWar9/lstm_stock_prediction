//! Configuration schema definitions matching the cross-language specification.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    #[serde(default = "default_env")]
    pub env: String,
    pub data: DataConfig,
    pub features: FeaturesConfig,
    pub training: TrainingConfig,
    pub inference: InferenceConfig,
    pub portfolio: PortfolioConfig,
    pub execution: ExecutionConfig,
    pub backtest: BacktestConfig,
}

fn default_env() -> String {
    "development".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataConfig {
    pub symbols: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_exchange")]
    pub exchange: String,
    #[serde(default = "default_dataset_version")]
    pub dataset_version: String,
    /// Directory containing one <symbol>.csv file per instrument, relative to the project root.
    #[serde(default = "default_input_dir")]
    pub input_dir: String,
}

fn default_provider() -> String {
    "csv".to_string()
}

fn default_input_dir() -> String {
    "datasets/import".to_string()
}

fn default_exchange() -> String {
    "NYSE".to_string()
}

fn default_dataset_version() -> String {
    "ds_2024_v1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeaturesConfig {
    #[serde(default = "default_feature_set")]
    pub feature_set: String,
    #[serde(default = "default_feature_set_version")]
    pub feature_set_version: u32,
    #[serde(default = "default_lookback")]
    pub lookback: usize,
    #[serde(default = "default_target_horizon")]
    pub target_horizon: u16,
    #[serde(default = "default_target_transformation")]
    pub target_transformation: String,
}

fn default_feature_set() -> String {
    "baseline_v1".to_string()
}

fn default_feature_set_version() -> u32 {
    1
}

fn default_lookback() -> usize {
    60
}

fn default_target_horizon() -> u16 {
    1
}

fn default_target_transformation() -> String {
    "log_return".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrainingConfig {
    #[serde(default)]
    pub walk_forward: bool,
    #[serde(default = "default_folds")]
    pub n_folds: usize,
    #[serde(default = "default_model_id")]
    pub model_id: String,
    #[serde(default = "default_hidden_size")]
    pub hidden_size: usize,
    #[serde(default = "default_num_layers")]
    pub num_layers: usize,
    #[serde(default = "default_dropout")]
    pub dropout: f64,
    #[serde(default = "default_lr")]
    pub learning_rate: f64,
    #[serde(default = "default_weight_decay")]
    pub weight_decay: f64,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_epochs")]
    pub epochs: usize,
    #[serde(default = "default_random_seed")]
    pub random_seed: u64,
    #[serde(default = "default_purge_gap")]
    pub purge_gap: usize,
    #[serde(default = "default_embargo_gap")]
    pub embargo_gap: usize,
}

fn default_folds() -> usize {
    5
}

fn default_model_id() -> String {
    "lstm_v1".to_string()
}

fn default_hidden_size() -> usize {
    128
}

fn default_num_layers() -> usize {
    2
}

fn default_dropout() -> f64 {
    0.3
}

fn default_lr() -> f64 {
    1e-3
}

fn default_weight_decay() -> f64 {
    1e-4
}

fn default_batch_size() -> usize {
    64
}

fn default_epochs() -> usize {
    50
}

fn default_random_seed() -> u64 {
    42
}

fn default_purge_gap() -> usize {
    5
}

fn default_embargo_gap() -> usize {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InferenceConfig {
    #[serde(default = "default_model_id")]
    pub model_version: String,
    #[serde(default = "default_threads")]
    pub intra_op_threads: usize,
    #[serde(default = "default_threads_one")]
    pub inter_op_threads: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
}

fn default_threads() -> usize {
    2
}

fn default_threads_one() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortfolioConfig {
    #[serde(default = "default_max_gross")]
    pub max_gross_exposure: f64,
    #[serde(default = "default_max_net")]
    pub max_net_exposure: f64,
    #[serde(default = "default_max_pos")]
    pub max_position_pct: f64,
    pub volatility_target: Option<f64>,
    #[serde(default = "default_long_short_mode")]
    pub long_short_mode: String,
}

fn default_max_gross() -> f64 {
    1.0
}

fn default_max_net() -> f64 {
    0.5
}

fn default_max_pos() -> f64 {
    0.2
}

fn default_long_short_mode() -> String {
    "LongOnly".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionConfig {
    #[serde(default = "default_fixed_commission")]
    pub fixed_commission: f64,
    #[serde(default = "default_half_spread_bps")]
    pub half_spread_bps: f64,
    #[serde(default = "default_slippage_factor")]
    pub slippage_factor: f64,
    #[serde(default = "default_participation_cap")]
    pub participation_cap: f64,
}

fn default_fixed_commission() -> f64 {
    0.001
}

fn default_half_spread_bps() -> f64 {
    2.0
}

fn default_slippage_factor() -> f64 {
    0.1
}

fn default_participation_cap() -> f64 {
    0.05
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BacktestConfig {
    #[serde(default = "default_initial_cash")]
    pub initial_cash: f64,
    #[serde(default = "default_risk_free_rate")]
    pub risk_free_rate: f64,
}

fn default_initial_cash() -> f64 {
    100_000.0
}

fn default_risk_free_rate() -> f64 {
    0.04
}
