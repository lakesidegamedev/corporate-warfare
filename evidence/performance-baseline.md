# Engine compute baseline (Linux relative)

Measured 2026-10-25 with `cargo build --release -p cw-cli` and `cw-cli bench`. These are Linux
host numbers for the engine tick only. They are a relative regression signal, not an iPhone
timing certificate: AI proposals, FFI transfer, view publication, save I/O, cold start and thermal
behaviour are excluded or measured elsewhere, and no A15-class device has run this build.

| Envelope | Firms | Facilities | Contracts | Weeks | p50 ms | p99 ms | max ms |
|---|---:|---:|---:|---:|---:|---:|---:|
| representative | 31 | 290 | 30 | 3 | 0.55 | 0.70 | 0.70 |
| grown | 217 | 2,030 | 210 | 3 | 6.46 | 6.51 | 6.51 |
| stress | 992 | 9,280 | 960 | 2 | 82.98 | 82.98 | 82.98 |

Commands:

```
cargo build --release -p cw-cli
./target/release/cw-cli bench --envelope representative --weeks 3
./target/release/cw-cli bench --envelope grown --weeks 3
./target/release/cw-cli bench --envelope stress --weeks 2
```

Fixture construction (`cw-sim/src/fixtures.rs`) clones the pinned corridor into the declared firm
and facility envelopes, keeps every ledger balanced, rebuilds supply contracts inside each copy
and gives the investor control of the clones so group and valuation traversal are exercised.
Fixtures are non-release constructors and cannot be reached from a session.

## Limits and next steps

- Contract counts are 960 at stress, not the declared 50,000. The fixture scales the seed network
  rather than synthesising the full contract envelope.
- AI is not run in this benchmark; the scheduler measures engine compute separately by design.
- The representative p50 is far inside the provisional 8 ms corridor target on this host, but the
  target is for A15-class hardware. Device profiling (P34/R37) still requires a signed build.
- p99 at three weeks equals the maximum by construction; longer runs are needed for a stable tail.
