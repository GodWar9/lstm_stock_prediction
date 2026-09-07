//! Golden value tests for technical indicators against precomputed mathematical reference values.

use quant_data::{Bar, Timestamp};
use quant_features::*;

fn generate_sample_bars(closes: &[f64]) -> Vec<Bar> {
    closes
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            Bar::same_bar(
                Timestamp(1_700_000_000 + i as i64 * 86400),
                c * 0.99,
                c * 1.02,
                c * 0.98,
                c,
                1_000_000,
            )
        })
        .collect()
}

#[test]
fn test_golden_sma() {
    let prices = [22.27, 22.19, 22.08, 22.17, 22.18, 22.13, 22.23, 22.43, 22.24, 22.29];
    let bars = generate_sample_bars(&prices);
    let mut win = BarWindow::new(10);
    for b in bars {
        win.push(b);
    }

    let sma5 = Sma::new(5);
    let val = sma5.compute(&win).expect("SMA-5 should compute");
    // Last 5: 22.13, 22.23, 22.43, 22.24, 22.29 -> Sum = 111.32 -> 111.32 / 5 = 22.264
    let expected = 22.264;
    assert!((val - expected).abs() < 1e-6, "SMA mismatch: got {}, expected {}", val, expected);
}

#[test]
fn test_golden_bollinger_bands() {
    let prices = [20.0, 22.0, 21.0, 23.0, 24.0];
    let bars = generate_sample_bars(&prices);
    let mut win = BarWindow::new(5);
    for b in bars {
        win.push(b);
    }

    let bb = BollingerBands::new(5, 2.0);
    let out = bb.compute_bands(&win).expect("Bollinger bands should compute");
    // Mean = (20 + 22 + 21 + 23 + 24) / 5 = 22.0
    // Var = ((20-22)^2 + (22-22)^2 + (21-22)^2 + (23-22)^2 + (24-22)^2) / 5
    //     = (4 + 0 + 1 + 1 + 4) / 5 = 10 / 5 = 2.0
    // Std = sqrt(2.0) = 1.41421356...
    // Upper = 22.0 + 2 * 1.41421356 = 24.828427
    // Lower = 22.0 - 2 * 1.41421356 = 19.171573
    assert!((out.middle - 22.0).abs() < 1e-6);
    assert!((out.upper - 24.82842712).abs() < 1e-5);
    assert!((out.lower - 19.17157288).abs() < 1e-5);
}

#[test]
fn test_golden_rsi_wilder() {
    // Known test sequence
    let prices = [
        44.34, 44.09, 44.15, 43.61, 44.33, 44.83, 45.10, 45.42, 45.84, 46.08,
        45.89, 46.03, 45.61, 46.28, 46.28,
    ];
    let bars = generate_sample_bars(&prices);
    let mut win = BarWindow::new(15);
    for b in bars {
        win.push(b);
    }
    let rsi14 = Rsi::new(14);
    let val = rsi14.compute(&win).expect("RSI-14 should compute");
    // Classic Wilder's 14-period RSI on this sequence produces ~70.53
    assert!(val > 68.0 && val < 73.0, "RSI out of expected range: {}", val);
}

#[test]
fn test_golden_log_return_property() {
    let prices = [100.0, 110.0];
    let bars = generate_sample_bars(&prices);
    let mut win = BarWindow::new(2);
    for b in bars {
        win.push(b);
    }
    let lr = LogReturn::new(1);
    let val = lr.compute(&win).unwrap();
    let expected = (1.10f64).ln();
    assert!((val - expected).abs() < 1e-9);
}
