<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 030 — Performance/overhead benchmark and budget

- Slug: `performance-overhead-benchmark`
- Implements plan section(s): `Phase 7 — Hardening and Docs#0`, `Risks and Rollback#0`, `Definition of Done#0`
- Depends on: capture-queue-setup-stage, capture-navigation-stage, capture-state-actions-settling-stages, capture-artifacts-finish-stages, writer-health-kill-switch

## Goal

Produce a documented benchmark comparing capture wall-time and disk I/O with instrumentation on vs. off, against the performance budget in Definition of Done item 3, resolving Risk R2.

## Why this exists

Definition of Done item 3 makes this benchmark a hard completion gate; Risk R2 names per-capture instrumentation overhead as unbudgeted and medium-impact.

## Settled investigation

- The plan proposes a default budget, pending confirmation: 'no more than 10% added wall-clock time per capture versus an uninstrumented baseline,' explicitly telling this task to 'Revise this number if Task 7.2's benchmark shows it is impractical.'
- Task 2.1's journal writer already documents a peak-memory bound and a chosen chunk-size constant; this benchmark is the first task to measure those choices' real wall-clock/I/O cost under a representative capture workload, not just memory.
- src/cli/root.rs's existing --async-capture-limit (default 5, line 59)/--async-capture-ramp-ms flags (lines 56-66) already control concurrency for a capture run; this benchmark should vary at least one realistic concurrency level since Risk R2 specifically calls out 'especially at high concurrency.'
- Task 2.6 (writer-health-kill-switch) defines the exact environment-variable/config switch this benchmark's 'instrumentation off' half relies on to disable archive writing; this task consumes that existing switch and does not define a second, separate toggle.

## Inputs

- Tasks 3.1-3.4's full capture instrumentation (this benchmark measures the fully-instrumented path, not a partial one)
- The proposed 10% default budget from Open Questions and Remaining Decisions
- Task 2.6's kill-switch (the exact mechanism toggled to produce the 'instrumentation off' half of this benchmark)

## Outputs

- A documented benchmark report comparing instrumented vs. uninstrumented capture wall-time and disk I/O
- A confirmed or revised numeric budget threshold for Definition of Done item 3

## Implementation steps

- Build a synthetic capture workload of 4 suites x 2 viewports (8 scenario/viewport captures) runnable under the existing --async-capture-limit 5 default, both with Phase 3's instrumentation enabled and with it disabled via Task 2.6's kill-switch.
- Measure wall-clock time and disk I/O (bytes written, write-call count) for both configurations across enough repetitions to be statistically meaningful (e.g. 5 runs each), not a single run.
- Compare the measured overhead percentage against the plan's proposed 10% default budget; if it exceeds that budget, document the actual measured number and either propose field-verbosity/batch-flush reductions or explicitly flag the budget needs revising.
- Write up the benchmark methodology, raw numbers, and conclusion in a tracked document for Definition of Done item 3 and Task 7.3 to reference.

## Done when

- A benchmark run executes a fixture of 4 suites x 2 viewports (8 scenario/viewport captures) under --async-capture-limit 5, once with Task 2.6's kill-switch unset (instrumentation on) and once with it set (instrumentation off), and records wall-clock seconds and total journal bytes written for each.
- The benchmark report states the measured overhead percentage ((instrumented - uninstrumented) / uninstrumented * 100) for both wall-time and disk I/O, and an explicit conclusion against the plan's proposed 10% budget.

## Evidence to capture

- The raw wall-clock seconds and total bytes-written numbers for both the instrumented and uninstrumented runs of the 4-suite x 2-viewport fixture, and the computed overhead percentage for each

## Do not change

- Do not silently accept an over-budget result as passing; Risk R2's mitigation path (reduce field verbosity, batch-flush more aggressively, or gate behind the kill-switch) must be explicitly considered if the measured overhead exceeds the agreed budget.

## Reopen triggers

- A later production report of slow captures correlated with instrumentation (per Risk R1/R2's rollback triggers) contradicts this benchmark's conclusion, warranting a re-measurement.

## Context

This is the only task in the plan whose own numeric target is explicitly still open (Open Questions and Remaining Decisions' one genuinely-open item); it is also a hard Definition of Done gate, meaning Phase 7 cannot be declared complete without this benchmark existing and reaching an explicit conclusion.

## Options & tradeoffs

Benchmarking only a single-scenario, low-concurrency baseline would be easier but would miss Risk R2's specific concern about high concurrency; benchmarking at a realistic multi-suite concurrent workload (chosen) is more setup work but directly tests the risk as stated.

## Decision

Benchmark a realistic concurrent multi-suite workload with instrumentation on vs. off (toggled via the existing Task 2.6 kill-switch), measure wall-time and disk I/O, and explicitly confirm or revise the plan's proposed 10% budget.

## Acceptance criteria

- The benchmark report states a specific measured overhead percentage for both wall-time and disk I/O from a 4-suite x 2-viewport (8-capture) fixture run, not only a qualitative impression.
- The report explicitly concludes whether the measured overhead is within the plan's proposed 10% budget, and if not, names a specific mitigation from Risk R2 or an explicit revised budget number.

## File touchpoints

- docs/plan/log-revamp-benchmark.md (proposed)
- src/diagnostics/journal.rs (reference)
- src/common/globals.rs (reference)

## Tests

- Manual/documented benchmark run: a synthetic 4-suite x 2-viewport capture workload executed with and without the Task 2.6 kill-switch set, under --async-capture-limit 5, comparing wall-clock seconds and total bytes written to the journal across 5 repetitions each.

## Dependencies

- capture-queue-setup-stage
- capture-navigation-stage
- capture-state-actions-settling-stages
- capture-artifacts-finish-stages
- writer-health-kill-switch

## Risks

- Measuring on a single developer machine with variable background load could produce noisy, non-reproducible numbers; mitigate by running enough repetitions (e.g. 5) to compute a stable average/variance and documenting the measurement environment alongside the numbers.
- This benchmark's 'instrumentation off' configuration now has a hard dependency on Task 2.6 shipping first; if Task 2.6's kill-switch mechanism changes shape after this task is written, this benchmark's harness must be updated to match rather than assuming the original env-var/config shape.
