# Product brief and scope

## Confirmed by owner, 2026-10-25

- First delivery: polished Great Lakes game through iOS TestFlight. National map and
  desktop releases are later work, not prerequisites for the first release.
- References: Capitalism Lab and A House Divided. Deep corporate mechanics matter.
- Starting era: 2000, with flexibility if source quality materially improves nearby.
- Rust + SwiftUI, offline single player, weekly turns, regions and fifteen industries
  remain the starting direction. The owner explicitly welcomes challenges.
- Player can build a holding company with effectively unlimited subsidiaries.
- Both open-ended sandbox and a five-year scored scenario are required.
- Losing a holding company or its solvency does not end the investor's career.
- Most if not all of subsidiaries, intercompany trade, public/private ownership,
  trading, dividends, buybacks, issuance, loans, boards, hostile takeovers, mergers,
  bankruptcy, bonds, playable banking/insurance, unions, lobbying and antitrust
  should be present. This plan includes all those named systems in the release.
- Premium offline. No ads, consumables, login or mandatory network dependencies.
- New private GitHub repo and CI are authorized. Apple developer/access/build/device
  resources exist; project-specific wiring still requires verification.

## Delivery interpretation

The player is an investor, distinct from every legal firm. Ownership, voting control,
management authority and operating assets are distinct. The investor can own stakes in
several groups and start again after a loss. The UI defaults to the current primary
holding company, with a portfolio view above it.

There is no small designer-imposed subsidiary count. Initial performance fixtures target
100 controlled firms, 1,000 active firms total, 10,000 facilities and 50,000 contracts in
the corridor stress world. These are proposed validation envelopes, not population
claims or gameplay caps. Measure and revise openly. Full national expansion is separate.

Start in January 2000. Treat this as an alternate-history economy, seeded by real
baseline data. Do not silently use modern wages, electric prices or population as
historical facts. If data is unavailable, retain the era and mark calibrated estimates;
changing the era requires an explicit recorded decision. The 2004 BEA definition uses
Census 2000 population and is a candidate boundary vintage, not a claim that it existed
in January 2000.

Open-ended does not mean historically scripted through the present. Later decades use
bounded generated macro conditions, technology opportunities and events. UI explains
that distinction. Test 30-year careers and recovery after company loss.

## Depth boundaries for the first release

All fifteen industries exist as distinct choices. Begin implementation with a coherent
small chain, then expand; a common recipe base is allowed, cosmetic reskins are not.
Corporate finance includes a simplified exchange, bank balance sheets, underwriting
and insurance claims. It excludes real payment processing, real securities and any
connection to live financial markets.

Unions include organisation, wage bargaining, strikes and settlements. Lobbying includes
budgeted influence, disclosed requests and uncertain policy responses. Antitrust includes
concentration screening, review delays, remedies and rejection. These systems must affect
firms and receive consequences from them. AHD-style elections, parties and legislative
simulation are outside scope. Record this bounded interpretation as proposed until the
owner changes it.

Proposed audience: enthusiasts of deep business simulations, with an accessible first
hour. Final price, App Store markets, legal rating answers and final brand assets remain
owner decisions; they do not block engine work. iPhone first, iPad adaptive layouts;
iOS 17 deployment floor provisionally retained. No iCloud sync in first release: manual
save export/import provides portable backup. Adding cloud sync is a separate decision.

## Playable promise

Within ten minutes, a new player can operate a firm, read why cash differs from profit,
find a regional opportunity and see a rival respond. Within one session they can
integrate a supplier, finance expansion, negotiate with workers, and encounter a control
or credit decision. A patient player can build a multilevel group, change management,
raise public capital, contest an acquisition, or recover from insolvency.

The five-year score uses investor total return, control-adjusted group value, financial
resilience and difficulty. Never score intra-group sales twice. Sandbox continues after
the score. Exact weights are balance parameters and must be disclosed to the player.

## Release outcomes

PREPARED: repository, planning, bootstrap checks and handoff are usable. This session's
objective is preparation; it does not assert delivery of the game.

SOFTWARE_READY: all software/content/automated gates and reproducible archive passed,
with named Apple-only owner dependencies if any. No placeholders in release flows.

TESTFLIGHT_AVAILABLE: signed upload processed and visible to the intended internal group.

RELEASE_ACCEPTED: a real tester installed via TestFlight, completed the acceptance route,
and device evidence supports performance, persistence and usability. Human playtest
judgment and device measurements cannot be replaced with fabricated reports.
