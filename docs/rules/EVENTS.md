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
