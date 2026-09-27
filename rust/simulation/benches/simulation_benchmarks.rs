//! Criterion micro-benchmarks for Monte Carlo resampling and regime guardrails.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quant_backtest::BacktestReport;
use quant_portfolio::Portfolio;
use quant_simulation::{
    MonteCarloResampler, Regime, RegimeDetector, RegimeSwitchingGuardrails,
    RuleBasedRegimeDetector, SimulationStrategy,
};

fn generate_mock_backtest_report(n_days: usize) -> BacktestReport {
    let mut returns = Vec::with_capacity(n_days);
    for i in 0..n_days {
        let ret = (((i * 29 + 17) % 100) as f64 - 48.0) / 1000.0;
        returns.push(ret);
    }

    BacktestReport {
        equity_curve: vec![],
        returns,
        initial_cash: 100_000.0,
        final_nav: 120_000.0,
        total_return_pct: 0.20,
        cagr: 0.20,
        sharpe: 1.5,
        sortino: 2.1,
        calmar: 2.5,
        max_drawdown: 0.08,
        profit_factor: 1.65,
        turnover: 2.0,
        hit_rate: 0.58,
        avg_win: 0.015,
        avg_loss: -0.010,
        total_trades: 50,
        winning_trades: 29,
        losing_trades: 21,
        deflated_sharpe: 1.35,
        trade_log: vec![],
        positions_curve: vec![],
        benchmark_curve: vec![],
        benchmark_returns: vec![],
        benchmark_total_return: 0.0,
    }
}

fn bench_monte_carlo_resampling(c: &mut Criterion) {
    let report = generate_mock_backtest_report(252);
    let resampler = MonteCarloResampler::new(42);

    c.bench_function("monte_carlo_1000_paths_252_days", |b| {
        b.iter(|| {
            let res = resampler.generate_paths(black_box(&report), black_box(1000));
            black_box(res);
        });
    });
}

fn bench_regime_detection(c: &mut Criterion) {
    let detector = RuleBasedRegimeDetector::default();
    let returns: Vec<f64> = (0..252)
        .map(|i| (((i * 13 + 7) % 50) as f64 - 24.0) / 1000.0)
        .collect();

    c.bench_function("regime_detection_252_bars", |b| {
        b.iter(|| {
            let regime = detector.label(black_box(&returns));
            black_box(regime);
        });
    });
}

fn bench_regime_guardrails(c: &mut Criterion) {
    let guardrails = RegimeSwitchingGuardrails::default();
    let portfolio = Portfolio::new(100_000.0);

    c.bench_function("guardrail_evaluate_portfolio", |b| {
        b.iter(|| {
            let status = guardrails.evaluate(black_box(&portfolio), black_box(Regime::Bull));
            black_box(status);
        });
    });

    c.bench_function("guardrail_clamp_order_quantity", |b| {
        b.iter(|| {
            let qty = guardrails.clamp_order_quantity(
                black_box(500.0),
                black_box(150.0),
                black_box(100_000.0),
                black_box(Regime::Bear),
            );
            black_box(qty);
        });
    });
}

criterion_group!(
    benches,
    bench_monte_carlo_resampling,
    bench_regime_detection,
    bench_regime_guardrails
);
criterion_main!(benches);
