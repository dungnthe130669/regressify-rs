<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 032 — Wire per-execution journal open/finalize into the real capture pipeline

- Slug: `capture-journal-lifecycle-wiring`
- Implements plan section(s): `Phase 3 — Capture Instrumentation#0`, `Full-Log Persistence#0`, `Producers and Consumers#0`
- Depends on: operation-kind-linkage, atomic-index-manifest-finalization, install-subscriber-entry-points

## Goal

Open a capture-kind operation journal writer when a real capture execution begins, route that execution's tracing spans/events into it through a dedicated journal-writing tracing layer, and finalize the writer before the execution's result is consumed by automatic comparison — giving Phase 3's stage instrumentation (Tasks 3.1-3.4) and Task 2.4's archive-state detection something real to read in a non-synthetic build.

## Why this exists

Tasks 2.1-2.6 build the generic journal mechanism and Tasks 3.1-3.4 specify per-stage diagnostic content, but nothing specifies who opens a per-execution writer for a real capture or who finalizes it before compare consumes it; Task 1.4's adapter converts DiagnosticRecordV1 -> progress::Event (old pipeline) and cannot be the route from tracing events into the new journal. This mirrors Task 4.1 (deployment-journal-reuse)'s equivalent role for deployments, closing the asymmetry the reviewer found.

## Settled investigation

- src/test/test_suites.rs::spawn_capture_jobs (lines 237-309) wraps each CaptureJob task body in tokio::spawn(progress::scope_scenario(index, async move {...})) (line 266) — this existing per-job boundary, which already establishes the task-local SCENARIO_JOB id, is the real (non-synthetic) point where a per-execution journal writer must be opened; Tasks 3.1-3.4 instrument code that runs inside this same scope.
- Task 1.4's ProgressAdapter (implement-progress-adapter) is explicitly one-way: DiagnosticRecordV1 -> progress::Event, for feeding the OLD progress.rs pipeline from new diagnostic records. It is not, and cannot be, the mechanism that converts a tracing::info!/tracing::error! call emitted by Tasks 3.1-3.4 into a DiagnosticRecordV1 written to the new journal; that conversion needs its own tracing_subscriber::Layer, which this task defines.
- Task 1.5 (install-subscriber-entry-points) installs exactly two layers at process start: a console-formatting layer and the Task 1.4 adapter layer. It does not install a journal-writing layer; this task adds that layer to the same install function rather than creating a second, competing subscriber-installation call site.
- Full-Log Persistence requires an operation-bound writer (not a single replaceable process-wide destination) and requires finalizing 'each capture execution after teardown and before automatic comparison or snapshot generation consumes its logs' — this task orders its finalize() call relative to progress::Event::Comparing, the existing lifecycle event marking comparison start for a capture.
- Task 4.1 (deployment-journal-reuse) is the already-specified deployment-side analog of this task: it opens a deployment-kind journal writer at operation start and finalizes it at operation end, around existing call sites, before Tasks 4.2/4.3 add richer per-stage content. This task does the same for the capture path, which Task Breakdown's Phase 3 section left unspecified.

## Inputs

- Task 2.3's (operation-kind-linkage) proven-generic journal reader/writer, usable for OperationKind::Capture
- Task 2.2's (atomic-index-manifest-finalization) finalize() function
- Task 1.5's (install-subscriber-entry-points) existing install_subscriber() function and its two existing layers (reference, extended not replaced)
- src/test/test_suites.rs::spawn_capture_jobs's existing per-job progress::scope_scenario boundary (reference)

## Outputs

- A journal-routing tracing_subscriber::Layer that converts a tracing span/event emitted within a capture job's scope into a DiagnosticRecordV1 and writes it to that job's currently-open journal writer
- A capture-kind journal writer opened at spawn_capture_jobs's per-job boundary, keyed by a freshly-minted ExecutionId, registered so the routing layer can find it for that job's scope
- The writer finalized at the job's teardown, before this job's progress::Event::Comparing is emitted
- A test proving one real (not synthetic-helper) capture job's tracing output round-trips through open -> write -> finalize -> read

## Implementation steps

