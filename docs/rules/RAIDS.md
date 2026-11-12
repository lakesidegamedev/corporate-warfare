# Raids (2.0)

A raid is a marketing campaign against a rival's division: the attacker spends a budget to win
over part of the rival's business. Since 2.0 a raid is announced first and decided later, so the
target has time to defend. Engine: `economy/sectors.rs` (`launch_raid`, `raid_exposure`,
`resolve_raid_campaigns`, `abandon_raid`, `whole_business_chance`). The warning is
`capacity.raid_warning_weeks` in `content/initial-balance.json` (4). A value of 0 resolves a raid
the day it is launched, the 1.x behaviour.

## Launching
The attacker names a rival's division (one industry in one region) where it also runs a division.
Refused when: the budget is not positive; the target is in the attacker's own group; the attacker
has no division there; the target's division is younger than 26 weeks (`RAID_GRACE_WEEKS`, counted
from its oldest plant's start); or the attacker lacks the cash for the budget plus the price and
surcharge below. One campaign per attacker against the same division at a time.

On launch the budget moves from cash to restricted cash (escrow). The campaign is recorded with
its announcement week and the week it resolves, and every party sees it (the target's alert, the
raider's letter, the paper, the Raids screen).

## Strength and chance (read at resolution)
- Attack = the budget plus 13 weeks of the attacker's standing weekly marketing.
- Defence = 13 weeks of the defender's weekly marketing plus 5 percent of the division's
  quarterly output value (`RAID_DEFENCE_FLOOR_BPS` 500). The defender's marketing is read on the
  resolution day, so raising it during the warning cuts the chance.
- Base chance = attack over (attack plus defence).
- Plants at stake: 1 percent of the division's blocks at even strength (`RAID_STAKE_MIN_BPS`
  100), rising in a straight line to the whole division at a five to one advantage, never fewer
  than one block.
- Chance = base chance times (1 minus 85 percent of the stake share) (`RAID_STAKE_DRAG_BPS`
  8,500), then the whole business rule below, kept between 5 and 95 percent.
- Whole business rule: with `s` the share of the defender's entire business (all its blocks) that
  the stake is, chance = chance x (1 - s x (1 - base)), floor 5 percent x (1 - s), cap 95 percent.
  A company's only block is taken only about as often as the attack would win twice.

## Resolution
Each week, campaigns due are resolved oldest first, each against the defence at that moment.
- The escrow is spent on the attacker's advertising whatever happens.
- On success the attacker takes the blocks at stake into its own division at its own strategy and
  pays the defender half their book value (`RAID_PRICE_BPS` 5,000). A raider that cannot pay the
  price and the surcharge when it falls due loses.
- Each raid still on the record against the same target (one is forgotten per quarter) adds a
  surcharge of 40 percent of the budget, paid to the outside legal sector as an expense.
- Reputation: a successful raid costs the defender 300 plus a fifth of the stake taken, a failed
  one 100; a repeat raider loses 120 per raid on record.
- A campaign whose raider or target division no longer exists lapses: the budget is spent and
  nothing is taken.

## Calling off
The attacker can abandon its own campaign (`AbandonRaid`). The escrow is spent all the same and
no surcharge or purchase follows.

## Rivals
A rival launches at most one raid a year, and only the aggressive temperaments raid at all
(raid or bid weight 3 or more). Before committing a budget a raider allows for the target
defending, and a target answers a raid in character: it raises its marketing when the division is
worth it to it, or lets the block go. Rival raids on a rival are reported in the paper.

## Saved state
`World.raids` is `Raids { log, campaigns }` (save layout 4; layouts 1 to 3 wrote the log alone and
load with no campaigns). Content file for every number above: `content/initial-balance.json`
(`capacity.raid_warning_weeks`); the other constants are in `sectors.rs`.
