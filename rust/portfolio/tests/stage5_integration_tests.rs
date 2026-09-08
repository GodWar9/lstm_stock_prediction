//! Stage 5 Integration Test: Signal -> TargetPositions -> Order -> Fill -> Portfolio -> RiskReport

use quant_data::types::{Bar, Timestamp};
use quant_execution::{CompositeExecutionModel, ExecutionModel, Order};
use quant_instruments::InstrumentId;
use quant_portfolio::{
    Portfolio, PortfolioConstraints, PortfolioConstructor, VolatilityTargetedConstructor,
};
use quant_risk::RiskEngine;
use quant_signals::{Direction, Signal};
use std::collections::HashMap;

#[test]
fn test_stage5_pipeline_end_to_end() {
    let initial_cash = 100_000.0;
    let mut portfolio = Portfolio::new(initial_cash);
    let constructor = VolatilityTargetedConstructor::new(0.20);
    let exec_model = CompositeExecutionModel::default();
    let risk_engine = RiskEngine::new();

    let id_aapl = InstrumentId(1);
    let id_msft = InstrumentId(2);

    let mut prices = HashMap::new();
    prices.insert(id_aapl, 150.0);
    prices.insert(id_msft, 250.0);

    // 1. Generate Signals
    let signals = vec![
        Signal {
            direction: Direction::Long,
            expected_return: 0.025,
            confidence: 0.85,
            horizon_bars: 1,
            instrument: id_aapl,
            symbol: "AAPL".to_string(),
            model_id: "lstm_v1".to_string(),
            as_of: 1000,
        },
        Signal {
            direction: Direction::Long,
            expected_return: 0.015,
            confidence: 0.70,
            horizon_bars: 1,
            instrument: id_msft,
            symbol: "MSFT".to_string(),
            model_id: "lstm_v1".to_string(),
            as_of: 1000,
        },
    ];

    // 2. Compute TargetPositions via PortfolioConstructor
    let constraints = PortfolioConstraints {
        max_gross_exposure: 0.80,
        max_position_pct: 0.20,
        ..Default::default()
    };
    let targets = constructor.target_positions(&signals, &portfolio, &prices, &constraints);

    assert!(targets.gross_weight <= constraints.max_gross_exposure);
    assert_eq!(targets.targets.len(), 2);

    // 3. Diff Targets vs Current Positions to generate Orders
    let mut orders = Vec::new();
    for (inst, target) in &targets.targets {
        let current_qty = portfolio
            .positions
            .get(inst)
            .map(|p| p.quantity)
            .unwrap_or(0.0);
        let delta_qty = target.target_quantity - current_qty;
        if delta_qty.abs() > 1e-4 {
            orders.push(Order::market(*inst, &target.symbol, delta_qty, 1000));
        }
    }
    assert_eq!(orders.len(), 2);

    // 4. Simulate Fills through ExecutionModel
    let bar_aapl = Bar::same_bar(Timestamp(1000), 150.0, 151.0, 149.0, 150.0, 1_000_000);
    let bar_msft = Bar::same_bar(Timestamp(1000), 250.0, 252.0, 248.0, 250.0, 500_000);

    let mut fills = Vec::new();
    for order in &orders {
        let bar = if order.instrument == id_aapl {
            &bar_aapl
        } else {
            &bar_msft
        };
        let fill = exec_model.simulate_fill(order, bar);
        assert!(fill.is_filled());
        fills.push(fill);
    }
    assert_eq!(fills.len(), 2);

    // 5. Update Portfolio State with Fills
    for fill in &fills {
        portfolio.apply_trade(
            fill.instrument,
            &fill.symbol,
            fill.fill_price,
            fill.fill_quantity,
            fill.commission,
        );
    }

    assert!(portfolio.positions.get(&id_aapl).unwrap().quantity > 0.0);
    assert!(portfolio.positions.get(&id_msft).unwrap().quantity > 0.0);
    assert!(portfolio.cash < initial_cash);
    assert!(portfolio.total_commissions > 0.0);

    // 6. Evaluate Portfolio Risk via RiskEngine
    let historical_returns = vec![0.005, -0.002, 0.008, 0.001, -0.004];
    let benchmark_returns = vec![0.004, -0.001, 0.006, 0.002, -0.003];
    let risk_report = risk_engine.evaluate(&portfolio, &historical_returns, &benchmark_returns);

    assert!(risk_report.var_95 > 0.0);
    assert!(risk_report.cvar_95 >= risk_report.var_95);
    assert!(risk_report.gross_exposure > 0.0);
    assert_eq!(risk_report.stress_scenarios.len(), 3);
}
