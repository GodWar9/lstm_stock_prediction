use quant_backtest::{AssetBars, BacktestConfig, BacktestEngine, ManualSignalStream};
use quant_data::{Bar, Timestamp};
use quant_execution::CompositeExecutionModel;
use quant_instruments::InstrumentId;
use quant_portfolio::{PortfolioConstraints, VolatilityTargetedConstructor};
use quant_signals::{Direction, Signal};

fn bars(price: f64) -> Vec<Bar> {
    (1..=4)
        .map(|i| Bar::same_bar(Timestamp(i * 100), price, price, price, price, 100000))
        .collect()
}
fn signals() -> ManualSignalStream {
    ManualSignalStream::from_signals(
        (1..=3)
            .flat_map(|t| {
                (1..=2).map(move |id| Signal {
                    instrument: InstrumentId(id),
                    symbol: format!("S{id}"),
                    as_of: t * 100,
                    model_id: "test".into(),
                    direction: Direction::Long,
                    confidence: 1.0,
                    expected_return: 0.01,
                    horizon_bars: 1,
                })
            })
            .collect(),
    )
}

#[test]
fn shared_cash_causal_fills_per_asset_positions_and_input_order_invariance() {
    let a = bars(100.0);
    let b = bars(50.0);
    let engine = BacktestEngine::new(BacktestConfig::default());
    let constructor = VolatilityTargetedConstructor::new(0.2);
    let execution = CompositeExecutionModel::new(0.001, 1.0, 0.0, 0.0, 1.0);
    let constraints = PortfolioConstraints {
        max_position_pct: 0.5,
        max_net_exposure: 1.0,
        ..Default::default()
    };
    let assets = [
        AssetBars {
            instrument: InstrumentId(1),
            symbol: "S1",
            bars: &a,
        },
        AssetBars {
            instrument: InstrumentId(2),
            symbol: "S2",
            bars: &b,
        },
    ];
    let first = engine
        .run_multi_asset(signals(), &constructor, &execution, &assets, &constraints)
        .unwrap();
    let reversed = [
        AssetBars {
            instrument: InstrumentId(2),
            symbol: "S2",
            bars: &b,
        },
        AssetBars {
            instrument: InstrumentId(1),
            symbol: "S1",
            bars: &a,
        },
    ];
    let second = engine
        .run_multi_asset(signals(), &constructor, &execution, &reversed, &constraints)
        .unwrap();
    assert_eq!(first, second);
    assert!(first.report.trade_log.iter().all(|f| f.as_of >= 200));
    assert!(first.cash.iter().all(|(_, c)| *c >= 0.0));
    assert_eq!(first.positions.len(), 8);
    assert!(first.report.trade_log.iter().any(|f| f.symbol == "S1"));
    assert!(first.report.trade_log.iter().any(|f| f.symbol == "S2"));
    assert!(first.report.final_nav < first.report.initial_cash);
    for (t, nav) in &first.report.equity_curve[1..] {
        let holdings: f64 = first
            .positions
            .iter()
            .filter(|p| p.timestamp == *t)
            .map(|p| p.market_value)
            .sum();
        let cash = first.cash.iter().find(|(ts, _)| ts == t).unwrap().1;
        assert!((nav - holdings - cash).abs() < 1e-8);
    }
}

#[test]
fn rejects_misaligned_availability() {
    let a = bars(100.0);
    let mut b = bars(50.0);
    b[1].availability_timestamp = Timestamp(250);
    let engine = BacktestEngine::new(BacktestConfig::default());
    let assets = [
        AssetBars {
            instrument: InstrumentId(1),
            symbol: "S1",
            bars: &a,
        },
        AssetBars {
            instrument: InstrumentId(2),
            symbol: "S2",
            bars: &b,
        },
    ];
    assert!(engine
        .run_multi_asset(
            signals(),
            &VolatilityTargetedConstructor::new(0.2),
            &CompositeExecutionModel::default(),
            &assets,
            &PortfolioConstraints::default()
        )
        .is_err());
}

