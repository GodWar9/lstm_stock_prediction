//! Property-based testing for market data validation and PIT leakage guards.

use proptest::prelude::*;
use quant_data::{
    check_duplicate_timestamps, check_feature_target_leakage, validate_bars_monotonic_and_sound,
    Bar, Timestamp,
};

proptest! {
    #[test]
    fn prop_duplicate_timestamps_always_rejected(
        t1 in 1i64..1_000_000,
        t2 in 1_000_001i64..2_000_000,
        price in 1.0f64..500.0,
    ) {
        let b1 = Bar::same_bar(Timestamp(t1), price, price * 1.01, price * 0.99, price, 100);
        let b2 = Bar::same_bar(Timestamp(t2), price, price * 1.01, price * 0.99, price, 100);
        let b_dup = Bar::same_bar(Timestamp(t1), price, price * 1.01, price * 0.99, price, 100);

        let bars_with_dup = vec![b1, b2, b_dup];
        prop_assert!(check_duplicate_timestamps(&bars_with_dup).is_err());
    }

    #[test]
    fn prop_future_feature_availability_always_rejected(
        t_feat in 1_000_001i64..2_000_000,
        t_target in 1i64..1_000_000,
    ) {
        // feature availability is strictly in the future relative to target observation
        let res = check_feature_target_leakage(Timestamp(t_feat), Timestamp(t_target));
        prop_assert!(res.is_err());
    }

    #[test]
    fn prop_past_feature_availability_always_accepted(
        t_feat in 1i64..1_000_000,
        t_target in 1_000_000i64..2_000_000,
    ) {
        // feature availability is in the past or concurrent relative to target
        let res = check_feature_target_leakage(Timestamp(t_feat), Timestamp(t_target));
        prop_assert!(res.is_ok());
    }

    #[test]
    fn prop_non_monotonic_timestamps_always_rejected(
        t1 in 1_000_000i64..2_000_000,
        t2 in 1i64..999_999,
        price in 1.0f64..500.0,
    ) {
        let b1 = Bar::same_bar(Timestamp(t1), price, price * 1.01, price * 0.99, price, 100);
        let b2 = Bar::same_bar(Timestamp(t2), price, price * 1.01, price * 0.99, price, 100);

        let non_monotonic = vec![b1, b2];
        prop_assert!(validate_bars_monotonic_and_sound(&non_monotonic).is_err());
    }
}
