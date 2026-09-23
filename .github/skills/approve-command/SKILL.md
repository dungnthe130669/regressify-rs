---
name: approve-command
description: 'Use when changing regressify approve, scenario-scoped baseline promotion, explicit run selection, error marker gating, or preservation of unselected references.'
---

# Approve References

## Start Here

[Approve routing](../../../src/compare/actions.rs),
[approve_screenshots_for_scenarios](../../../src/compare/report_gen.rs), and
[atomic copying](../../../src/helpers/io.rs).

## Procedure and Invariants

1. Resolve the requested suite/run and validate capture inventory before mutation.
   Explicit run selection must not silently fall back to a different capture run.
1. Scenario-scoped approval needs trustworthy scenario capture metadata; refuse
   legacy/missing metadata rather than approving more than the user selected.
1. Skip failed capture placeholders, including malformed error markers. Promote
   complete files atomically; preserve good and unselected references.
1. Clear stale reference error markers only after a successful replacement.
   Report failed I/O as a process error, not an approved visual result.

## Checks

```sh
cargo test --lib compare::report_gen::tests
cargo test --lib compare::actions::tests
```

Use temporary output trees. Cover partial selection, failed/malformed markers,
missing inventory, explicit run IDs, and copy failures without touching real baselines.

## Maintain

Update [command docs](../../../docs/commands.md), relevant desktop approval docs,
and this skill. Keep comments and tests explaining why promotion is gated.
