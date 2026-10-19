# Browser Runtime

Verified: 2026-10-19

- [Script hooks](../../src/test/hooks.rs) run entirely in Chromium: document-start
  `onBeforeScript`, then CSS/`jsOnReadyPath`/`onReadyScript` after readiness and
  before actions. Ready hooks follow each selected document once. The browser
  hook and [full capture](../../src/test/browser_tests.rs) fixtures pass with no
  workspace npm packages and an empty worker PATH, including failed-reference
  preservation. The Node runner and Playwright attachment were removed.
- User files need classic expression evaluation, not `Page::evaluate`'s function
  heuristic, which invoked a leading declaration in a multi-statement script.
  DOM readiness, hooks, [screenshot preparation](../../src/test/page_stability.rs),
  [selector capture](../../src/test/capture.rs), and [storage capture](../../src/test/storage_state.rs)
  use [current-document evaluation](../../src/test/runtime.rs) without a cached
  context ID. `hooks_do_not_reuse_destroyed_execution_contexts` in
  [hook tests](../../src/test/hooks.rs) disables Runtime events during local
  navigation and verifies CSS, both scripts, settling, screenshot preparation,
  and storage capture on each new document.
- [Action viewport filters](../../src/test/test_scenario.rs) share one predicate
  across the executor, tab-tracker guard, and save inventory. Strict-subset
  [preflight](../../src/test/test_suites.rs) validates full consumer/prerequisite
  contexts, while legacy actions stay unset. String and array filters share
  `Selectors` with scenarios and normalize through a borrowed-slice accessor;
  serialization preserves either form. Case variants are membership only;
  blank strings fail validation and unnamed captures accept only unset/null/empty lists.
- The ignored scenario `filtered_reference_and_test_manifests_preserve_baselines`
  runs real `ref`/`test`/failing `ref` against loopback pages and cached Chromium.
  It verifies ordered manifests and pixels, all-skipped final captures, scoped
  placeholders, peer isolation, and unchanged historical references.
- [Capture selectors](../../src/test/capture.rs) keep `save.selector` independent
  from scenario `selector` and retain save/`FINAL` filenames. The last applicable
  own save suppresses the automatic final and its wait/preparation, not explicit
  plural `selectors` or a consumer's final after prerequisite saves. The ignored
  `selector_saves_and_final_captures_match_pixels_and_inventory` regression in
  [scenario tests](../../src/test/test_scenario.rs) verifies 13 local-browser cases,
  including pixels, filtering, invalid targets, scoped placeholders, and scroll.
- The [responsive browser fixture](../../src/test/browser_tests.rs)
  `responsive_hamburger_menu_mobile_only_click_and_save` verifies observed
  interaction order, a mobile-only DOM menu/checkpoint, both final images, and
  no desktop dispatch of the menu selector in its completed action journal.
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
- The lazy-load pass now holds visible native/deferred image and picture sources
  and common lazy-loader backgrounds until ready, rather than accepting decoded
  placeholders. Intersections exclude clipped slides; growing page height is
  rechecked. The slow-image pixel fixture failed before the fix; delayed loopback
  resource, clipping, timeout-cleanup, and scenario-hover regressions pass.
  `lazyLoadScrollDelay` (250 ms, zero allowed) and `lazyLoadTimeout` (20000 ms,
  positive) cascade through all three scopes; CLI and desktop tests cover them.
- [Timed main-document evaluation](../../src/test/runtime.rs) uses the existing
  scenario CDP client for long lazy-load waits. Chromiumoxide checks expired
  commands periodically: a 31-second probe passed on the old path. The retained
  61-second [regression](../../src/test/page_stability.rs) verifies the explicit budget.
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
- [Expanded actions](../../src/test/gestures.rs) use complete CDP gestures with
  owned input cleanup; the pinned bindings need explicit empty touchPoints and
  drag items arrays. Native HTML5 data and confined regular file drops are distinct.
- [Touch setup](../../src/test/tabs.rs) must pause/configure OOPIF targets before
  first scripts, recursively, even in jobs without switchTab. Top-level touch
  emulation alone left cross-origin frames at zero maxTouchPoints in the live fixture.
- The ignored `expanded_actions_` [browser fixtures](../../src/test/browser_tests.rs)
  verify trusted events, actual drop payloads, focus, timing, cancellation, and
  strict dialogs/history, including intentional beforeunload cancellation.
