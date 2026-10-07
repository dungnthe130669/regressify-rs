# 010: Clipboard Controls

Status: Implementation and acceptance complete; authorized local commit pending.
Depends on 001.

## Scope

R7 always-available copy actions, starting with the saved publication URL in
[deployment](../../src/desktop/ui/deployment.rs). Evaluate Git activity, source
diffs, update failure details and Results URLs individually. Gated copies keep
guarded Buttons where Clipboard lacks the required availability control.

## Ordered Work

1. Baseline the relevant deployment, source, image and Git native copy tests;
   record nonzero counts and identify the exact source text before edits.
1. Replace eligible actions with Clipboard and preserved accessible names.
   Use immutable value or a current snapshot via value_fn. Copy exact source or
   validated URLs, never line numbers, truncated labels or masked credentials.
1. Use value/generation identity so changed text is immediately copyable during
   feedback; unchanged text may retain the approved two-second suppression.
   Never add secret-copy features or weaken incomplete-log/unavailable-URL gates.
1. Immediately test one replacement, then extend existing tests for exact bytes,
   changed selection during feedback, repeated unchanged value, keyboard action,
   stale identity, failures and disabled export/copy paths.
1. Preserve and restore clipboard in native smoke. Maintain docs/guidance/memory
   and record adopted surfaces, justified gated exceptions and adapter cost.

## Acceptance And Checks

Run relevant `deployment_activity_`, `source_line_commands`, `image_previews_`
and Git tests using the gui-tests feature, all with nonzero counts. Add an
explicit new `clipboard_copy_` filter in existing tests if required, then run it.
Run `cargo fmt --all -- --check` and affected feature/native smoke gates.

Follow [deployment targets](../../.github/skills/deployment-targets/SKILL.md),
[landing apps](../../.github/skills/desktop-landing-apps/SKILL.md),
[results review](../../.github/skills/desktop-results-review/SKILL.md),
[Git review](../../.github/skills/desktop-git-review/SKILL.md) and
[native UI](../../.github/skills/desktop-native-ui/SKILL.md).
Commit only copy controls/tests/maintenance. Reopen for inexact text, suppressed
new content or lost gates; masked/display values are not acceptable substitutes.

## Implementation Evidence

- Task 009 is committed as `b77d5a6`; this task is uncommitted.
- `design::CopyText` hosts toolkit Clipboard with a stable outer observation ID
  and a value-generation inner key. Exact changed text, including a return to
  previously copied text, immediately bypasses old feedback; unchanged text
  retains the toolkit's two-second suppression. Text is never embedded in IDs.
- Adopted saved report/landing URLs, Git activity, both valid source-diff sides,
  update failure details and capture summaries. Preserve accessible names,
  custom summary tooltip and weak copied-status notification. Copy source lines
  without numbers and URL strings without decoding or truncating them.
- Retained gated Buttons for empty deployment/live logs, incomplete archived
  run/capture log pages and exports, and unavailable Results URLs/context-menu
  actions. Clipboard has no disabled API; their existing validation/availability
  rules remain unchanged. No new secret-copy surface.
- Baseline and post-edit saved-URL copy pass (1), Git diff pass (3), update details
  pass (1). New `clipboard_copy_` passes (1) for exact encoded bytes, Tab/Space,
  unchanged suppression, changed/revisited values and missing-URL removal.
- Extended capture-name/copy regression passes (1), including changed capture
  during feedback. Update details copy exact full/changed messages in both themes
  and sizes. Required source-line checks pass (12), image previews (3), run-log
  gates (7), deployment activity (2) and Git remote/activity workflow (1).
- GUI Clippy, desktop build, Rustfmt, editor diagnostics and headless harness
  pass. The copied-callback type alias satisfies Clippy without suppression.
- Native Deployment, Git, Comparison and offline Update flows pass with the
  current binary and original assertions. Clipboard/preferences restored;
  reviewed compact URL and diff-copy layouts remain intact. Native Update covers
  unavailable-feed layout/guards; failure-detail bytes use the focused GUI test,
  not a live update. No remote publication or real update execution.
- Production accounting: +98/-61 (net +37), including the 72-line wrapper;
  GUI tests +103. The wrapper retains value/generation state but delegates the
  copy/check timer to Clipboard. No new native harness logic or gates bypassed.
- Docs and relevant skills/shared notes now record feedback and gated exceptions.
  Final maintenance validation runs before commit. No new deferral.
