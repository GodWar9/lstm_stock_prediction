use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use quant_api::{
    artifacts::{publish_backtest, write_manifest},
    contract::Provenance,
    router,
};
use quant_backtest::BacktestReport;
use tower::ServiceExt;

#[tokio::test]
async fn real_report_roundtrip_arrow_schema_and_errors() {
    let temp = tempfile::tempdir().unwrap();
    let report = BacktestReport::compute(
        1000.0,
        vec![
            (0, 1000.0),
            (86_400_000_000_000, 990.0),
            (172_800_000_000_000, 1010.0),
        ],
        vec![],
        0.0,
        1,
    );
    let provenance = Provenance {
        git_commit: "test".into(),
        config_hash: "test".into(),
        data_version: "fixture".into(),
        model_artifact_id: "test".into(),
        source: "synthetic".into(),
    };
    let m = publish_backtest(
        &temp.path().join("reports/runs"),
        &report,
        provenance,
        "TEST",
        "test",
    )
    .unwrap();
    write_manifest(&temp.path().join("reports/runs").join(&m.run_id), &m).unwrap();
    let app = router(temp.path().into());
    for (suffix, status) in [
        ("manifest", 200),
        ("equity.arrow", 200),
        ("trades.arrow", 200),
        ("equity.arrow?max_points=0", 400),
        ("equity.arrow?max_points=no", 400),
        ("signals.arrow", 404),
        ("validation", 404),
        ("secrets.txt", 404),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/runs/{}/{suffix}", m.run_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), status, "{suffix}");
        if suffix == "equity.arrow" {
            assert!(response.headers()["cache-control"]
                .to_str()
                .unwrap()
                .contains("immutable"));
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let mut cursor = std::io::Cursor::new(bytes);
            let metadata = arrow2::io::ipc::read::read_file_metadata(&mut cursor).unwrap();
            assert_eq!(
                metadata
                    .schema
                    .fields
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["timestamp_ms", "nav", "drawdown"]
            );
            assert!(metadata
                .schema
                .fields
                .iter()
                .all(|f| f.data_type == arrow2::datatypes::DataType::Float64));
        }
    }
    let mut invalid = m.clone();
    invalid.schema_version = 99;
    write_manifest(&temp.path().join("reports/runs").join(&m.run_id), &invalid).unwrap();
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/runs/{}/manifest", m.run_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 422);
}

#[tokio::test]
async fn test_benchmark_positions_signal_outcomes_and_risk_endpoints() {
    let temp = tempfile::tempdir().unwrap();
    let report = BacktestReport::compute(
        1000.0,
        vec![
            (0, 1000.0),
            (86_400_000_000_000, 990.0),
            (172_800_000_000_000, 1010.0),
        ],
        vec![],
        0.0,
        1,
    )
    .with_benchmark_and_positions(
        vec![
            (0, 1000.0),
            (86_400_000_000_000, 1005.0),
            (172_800_000_000_000, 1020.0),
        ],
        vec![0.005, 0.015],
        0.02,
        vec![
            (0, 10.0, 1000.0),
            (86_400_000_000_000, 10.0, 990.0),
            (172_800_000_000_000, 10.0, 1010.0),
        ],
    );
    let provenance = Provenance {
        git_commit: "test".into(),
        config_hash: "test".into(),
        data_version: "fixture".into(),
        model_artifact_id: "test".into(),
        source: "synthetic".into(),
    };
    let mut m = publish_backtest(
        &temp.path().join("reports/runs"),
        &report,
        provenance,
        "TEST",
        "test",
    )
    .unwrap();
    let dir = temp.path().join("reports/runs").join(&m.run_id);

    // Write signal_outcomes.arrow
    let so_bytes = quant_api::artifacts::arrow_bytes(&[
        ("timestamp_ms", vec![0.0, 86_400_000.0]),
        ("expected_return", vec![0.01, 0.02]),
        ("realized_return", vec![0.005, 0.015]),
        ("residual", vec![0.005, 0.005]),
    ])
    .unwrap();
    std::fs::write(dir.join("signal_outcomes.arrow"), so_bytes).unwrap();
    m.capabilities.push("signal_outcomes".into());
    m.artifacts
        .insert("signal_outcomes".into(), "signal_outcomes.arrow".into());

    // Write risk.json
    let risk_json = serde_json::json!({
        "var_95": 15.2,
        "cvar_95": 22.1,
        "volatility": 0.18,
        "max_drawdown": 0.05,
        "beta": 1.05,
        "gross_exposure": 1.0,
        "net_exposure": 1.0,
        "turnover": 0.5,
        "factor_exposure": {"market": 1.05},
        "stress_scenarios": [{"scenario_name": "MarketCrash_10Pct", "estimated_pnl": -100.0, "estimated_pnl_pct": -0.10}]
    });
    std::fs::write(
        dir.join("risk.json"),
        serde_json::to_vec_pretty(&risk_json).unwrap(),
    )
    .unwrap();
    m.capabilities.push("risk".into());
    m.artifacts.insert("risk".into(), "risk.json".into());

    write_manifest(&dir, &m).unwrap();

    let app = router(temp.path().into());
    for suffix in [
        "benchmark.arrow",
        "positions.arrow",
        "signal_outcomes.arrow",
        "risk.json",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/runs/{}/{suffix}", m.run_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200, "Artifact {suffix} failed");
    }
}
