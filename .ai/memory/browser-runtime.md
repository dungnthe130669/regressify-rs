# Browser Runtime

Verified: 2026-09-28

- [Target URL reads](../../src/test/runtime.rs) use `Page.getFrameTree`, not
  JavaScript evaluation. Navigation can invalidate a cached execution context
  before its destruction event is consumed. The isolated browser regression
  `url_reads_do_not_require_a_live_execution_context` failed with the original
  missing-context error before the change and passes afterward.
- Frame metadata separates `url` and `urlFragment`; URL matching needs both.
  An out-of-process iframe is absent from its parent's frame tree, so URL reads
  fall back to the already-attached frame session and still match the frame ID.
- The [browser fixtures](../../src/test/browser_tests.rs) exercise delayed
  cross-origin redirects, subsequent visible-selector waits, SPA query/fragment
  changes, URL timeout expiry, and frame-scoped URL matching. Run their ignored
  tests with the managed Chromium cache; do not use real credentials or baselines.