- Define a tracing_subscriber::Layer implementation (e.g. JournalLayer) whose on_event/on_enter hooks look up the current job's registered journal-writer handle via the same task-local storage mechanism progress::scope_scenario already uses for SCENARIO_JOB, convert the tracing event's fields into a DiagnosticRecordV1 (reusing Task 1.3's types), and call that writer's write().
- Extend Task 1.5's install_subscriber() function (src/diagnostics/install.rs) to register this JournalLayer alongside its existing console-formatting and Task 1.4 adapter layers, without modifying or removing either existing layer.
- At spawn_capture_jobs's per-job boundary (test_suites.rs line 266, inside progress::scope_scenario), open a capture-kind journal writer (Task 2.1) keyed by a new ExecutionId for this job, and register it in the same task-local slot the JournalLayer reads from.
- At the job's teardown point, after this job's TaskStatus/ScenarioOutcome is determined and before this job's progress::Event::Comparing is emitted, call finalize() (Task 2.2) on this job's writer and deregister it from task-local storage.
- Write capture_job_journal_opens_writes_and_finalizes_before_comparing: run one real capture job through spawn_capture_jobs (or an equivalent harness invoking the same function), emit at least one tracing event (e.g. a tracing::info! test marker) inside its scope, capture the ExecutionId assigned via a test-only accessor, and assert (a) a Complete-state journal exists for that ExecutionId after the job finishes, (b) that journal's records include the emitted event, (c) finalize() completed before this job's progress::Event::Comparing was observed by a test LiveProgress.

## Done when

- Test capture_job_journal_opens_writes_and_finalizes_before_comparing runs one real capture job through spawn_capture_jobs with at least one tracing event emitted inside its scope, and asserts the resulting journal for that job's ExecutionId is ArchiveState::Complete and contains that event.
- The same test asserts the job's journal finalize() call completed before this job's progress::Event::Comparing was observed, by recording both events' relative order in the test harness.

## Evidence to capture

- capture_job_journal_opens_writes_and_finalizes_before_comparing's asserted ExecutionId, its resulting ArchiveState, and the recorded relative order of finalize() vs. progress::Event::Comparing
- The JournalLayer's registration point in Task 1.5's install_subscriber() function, confirmed via git diff to add a third layer without altering the existing two

## Do not change

- Do not remove or alter Task 1.5's existing console-formatting layer or Task 1.4 adapter layer; this task adds a third layer alongside them.
- Do not route journal-writer records through Task 1.4's ProgressAdapter, and do not make the new JournalLayer call progress::emit; the two layers must remain independent, one feeding progress::Event (old pipeline, Task 1.4), the other feeding the journal (new pipeline, this task).
- Do not open a single process-wide journal writer shared across concurrent jobs; Full-Log Persistence requires an operation-bound (per-execution) writer, and Identity and Record Contract requires each execution retain its own identity.

## Reopen triggers

- Task 3.1's (capture-queue-setup-stage) actual field-recording implementation finds the JournalLayer's event-to-DiagnosticRecordV1 conversion insufficient for a field Capture Diagnostics requires (e.g. a span-nested duration), requiring the layer's conversion logic to change.
- Task 7.1's (concurrency-crash-recovery-suite) interleaved-concurrent-capture testing finds two simultaneous jobs' records cross-contaminating in the journal, indicating the task-local registration this task relies on is not properly isolated per job.

## Context

Tasks 2.1-2.6 build the generic journal mechanism and prove it works for synthetic records and, separately, for a real mocked deployment (Task 4.1); Tasks 3.1-3.4 specify exactly what diagnostic content each capture stage must record. Nothing between those two groups specifies how a real capture execution's tracing output actually reaches a real, per-execution journal file, or who closes that file out before compare consumes it. This task is that missing link, deliberately narrow in the same way Task 4.1 is narrow: it proves the open-write-finalize plumbing around the existing per-job boundary, before Tasks 3.1-3.4 add their full required-context fields.

## Options & tradeoffs

A single process-wide journal writer shared by all concurrent jobs would need no per-job registration/lookup, but directly violates Full-Log Persistence's 'operation-bound writer' requirement and would conflate multiple executions' records in one file, defeating Task 2.3's operation-kind-linkage and Identity and Record Contract's per-execution identity requirement; a per-job writer registered in task-local storage and routed to by a dedicated tracing Layer (chosen) costs more plumbing but keeps each execution's archive genuinely its own, matching how Task 4.1 scopes a journal to one deployment operation.

## Decision

Add a dedicated journal-writing tracing Layer to Task 1.5's existing subscriber installation, and open/register a per-job capture-kind journal writer at spawn_capture_jobs's existing per-job boundary, finalizing it at job teardown before comparison consumes it — mirroring Task 4.1's deployment-side open/finalize pattern for the capture path.

## Acceptance criteria

- Test capture_job_journal_opens_writes_and_finalizes_before_comparing proves a real capture job's tracing output is readable back from a Complete-state journal keyed by that job's own ExecutionId after the job finishes.
- The same test proves finalize() for that job's journal completed before the job's progress::Event::Comparing was observed, establishing the ordering Full-Log Persistence requires.

## File touchpoints

- src/diagnostics/journal.rs (reference)
- src/diagnostics/install.rs (proposed)
- src/test/test_suites.rs
- src/progress.rs (reference)

## Tests

- cargo test --lib test::test_suites::tests -- capture_job_journal_opens_writes_and_finalizes_before_comparing

## Dependencies

- operation-kind-linkage
- atomic-index-manifest-finalization
- install-subscriber-entry-points

## Risks

- A task-local registration keyed incorrectly (e.g. keyed by thread rather than by the async task/job) could route one job's tracing events into a different concurrently-running job's journal; mitigate by keying registration off the same SCENARIO_JOB task-local mechanism progress::scope_scenario already uses successfully for this exact concurrent-job-isolation problem, not a new mechanism.
- Finalizing too early (e.g. before a retry's async cleanup completes) could truncate real records; mitigate by finalizing only after the job's TaskStatus/ScenarioOutcome is fully determined, the same point Task 3.4's finish-stage record is written.
