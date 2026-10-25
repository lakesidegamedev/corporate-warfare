# Implementation authority

Updated 2026-10-25. Owner interview rounds 1 and 2 incorporated.

Read in this order:

1. [Product and decisions](PRODUCT.md): what the owner asked to receive.
2. [Audit](AUDIT.md) and [primary-source research](../RESEARCH.md): why the original
   document needs correction.
3. [Economy contract](ECONOMY.md): accounting, markets, labour, control and failure.
4. [Architecture](ARCHITECTURE.md): boundaries, state, commands, saves and FFI.
5. [Data and content](DATA-CONTENT.md): geography, circa-2000 inputs and industries.
6. [Player experience](EXPERIENCE.md): decisions, screens, explanations and polish.
7. [Delivery work packets](DELIVERY.md): dependency order and exact acceptance work.
8. [Testing](TESTING.md) and [Apple delivery](APPLE.md): evidence required for release.
9. [Requirements ledger](requirements.json): machine-readable coverage and dependencies.
10. [DeepSeek handoff](DEEPSEEK.md): execution loop and completion rules.
11. [Current status](STATUS.md): exact next move and outstanding dependencies.

Precedence: owner instructions, PRODUCT owner decisions, approved changes to contracts,
then the requirement ledger and work packets. Proposals below are implementation defaults,
not claims that the owner specified each formula. Record substantive changes and reasons
in DECISIONS.md. `docs/PLAN.md` is preserved historical input and is superseded by this set.

Preparation completion is different from game completion. The ledger starts pending.
Passing its structural validator does not prove a single gameplay requirement.
