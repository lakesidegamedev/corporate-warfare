# Corporate Warfare

An offline corporate strategy game for iPhone. Build a holding company in the
Great Lakes economy, compete with rival firms, and grow through subsidiaries,
production, finance and acquisitions. References: Capitalism Lab and A House Divided.

This repository is in planning and bootstrap. It is not a playable game.

Start with [the planning index](docs/planning/README.md), then the
[audit](docs/planning/AUDIT.md). The original [DeepSeek plan](docs/PLAN.md) is
preserved as historical input and is not the implementation authority.

## Bootstrap

Rust is pinned in `rust-toolchain.toml`. No database, server or production service
is required. `cargo run -p cw-cli -- doctor` reports the bootstrap versions.
`python3 tools/plan_check.py` validates requirement dependencies and evidence records.

On this shared VPS, full builds/tests go through the host check scheduler. See
[AGENTS.md](AGENTS.md). macOS is required for the Apple smoke build; see
[Apple delivery](docs/planning/APPLE.md).

## Release target

A polished Great Lakes game installed through TestFlight. National expansion and
desktop ports follow separately. Apple console setup can be a named owner dependency;
it cannot be reported as a successful TestFlight install.
