<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 011 — Writer-health signal and kill-switch

- Slug: `writer-health-kill-switch`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Risks and Rollback#0`
- Depends on: operation-journal-writer

## Goal

Surface degraded-writer warnings through a channel outside the failing writer itself, and implement an explicit environment-variable/config switch that disables durable-archive writing entirely (keeping only the existing bounded live buffer), as the Risk R3 last-resort mitigation.

## Why this exists

This is both Phase 2's explicit scope item (writer-health signal) and the concrete implementation of Risk R3's mitigation and Task 5.5's dependency (log-health warnings need a real signal to surface).

## Settled investigation

- src/common/globals.rs already has the precedent pattern this task should follow: pub static DEBUG: OnceLock<bool> and pub static VERBOSE: OnceLock<bool>, both set once from Cli fields in src/application.rs::set_global_vars (line 76 for DEBUG) — a new kill-switch global should follow this same OnceLock<bool> shape.
- The plan's Confirmed Product Decisions establishes that diagnostic failure is warning-only and must never flip a passed capture to failed or change CLI exit status — this task's warning-delivery channel must route around (not through) progress::emit's existing Event::Captured{error}/Event::Finished{error} fields.
- Istari's reviewed LoggingState::emit 'ignores write results' and LoggingGuard::shutdown 'returns no writer-health result' — this is the exact gap this task closes.

## Inputs

- Task 2.1's journal writer
- src/common/globals.rs's existing OnceLock<bool> pattern as the precedent to follow

## Outputs

- A writer-health signal type/channel independent of the writer's own (possibly-failed) output path
- An environment-variable/config switch that disables durable-archive writing while preserving the existing bounded live buffer

## Implementation steps

- Add a writer-health state (an AtomicBool/OnceLock-backed flag or small channel) that the journal writer updates on each write failure, independent of its own output stream.
- Define a new environment-variable/config switch (e.g. REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE), following the existing DEBUG/VERBOSE OnceLock<bool> pattern in src/common/globals.rs, that makes the journal writer a no-op while LiveProgress continues unchanged.
- Wire this health signal to a warning delivered through a surviving channel (not the failing writer itself) so the CLI/desktop can display it without depending on the archive write having succeeded.
- Write forced_write_failure_surfaces_warning_and_preserves_exit_status: point the journal at a read-only/nonexistent directory, run a synthetic capture, and assert a warning surfaces, capture results/exit status are preserved.
- Write kill_switch_disables_archive_writing_without_affecting_live_buffer: set the kill-switch env var/config, run a synthetic capture, and assert zero archive-write attempts while LiveProgress still records events.

## Done when

- Test forced_write_failure_surfaces_warning_and_preserves_exit_status points the journal writer at a nonexistent/read-only directory, runs a synthetic capture, and asserts a warning is surfaced, the capture's result/exit status is unchanged, and LiveProgress still contains the run's events.
- Test kill_switch_disables_archive_writing_without_affecting_live_buffer sets the documented kill-switch env var/config before a synthetic run and asserts zero journal-write attempts occur while LiveProgress's bounded history still records the run's events.

## Evidence to capture

- The exact injected failure ('journal root directory is read-only') and the resulting warning payload from forced_write_failure_surfaces_warning_and_preserves_exit_status
- The exact kill-switch variable name (e.g. REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE) and its documented on/off behavior, for Task 7.3 to document

## Do not change

- Do not route writer-health warnings through progress::Event::Captured{error}/Event::Finished{error} — those fields already mean 'the operation itself failed' and conflating them would violate the warning-only decision.
- Do not remove or alter the existing bounded LiveProgress live buffer (src/progress.rs) when the kill-switch is active; it must keep working exactly as today.

## Reopen triggers

- Task 5.5's UI wiring finds the health signal's granularity insufficient (e.g. it needs a missing-byte-range count, not just a boolean) to render the required log-health warning.

## Context

Risk R3 in Risks and Rollback explicitly requires this task as the last-resort fallback if the archive pipeline misbehaves in production; without it, the only existing mitigation would be the generic warning-only policy, which Risk R3 itself says is 'a partial mitigation by design,' not a complete one.

## Options & tradeoffs

Reporting writer failures only via a return value on the next write call would miss failures between writes; a dedicated independent health-signal channel (chosen) adds a small amount of synchronization but can report a failure even when no further writes occur.

## Decision

Implement an independent writer-health signal (not reusing the lifecycle Event enum) plus a OnceLock<bool>-style kill-switch following the existing DEBUG/VERBOSE global pattern.

## Acceptance criteria

- Test forced_write_failure_surfaces_warning_and_preserves_exit_status shows a surfaced warning, preserved capture results, and unchanged exit status in the same test run.
- Test kill_switch_disables_archive_writing_without_affecting_live_buffer shows zero archive-write attempts while LiveProgress's existing bounded history still records events, when the documented switch is set.

## File touchpoints

- src/diagnostics/journal.rs (proposed)
- src/common/globals.rs
- src/application.rs (reference)

## Tests

- cargo test --lib diagnostics::journal::tests -- forced_write_failure_surfaces_warning_and_preserves_exit_status
- cargo test --lib diagnostics::journal::tests -- kill_switch_disables_archive_writing_without_affecting_live_buffer

## Dependencies

- operation-journal-writer

## Risks

- A kill-switch read via OnceLock set only once at process start (mirroring DEBUG/VERBOSE) cannot be toggled mid-run; this matches the existing codebase pattern but means a user must restart to flip it — acceptable per the plan's own framing of this as a last-resort, not a live control, mitigation.
