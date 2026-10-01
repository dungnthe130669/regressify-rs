<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 029 — Concurrency and crash-recovery regression suite

- Slug: `concurrency-crash-recovery-suite`
- Implements plan section(s): `Phase 7 — Hardening and Docs#0`, `Verification Strategy#0`
- Depends on: bootstrap-handoff-fallback, writer-health-kill-switch, capture-queue-setup-stage, capture-navigation-stage, capture-state-actions-settling-stages, capture-artifacts-finish-stages, deployment-journal-reuse, deployment-preflight-staging-packaging-stages, deployment-provider-upload-stages, deployment-cancellation-partial-publication, legacy-migration-behavior

## Goal

Provide one automated test per Verification Strategy engine-level scenario reachable from this task's dependencies (Tasks 2.4, 2.5, 2.6, 3.1-3.4, 4.1-4.4): interleaved/concurrent captures, bootstrap/writer failure injection, archive-state detection (missing/corrupt logs), capture stage-level failure cases, and deployment provider-mock failure/cancellation cases.

## Why this exists

Task 7.1's own Task Breakdown acceptance criterion requires every such engine-level scenario in Verification Strategy to have a corresponding passing automated test on the target platforms, explicitly excluding desktop/CLI-view/frontend scenarios already proven by Tasks 5.2/5.4/5.5/6.2's own acceptance criteria.

## Settled investigation

- This task explicitly does not cover 'reload desktop history without rerunning,' lazy loading, filtering/search, downloads, refresh/deep links, or desktop/mobile layouts — those are proven by Task 5.2's, Task 5.4's, Task 5.5's, and Task 6.2's own acceptance criteria per Task Breakdown's explicit carve-out; this task's own scope is engine-level only.
- Verification Strategy's bullet list is the authoritative scenario list this task's tests must cover: interleaved passed/failed scenarios across suites/viewports/attempts; exceeding live byte/entry limits; setup/action/navigation/capture/teardown/zero-image/cancel/abrupt-exit/flush failures; diagnostic-only create/write/flush failure injection with warning delivery and unchanged exit status; bootstrap-copy/open failure injection; span-only multi-threaded identity; standalone-CLI-persists-without-desktop; mock provider timeouts/rate-limits/transient-permanent-errors/malformed-responses/partial-uploads/failed-polling/cancellation.
- The existing 'Affected checks to turn into task-specific gates' code block (cargo test --lib progress::tests, test::test_suites::tests, test::test_scenario::tests, test::capture::tests, --features desktop --lib desktop::runner::tests, desktop::worker::tests, --features gui --test desktop_worker, --lib snapshot::, --features desktop --lib snapshot::, --features desktop --lib desktop::deployment::, cargo fmt --all -- --check) is the existing baseline gate set this new suite extends, not replaces.
- Verification Strategy's 'missing/corrupt log assets' scenario (within 'Cover compare/snapshot regeneration, approval provenance, custom output roots, suite mutation, older captures, missing/corrupt log assets, and legacy schemas') is engine-level and is already covered by Task 2.4's (legacy-migration-behavior) archive_missing_when_expected_is_distinct_from_legacy and loading_error_when_segment_corrupt_despite_finalized_index tests; this task's traceability list cites those two tests directly for that bullet rather than adding a third, duplicate test.
- Tasks 3.2 (capture-navigation-stage) and 3.3 (capture-state-actions-settling-stages) — both direct dependencies of this task — place their own named acceptance tests in the test::test_action::tests module, not in test::test_suites::tests or test::capture::tests: 'cargo test --lib test::test_action::tests -- navigation_timeout_identifies_stage_and_cause_without_credentials' (Task 3.2) and 'cargo test --lib test::test_action::tests -- failing_action_records_index_type_and_last_unmet_condition_without_script_contents' (Task 3.3). This task's own gate-command set and Done-when/Tests lists must include that module explicitly, or running exactly this task's own specified commands would not catch a regression in either dependency's named test.

## Inputs

- Tasks 2.5, 2.6, 3.1-3.4, 4.1-4.4's individual stage/failure tests (this task aggregates/extends their scenario coverage, it does not duplicate each one)
- Verification Strategy's full bullet list
- Task 2.4's (legacy-migration-behavior) archive-state tests, specifically covering the 'missing/corrupt log assets' Verification Strategy scenario
- Tasks 3.2's (capture-navigation-stage) and 3.3's (capture-state-actions-settling-stages) named tests in the test::test_action::tests module

## Outputs

- One passing automated test per in-scope Verification Strategy scenario, run on the target platforms

## Implementation steps

- Enumerate every Verification Strategy bullet that is engine-level (not desktop/CLI-view/frontend) and cross-reference it against the tests Tasks 2.4/2.5/2.6/3.1-3.4/4.1-4.4 already produced, identifying any scenario not yet covered by an existing test.
- For each gap found, add a new automated test, reusing existing test fixtures/mocks from the dependency tasks where possible.
- Explicitly add interleaved-concurrent-capture coverage (multiple suites/viewports/attempts running simultaneously) if not already exercised, asserting identity/ordering/complete-history/no-duplicate-records through CLI, worker transport, and archive (engine-level, not UI).
- Include `cargo test --lib test::test_action::tests` in this task's own combined gate-running command set and traceability list, since Tasks 3.2/3.3's named acceptance tests live in that module, not in test_suites::tests or capture::tests.
- Run the resulting combined test command set on the target platform(s) (at minimum Windows) and record pass/fail per scenario in the traceability list.
- Explicitly map Verification Strategy's 'missing/corrupt log assets' bullet to Task 2.4's archive_missing_when_expected_is_distinct_from_legacy and loading_error_when_segment_corrupt_despite_finalized_index tests in the traceability list, rather than leaving it unclaimed by any task.

