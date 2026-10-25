//! Money, basis points and deterministic rounding.
//!
//! Money is USD cents in a checked `i64`. Every proportional computation widens to `i128`,
//! rounds half to even, and only then narrows back. Splits use largest-remainder
//! allocation with stable entity-ID tie breaks so that no table order can change a result.

use serde::{Deserialize, Serialize};

/// One US cent.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);
    pub const ONE: Cents = Cents(1);

    pub const fn from_dollars(dollars: i64) -> Self {
        Cents(dollars * 100)
    }

    pub const fn dollars(self) -> i64 {
        self.0 / 100
    }

    pub fn checked_add(self, other: Cents) -> Option<Cents> {
        self.0.checked_add(other.0).map(Cents)
    }

    pub fn checked_sub(self, other: Cents) -> Option<Cents> {
        self.0.checked_sub(other.0).map(Cents)
    }

    pub fn checked_neg(self) -> Option<Cents> {
        self.0.checked_neg().map(Cents)
    }

    pub fn saturating_add(self, other: Cents) -> Cents {
        Cents(self.0.saturating_add(other.0))
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub fn max_zero(self) -> Cents {
        Cents(self.0.max(0))
    }

    /// Multiply by a basis-point rate (10000 bps = 100 percent), rounding half to even.
    pub fn mul_bps(self, bps: i32) -> Option<Cents> {
        let value = round_even_div(self.0 as i128 * bps as i128, 10_000);
        i64::try_from(value).ok().map(Cents)
    }

    /// Divide by an integer count, rounding half to even. Zero counts are rejected.
    pub fn div_even(self, count: i64) -> Option<Cents> {
        if count == 0 {
            return None;
        }
        let value = round_even_div(self.0 as i128, count as i128);
        i64::try_from(value).ok().map(Cents)
    }

    /// Ratio of this amount to `other` in basis points, or `None` when `other` is zero.
    pub fn ratio_bps(self, other: Cents) -> Option<i32> {
        if other.0 == 0 {
            return None;
        }
        let bps = round_even_div(self.0 as i128 * 10_000, other.0 as i128);
        i32::try_from(bps).ok()
    }
}

/// Round a rational to the nearest integer, ties to even. `den` must be positive.
pub fn round_even_div(num: i128, den: i128) -> i128 {
    assert!(den > 0, "denominator must be positive");
    let negative = num < 0;
    let magnitude = num.unsigned_abs();
    let denominator = den as u128;
    let mut quotient = magnitude / denominator;
    let remainder = magnitude % denominator;
    let doubled = remainder * 2;
    if doubled > denominator || (doubled == denominator && quotient & 1 == 1) {
        quotient += 1;
    }
    let quotient = quotient as i128;
    if negative { -quotient } else { quotient }
}

/// Split `total` across stable-ID weighted shares with largest remainders.
///
/// Each weight receives `floor(total * weight / sum_weights)`. The residual cents are then
/// assigned one at a time in descending fractional-remainder order, ties broken by the
/// lower stable ID. The returned amounts always sum exactly to `total`.
pub fn allocate_cents(total: Cents, weights: &[(u64, i64)]) -> Vec<(u64, Cents)> {
    let mut shares: Vec<(u64, Cents)> = weights.iter().map(|(id, _)| (*id, Cents::ZERO)).collect();
    let positive: Vec<(usize, u64, i64)> = weights
        .iter()
        .enumerate()
        .filter(|(_, (_, w))| *w > 0)
        .map(|(index, (id, w))| (index, *id, *w))
        .collect();
    if positive.is_empty() || total.0 == 0 {
        return shares;
    }
    let negative = total.0 < 0;
    let magnitude = total.0.unsigned_abs() as i128;
    let sum: i128 = positive.iter().map(|(_, _, w)| *w as i128).sum();
    let mut assigned: i128 = 0;
    let mut remainders: Vec<(usize, u64, i128)> = Vec::with_capacity(positive.len());
    for (index, id, weight) in &positive {
        let numerator = magnitude * *weight as i128;
        let base = numerator / sum;
        let remainder = numerator % sum;
        assigned += base;
        shares[*index].1 = Cents(base as i64);
        remainders.push((*index, *id, remainder));
    }
    let mut residual = magnitude - assigned;
    remainders.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
    let mut cursor = 0;
    while residual > 0 && !remainders.is_empty() {
        let (index, _, _) = remainders[cursor % remainders.len()];
        shares[index].1 = Cents(shares[index].1.0 + 1);
        residual -= 1;
        cursor += 1;
    }
    if negative {
        for share in &mut shares {
            share.1 = Cents(-share.1.0);
        }
    }
    shares
}

