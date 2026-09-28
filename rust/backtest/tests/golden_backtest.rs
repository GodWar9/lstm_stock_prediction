//! Deterministic long-period golden backtest test.
//!
//! Simulates a 10-year (2,520 trading days) deterministic backtest run,
//! verifying:
//! 1. Byte-for-byte report determinism across consecutive runs.
//! 2. Invariance of performance metrics (Sharpe, Drawdown, Win Rate, CAGR).
//! 3. Preservation of numerical precision without floating-point drift.

use quant_backtest::engine::{BacktestConfig, BacktestEngine};
use quant_backtest::report::BacktestReport;
use quant_backtest::stream::ManualSignalStream;
use quant_data::types::{Bar, Timestamp};
use quant_execution::CompositeExecutionModel;
use quant_instruments::InstrumentId;
use quant_portfolio::{PortfolioConstraints, VolatilityTargetedConstructor};
use quant_signals::{Direction, Signal};

/// Deterministic Linear Congruential Generator (LCG) for reproducible PRNG.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_f64(&mut self) -> f64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.state >> 33) as f64) / ((1u64 << 31) as f64)
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }
}

/// Generates 2,520 trading days (10 years) of synthetic daily OHLCV bars.
fn generate_10yr_synthetic_bars(start_price: f64) -> Vec<Bar> {
    const NUM_BARS: usize = 2520;
    let mut bars = Vec::with_capacity(NUM_BARS);
    let mut rng = Lcg::new(42);
    let mut price = start_price;

    for i in 0..NUM_BARS {
        let ts = Timestamp((i as i64 + 1) * 86_400_000_000_000);
        let u = rng.next_f64();
        // Return with slight positive drift (0.04% daily ≈ 10% annual)
        let daily_return = (u - 0.48) * 0.025;
        price = (price * (1.0 + daily_return)).max(1.0);

        let high = price * (1.0 + rng.next_f64() * 0.008);
        let low = price * (1.0 - rng.next_f64() * 0.008);
        let open = price * (1.0 + (rng.next_f64() - 0.5) * 0.004);
        let volume = 500_000 + (rng.next_u64() % 200_000);

        bars.push(Bar::same_bar(
            ts,
            open,
            high.max(open),
            low.min(open),
            price,
            volume,
        ));
    }

    bars
}

/// Generates deterministic signals spanning the 10-year dataset.
fn generate_10yr_signals(bars: &[Bar], instrument: InstrumentId, symbol: &str) -> Vec<Signal> {
    let mut signals = Vec::new();
    let mut rng = Lcg::new(1337);

    // Generate signals every 5 trading days (weekly rebalance)
    for (i, bar) in bars.iter().enumerate() {
        if i % 5 == 0 {
            let u = rng.next_f64();
            let (direction, exp_ret) = if u > 0.55 {
                (Direction::Long, 0.03)
            } else if u < 0.40 {
                (Direction::Short, -0.02)
            } else {
                (Direction::Flat, 0.0)
            };

            let confidence = 0.5 + rng.next_f64() * 0.45;
            signals.push(Signal {
                direction,
                expected_return: exp_ret,
                confidence,
                horizon_bars: 5,
                instrument,
                symbol: symbol.to_string(),
                as_of: bar.timestamp.as_nanos(),
                model_id: "golden_v1".to_string(),
            });
        }
    }

    signals
}

fn run_golden_simulation(bars: &[Bar], signals: &[Signal]) -> BacktestReport {
    let instrument = InstrumentId(1);
    let symbol = "GOLDEN_EQ";
    let config = BacktestConfig {
        periods_per_year: 252.0,
        initial_cash: 1_000_000.0,
        risk_free_rate: 0.03,
        num_prior_trials: 1,
        allow_reuse: true,
        split: "test".to_string(),
    };

    let engine = BacktestEngine::new(config);
    let constructor = VolatilityTargetedConstructor::new(0.18);
    let execution_model = CompositeExecutionModel {
        commission_rate: 0.0002, // 2 bps
        min_commission: 0.50,
        half_spread_bps: 1.0,
        slippage_factor: 0.05,
        default_participation_cap: 0.05,
    };
    let constraints = PortfolioConstraints::default();

    let stream = ManualSignalStream::from_signals(signals.to_vec());
    engine
        .run(
            stream,
            &constructor,
            &execution_model,
            bars,
            instrument,
            symbol,
            &constraints,
            "golden_v1",
        )
        .expect("Backtest run must succeed")
}

#[test]
fn test_10yr_golden_backtest_determinism() {
    let bars = generate_10yr_synthetic_bars(100.0);
    assert_eq!(
        bars.len(),
        2520,
        "Must be exactly 2,520 trading days (10 yrs)"
    );

    let instrument = InstrumentId(1);
    let symbol = "GOLDEN_EQ";
    let signals = generate_10yr_signals(&bars, instrument, symbol);
    assert!(!signals.is_empty(), "Must have generated signals");

    // Run 1
    let report1 = run_golden_simulation(&bars, &signals);

    // Run 2
    let report2 = run_golden_simulation(&bars, &signals);

    // 1. Strict byte-for-byte JSON determinism
    let json1 = serde_json::to_string_pretty(&report1).unwrap();
    let json2 = serde_json::to_string_pretty(&report2).unwrap();
    assert_eq!(
        json1, json2,
        "Backtest output must be byte-for-byte identical across runs"
    );

    // 2. Metrics sanity and bounds checks
    assert!(report1.total_trades > 0, "Must have executed trades");
    assert!(report1.hit_rate >= 0.0 && report1.hit_rate <= 1.0);
    assert!(report1.max_drawdown >= 0.0 && report1.max_drawdown <= 1.0);
    assert!(report1.sharpe.is_finite());
    // 3. Golden fixture comparison
    let fixture_path = std::path::Path::new("tests/fixtures/golden_report_10yr.json");
    if let Some(parent) = fixture_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(fixture_path, &json1).expect("Explicit golden update");
    }
    if fixture_path.exists() {
        let fixture_data = std::fs::read_to_string(fixture_path).expect("Read fixture file");
        let golden_report: BacktestReport =
            serde_json::from_str(&fixture_data).expect("Parse golden report fixture");

        assert_eq!(report1.total_trades, golden_report.total_trades);
        assert!((report1.total_return_pct - golden_report.total_return_pct).abs() < 1e-10);
        assert!((report1.sharpe - golden_report.sharpe).abs() < 1e-10);
        assert!((report1.max_drawdown - golden_report.max_drawdown).abs() < 1e-10);
        assert!((report1.hit_rate - golden_report.hit_rate).abs() < 1e-10);
        assert!((report1.final_nav - golden_report.final_nav).abs() < 1e-6);
    } else {
        panic!("Missing golden fixture; regenerate explicitly with UPDATE_GOLDEN=1");
    }

    println!(
        "Golden 10yr Backtest Verified: Trades={}, Return={:.2}%, Sharpe={:.2}, MaxDD={:.2}%, HitRate={:.1}%",
        report1.total_trades,
        report1.total_return_pct * 100.0,
        report1.sharpe,
        report1.max_drawdown * 100.0,
        report1.hit_rate * 100.0,
    );
}
