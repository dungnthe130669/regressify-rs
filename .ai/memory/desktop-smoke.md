# Desktop Smoke Harness

Verified: 2026-10-07

- Run/download bars use Kit Progress with percentages and accessible names.
  Native default smoke checks `Scenario job progress` as a ProgressBar with
  range 0..100 and zero value for workspace initialization (no scenario jobs).
  The update bar retains its observed `update-download-progress` wrapper;
  styled Progress alone is not registered by Kit's headless test-support API.
- The [entry point](../../ci/desktop-smoke.ps1) owns parameters, Windows safety
  guards, build, flow precedence, and the run/cleanup boundary. It dot-sources
  [support and flow scripts](../../ci/desktop-smoke) into the same script scope.
- [Fixture setup](../../ci/desktop-smoke/fixture.ps1) initializes shared process,
  workspace, and preference state. [Cleanup](../../ci/desktop-smoke/cleanup.ps1)
  runs in the entry point's `finally`, including early-return and failure paths.
- [The default flow](../../ci/desktop-smoke/flows/default.ps1) explicitly assigns
  `$script:cleanupPath` and `$script:cleanupSnapshot` so its local function scope
  does not hide the dirty-draft verification state from cleanup.
- [Headless checks](../../ci/test-desktop-smoke.ps1) pass with PowerShell 7 and
  cover loading, all switch dispatch paths, precedence, failure cleanup, and
  shared draft state. Parser-created dispatch blocks retain the entry filename
  because `[scriptblock]::Create()` alone leaves `$PSScriptRoot` empty.
- A filename on a parser-created block does not create a new script scope. Full
  native baseline comparisons must run from a real script file beside the entry
  point so local initialization and `$script:` assignments share ownership.
- [Keyboard input](../../ci/desktop-smoke/native.ps1) builds balanced `SendInput`
  events from `ConsoleKey` plus Control/Shift/Alt, including extended navigation flags.
  Zero and undefined keys are rejected before input is sent. Headless checks
  compile this code and inspect its event sequences without invoking Windows.
- Inline `Add-Type` definitions are cached for the PowerShell process. Native
  tests of changed C# need a fresh host; rerunning in an old terminal can otherwise
  retain the previous implementation.
- Run interactive flows without concurrent user mouse/keyboard input. Pointer
  movement dismissed a metadata hover card and redirected image-tab clicks;
  the undisturbed fresh-STA `-ComparisonFlows` run passed all four size/theme
  combinations without a popup lifecycle change.
- [Clipboard checks](../../ci/desktop-smoke/controls.ps1) wait for the native
  sequence number to change after a synthetic sentinel and copy action before
  reading exact text. This avoids a tight read loop competing with GPUI's write.
  Only clipboard-busy `CLIPBRD_E_CANT_OPEN` is retried, for a bounded duration;
  non-busy errors propagate. Results/deployment preserve the clipboard in
  `finally`; headless checks cover order, exact text, retries, and failure cases.
- [Schema smoke](../../ci/desktop-smoke/flows/schema.ps1) verifies Enter/Tab/Escape
  after Alt+Tab to an owned temporary window and native return activation without
  clicking the editor. Compare `AutomationElement.FocusedElement` across the switch;
  the source wrapper's `HasKeyboardFocus` stays false even while its input works.
- Schema smoke also verifies external-save reload, draft preservation, and native
  Cancel/Discard choices in compact/large light/dark windows. The painted unsaved
  dialog title did not expose the expected standalone UIA name; wait for its
  Discard/Cancel controls instead. The native flow passes with preferences and
  clipboard restored; confirmation and final reloaded-source screenshots were inspected.
- Exact line-command selections finish with `Shift+Home` after `Shift+Down`.
  The keyboard-driven [GUI regression](../../src/desktop/ui/tests.rs) reproduced
  a one-character drift into the third line without it. Keep the column-zero
  status/range checks and exact output assertions; do not increase the wait or
  change the expected label. The full native `-SchemaFlows` run passes with this
  boundary normalization, including line edits and restored preferences.
- The [default flow](../../ci/desktop-smoke/flows/default.ps1) includes pointer-focused
  Ctrl+A, Ctrl+Shift+A, and Escape checks for suite/scenario selection and the Runs
  checklist. It verifies accessible selection counts without launching a worker.
- [Pointer clicks](../../ci/desktop-smoke/controls.ps1) flush the compositor after
  movement and press, with release in `finally`. Back-to-back mouse events missed
  native scenario clicks despite passing frame-stepped GPUI tests. The complete
  default native flow passed after synchronization; preferences were restored.
- [List keyboard state](../../src/desktop/ui/lists.rs) is scoped to each list's
  focus handle. [Navigator tests](../../src/desktop/ui/tests.rs) cover filters,
  other-suite preservation, text-input isolation, disabled state, and scrolling.
- [Suite filename dialogs](../../src/desktop/ui/files.rs) focus/select the stem
  after opening. `suite_name_dialog_` GUI tests cover Enter in inputs/buttons,
  keyboard/pointer X and Cancel, failed writes, retry, and busy locks. gpui-kit
  styled buttons replace supplied focus handles: track a parent focus region to
  distinguish the primary action from Cancel/X in the dialog-wide Enter handler.
  Button wrappers may omit disabled metadata; verify ignored interactions instead.
  X dismissal is deferred, so wait for the dialog to disappear before asserting it.
  The native artifact flow passes creation/rename focus, Enter, and X checks. Scope
  the New suite lookup by button role because status text repeats that name after
  cancellation. Set and verify native Create focus before Enter; GUI tests cover Tab.
- Keyboard-scrolled rows share an explicit `track_scroll`/`scrollbar` handle;
  append the scrollbar after rows so direct-child indices stay aligned. The
  convenience scroll wrapper owns a separate handle. [Workspace-file tests](../../src/desktop/ui/tests.rs)
  exercise both ends in compact/large windows and both themes.
- Windows interaction prerequisites and native commands remain in the
  [native UI skill](../../.github/skills/desktop-native-ui/SKILL.md).
