# 018: Source Font Size NumberInput

Status: Committed as `ecbc562`. Native Default passes;
integrated UI has only the approved task 019 header failure.
Depends on 001, 003 and 007 (all committed).

## Scope

Replace minus/value/plus in [Source toolbar](../../src/desktop/ui/source.rs)
with a direct integer NumberInput for the saved display preference, inclusive
range 10..24. Configuration drafts remain lossless; image fit percentage remains
derived from image layout.

## Ordered Work

1. Baseline `source_line_commands` and relevant existing preference tests with
   nonzero counts; inspect change_source_size and saved preference ownership.
1. Add direct integer entry/step within 10..24. Valid integers apply/persist.
   Invalid, fractional and out-of-range drafts never persist; blur restores the
   last valid size. Preserve reset and preference save serialization.
1. Immediately test the control. Do not share a coercing adapter with schema
   numeric fields or change fit-mode image zoom policy.
1. Extend existing native tests for 10/24/interior, below/above range, fractional
   and invalid text, blur recovery, keyboard entry/step, reset, persistence/reload
   and unchanged lossless configuration drafts.
1. Review screenshots and update docs/skills/memory; record control/adapter cost.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib source_line_commands -- --test-threads=1`
plus nonzero preference tests. Introduce `source_font_size_` in existing UI tests
for new behavior and execute it. Run `cargo fmt --all -- --check` and affected
feature/native smoke gates; verify actual input value and saved preference.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md) and
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md).
Commit font-size control/tests/maintenance only. Reopen for unrecoverable invalid
input or lost last-valid behavior; never coerce source configuration drafts.

## Verification

- Pre-edit Source line commands (12) and preferences (5) pass. Direct-entry
  regression failed before migration and passes afterward.
- `source_font_size_` passes all 3 cases for typed values, invalid drafts and
  actual blur, keyboard/pointer bounds, Reset, persistence/reload, exact invalid
  Source preservation and both themes/sizes. See the [ledger](README.md#task-018-verification)
  for exact commands, neighbor counts and fixture corrections.
- Rustfmt, production GUI Clippy and desktop build pass. Native harness
  passes. Full UI: 270 passed, 1 failed (only the approved task 019 header),
  0 ignored. Scoped Markdown, guidance validation and 10 self-tests pass.
- Two rebuilt Default attempts failed in existing Overview clipboard handling
  before reaching the font-size checks; the retry also failed to restore its
  clipboard backup. Preferences were restored. The owner authorized a bounded
  harness repair: Overview now uses the existing sequence-before-read helper,
  with a failing-before/passing-after headless regression and unchanged guards.
- Final rebuilt native Default passes all font-size values, steps, invalid
  drafts, blur, Reset, exact Source/undo, preferences/restart and cleanup. Its
  new focus check observes the exact Spinner that owns the sole Edit child;
  actual Edit values remain asserted. Both-theme Source screenshots at
  1080x720/1600x1000 were inspected; task 019 still owns header fit.
- Production net +50 lines, one InputState and two subscriptions. No toolkit
  or dependency edits. Committed as `ecbc562`; the post-commit worktree was clean.
