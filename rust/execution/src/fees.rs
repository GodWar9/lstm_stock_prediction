//! Variable fee schedules: Tiered exchange fees, maker/taker pricing, and short borrow rates.

use serde::{Deserialize, Serialize};

/// Fee tier defined by minimum cumulative volume threshold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeeTier {
    /// Minimum monthly or cumulative volume in shares to qualify for this tier.
    pub min_volume: f64,
    /// Per-share fee for aggressive/taker orders (e.g. 0.0030 = $0.0030/sh).
    pub taker_per_share: f64,
    /// Per-share rebate (negative) or fee for passive/maker orders (e.g. -0.0015 = rebate).
    pub maker_per_share: f64,
    /// Percentage fee on notional (e.g. 0.0002 = 2 bps).
    pub notional_bps: f64,
}

/// Variable fee schedule supporting volume-tiered pricing and maker/taker rebates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableFeeSchedule {
    /// Ordered fee tiers from lowest to highest volume.
    pub tiers: Vec<FeeTier>,
    /// Minimum flat commission charge per transaction (e.g. $1.00).
    pub min_commission: f64,
    /// Annualized borrowing fee for short equity positions (e.g. 0.01 = 100 bps / year).
    pub short_borrow_rate_annual: f64,
}

impl Default for VariableFeeSchedule {
    fn default() -> Self {
        Self {
            tiers: vec![
                FeeTier {
                    min_volume: 0.0,
                    taker_per_share: 0.0030,  // $0.0030/sh taker
                    maker_per_share: -0.0015, // -$0.0015/sh maker rebate
                    notional_bps: 0.0,
                },
                FeeTier {
                    min_volume: 100_000.0,
                    taker_per_share: 0.0020,
                    maker_per_share: -0.0020,
                    notional_bps: 0.0,
                },
                FeeTier {
                    min_volume: 1_000_000.0,
                    taker_per_share: 0.0010,
                    maker_per_share: -0.0025,
                    notional_bps: 0.0,
                },
            ],
            min_commission: 0.50,
            short_borrow_rate_annual: 0.015, // 150 bps per annum
        }
    }
}

impl VariableFeeSchedule {
    /// Create a flat fee schedule for simple testing.
    pub fn flat(commission_bps: f64, min_commission: f64) -> Self {
        Self {
            tiers: vec![FeeTier {
                min_volume: 0.0,
                taker_per_share: 0.0,
                maker_per_share: 0.0,
                notional_bps: commission_bps,
            }],
            min_commission,
            short_borrow_rate_annual: 0.0,
        }
    }

    /// Determine active fee tier for a given cumulative volume.
    pub fn get_tier(&self, cumulative_volume: f64) -> &FeeTier {
        self.tiers
            .iter()
            .rev()
            .find(|tier| cumulative_volume >= tier.min_volume)
            .unwrap_or(&self.tiers[0])
    }

    /// Calculate fee for a trade.
    ///
    /// # Arguments
    /// * `shares` - Quantity executed (positive magnitude)
    /// * `price` - Execution price per share
    /// * `is_maker` - True if order was passive (limit), False if taker (market)
    /// * `cumulative_volume` - Trader's current monthly or cumulative volume
    pub fn calculate_trade_fee(
        &self,
        shares: f64,
        price: f64,
        is_maker: bool,
        cumulative_volume: f64,
    ) -> f64 {
        let shares_abs = shares.abs();
        let notional = shares_abs * price;
        let tier = self.get_tier(cumulative_volume);

        let per_share_rate = if is_maker {
            tier.maker_per_share
        } else {
            tier.taker_per_share
        };

        let share_fee = shares_abs * per_share_rate;
        let notional_fee = notional * (tier.notional_bps * 1e-4);
        let raw_fee = share_fee + notional_fee;

        // If it's a net fee (positive), enforce min_commission. If net rebate (negative), allow rebate.
        if raw_fee > 0.0 {
            raw_fee.max(self.min_commission)
        } else {
            raw_fee
        }
    }

    /// Calculate daily borrow fee for holding a short position overnight.
    pub fn daily_short_borrow_fee(&self, short_notional: f64) -> f64 {
        let daily_rate = self.short_borrow_rate_annual / 365.25;
        short_notional.abs() * daily_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_fee_tiers() {
        let schedule = VariableFeeSchedule::default();

        // Low volume -> Tier 0 taker fee
        let fee_low = schedule.calculate_trade_fee(1_000.0, 100.0, false, 50_000.0);
        assert_eq!(fee_low, 3.0); // 1,000 * 0.0030 = $3.00

        // High volume -> Tier 2 taker fee
        let fee_high = schedule.calculate_trade_fee(1_000.0, 100.0, false, 2_000_000.0);
        assert_eq!(fee_high, 1.0); // 1,000 * 0.0010 = $1.00
    }

    #[test]
    fn test_maker_rebate() {
        let schedule = VariableFeeSchedule::default();

        // Maker order gets negative fee (rebate)
        let rebate = schedule.calculate_trade_fee(10_000.0, 50.0, true, 0.0);
        assert_eq!(rebate, -15.0); // 10,000 * -0.0015 = -$15.00
    }

    #[test]
    fn test_short_borrow_fee() {
        let schedule = VariableFeeSchedule::default();
        let short_notional = 100_000.0;
        let fee = schedule.daily_short_borrow_fee(short_notional);
        assert!(fee > 0.0);
        let expected = 100_000.0 * (0.015 / 365.25);
        assert!((fee - expected).abs() < 1e-6);
    }
}
