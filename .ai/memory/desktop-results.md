# Results Review

Verified: 2026-09-29

- Suite `displayName` is independent of file/output identity. The
  [workspace model](../../src/desktop/workspace.rs) stages reference/history/site
  changes across known output roots for rename/delete and label saves. Explicit
  IDs remain stable; unknown custom roots require the caller to supply them.
  [Artifact tests](../../src/desktop/workspace/artifacts.rs) cover rollback and
  concurrent changes; this is not a crash-atomic or cross-process transaction.
- [Suite menu tests](../../src/desktop/ui/tests.rs) use `suite_panel_` to cover
  right-click Rename/Delete, F2/Delete focus isolation, Fields labels, artifact
  migration, and cancelled unsaved work. Global F2 bindings defeat input isolation.
- Post-mutation activity persistence needs its own UI polling task because the
  worker poller has stopped. `suite_panel_activity_remap_finishes_persistence_and_unlocks_catalog`
  verifies completion; native `-ArtifactFlows` additionally checks label accessibility,
  F2 rename, preserved image hashes, and cancelled/confirmed deletion.
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
- [Pixel tolerance](../../src/config/mod.rs) cascades as `pixelMatchThreshold`,
  0-100 with default 5 and desktop steps of 0.1. [Comparison](../../src/compare/report_gen.rs)
  reads `.report-pixel-thresholds.json` and stores the value on report pairs.
  Approval retains reviewed values; missing fields on legacy reports mean 10
  (their original normalized 0.1), while a fresh comparison without metadata uses 5.
  `cargo test --features gui-tests --lib pixel_match_threshold -- --test-threads=1`
  covers the cascade, reports, validation, and native editing/reset.
- Regression evidence: `suite_panel_restores_all_saved_statuses_and_keeps_them_during_other_runs`
  and `approval_completion_preserves_comparison_and_updates_suite_counts`, run with
  `cargo test --features gui-tests --lib <filter> -- --test-threads=1`.
- [Zoom readouts](../../src/desktop/ui/reports.rs) defer layout until preview bounds
  are current; fit percentages use rendered pixel bounds, including layout rounding,
  while manual percentages use the selected preset. The `image_previews_` GUI tests
  cover one-decimal labels, independent modes, pointer zoom, and resized previews.
