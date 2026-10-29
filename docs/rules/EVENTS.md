# Event deck v0.1

Events reveal or modify the same rules, not a second unaccounted penalty engine.
Random opportunity events are checked at quarter boundaries: one eligible card per
region at most, 1000 BPS chance, stable sorted eligible list and event RNG selection.
A firm can have at most two unresolved optional cards. Mandatory state-transition
notifications bypass that cap. No repeated subject/card within26 weeks. Every choice
has a preview, deadline and persisted result. Deadline defaults below never advance
while mandatory player input has paused time.

| Card | Trigger | Choices and exact consequences |
|---|---|---|
| Supplier default | Actual supplier enters restructuring | Terminate future contract with escrow refund; or keep claim and wait. Existing insolvency handles stock/debt, no extra loss. |
| Energy outage | Random eligible energy region | Pay 2 weeks normal power expense for backup, retain75% capacity for2 weeks; or accept50% capacity, no extra fee. Supplier compensation follows contract. |
| Fuel spike | Macro opportunity, no active fuel shock | Reference fuel price +25% for13 weeks; buy finite available stock at current executable price or keep policy. No forced arbitrary cash loss. |
| Bridge closure | Random active freight lane | Reroute affected future shipments at+20% freight and+1 week for4 weeks; or pause new lane orders. Already dispatched reroutes require paid carrier quote. |
| Freight congestion | A lane the firm ships on has run above 90% full for 4 weeks (read from the lane fill statistics, drawn weekly, no random roll) | Reserve a 26-week contract on that lane (a real lane contract at the lane's rate less the commitment discount, sized to what the firm shipped last week); or use spot with +1-week delay for 4 weeks. No duplicate bridge effect on same lane. |
| Crop loss | Quarter crop event | Spend$10,000/farm block mitigation, yield-10% for13 weeks; or yield-25%. Physical insured loss invokes normal policy once. |
| Mine warning | Deposit <26 weeks planned extraction | Fund retool to another sourced deposit at50% new mine capex and13 weeks; or plan depletion/mothball. No free ore increase. |
| Quality recall | Quality <8000 and consumer goods sales | Pay sold affected lots*10% unit price, reputation-250 BPS; or voluntary refund at25% price, reputation-50. One4-week sales cohort, no endless recurring recall of same goods. |
| Talent poaching | Actual staffing churn >10% last4 weeks | Raise offered wage5% for next hires; or fund training at actual provider quote. Does not independently delete another worker. |
| Union drive | Organisation crosses5000 | Allow ballot with no fee; or pay documented training/conditions budget1% quarterly payroll to reduce next ballot support250 BPS. No illegal direct cash deletion. |
| Wage negotiation | Scheduled bargaining | Accept/counter/reject according to union rules. Card does not apply additional wage shock. |
| Strike escalation | Actual unresolved strike | Accept current legal settlement; or continue prescribed strike/fallback. No extra random strike duration. |
| Training grant | Government budget can fund | Commit firm match80%, government20% up to$100,000 for26 weeks; or decline. Grant reserved before award. |
| Permit delay | New project no active permit bonus | Pay expedited lawful engineering service2% capex for no delay; or add4 weeks. Fee supplier is explicit outside service. |
| Lobby backlash | Actual funded campaign plus layoffs | Publish disclosure, reputation-250; or cancel remaining request with no sunk-fee refund, reputation-100. No tax effect unless request actually won. |
| Antitrust review | Actual review threshold crossed | Submit feasible divestiture or withdraw bid. Normal legal engine determines release/refund. |
| Hostile stake | Other holder crosses25% votes | Schedule meeting/diligence or take no action. Cash and shares already transferred through exchange, never free event transfer. |
| Tender offer | Actual FUNDED/OFFER_OPEN bid | Tender held shares at stated terms or refuse; management can propose financed alternative subject to normal authority. |
| Covenant warning | Coverage/leverage breaches written covenant | Raise equity/repay or enter lender renegotiation, using real credit commands. No extra debt created by notification. |
| Bank surge | Region crisis or eligible bank random shock | Request25% deposit withdrawals pro rata; raise funded liquidity or suspend new loans. Withdrawal requests remain liabilities if unfunded. |
| Insurance catastrophe | Region random disaster, no active disaster | Physical loss30% insured asset values once; claim under policy; choose rebuild quote or mothball. A mothballed plant releases its whole workforce and pays no payroll, maintenance, rent or depreciation; reopening costs 5 percent of its capex (at least $1,000) and staff are rehired through the labour market. Rivals reopen when the plant earns a contribution at public prices, the corridor imports at least half its output, and the restart leaves their cash reserve intact. Funded payouts and correlated reserves follow normal insurer rules. |
| Tech opportunity | At least1 tech level below frontier | Pay normal next-level R&D threshold for90% success (one saved draw); or continue normal75% research. No instant unfunded level. |
| Recession | Macro selection, no regime active | Demand regime-10% for26 weeks; suggest cost/credit actions, no required fee. Does not delete cash or stock. |
| Demand boom | Macro selection, no regime active | Demand opportunity+10% for26 weeks, still income/budget constrained; choose expansion or keep policies. No free cash/market share. |

Commodity, yield and cost modifiers compose once per tagged cause and remain bounded by
CONTENT macro caps. Signed penalties/refunds post to actual counterparties. If exact
contract authority is unavailable, show why a response is disabled; do not bypass rules
for event convenience. Cards above require copy, icons and scenario tests, not new model
design by the implementation agent.

## Systemic events (1.7)

Systemic events hit the logistics network and the plants and storage behind it, not cash. They
are not cards: nothing to answer, they happen, are recorded as incidents and appear in the
news. Every roll comes from a stream keyed by the game seed and the week, so a saved and
reloaded game draws what an unbroken game draws. All five work in every scenario.

| Event | Weekly chance | Duration | Effects |
|---|---|---|---|
| Pipeline spill | 0.08% per pipeline lane with capacity | 3 to 8 weeks | Lane capacity 0; each operator pays a clean-up bill of 2 weeks of its lane capacity at the empty-lane rate. Pipelines have no outside carriers, so shippers take another mode. |
| Port strike | 0.06% per port (strong union law doubles it, weak halves it) | 2 to 5 weeks | Every sea lane touching the port has capacity 0; shipments go to outside carriers at the premium. |
| Refinery fire | 0.05% per running plant that makes by-products of liquid or gas goods | 8 to 16 weeks | The plant is shut (outage 0); 10% of the owner's stock of its products in that region burns, booked as an operating loss. |
| Lane outage | 0.03% per truck or rail lane with capacity | 1 to 4 weeks | Capacity 0 or 25% (even chance). |
| Harvest failure | 8% per farming region, once a year in the harvest quarter's last week | 26 weeks | Every crop yield in the region falls 25 to 40%. |

Closures reduce lane capacity in the week the lane is built, before contracts and shipments,
so conservation of freight money holds: shippers pay exactly what carriers and the outside
sector receive. A fire or spill never creates or destroys cash except the booked loss and the
clean-up payment to the outside services sector. Incidents name the firms they touch;
the overview alerts only for incidents touching the player's companies, within four weeks.

## Scenario timelines (1.9)

Each scenario has dated events from its own period in `content/scenarios/<id>/timeline.json`.
They are not cards: nothing to answer. They are announced, then they happen, are recorded as an
incident (kind `timeline`) and a news line, and change the game only through mechanics that
already exist. A scenario plays 8 to 12 events. An event has an id, a date (calendar year and
week of that year, 1 to 52, within 15 years of the scenario's first year, never in week 1), a
title of at most 60 characters, a one or two sentence summary of at most 260 characters in
fictional framing with no real company or person names, `foreshadow_weeks`, and one or more
effects. An effect that would need a new mechanic is left out of the file.

| Effect | Fields | What it does | Cap |
|---|---|---|---|
| `price_shock` | good, percent, weeks | The good's world price family index moves by `percent` at once and the move fades evenly to nothing over `weeks`. It is a layer over the path, never written into it, and feeds the business cycle and the cycle's tilt like the path does. Stacked shocks on one family are limited to 60 percent. | 40 percent, 52 weeks |
| `demand_regime` | sector or `all`, percent, weeks | A regional demand modifier on every region (the same kind recessions and booms use) for the goods the sector makes, or all goods. It composes with other causes under the existing demand clamp. | 10 percent, 52 weeks |
| `central_bank_stance` | shift | Moves the chair's stance (dovish -100 to hawkish +100) once. A new chair at the end of a term replaces it. | 40 points |
| `credit_tightening` | spread_bps, weeks | Adds to the lending premium every lender charges for `weeks`. | 300 bps, 78 weeks |
| `law_change` | law, value | Sets one of the existing laws: `corporate_tax_bps`, `wage_floor_cents`, `merger_review_bps`, `unions` (0 weak, 1 standard, 2 strong), `common_carrier`, `vertical_review_bps`, `reserve`, `privatise` (0 or 1), `quota_bps`. The next election can change it back like any law. | The law's own range |
| `systemic_event` | event | Strikes one of `spill`, `port_strike`, `refinery_fire`, `lane_outage`, `harvest_failure` at the first place it can (lowest lane, port, plant or region key) with the middle of its usual duration, recorded as that kind's own incident as well. Nothing happens where nothing can be struck. | One event |
| `import_ceiling` | good, percent | Scales the capacity of every gateway for the good, for good. | 50 percent |

The loader rejects an unknown good, law, sector or kind, a good with no world price in the
scenario, a date outside the span, and magnitudes above the caps, which follow the recession and
boom sizes in CONTENT-PACING.

**Notice.** `foreshadow_weeks` before the date (at least week 1, none when zero) a news line
"Coming in about N weeks: title" goes out and the event joins the upcoming list. An event with
no notice is never listed before it happens. The upcoming list the interface gets shows only
announced events, never unannounced ones; the past list shows what happened and when.

**Modes.** Historical plays the dates as authored. Off plays nothing. Shuffled moves each date by
a draw of up to 26 weeks either way from the game seed and the event's id (so adding an event to
the file moves no other), never earlier than week 2. A career copies its events, with the dates it
will play them on, into the save when it starts, so editing the file never changes a career in
progress. New games default to Historical; a save from before schema 3 plays none. Everything is
decided when the game starts or read from the saved cursor, so the timeline draws no random
numbers during play and a saved and reloaded career plays what an unbroken one does.

**Order.** Each week the timeline runs just before the world prices and the business cycle, so a
shock, squeeze or demand regime fired this week is in force for them this week.

## The weekly paper

The paper (`cw_session::paper::weekly_paper`) is a presentation of facts the world already
records. It adds no saved state, uses no random draws and invents nothing: a back issue is
recomputed from the histories the world keeps, so it reads the same every time. An issue reads
the facts of its own week and the three before it.

- Sources: market raids, rival wind-downs and ventures, deals (dated by their start, or by their
  offer deadline once finished), strikes, activist campaigns and other news items, central bank
  decisions with the bank's stated reason, elections and new laws, systemic incidents, timeline
  events that fired, timeline foreshadow notices (written as a hint attributed to the paper's
  analysts desk), companies missing a payment, and the player's quarterly results on a quarter
  close.
- Ranking: stories the player is party to first, then the size of the kind of fact (a won raid, a
  finished takeover and the player's results rank highest), then freshness, then the money
  involved. The top story leads and the next three are briefs. A week with no facts prints no
  stories, only the markets column and the classifieds.
- Markets column: the index move (market cap weighted over listed shares), the three biggest
  share moves and the two biggest good moves on the week. A good's reason is the strongest
  driver the market screen shows; only today's issue carries it.
- Classifieds: open bids, and companies restructuring or being wound up (today's issue only).
- Mastheads by scenario: The Corridor Times (Great Lakes 2000), The Gulf Ledger (Texas 1981), The
  Valley Wire (Silicon Valley 1995), Rhein-Ruhr Kurier (Ruhr and Rhine 1991). A custom scenario
  uses its base scenario's.
- Back issues cover the last 52 weeks, the span of weekly prices the world keeps.