#[test]
fn sizes_with_previous_price_before_execution_price_is_known() {
    let mut a = bars(100.0);
    a[1] = Bar::same_bar(Timestamp(200), 50.0, 50.0, 50.0, 50.0, 100000);
    let stream = ManualSignalStream::from_signals(vec![Signal {
        instrument: InstrumentId(1),
        symbol: "S1".into(),
        as_of: 100,
        model_id: "test".into(),
        direction: Direction::Long,
        confidence: 1.0,
        expected_return: 0.01,
        horizon_bars: 1,
    }]);
    let result = BacktestEngine::new(BacktestConfig::default())
        .run_multi_asset(
            stream,
            &VolatilityTargetedConstructor::new(0.2),
            &CompositeExecutionModel::new(0.0, 0.0, 0.0, 0.0, 1.0),
            &[AssetBars {
                instrument: InstrumentId(1),
                symbol: "S1",
                bars: &a,
            }],
            &PortfolioConstraints {
                max_position_pct: 0.5,
                volatility_target: None,
                ..Default::default()
            },
        )
        .unwrap();
    let fill = &result.report.trade_log[0];
    assert_eq!(fill.as_of, 200);
    assert_eq!(fill.fill_quantity, 500.0);
    assert_eq!(fill.fill_price, 50.0);
}

#[test]
fn rejects_missing_bars_duplicates_and_overlapping_availability() {
    let a = bars(100.0);
    let mut delayed = a.clone();
    delayed[0].availability_timestamp = Timestamp(200);
    for (b, id, symbol) in [
        (&a[..3], 2, "S2"),
        (&a[..], 1, "S2"),
        (&a[..], 2, "S1"),
        (&delayed[..], 2, "S2"),
    ] {
        assert!(BacktestEngine::new(BacktestConfig::default())
            .run_multi_asset(
                signals(),
                &VolatilityTargetedConstructor::new(0.2),
                &CompositeExecutionModel::default(),
                &[
                    AssetBars {
                        instrument: InstrumentId(1),
                        symbol: "S1",
                        bars: &a
                    },
                    AssetBars {
                        instrument: InstrumentId(id),
                        symbol,
                        bars: b
                    }
                ],
                &PortfolioConstraints::default(),
            )
            .is_err());
    }
    assert!(BacktestEngine::new(BacktestConfig::default())
        .run_multi_asset(
            signals(),
            &VolatilityTargetedConstructor::new(0.2),
            &CompositeExecutionModel::default(),
            &[AssetBars {
                instrument: InstrumentId(1),
                symbol: "S1",
                bars: &delayed
            }],
            &PortfolioConstraints::default(),
        )
        .is_err());
}

#[test]
fn rejects_nonfinite_execution_results() {
    struct InvalidExecution;
    impl quant_execution::ExecutionModel for InvalidExecution {
        fn simulate_fill(
            &self,
            order: &quant_execution::Order,
            bar: &Bar,
        ) -> quant_execution::Fill {
            let mut fill = quant_execution::ExecutionModel::simulate_fill(
                &CompositeExecutionModel::default(),
                order,
                bar,
            );
            fill.commission = f64::NAN;
            fill
        }
    }
    let a = bars(100.0);
    let b = bars(50.0);
    assert!(BacktestEngine::new(BacktestConfig::default())
        .run_multi_asset(
            signals(),
            &VolatilityTargetedConstructor::new(0.2),
            &InvalidExecution,
            &[
                AssetBars {
                    instrument: InstrumentId(1),
                    symbol: "S1",
                    bars: &a
                },
                AssetBars {
                    instrument: InstrumentId(2),
                    symbol: "S2",
                    bars: &b
                }
            ],
            &PortfolioConstraints::default(),
        )
        .is_err());
}
