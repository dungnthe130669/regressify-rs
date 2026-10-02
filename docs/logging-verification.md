# Logging Verification

Verified on Windows, 2026-10-01, with final CLI, worker and frontend gates rerun
on 2026-10-02. Fixtures use temporary workspaces, synthetic credentials, loopback
providers, and cached managed Chromium. No remote deployment,
credential modification, personal browser profile, or reference mutation outside
fixtures is required. Ignored tests are not counted as passing coverage.

## Engine Traceability

| Risk or Scenario                                                          | Automated Evidence                                                                                                                                                                                                                                                                                           |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Interleaved suites/viewports/attempts, pass/failure, no cross-attribution | `diagnostics::runtime::tests::interleaved_executions_survive_live_eviction_and_propagate_blocking_context`: two concurrent executions, 4,207 records each, exact sequence/identity, blocking work, nested span parents, and eviction beyond 4,096 live entries.                                              |
| Segmented storage and memory limits                                       | `journal_writer_reconstructs_ten_thousand_records_across_segments` and `chunk_size_flag_and_env_var_override_default`: 10,001 records, bounded writer-buffer proxy, ordered streaming reads, and effective flag/environment limits.                                                                          |
| Abrupt process exit and incomplete final records                          | `abrupt_process_exit_retains_flushed_records_and_never_infers_success`: child exits without destructors after 500 flushed records; partial final JSON remains an interrupted tail, not completion.                                                                                                           |
| Diagnostic create/write/budget/flush failures                             | `forced_write_failure_surfaces_warning_and_preserves_exit_status`, `background_budget_failure_preserves_readable_prefix`, `forced_flush_failure_retains_prefix_and_refuses_completion`, and `failed_index_write_cannot_advertise_a_complete_archive`.                                                        |
| Queue saturation and stalled finalization                                 | `queue_exhaustion_is_nonblocking_and_retains_every_accepted_record` and `stalled_writer_has_bounded_finalization_and_unknown_missing_count`: bounded queue, independent warning, readable prefix, bounded wait, and no invented missing count.                                                               |
| Oversized, Unicode, control-character and unreviewed data                 | `oversized_records_and_tampered_indexes_never_look_complete`, `capture_errors_never_export_arbitrary_payloads_or_provider_details`, and `known_secrets_urls_and_controls_are_removed_before_truncation`.                                                                                                     |
| Bootstrap destination failure and repeated handoff                        | `bootstrap_handoff_retains_source_on_destination_failure` and `bootstrap_handoff_marks_source_consumed_only_after_verified_append_open`: byte-identical retained source, independent warning, no false success, verified destination and stable ordering.                                                    |
| Startup/configuration failures and retained local history                 | `startup_failures_are_durable_sanitized_and_never_pruned_on_restart`: categorical failure records, independent local application-data ownership, retained history across starts, and warning-only unavailable storage.                                                                                       |
| Span-only identity and quiet display                                      | `span_context_is_inherited_and_arbitrary_tracing_fields_are_not_forwarded`, `per_execution_console_levels_do_not_filter_archives`, and `log_level_precedence_matrix`.                                                                                                                                        |
| Setup/navigation/action failures                                          | `queue_setup_stage_records_for_passed_and_timed_out_captures`, `navigation_timeout_identifies_stage_and_cause_without_credentials`, and `failing_action_records_index_type_and_last_unmet_condition_without_script_contents`. The last two live in `test::test_action::tests`, not the orchestration module. |
| Zero-image and infrastructure/teardown failures                           | `capture_job_journal_opens_writes_and_finalizes_before_comparing`, `artifact_write_failure_is_infrastructure_failure`, and `unconfirmed_browser_shutdown_is_an_infrastructure_failure`.                                                                                                                      |
| Capture success versus mismatch and promotion                             | `passed_capture_with_mismatch_shows_distinct_capture_and_comparison_outcomes` and `reference_promotion_does_not_fabricate_log_and_rejects_stale_image_links`: real PNG comparison, immutable source outcome, original operation identity and image digest binding.                                           |
| Cancellation and worker drainage                                          | `cancellation_mid_upload_reports_unknown_remote_state`, `cancellation_yields_before_draining_a_progress_backlog`, and `cancellation_stops_a_running_process_group`. The known parent-exits-before-descendant-pipe issue remains separately tracked in the issue list.                                        |
| Missing/corrupt logs and legacy metadata                                  | `archive_missing_when_expected_is_distinct_from_legacy`, `loading_error_when_segment_corrupt_despite_finalized_index`, `legacy_report_without_archive_is_explicit_not_an_error`, and `corrupt_capture_log_is_loading_error_not_legacy_or_capture_failure`.                                                   |
| Public partial logs and publication safety                                | `partial_publication_retains_readable_prefix_and_unknown_missing_count` and `public_capture_logs_are_bounded_shared_and_never_include_deployment_history`.                                                                                                                                                   |
| Selected-only packages and provider limits                                | `package_manifest_includes_only_selected_logs` compares exact ZIP inventory; `log_budget_exceeded_fails_actionably` rejects 260 MiB of logs against 250 MiB without changing sources.                                                                                                                        |
| Suite mutation and active history                                         | `suite_rename_preserves_log_assets` and `suite_delete_removes_only_unreferenced_logs_and_preserves_active_history`: immutable operation paths, selected public cleanup, source retention and continued active writes.                                                                                        |
| Provider retries, rate limits, malformed responses and publication        | Existing Azure/Netlify/Cloudflare test modules, extended by `deployment_protocol_retries_upload_and_never_sends_token_to_blob`, `netlify_retry_then_success_records_attempts`, and `cloudflare_retry_then_success_records_attempts`. Original authorization assertions remain.                               |
| Standalone CLI without desktop initialization                             | `standalone_cli_persists_passed_and_failed_logs_without_desktop_initialization`: real `ref` then mixed `test`, two suites/two viewports, complete reference/test links, failed page with zero exit, output/archive secret scan, and kill-switch with no new journal directory.                               |

