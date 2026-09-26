use quant_backtest::BacktestReport;
use quant_execution::Fill;
use quant_instruments::InstrumentId;

fn fill(qty: f64, price: f64, fee: f64) -> Fill {
    Fill {
        instrument: InstrumentId(1),
        symbol: "TEST".into(),
        fill_price: price,
        fill_quantity: qty,
        commission: fee,
        slippage: 0.0,
        as_of: 1,
    }
}

#[test]
fn realized_outcomes_include_both_entry_and_exit_fees() {
    let report = BacktestReport::compute(
        1000.0,
        vec![(0, 1000.0), (1, 1008.0)],
        vec![fill(10.0, 10.0, 1.0), fill(-10.0, 11.0, 1.0)],
        0.0,
        1,
    );
    assert_eq!(report.winning_trades, 1);
    assert_eq!(report.losing_trades, 0);
    assert_eq!(report.hit_rate, 1.0);
    assert!((report.avg_win - 0.08).abs() < 1e-12);
    assert!((report.turnover - 0.21).abs() < 1e-12);
}

#[test]
fn open_positions_are_not_counted_as_winning_trades() {
    let report = BacktestReport::compute(
        1000.0,
        vec![(0, 1000.0)],
        vec![fill(10.0, 10.0, 1.0)],
        0.0,
        1,
    );
    assert_eq!(report.winning_trades, 0);
    assert_eq!(report.hit_rate, 0.0);
}

#[test]
fn short_cover_and_reversal_realize_correct_side() {
    let report = BacktestReport::compute(
        1000.0,
        vec![(0, 1000.0)],
        vec![
            fill(-10.0, 10.0, 0.0),
            fill(15.0, 9.0, 0.0),
            fill(-5.0, 8.0, 0.0),
        ],
        0.0,
        1,
    );
    assert_eq!(report.winning_trades, 1);
    assert_eq!(report.losing_trades, 1);
    assert_eq!(report.hit_rate, 0.5);
    assert_eq!(report.profit_factor, 2.0);
}
