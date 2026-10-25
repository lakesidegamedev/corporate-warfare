# Prescriptive design baseline

These rules are the architect's selected v0.1 implementation design, based on the owner's
product decisions. They intentionally settle questions the implementation agent should
not have to invent. They are not claims that the owner individually approved every
coefficient, or that calibrated prices are real measurements.

Order: [markets and weekly settlement](MARKETS.md), [corporate state machines](CORPORATE.md),
[finance, labour, policy and AI](FINANCE-LABOUR-AI.md), [content and pacing](CONTENT-PACING.md),
[session/commands/phone flows](INTERFACE.md), and [exact event deck](EVENTS.md), and [phone visual specification](VISUAL.md). Machine-readable initial parameters:
[initial-balance.json](../../content/initial-balance.json). Execution checklist:
[execution.json](../planning/execution.json).

These exact rules override broad alternatives in the earlier planning contracts.
Preserve confirmed owner scope. Implement without changing the design to whatever is
easiest. Refactoring that preserves observable results is routine. A contradictory
worked example, exploit or balance failure is evidence to raise a design correction,
not permission to silently change formulas. Record approved revisions in DECISIONS and
update related examples, parameters and tests together.

Research gaps remain explicitly named: actual county/EA membership, historical source
coverage, project-specific Apple identity/secrets, device evidence and human playtests.
Use labelled engineering fixtures while independent implementation proceeds. Do not
invent historical observations or TestFlight evidence to close those gaps.

Business rules here are deliberately simplified game institutions. They are not a
claim to reproduce every US legal, banking or tax rule. Store-facing copy should say
so plainly without cluttering every gameplay screen with disclaimers.
