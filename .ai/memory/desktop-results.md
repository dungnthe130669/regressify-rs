# Results Review

Verified: 2026-09-24

- [Scenario naming](../../src/helpers/scenario_names.rs) is shared by desktop
  catalogs/drafts/results and Rust snapshot conversion. URL-only scenarios use
  shortest unique readable suffixes, optional mixed origins, then query/fragment
  details or ordinals. Multiple captures of one scenario share a name.
- [Capture manifests](../../src/test/test_suites.rs) record `scenarioDisplayName`
  against the whole suite before filtering/replacements. It is presentation-only:
  source labels, IDs, filenames, and routes stay independent. Legacy raw-URL labels
  are resolved during report loading. Evidence: `cargo test --features desktop
  --lib names` covers cross-client parity, stable IDs, and captured-name preference.
- [Results navigation](../../src/desktop/ui/navigation.rs) uses loaded report
  scenario/viewport counts while idle; saved Activity counts can predate approval.
  Live run counts still take precedence during execution.
- [Results approval](../../src/desktop/ui/reports.rs) confirms inline, clears pending
  confirmation on capture changes/refresh, and refreshes its existing output root
  without resetting the current capture or view. It still uses the
  [run guard](../../src/desktop/ui/mod.rs) for drafts and suite preflight.
- Regression evidence: `suite_panel_restores_all_saved_statuses_and_keeps_them_during_other_runs`
  and `approval_completion_preserves_comparison_and_updates_suite_counts`, run with
  `cargo test --features gui-tests --lib <filter> -- --test-threads=1`.
