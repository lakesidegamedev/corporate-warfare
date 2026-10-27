//! Paying for services from the corridor's own producers.
//!
//! A service cannot be stockpiled: whatever a firm makes in a week expires at its close. Buyers
//! that pay for freight, advertising or a service input must therefore pay the firms that
//! actually hold that service this week, in proportion to what they hold, so the sale is
//! recorded, the stock is relieved and the producer's planner sees demand. Only what the
//! domestic producers cannot supply is bought from the finite outside sector.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::World;

/// Pay `amount` cents for a service. The buyer's own side of the posting is the caller's; this
/// settles the sellers. Domestic holders of the service are paid first, each up to the value of
/// the stock it holds at the reference price, in proportion to holdings, and the remainder goes
/// to the outside sector. The buyer never pays itself.
pub fn pay_for_service(
    world: &mut World,
    dataset: &Dataset,
    buyer: Option<Handle>,
    product: u16,
    amount: i64,
    memo: &str,
) {
    if amount <= 0 {
        return;
    }
    let price = dataset.reference_price(product).max(1);
    // (firm, value of the stock it holds in cents)
    let holders: Vec<(Handle, i64)> = world
        .firms
        .iter()
        .filter(|(handle, firm)| Some(*handle) != buyer && firm.operative())
        .filter_map(|(handle, firm)| {
            let held: i64 = firm
                .inventory
                .iter()
                .filter(|((p, _), _)| *p == product)
                .map(|(_, lot)| lot.qty.0.max(0))
                .sum();
            let value = round_even_div(held as i128 * price as i128, 1_000) as i64;
            (value > 0).then_some((handle, value))
        })
        .collect();
    let total_value: i64 = holders.iter().map(|(_, value)| *value).sum();
    let domestic = amount.min(total_value);
    let mut paid_out = 0i64;
    if domestic > 0 {
        let weights: Vec<(u64, i64)> = holders
            .iter()
            .map(|(handle, value)| (handle.0, *value))
            .collect();
        let split = allocate_cents(Cents(domestic), &weights);
        for ((handle, value), (_, share)) in holders.iter().zip(split) {
            let pay = share.0.min(*value);
            if pay <= 0 {
                continue;
            }
            let qty = MilliLots(round_even_div(pay as i128 * 1_000, price as i128) as i64);
            let tick = world.tick;
            let Some(firm) = world.firms.get_mut(*handle) else {
                continue;
            };
            // Relieve the stock the sale used, region by region, and carry its cost to expense.
            let mut remaining = qty;
            let mut sold = MilliLots::ZERO;
            let mut carrying = 0i64;
            let regions: Vec<u8> = firm
                .inventory
                .iter()
                .filter(|((p, _), lot)| *p == product && lot.qty.0 > 0)
                .map(|((_, region), _)| *region)
                .collect();
            for region in regions {
                if remaining.0 <= 0 {
                    break;
                }
                let take = remaining.min(firm.stock(product, region));
                if take.0 <= 0 {
                    continue;
                }
                carrying += firm.issue_stock(product, region, take).0;
                firm.record_sale(region, product, take);
                remaining = MilliLots(remaining.0 - take.0);
                sold = sold.checked_add(take).unwrap_or(sold);
            }
            let _ = firm.book.commit(
                tick,
                Txn::new()
                    .debit(Account::Cash, pay, memo)
                    .credit(Account::Revenue, pay, memo)
                    .credit(Account::Inventory, carrying, "service cost")
                    .debit(Account::CostOfGoods, carrying, "service cost"),
            );
            paid_out += pay;
        }
    }
    let outside = amount - paid_out;
    if outside > 0 {
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, outside, memo)
                .credit(Account::Revenue, outside, memo),
        );
    }
}
