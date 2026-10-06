# 010: Clipboard Controls

Status: Not started. Depends on 001.

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