## Focused Gates

The final Windows default gate, `cargo test --locked --all-targets`, passed 446
tests with 18 ignored. The serialized `cargo test --locked --features gui-tests
--all-targets -- --test-threads=1` gate passed 974 library tests with 27 ignored
before the startup follow-up; that follow-up passed its focused regression and
the final default/worker gates. The latest worker integration run passed 11 tests
with four opt-in checks ignored. The standalone CLI browser regression and archive
benchmark were also run explicitly and passed. GUI build and GUI Clippy with
warnings denied passed. The serialized GUI suite took about 42 minutes.

Run these before broader feature gates:

```sh
cargo test --lib progress::tests
cargo test --lib test::test_suites::tests
cargo test --lib test::test_scenario::tests
cargo test --lib test::test_action::tests
cargo test --lib test::capture::tests
cargo test --lib diagnostics::
cargo test --lib snapshot::
cargo test --features desktop --lib desktop::deployment::
cargo test --features desktop --lib desktop::worker::tests
cargo test --features gui --test desktop_worker
```

The real CLI regression is opt-in and requires the existing managed browser:

```sh
cargo test --features gui --test desktop_worker standalone_cli_persists_passed_and_failed_logs_without_desktop_initialization -- --ignored --exact
```

Set `REGRESSIFY_LOG_BROWSER_FIXTURE` to a **new** temporary destination to retain
that test's synthetic output. Serve it with the frontend preview script. This
does not contact production sites or install Chromium.

## Client Evidence

Client tests belong to their implementation units rather than the engine suite:

- Native archive restart/corruption, reference/test switching, large-log
  filter/search/download, and partial-log warning tests passed under `gui-tests`.
  The 10,001-record workflow verifies downloaded bytes against filtered view text.
