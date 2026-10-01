# Browser Runtime

Verified: 2026-10-01

- [Initial navigation](../../src/test/runtime.rs) uses a bounded 60000 ms default,
  matching the legacy Node/VS Code runner. Omitted or zero
  `engineOptions.gotoParameters.timeout` uses that default; positive values
  override it independently of `elementTimeout` (still 30000 ms by default).
  The `navigation_timeout` unit regressions verified the original 30-second
  mismatch before the fix and cover default, zero, and positive overrides.
- The [CDP client](../../src/test/cdp.rs) must receive the navigation budget for
  `Page.navigate`; other commands retain their 30-second deadline. The ignored
  `navigation_default_allows_slow_document_responses` browser fixture failed at
  that internal cutoff before the fix, then passed with a 31-second response
  and confirmed that an explicit short navigation timeout still fails.
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
- The [lazy-load pass](../../src/test/page_stability.rs) bypasses CSS smooth
  scrolling and checks both scroll offsets for a quiet window at the top. A fixed
  500 ms delay previously returned mid-scroll in action-free scenarios. The live
  [scenario regression](../../src/test/test_scenario.rs) reproduces this with
  capture settling disabled; page-stability fixtures cover delayed/interrupted
  returns and the blocked-scroll deadline. Both fixtures require managed Chromium.
- [Screenshot preparation](../../src/test/page_stability.rs) is shared by `save`
  and final captures: resolved delay, freeze, bounded settling, then freeze again
  for late animations. Initial animation stabilization also runs before lazy
  loading/actions; checkpoint preparation never repeats scrolling. `delay` resolves
  scenario -> workspace -> default, not through the suite; `postInteractionWait`
  remains final-only. The [save regression](../../src/test/test_action.rs) checks
  pixels, repeated checkpoints, opt-outs, and animations started by finish callbacks.
- [Caret suppression](../../src/test/page_stability.rs) runs after preparation even
  with animation freezing and settling disabled. The ignored
  `screenshot_preparation_hides_carets_without_blurring_inputs` fixture failed
  before this separation and verifies stable pixels, focus, selection, and values
  for inputs, textareas, and editable content with both animation settings.
- [Tab selection](../../src/test/tabs.rs) is explicit: `new` selects an unvisited
  direct popup of the active page (including iframe openers and `noopener`),
  `previous` steps through selection history, and `original` selects the initial
  page. URL filters share wait matching/replacements; multiple matches fail.
- Popup CDP setup must queue commands before resuming, then await responses:
  `Page.enable` can block while the target is paused. Chromiumoxide also reapplies
  launch emulation to popups, so each capture's launch viewport must match.
  The live tab fixtures verify first-script dimensions, redirects, and nested tabs.
- The [popup capture regression](../../src/test/test_scenario.rs) verifies actual
  `_blank` screenshot pixels, cross-origin Basic Auth and pre-script storage,
  persistence across tabs, once-per-document hooks, and return after popup closure.
  Desktop schema/completion checks and the native `switch_tab_can_be_added_from_navigation_menu`
  regression verify authoring the same action in the app.
