# Desktop Smoke Harness

Verified: 2026-09-23

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
  events from `ConsoleKey` plus Control/Shift, including extended navigation flags.
  Zero and undefined keys are rejected before input is sent. Headless checks
  compile this code and inspect its event sequences without invoking Windows.
- Inline `Add-Type` definitions are cached for the PowerShell process. Native
  tests of changed C# need a fresh host; rerunning in an old terminal can otherwise
  retain the previous implementation.
- The complete default and `-SchemaFlows` native checks pass with the shared input
  helper, including completion, saving, clipboard operations, and preference
  restoration. These checks do not imply that optional browser captures were run.
- Windows interaction prerequisites and native commands remain in the
  [native UI skill](../../.github/skills/desktop-native-ui/SKILL.md).
