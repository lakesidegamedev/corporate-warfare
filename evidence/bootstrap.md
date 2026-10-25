# Bootstrap verification

Verified 2026-10-25 against commit `4742c1d0a74721296ea30be85ad01dba337343b3`.
This records bootstrap evidence only. No gameplay acceptance requirement is marked done.

- [Hosted Rust/clippy/format and planning checks](https://github.com/lakesidegamedev/corporate-warfare/actions/runs/40158080889): passed.
- [Hosted Apple smoke](https://github.com/lakesidegamedev/corporate-warfare/actions/runs/40158080902): passed, including generated Swift compilation, EngineBridgeTests execution in an iPhone simulator, and unsigned device archive.
- Apple artifact `unsigned-apple-smoke` includes archive and native test result bundle.
- Host scheduler job `20261025T032128Z-0427b193`: passed at bootstrap Rust source; queued `cargo test --locked --workspace` under registered task worktree. This executes bootstrap crate tests only, not the unimplemented economy.
- Python planning tests, plan graph validation and authored calibration arithmetic were run locally after the later design additions. Final hosted checks validate their committed form.

Limitations: no gameplay session, no real-device timing, no signing/upload, no TestFlight
installation, no real-world geography dataset, no human playtest. R31 remains pending
until real session/command/save/error cross-platform tests exist.

GitHub rejected private-repository branch protection with HTTP403 requiring a paid plan
or public repository. Repository remains private; workflows run, branch protection is
not enforced. Agents must use reviewed PRs and green checks by convention.
