# 017: Command Palette

Status: Verified, awaiting the authorized local commit. Focused and native gates
pass; the integrated gate has only the approved task 019 header failure.
Depends on 001.

## Scope

Kit Command in [commands.rs](../../src/desktop/ui/commands.rs), preserving the
catalog, enablement and execution paths. Use approved stock substring search;
do not introduce all-words compatibility or another command-routing framework.

## Ordered Work

1. Baseline `command_palette_`, recording nonzero counts and current search,
   Escape, focus and dispatch behavior before edits. Load relevant skills.
1. Prototype actionless CommandItem plus explicit on_confirm. Stock search
   matches substrings; first Escape clears a nonempty query, next dismisses.
1. Confirmation closes the palette, restores workbench context/focus and
   dispatches exactly once in that order. Preserve disabled commands and all
   current callbacks/context; do not depend on private toolkit fields.
1. Immediately run the focused check. Extend existing tests for substring
   positive/negative matches, rejection of noncontiguous characters and former
   all-words-only matches, empty query, keyboard choice, Escape sequencing,
   disabled items, focus return and duplicate-event prevention.
1. Update docs/guidance/memory for intentional search/Escape changes and measure
   deleted delegate code versus retained adapter/catalog.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib command_palette_ -- --test-threads=1`
with nonzero count, extending that existing test module. Run
`cargo fmt --all -- --check` and affected feature/native smoke gates. Do not
weaken exact-once or action-target assertions to accommodate direct dispatch.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md) and
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Commit palette/tests/maintenance only. Investigate a genuine public API blocker
and ask before deferring; approved search and Escape behavior are not open.

## Verification

- The locked `command_palette_` baseline passed 3 tests. The final five palette
  cases pass, covering substring/category matching, disabled/empty results,
  keyboard/pointer confirmation, duplicate-event suppression, Escape, focus,
  draft guards and owner release.
- Full locked desktop UI gate: 267 passed, 1 failed, 0 ignored. The sole failure
  remains the unchanged task 019 compact Git header assertion.
- Rustfmt, production GUI Clippy with `-D warnings` and desktop build pass.
- Rebuilt native Default smoke passes on retry. The first attempt passed every
  new palette check, then failed in existing Results clipboard restoration.
  The unchanged complete retry passed with preferences restored. Reviewed empty
  and cleared palettes in Light/Dark at 1080x720 and 1600x1000, plus filtered
  native theme confirmation. Wide-only theme switching remains task 019's owner.
- Production net -78 lines; tests net +161; smoke +9. No toolkit, dependency,
  action catalog or execution-handler changes. Linux remains deferred.
