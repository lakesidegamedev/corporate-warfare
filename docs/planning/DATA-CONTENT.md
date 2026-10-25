# Data, geography and content specification

## Geography gate

Use verified BEA economic areas, not a list of cities relabelled as EAs. Start from
the candidate roster in `../RESEARCH.md`; recover a provenance-traceable official
county-to-area mapping before accepting final corridor IDs. Cleveland/Akron is one area.
Do not assert Canton, Youngstown, Lima/Findlay or Wheeling as independent EAs without
source evidence. An 11-area or differently sized verified corridor is preferable to
inventing a 13-area map. Publish the actual names and membership for owner review.

If the official crosswalk is inaccessible, record attempted official sources and use
an explicitly labelled temporary fixture for engineering. A fixture does not satisfy
real-world-data acceptance. A custom region scheme is a product decision, not a silent
fallback. County-level source processing never adds county gameplay entities.

Pin region IDs/vintage independently of display labels. Cross-state regions retain
jurisdiction-weighted tax/policy inputs; one universal state tax cannot be inferred from
the largest city. Keep urban/rural shares and data quality flags. Map geometry and labels
must match IDs and remain readable on the smallest supported phone.

## Source pipeline

Each source manifest records source URL, publisher, download date, data/reference year,
license/terms URL, checksum, units, classification vintage, transformations, suppression
handling and output use. Raw downloads live outside Git; small legally redistributable
fixtures and canonical transformed outputs can be committed with provenance.

Preferred baseline: Census 2000 population/housing/income, historical BLS QCEW employment
and wages, BEA historical industry output, CBP establishments, EIA historical energy,
BTS/FHWA freight proxies. This is a research queue, not a claim that every needed 2000
cell exists. Distinguish published observations from calibrated gameplay estimates.

Reconcile county FIPS changes and NAICS/SIC revisions explicitly. Do not combine 2000
employment denominators with 2025 wages without deflation and visible documentation.
Nominal dollars and price base are declared. National proxies carry uncertainty.
Suppressed cells are not zero. Imputation uses documented regional/industry hierarchies
and flags, with sensitivity checks. Do not fabricate skill tiers, union density, land
or permitting observations that sources do not provide.

Output includes dataset schema, vintage map, source hashes, recipes, region rows,
industry mappings, gateway configuration and a reproducibility report. Validators check
uniqueness, coverage, positive units, source totals, plausible bounds and corridor totals.
Builds consume checked-in validated output without network access; regeneration is an
explicit tooling command. Deterministic data artifacts must rebuild byte-for-byte from
pinned inputs. Data updates are reviewed changes, never runtime downloads.

## Industry coverage contract

All rows require costs, labour mix, capital/lead time, demand counterparties, failure
conditions, differentiated player decisions and at least one linked rival/event.
Products below are proposed exemplars, not final recipes or verified regional output.

| Industry | Minimum distinctive release model | Dependency / test focus |
|---|---|---|
| Agriculture | Seasonal crop capacity, yield/weather and perishable stock | Land/capex, food demand, spoilage |
| Extraction | Deposit quality, depletion, commodity output | Finite resources, energy, reclamation cost |
| Energy | Capacity, fuel use and dispatch/service supply | Fuel scarcity, finite demand, outages |
| Chemicals | Multi-input conversion and industrial quality | Stoichiometric quantity units, input bottlenecks |
| Manufacturing | Components to finished goods, quality and retooling | Vertical integration, inventories, demand elasticity |
| Construction | Project/service capacity and phased delivery | Backlog, deposits, cancellation, CIP |
| Logistics | Finite freight capacity, lane cost and lead time | Shipment conservation, failed delivery, utilisation |
| Retail | Regional distribution, assortment, markup and stock | Final demand, delivered margin, stockouts |
| Healthcare | Skill-constrained care/service capacity | Household/public payer budgets, no stored appointments |
| Financial | Banking plus insurance business lines | Balance sheets, reserves, claims/defaults, contagion |
| Technology | R&D, product/knowledge progress and obsolescence | Bounded compounding, demand and royalty accounting |
| Media | Audience, content investment and advertising service | Advertiser budgets, reputation and audience competition |
| Telecommunications | Installed infrastructure and subscription capacity | Coverage/investment, churn, service reliability |
| Real estate | Development, leases, occupancy and maintenance | Rental cash flow, vacancy and asset valuation |
| Defense | Procurement contracts, qualification and delivery | Government budgets, milestone payments, penalties |

A shared goods/service foundation should reduce code duplication. It must still support
services, durable assets, subscriptions, resource stocks and financial balance sheets.
Industry rows cannot be marked done because their names appear in a picker.

## Content acceptance

Initial authored content targets: at least 30 named rival firms across all industries,
five distinct policy archetypes, a guided struggling-company start and a blank founder
start, at least 24 tested event definitions spanning labour, credit, supply, policy,
technology, control and macro conditions. These are proposed minimums; quality includes
clear choices, economic consequences, cooldowns and text, not merely record counts.

Each event declares prerequisites, possible recipients, severity, choices, deterministic
RNG stream, financial/resource postings, follow-up, localisation key and test fixture.
Events cannot ignore the same solvency/control restrictions applied to commands.
No event fires into a dead firm or removes the only recoverable save.

A circa-2000 baseline uses fictional firm names unless naming rights are established.
Do not reproduce logos, brands, text or code from reference games. Study AHD as design
input, record source lessons and re-derive rules. Do not blindly copy its licensed code
or carry over its database assumptions.

Asset manifest covers icons, maps, fonts, illustrations, sounds and notices. Release
requires a complete app icon, coherent visual system, readable charts, empty/error
states, tutorial copy and About/data attribution. Native/system assets are acceptable
where coherent; temporary debug art is not release evidence.