/// A rate expressed in basis points (100 bps = 1 percent).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Bps(pub i32);

impl Bps {
    pub const ZERO: Bps = Bps(0);
    pub const ONE_HUNDRED_PERCENT: Bps = Bps(10_000);

    pub const fn new(bps: i32) -> Self {
        Bps(bps)
    }

    pub fn clamp(self, low: i32, high: i32) -> Bps {
        Bps(self.0.clamp(low, high))
    }

    pub fn checked_add(self, other: Bps) -> Option<Bps> {
        self.0.checked_add(other.0).map(Bps)
    }

    pub fn of(self, amount: Cents) -> Option<Cents> {
        amount.mul_bps(self.0)
    }
}

/// Accumulates sub-cent interest or payroll remainders so repeated rounding cannot create
/// or destroy value. Complete cents are posted; the integer remainder is persisted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubCentCarry {
    /// Numerator in units of 1/`denominator` of a cent.
    pub numerator: i64,
    /// Positive denominator pinned by the computation that produced the carry.
    pub denominator: i64,
}

impl SubCentCarry {
    pub fn new(denominator: i64) -> Self {
        SubCentCarry {
            numerator: 0,
            denominator: denominator.max(1),
        }
    }

    /// Add an exact rational amount, returning whole cents to post and retaining the rest.
    pub fn accrue(&mut self, numerator: i128, denominator: i128) -> Cents {
        assert!(denominator > 0);
        let scaled = numerator * self.denominator as i128;
        let combined = self.numerator as i128 * denominator + scaled;
        let unit = denominator * self.denominator as i128;
        let whole = round_even_div(combined, unit);
        let remainder = combined - whole * unit;
        self.numerator = i64::try_from(remainder / denominator).unwrap_or(0);
        Cents(i64::try_from(whole).unwrap_or(0))
    }

    pub fn reset(&mut self) {
        self.numerator = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_even_rounding_is_symmetric() {
        assert_eq!(round_even_div(5, 2), 2);
        assert_eq!(round_even_div(7, 2), 4);
        assert_eq!(round_even_div(-5, 2), -2);
        assert_eq!(round_even_div(-7, 2), -4);
        assert_eq!(round_even_div(1, 3), 0);
        assert_eq!(round_even_div(2, 3), 1);
    }

    #[test]
    fn bps_rounding_is_half_even() {
        // 25 bps of $1.00 is exactly 0.25 cents -> 0.
        assert_eq!(Cents(100).mul_bps(25), Some(Cents(0)));
        // 50 bps of $1.00 is exactly 0.5 cents, ties to even -> 0.
        assert_eq!(Cents(100).mul_bps(50), Some(Cents(0)));
        // 150 bps of $1.00 is 1.5 cents, ties to even -> 2.
        assert_eq!(Cents(100).mul_bps(150), Some(Cents(2)));
    }

    #[test]
    fn allocation_conserves_total_and_is_stable() {
        let split = allocate_cents(Cents(100), &[(7, 1), (3, 1), (9, 1)]);
        assert_eq!(split.iter().map(|(_, c)| c.0).sum::<i64>(), 100);
        // 33.33 each; the residual cent goes to the lowest stable ID among equal remainders.
        assert_eq!(split, vec![(7, Cents(33)), (3, Cents(34)), (9, Cents(33))]);
    }

    #[test]
    fn allocation_handles_zero_and_debt() {
        assert_eq!(allocate_cents(Cents(0), &[(1, 5)]), vec![(1, Cents(0))]);
        let split = allocate_cents(Cents(-10), &[(1, 1), (2, 1)]);
        assert_eq!(split.iter().map(|(_, c)| c.0).sum::<i64>(), -10);
    }

    #[test]
    fn sub_cent_carry_never_creates_value() {
        // Ten weeks of 1/3 cent interest must post exactly 3 cents and retain 1/3.
        let mut carry = SubCentCarry::new(3);
        let mut total = 0i64;
        for _ in 0..10 {
            total += carry.accrue(1, 3).0;
        }
        assert_eq!(total, 3);
        assert_eq!(carry.numerator, 1);
    }
}
