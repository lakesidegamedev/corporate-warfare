//! Physical quantity in milli-lots.
//!
//! One lot is a standardised game batch. Recipes and prices are tabulated per whole lot for
//! readability; authoritative stock is tracked to 1/1000 lot so fractional weekly production
//! and pro-rata rationing never need floating point.

use serde::{Deserialize, Serialize};

use crate::money::{Cents, round_even_div};

/// Thousandths of a lot per whole lot.
pub const MILLI_PER_LOT: i64 = 1000;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct MilliLots(pub i64);

impl MilliLots {
    pub const ZERO: MilliLots = MilliLots(0);

    pub const fn from_lots(lots: i64) -> Self {
        MilliLots(lots * MILLI_PER_LOT)
    }

    pub const fn lots(self) -> i64 {
        self.0 / MILLI_PER_LOT
    }

    pub fn whole_lots_floor(self) -> i64 {
        self.0.div_euclid(MILLI_PER_LOT)
    }

    pub fn checked_add(self, other: MilliLots) -> Option<MilliLots> {
        self.0.checked_add(other.0).map(MilliLots)
    }

    pub fn checked_sub(self, other: MilliLots) -> Option<MilliLots> {
        self.0.checked_sub(other.0).map(MilliLots)
    }

    pub fn saturating_sub(self, other: MilliLots) -> MilliLots {
        MilliLots(self.0.saturating_sub(other.0))
    }

    pub fn min(self, other: MilliLots) -> MilliLots {
        MilliLots(self.0.min(other.0))
    }

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn max_zero(self) -> MilliLots {
        MilliLots(self.0.max(0))
    }

    /// Cost of this quantity at a per-whole-lot price, rounded half to even.
    pub fn value_at(self, price: Cents) -> Option<Cents> {
        let value = round_even_div(price.0 as i128 * self.0 as i128, MILLI_PER_LOT as i128);
        i64::try_from(value).ok().map(Cents)
    }

    /// Scale by a basis-point fraction (for partial capacity or yield adjustments).
    pub fn scale_bps(self, bps: i32) -> MilliLots {
        let value = round_even_div(self.0 as i128 * bps as i128, 10_000);
        MilliLots(i64::try_from(value).unwrap_or(i64::MAX))
    }

    /// Pro-rata share of `total` for each weight in stable-ID order, conserving exactly.
    pub fn allocate(total: MilliLots, weights: &[(u64, i64)]) -> Vec<(u64, MilliLots)> {
        let cents = crate::money::allocate_cents(
            Cents(total.0),
            &weights.iter().map(|(id, w)| (*id, *w)).collect::<Vec<_>>(),
        );
        cents
            .into_iter()
            .map(|(id, amount)| (id, MilliLots(amount.0)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_uses_half_even_rounding() {
        assert_eq!(
            MilliLots::from_lots(1).value_at(Cents(10_000)),
            Some(Cents(10_000))
        );
        // 0.5 lot at 1 cent is half a cent, ties to even -> 0.
        assert_eq!(MilliLots(500).value_at(Cents(1)), Some(Cents(0)));
        // 1.5 lots at 1 cent is 1.5 cents -> 2.
        assert_eq!(MilliLots(1500).value_at(Cents(1)), Some(Cents(2)));
    }

    #[test]
    fn allocation_conserves_quantity() {
        let split = MilliLots::allocate(MilliLots(1000), &[(1, 1), (2, 1), (3, 1)]);
        assert_eq!(split.iter().map(|(_, q)| q.0).sum::<i64>(), 1000);
        assert_eq!(split[0].1, MilliLots(334));
    }
}
