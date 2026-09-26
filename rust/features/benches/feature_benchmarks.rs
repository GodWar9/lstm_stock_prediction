//! Criterion micro-benchmarks for hot execution paths in quant_features.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quant_data::types::{Bar, Timestamp};
use quant_features::{compute_fracdiff_weights, normalize_cross_section, FeatureGraph, FracDiff};

fn generate_synthetic_bars(n: usize) -> Vec<Bar> {
    let mut bars = Vec::with_capacity(n);
    let mut price = 100.0;
    for i in 0..n {
        let ret = (((i * 17 + 31) % 100) as f64 - 50.0) / 1000.0;
        price = (price * (1.0 + ret)).max(1.0);
        let high = price * 1.005;
        let low = price * 0.995;
        bars.push(Bar {
            timestamp: Timestamp(i as i64 * 60_000),
            availability_timestamp: Timestamp(i as i64 * 60_000),
            open: price,
            high,
            low,
            close: price,
            volume: 10_000 + (i as u64 % 5_000),
            adjusted: false,
        });
    }
    bars
}

fn bench_feature_graph_batch(c: &mut Criterion) {
    let bars = generate_synthetic_bars(1000);

    c.bench_function("feature_graph_compute_1000_bars", |b| {
        b.iter(|| {
            let mut graph = FeatureGraph::new(vec![
                Box::new(quant_features::Sma::new(20)),
                Box::new(quant_features::Rsi::new(14)),
                Box::new(quant_features::BollingerBands::new(20, 2.0)),
            ]);
            let features = graph.compute_batch(black_box(&bars));
            black_box(features);
        });
    });
}

fn bench_fracdiff(c: &mut Criterion) {
    let weights = compute_fracdiff_weights(0.5, 1e-5, 500);
    let fracdiff = FracDiff::standard();
    let values: Vec<f64> = (0..500).map(|i| 100.0 + (i as f64) * 0.1).collect();

    c.bench_function("fracdiff_weights_generation", |b| {
        b.iter(|| {
            black_box(compute_fracdiff_weights(
                black_box(0.5),
                black_box(1e-5),
                black_box(500),
            ));
        });
    });

    c.bench_function("fracdiff_apply_500_slice", |b| {
        b.iter(|| {
            let val = fracdiff.apply_to_slice(black_box(&values));
            black_box(val);
        });
    });

    let _ = weights;
}

fn bench_cross_sectional_normalization(c: &mut Criterion) {
    let universe_100: Vec<(u32, f64)> = (0..100)
        .map(|id| (id, 100.0 + (id as f64) * 0.5 + ((id % 7) as f64) * 0.1))
        .collect();

    c.bench_function("cross_sectional_normalize_100_universe", |b| {
        b.iter(|| {
            let res = normalize_cross_section(black_box(&universe_100));
            black_box(res);
        });
    });
}

criterion_group!(
    benches,
    bench_feature_graph_batch,
    bench_fracdiff,
    bench_cross_sectional_normalization
);
criterion_main!(benches);
