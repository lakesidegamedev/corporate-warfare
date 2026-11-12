# Corporate Warfare

An offline economic strategy game. You run a holding company in a living economy: build and
run plants, set prices, hire, borrow, issue shares, buy and sell companies, and out-compete rival
firms that follow the same rules you do. There is no server and no account. The game runs on
iPhone, iPad, Mac, Android, Windows and Linux.

Four scenarios: the Great Lakes in 2000, Texas in 1981, Silicon Valley in 1995 and the Ruhr and
Rhine in 1991. Each has its own geography, firms, prices and dated history. References:
Capitalism Lab and A House Divided.

## What 2.0 plays like

- **Careers.** Found a company, or take the helm of an established one from the middle of the
  league. Pick an ambition (Richest, Biggest, Dominant or Survivor) and a length of 5, 10 or 20
  years, and play for a bronze, silver or gold medal. A career can be lost: no company left, a
  rival taking control, or a board that removes you after long losses. Start without one for a
  sandbox. See [careers](docs/rules/CORPORATE.md).
- **Capacity is the contest.** A new plant costs what building costs today. The Openings board
  shows where a plant would pay; rivals read the same signal, build into gaps, close losing
  plants and pay out idle cash. See [capacity](docs/rules/CAPACITY.md).
- **Raids are campaigns.** A rival that wants part of your business must announce it four weeks
  ahead, so you can raise your marketing and cut its chance. See [raids](docs/rules/RAIDS.md).
- **Plants change hands.** A failed company's plants go to a four week break-up auction, you can
  list your own, and a sale always needs a buyer. Companies can fold in a firm they own nine
  tenths of. See [auctions and mergers](docs/rules/AUCTIONS.md).
- **Markets and world prices.** Prices follow the outside world, with weather around the
  historical path, ports with finite room, and demand that moves with the business cycle. See
  [markets](docs/rules/MARKETS.md) and [world prices](docs/rules/WORLD-PRICES.md).
- **Power on a grid.** Electricity is pooled over a currency area and shared fairly when short.
  See [power](docs/rules/POWER.md).
- **The 2.0 layout.** Tables you can shape and keep, one timeframe, a rail of things that need
  you, and a map that docks beside any page. See [the layout](docs/rules/LAYOUT.md).

## Build and run

Rust is pinned in `rust-toolchain.toml`. No database, server or network service is needed.

```
cargo run -p cw-cli -- doctor        # engine, rules and content versions
cargo test --locked --workspace      # engine and session tests
python3 tools/plan_check.py          # execution and acceptance ledgers
python3 tools/design_check.py        # authored parameters and worked arithmetic
python3 tools/changelog_check.py     # changelog format and version agreement
```

`cw-cli` also plays headless games for balance work (`run`, `sweep`, `arc`, `replay`). The
desktop and Android apps are Kotlin Multiplatform under `compose/` ([README](compose/README.md));
the Apple apps are under `apple/` ([release notes](apple/RELEASE.md)). Apple builds need macOS.
Contributor conventions are in [AGENTS.md](AGENTS.md).

## Where the rules are

[docs/rules/README.md](docs/rules/README.md) indexes the exact rules, each with the content file
its numbers live in. The planning set starts at [docs/planning/README.md](docs/planning/README.md).
The original [DeepSeek plan](docs/PLAN.md) is historical and is not the implementation authority.

## Releases

Changes by version: [CHANGELOG.md](CHANGELOG.md). Versioning and the release checklist:
[docs/RELEASING.md](docs/RELEASING.md).