## Done when

- A traceability list maps each in-scope Verification Strategy bullet to at least one named, passing test from Tasks 2.4, 2.5, 2.6, 3.1-3.4, or 4.1-4.4, with zero unmapped bullets remaining (for example, 'missing/corrupt log assets' maps explicitly to Task 2.4's archive_missing_when_expected_is_distinct_from_legacy and loading_error_when_segment_corrupt_despite_finalized_index tests, and the navigation/action-failure bullets map explicitly to Tasks 3.2/3.3's test::test_action::tests cases).
- Running `cargo test --lib progress::tests`, `cargo test --lib test::test_suites::tests`, `cargo test --lib test::test_scenario::tests`, `cargo test --lib test::test_action::tests`, `cargo test --lib test::capture::tests`, `cargo test --lib diagnostics::journal::tests`, `cargo test --features desktop --lib desktop::deployment::`, and `cargo test --features desktop --lib desktop::worker::tests` together passes with 0 failures.

## Evidence to capture

- The full traceability list (Verification Strategy bullet -> test name), explicitly including the 'missing/corrupt log assets' -> Task 2.4 test mapping and the navigation/action-failure -> Task 3.2/3.3 test::test_action::tests mapping, and the combined test run's pass count/failure count (expected: 0 failures across all 8 listed commands, including test::test_action::tests)

## Do not change

- Do not add desktop/CLI-view or frontend browser tests here — those are explicitly owned by Tasks 5.2/5.4/5.5/6.2's own acceptance criteria, not this task.
- Do not weaken or skip an existing passing test from Tasks 2.4/2.5/2.6/3.1-3.4/4.1-4.4 to make this aggregation easier.

## Reopen triggers

- A new Verification Strategy scenario is added to the plan after this task closes, or an existing dependency task's test is later found to not actually cover the scenario this task credited it for.
- A future refactor moves Task 3.2/3.3's named tests out of the test::test_action::tests module, requiring this task's gate-command set to be updated to match their new location.

## Context

Phase 7 is explicitly a hardening phase; this task is where the many individual stage/failure tests built throughout Phases 2-4 are checked against the plan's own cross-task verification strategy for gaps, rather than trusting that per-task acceptance criteria summed to full coverage by accident. Because two of this task's own direct dependencies (Tasks 3.2 and 3.3) place their named tests in test::test_action::tests specifically, this task's own gate list must name that module explicitly rather than assuming it is covered by the similarly-named test_suites::tests/capture::tests modules.

## Options & tradeoffs

n/a — this is a coverage-completion task against an already-fixed scenario list in Verification Strategy; the only choice is how to organize the traceability (a single new test module vs. scattering gap-filling tests into each dependency task's existing module). Scattering gap-filling tests into the already-relevant existing module (chosen) keeps related tests co-located with the code they exercise.

## Decision

Add any missing engine-level scenario's test into the most relevant existing test module from its owning dependency task, and produce one consolidated traceability list mapping Verification Strategy bullets to concrete tests — explicitly including test::test_action::tests in this task's own gate-command set, Done-when, and Tests lists, since that module (not test_suites::tests or capture::tests) holds Tasks 3.2/3.3's named acceptance tests.

## Acceptance criteria

- Every in-scope Verification Strategy bullet (per the explicit carve-out excluding desktop/CLI-view/frontend scenarios already owned by Tasks 5.2/5.4/5.5/6.2) has at least one named, passing automated test identified in a traceability list, with zero bullets left unmapped — for example, 'missing/corrupt log assets' maps to Task 2.4's archive_missing_when_expected_is_distinct_from_legacy and loading_error_when_segment_corrupt_despite_finalized_index tests, and navigation/action-failure scenarios map to Tasks 3.2/3.3's test::test_action::tests cases.
- The full combined cargo test command set for the affected modules (progress::tests, test::test_suites::tests, test::test_scenario::tests, test::test_action::tests, test::capture::tests, diagnostics::journal::tests, desktop::deployment::, desktop::worker::tests) passes with 0 failures in one invocation.

## File touchpoints

- src/diagnostics/journal.rs (reference)
- src/test/test_suites.rs (reference)
- src/test/test_action.rs (reference)
- src/desktop/deployment.rs (reference)
- src/desktop/deployment/swa.rs (reference)
- src/desktop/deployment/netlify.rs (reference)
- src/desktop/deployment/cloudflare.rs (reference)

## Tests

- cargo test --lib progress::tests
- cargo test --lib test::test_suites::tests
- cargo test --lib test::test_scenario::tests
- cargo test --lib test::test_action::tests
- cargo test --lib test::capture::tests
- cargo test --lib diagnostics::journal::tests
- cargo test --features desktop --lib desktop::deployment::
- cargo test --features desktop --lib desktop::worker::tests

## Dependencies

- bootstrap-handoff-fallback
- writer-health-kill-switch
- capture-queue-setup-stage
- capture-navigation-stage
- capture-state-actions-settling-stages
- capture-artifacts-finish-stages
- deployment-journal-reuse
- deployment-preflight-staging-packaging-stages
- deployment-provider-upload-stages
- deployment-cancellation-partial-publication
- legacy-migration-behavior

## Risks

- A traceability list built manually (by reading test names) could miscredit a test that superficially matches a scenario name but does not actually assert the scenario's key property; mitigate by re-reading each credited test's assertions, not just its name, when building the traceability list.
- Omitting a dependency's own test module from this task's gate-command set (as test::test_action::tests was previously omitted despite Tasks 3.2/3.3 depending on it) would let a regression in that dependency go undetected by this task's own specified commands; mitigate by cross-checking every direct dependency's '## Tests' section against this task's own Tests list before closing, not just the modules with matching names.