- Playwright `capture log` tests passed on desktop and mobile: no eager fetch,
  one request on opening, reference/test deep links and refresh, plain-text markup,
  downloads, distinct availability states, layout and accessibility.
- The complete frontend browser suite passed 109 tests with seven configuration
  skips. Ten focused production-bundle checks also passed on desktop and mobile.
  The initial full run caught suite-list state loss and an off-screen Info close
  button; existing regressions now cover list/log/refresh round trips and full
  dialog visibility, and fail before the repair.
- Frontend unit tests passed 19 cases and 137 assertions; typecheck and production
  build passed. Scoped Biome checks passed for changed sources. Full frontend lint
  still reports five existing formatting failures in untouched files, listed in
  [outstanding issues](../OUTSTANDING_ISSUES.md#frontend-formatting-baseline-2026-10-01).
- The real CLI-generated report was independently opened in the production bundle:
  59 test records and 59 reference records, complete state, preserved reference
  selection after refresh, no injected diagnostic secrets, and no overflow at
  375 x 667. This was not a mocked report response.
- The full worker integration inventory regression now passes with exact original
  and preview dependencies, rather than a stale two-image assumption.

## Documentation Traceability

Task 031 cross-checks these maintained entry points against the shipped behavior:

| File                                                                 | Maintained Contract                                                                                          |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| [Command documentation](commands.md#diagnostic-verbosity)            | Exact display flags/precedence, startup and capture storage, warning-only health, provenance and privacy.    |
| [Desktop documentation](desktop.md#reviewing-results)                | Paged Activity and Results Capture Log views, filters, complete-log export and independent health warnings.  |
| [Frontend guide](../frontend/README.md#data-and-hosting)             | Capture-log deep links, suite-layout retention, lazy bounded chunks, version checks and export limits.       |
| [Agent guide](../AGENTS.md)                                          | Diagnostic outcome/provenance rule and a direct logging entry point.                                         |
| [Diagnostics skill](../.github/skills/logging-diagnostics/SKILL.md)  | Entry-point ownership, typed privacy, bounded writing/reading, five states, cancellation and focused checks. |
| [Progress skill](../.github/skills/desktop-progress-events/SKILL.md) | Archive-backed completed logs, stale-load rejection and live omissions versus durable completeness.          |
| [Results skill](../.github/skills/desktop-results-review/SKILL.md)   | Original reference/test execution selection and independent health/export availability.                      |
| [Worker skill](../.github/skills/desktop-worker-lifecycle/SKILL.md)  | Startup/deployment ownership and exact-operation cancellation recovery.                                      |
| [Snapshot skill](../.github/skills/snapshot-command/SKILL.md)        | Additive version-2 log fields, bounded public chunks, deep links and raw-journal exclusion.                  |
| [Packaging skill](../.github/skills/deployment-packaging/SKILL.md)   | Selected-only log dependencies, integrity checks, deduplication and provider budgets without truncation.     |
| [Logging memory](../.ai/memory/logging-diagnostics.md)               | Verified startup/capture lifecycle, bounded futures, provenance and measured overhead.                       |
| [Frontend memory](../.ai/memory/snapshot-frontend.md)                | Mobile popup bounds and list/log navigation regressions.                                                     |
| [Task index](plan/log-revamp-tasks/README.md)                        | Completed task groups, maintained contracts and explicit verification limits.                                |
| [Outstanding issues](../OUTSTANDING_ISSUES.md)                       | Remaining formatting baseline and the resolved exact worker-inventory assertion.                             |

## Limits

Unix-only symlink tests were not executed on Windows. Static containment guards
and Windows tests do not establish race-free filesystem access. The writer's
deadline bounds waiting, not an OS write already in progress. Browser export has
an explicit 128 MiB filtered-output cap; exceeding it fails rather than truncating.
The benchmark uses the archive kill-switch, not a build with instrumentation
compiled out. See the benchmark report for that distinction and measured overhead.
