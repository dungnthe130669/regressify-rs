# 018: Source Font Size NumberInput

Status: Not started. Depends on 001, 003 and 007.

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
