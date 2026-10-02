# Logging Diagnostics

Verified: 2026-10-01

- [Diagnostic records](../../src/diagnostics/record.rs) are versioned separately
  from live progress. Public capture records use templates, typed fields, opaque
  identities, and categorical errors; provider detail is deployment-only.
- [Startup journals](../../src/diagnostics/startup.rs) are entry-point-owned in
  local application data, finalized explicitly, and excluded from public reports.
- The user confirmed a 10% mean wall-time budget for the documented five-pair
  capture workload. Final measured archive overhead, including startup lifecycle,
  was 7.48%; see the [benchmark](../../docs/logging-benchmark.md) for raw data and
  the archive-kill-switch baseline and I/O-accounting limitations.
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
