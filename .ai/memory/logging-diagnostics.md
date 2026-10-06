# Logging Diagnostics

Verified: 2026-10-06

- [Diagnostic records](../../src/diagnostics/record.rs) are versioned separately
  from live progress. Public capture records use templates, typed fields, opaque
  identities, and categorical errors; provider detail is deployment-only.
- Capture failure summaries (`FailureSite` in the record module, tracked by
  `OperationContext::track` in the [runtime](../../src/diagnostics/runtime.rs))
  use only typed record fields and the category, so markers, `engineErrorMsg`,
  and progress errors stay publishable. The first failed record wins each field;
  enclosing failures fill the action. Field-less wrapper records are skipped
  because their zero durations would replace the step's. Scenario deadlines
  read the unfinished-stage stack, since cancelled stages record no completion.
- The sanitized error chain is printed only by `log_capture_failure` in
  [test_suites](../../src/test/test_suites.rs) (console and live desktop log,
  hidden by `--no-print`); never put it in journals, markers, or reports.
  `ErrorCategory::classify` treats serde phrases as `InvalidData`, matches
  `timeout` as a word, and keeps a trailing `(Category)` from a summary.
- [Startup journals](../../src/diagnostics/startup.rs) are entry-point-owned in
  local application data, use the bounded operation queue for native warnings,
  finalize explicitly, and are excluded from public reports. The desktop installs
  its subscriber before parsing/worker dispatch; a nested worker reuses ownership.
- [Engine commands](../../src/application.rs) own separate local journals except
  version output; deployment staging retains its enclosing deployment context.
  [Desktop Activity](../../src/desktop/logs.rs) reopens saved command operation IDs,
  while image Capture Log preserves original capture provenance. Quiet-mode engine
  subprocess tests cover success/failure; desktop tests cover exact reload/export.
- Outcome severity is shared by runtime/startup and journal finalization. Rust and
  frontend schemas reject timestamps outside JavaScript's date range; the browser
  regression verifies loading-error recovery on desktop/mobile. Raw ordinary
  tracing events remain intentionally excluded from the reviewed-record adapter.
- The user confirmed a 10% mean wall-time budget for the documented five-pair
  capture workload. Final measured archive overhead, including startup lifecycle,
  was 7.48%; see the [benchmark](../../docs/logging-benchmark.md) for raw data and
  the archive-kill-switch baseline and I/O-accounting limitations. That measurement
  predates the October 2 command/application ownership follow-up; it is not a
  remeasurement of the newer code.
- [Runtime stage wrappers](../../src/diagnostics/runtime.rs) box their work before
  constructing the wrapper future. Leaving nested action/hook futures inline
  overflowed a default Windows Tokio worker stack, even on the closed-semaphore
  capture path. Boxing restored the passing regression without larger stacks.
- The owning regression is
  `cargo test --lib capture_job_journal_opens_writes_and_finalizes_before_comparing`.
  It invokes the real spawn path in an isolated subprocess, preserving global
  configuration/subscriber isolation and requiring no browser.
- [Journal ownership](../../src/diagnostics/health.rs) keeps warnings separate
  from capture/result errors. A completed capture can have a partial or missing
  archive; finalization and checksum verification determine completeness.
- Browser validation for the action/hook instrumentation used existing loopback
  fixtures with `prepare_browser(Some(false))`: no downloads or personal profiles.
- [Capture log links](../../src/test/capture.rs) bind to image SHA-256 digests.
  `ref` and `approve` preserve the original archive reference after replacing an
  image; failed link writes cannot silently attribute changed bytes to old logs.
  The staged suite-mutation regressions retain immutable operation paths and
  source journals while pruning only unreferenced public chunks.
