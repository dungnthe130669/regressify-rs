<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 006 — Operation-owned incremental journal writer

- Slug: `operation-journal-writer`
- Implements plan section(s): `Phase 2 — Durable Operation History (Captures and Deployments)#0`, `Full-Log Persistence#0`, `Configuration-cascade integration (proposed)#0`
- Depends on: define-diagnostic-record-v1

## Goal

Implement a streaming/segmented, bounded-memory journal writer that persists DiagnosticRecordV1s incrementally under the validated active output root, with a paired reader that reconstructs the full ordered sequence, and with its chunk-size and disk-budget parameters exposed as an operator-configurable CLI flag/environment variable (not only a hardcoded constant), read once at process start.

## Why this exists

This is the storage engine every later phase builds on; Phase 2's exit criterion requires passed/failed concurrent captures to reload complete logs after live-buffer eviction. The plan's Configuration-cascade integration section also explicitly assigns this task (Task 2.1) and Task 5.1 to deliver/confirm archive-writer behavior (chunk size, disk budget, retention) as an operator-configurable CLI flag or environment variable, not a hardcoded constant.

## Settled investigation

- progress.rs's own EventWriter/EventReader (src/progress.rs lines 131-262) already implements a comparable length-prefixed-by-newline JSON-lines pattern with a 1 MiB per-record cap (MAX_EVENT_BYTES) and batched draining (lines 16-18) — a useful framing precedent, but it is the live progress-transport format (schema VERSION=1) and must not be reused or extended as the durable archive itself.
- The release profile sets panic="abort" (Cargo.toml:98), so the writer cannot rely on Drop running during a panic; flush must happen at well-defined checkpoints, not only via a drop guard.
- tracing-appender's NonBlockingBuilder::lossy(false) is the Istari-reviewed starting point for non-lossy queuing, but is 'a starting point, not proof of durable writes or safe UI-thread backpressure' — this task must still prove bounded memory under backpressure itself.
- The plan's 'Configuration-cascade integration (proposed)' section states archive writer behavior (journal chunk size, disk budget, retention) 'is proposed to be operation-level/global only — a CLI flag or environment variable read at process start — not scenario-cascaded... Task 2.1 and Task 5.1 confirm it against the real configuration loader in `src/config`' — this task owns the actual flag/env var implementation since it owns the journal; Task 5.1 (cli-verbosity-flags-precedence) separately confirms the two settings families stay split.
- src/common/globals.rs already has the precedent pattern for a process-start-only, non-cascaded setting: pub static DEBUG: OnceLock<bool> and pub static VERBOSE: OnceLock<bool>, both set once from Cli fields in src/application.rs::set_global_vars (line 76) — this task's chunk-size/disk-budget flags should follow this same OnceLock-set-once-at-startup shape, not the scenario/suite/workspace cascade Task 5.1 separately owns for display verbosity.
- Non-Goals explicitly excludes 'Automatic age-based pruning of capture logs,' and Confirmed Product Decisions states capture logs 'remain tied to their captures/reports' with no automatic pruning — so the 'retention' named in Configuration-cascade integration's parenthetical is addressed by this task's disk-budget cap (bounding how much is retained before Task 2.6's writer-health kill-switch/backpressure engages), not a separate time-based deletion schedule, which stays out of scope per Non-Goals.

## Inputs

- Task 1.3's DiagnosticRecordV1
- The validated active output root path (existing common::REGRESSIFY_DIR/output-directory resolution used elsewhere, e.g. src/test/test_suites.rs::suite_output_dir)
- src/common/globals.rs's existing OnceLock<bool> pattern and src/application.rs::set_global_vars as the precedent for a process-start-only, non-cascaded flag/env var (reference)

## Outputs

- A journal writer type that appends DiagnosticRecordV1s to disk incrementally under bounded memory
- A paired reader that reconstructs the full ordered record sequence from one or more segment files
- A documented peak-memory bound
- A CLI flag and environment variable pair for both chunk size and disk budget, resolved once at process start independent of the scenario/suite/workspace/defaults cascade, with documented default constants used when neither is set

## Implementation steps

- Choose a concrete on-disk layout (newline-delimited JSON segments capped at a configurable byte size, rolling to a new segment once exceeded).
- Add a CLI flag (e.g. --diagnostic-chunk-size-bytes <N>) and a matching environment variable (e.g. REGRESSIFY_DIAGNOSTIC_CHUNK_SIZE_BYTES) to src/cli/root.rs's Cli struct and thread it through src/application.rs::set_global_vars into a new OnceLock<usize>-style global in src/common/globals.rs, following the existing DEBUG/VERBOSE pattern, with a documented default constant used when neither is set.
- Add a second CLI flag/environment variable pair for the disk budget (e.g. --diagnostic-disk-budget-bytes / REGRESSIFY_DIAGNOSTIC_DISK_BUDGET_BYTES) bounding total on-disk journal size across segments for one operation, following the same pattern, with its own documented default.
- Implement the writer so each write() call serializes one record, enforces the resolved (flag/env/default) per-segment chunk-size cap, and flushes at a bounded cadence rather than per-record.
- Implement the reader to iterate segments in order and yield records via sequence number, not wall-clock or file-mtime sort.
- Write a synthetic test that writes over 10,000 records across multiple segments, measures peak memory, and asserts the reader reconstructs the exact ordered sequence using the default chunk size.
- Write chunk_size_flag_and_env_var_override_default: run the same 10,000-record write twice, once with only the documented default in effect and once with the CLI flag (or, absent a flag, the environment variable) set to a materially smaller chunk size, and assert the overridden run produces more segment files than the default run for the identical record count.
- Document the chosen default chunk-size/disk-budget constants and the exact flag/env var names inline so Task 2.6's kill-switch, Task 5.1's split-confirmation test, and Task 7.2's benchmark can reference them.

## Done when

- A synthetic test writes over 10,000 records across multiple segments.
- A reader reconstructs the full ordered sequence from those segments.
- Peak memory during the write stays within the documented bound asserted by the test.
- Test chunk_size_flag_and_env_var_override_default shows a run with the CLI flag (or environment variable) set to a smaller-than-default chunk size produces a strictly greater segment-file count than a default-configuration run of the identical 10,000-record set.

## Evidence to capture

- The test's measured peak-memory value (or internal buffer-size proxy) against its documented bound
- The chosen default segment/chunk-size and disk-budget constants and their rationale
- The exact CLI flag names (e.g. --diagnostic-chunk-size-bytes, --diagnostic-disk-budget-bytes) and environment variable names (e.g. REGRESSIFY_DIAGNOSTIC_CHUNK_SIZE_BYTES, REGRESSIFY_DIAGNOSTIC_DISK_BUDGET_BYTES), plus the segment-file-count comparison from chunk_size_flag_and_env_var_override_default proving the override actually changes writer behavior

## Do not change

- Do not reuse or extend progress.rs's EventWriter/EventReader/VERSION for this archive; they remain the live progress-transport implementation.
- Do not implement automatic age-based pruning here — Confirmed Product Decisions explicitly excludes it; this task's disk-budget flag bounds how much is written/retained before Task 2.6's kill-switch engages, it does not delete existing records by age.
- Do not cascade the chunk-size/disk-budget setting through the scenario -> suite -> workspace -> defaults config cascade — per Configuration-cascade integration, it must be resolved once at process start, independent of Task 5.1's separate, cascaded display-verbosity setting.

## Reopen triggers

- Task 2.6's writer-health signal needs a different backpressure shape than this task assumed (e.g. a bounded channel instead of direct synchronous writes).
- Task 7.2's benchmark finds the chosen default chunk size causes unacceptable overhead, requiring the default constant (not the flag/env var mechanism itself) to change.
- Task 5.1's split-confirmation test finds this task's flag/env var naming or reading mechanism conflicts with, or is reachable from, the display-verbosity cascade.

## Context

This is the single storage implementation Shared Architecture: Producers and Consumers insists on reuse of by every later operation kind. The plan's Configuration-cascade integration section additionally names this task (Task 2.1) as the owner of making chunk size and disk budget operator-configurable via a CLI flag/environment variable, not only a hardcoded constant — a deliverable this task's implementation steps now deliver directly, closing the gap a prior round's task set left unaddressed.

## Options & tradeoffs

A single unbounded append-only file is simplest but cannot bound memory/IO burst behavior; segmented/chunked storage with a fixed per-segment byte cap (chosen) adds rollover complexity but satisfies 'use streaming or segmented storage with bounded memory.' For making that cap operator-configurable, a purely hardcoded constant would be simplest but does not satisfy the plan's explicit 'CLI flag or environment variable read at process start' requirement; a OnceLock-backed flag/env var pair following the existing DEBUG/VERBOSE pattern (chosen) adds one more global but matches both the plan's requirement and this codebase's established mechanism.

## Decision

Implement segmented, newline-delimited-JSON storage with a fixed per-segment byte cap and bounded-cadence flushing, independent of progress.rs's existing transport format, with the chunk-size and disk-budget caps resolved once at process start from a CLI flag or environment variable (falling back to a documented default), following the existing DEBUG/VERBOSE OnceLock pattern rather than the scenario/suite/workspace display-verbosity cascade.

## Acceptance criteria

- A test (e.g. journal_writer_reconstructs_ten_thousand_records_across_segments) writes >10,000 synthetic records spanning multiple segments and the paired reader yields them back in original sequence order.
- The same test asserts a documented peak-memory/buffer-size bound is not exceeded during the write.
- Test chunk_size_flag_and_env_var_override_default proves that setting the new CLI flag (or, absent a flag, the corresponding environment variable) to a smaller-than-default chunk size produces a different (greater) segment-file count than a default-configuration run for the same 10,000-record set.

## File touchpoints

- src/diagnostics/journal.rs (proposed)
- src/progress.rs (reference)
- src/cli/root.rs
- src/application.rs
- src/common/globals.rs

## Tests

- cargo test --lib diagnostics::journal::tests -- journal_writer_reconstructs_ten_thousand_records_across_segments
- cargo test --lib diagnostics::journal::tests -- chunk_size_flag_and_env_var_override_default

## Dependencies

- define-diagnostic-record-v1

## Risks

- A naive implementation that buffers all pending records before flush could pass a small test but blow its memory bound at the plan's real target scale; mitigate by making the bounded-memory assertion part of the required test itself, not a separate manual check.
- A CLI-flag/env-var-configurable chunk size/disk budget that is read but never actually threaded into the writer's rollover check would look identical to a correctly-wired one in a casual code read; mitigate by making the override-vs-default segment-file-count comparison (chunk_size_flag_and_env_var_override_default) a required test, not a documented-only claim.
