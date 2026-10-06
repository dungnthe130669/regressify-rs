# 002: Progress And Binary Git Checkbox

Status: Verified; local commit pending. Depends on 001. Application-owned
accounting and selection policy remain unchanged; evidence is in README.

## Scope

Implement R1 progress at [run activity](../../src/desktop/ui/runs/activity.rs)
and [update progress](../../src/desktop/ui/updates.rs), plus R11 individual
Git-file checkbox in [Git view](../../src/desktop/ui/git/view.rs). Preserve
application-owned progress accounting and selection policy. Keep the mixed-state
adapter in [design.rs](../../src/desktop/ui/design.rs).

## Ordered Work

1. Before edits, run `cargo test --locked --features gui-tests --lib update_progress_ -- --test-threads=1`
   and `cargo test --locked --features gui-tests --lib desktop::ui::git::tests -- --test-threads=1`;
   confirm nonzero counts and record baseline. Read native UI and Git-review skills.
1. Replace only painted progress tracks with Kit `Progress`; convert fractions
   to 0..100, retain denominator guard/clamp, stable IDs, accessible name,
   failure/cancel/captured colors, textual counts, cancellation, and update
   lifecycle. Keep `update-download-progress` discoverable and do not invent
   indeterminate completion.
1. Replace only individual-file binary checkboxes with styled Kit Checkbox.
   Preserve file path, accessible name, conflict/submodule restrictions,
   active-operation disabling, and pointer/keyboard behavior. Keep folder/suite
   aggregates on the tri-state adapter.
1. Extend existing native tests for zero/partial/full and zero-total progress,
   all run outcomes and settled animation; pointer and keyboard toggling in both
   Changes groups must not open a file or alter the other group's checked set.
1. Review rendered child semantics and operation guards; measure removed render
   code against added adapter/state. Update README evidence and affected
   docs/skills/memory.

## Acceptance And Checks

Run both exact commands above after changes, plus `cargo fmt --all -- --check`.
The `update_progress_` and `desktop::ui::git::tests` filters are known names;
verify nonzero execution. Add a new `git_file_checkbox_` regression in the
existing module if current tests do not exercise both groups and keyboard/pointer
paths. Cover disabled/restricted files and ensure progress never confuses
download completion with verification/install completion.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[desktop Git review](../../.github/skills/desktop-git-review/SKILL.md), and
[Rust feature checks](../../.github/skills/rust-feature-checks/SKILL.md).
Maintain relevant docs/guidance/memory; record confirmed gaps in
[OUTSTANDING_ISSUES.md](../../OUTSTANDING_ISSUES.md). Commit only this task's
source, test, and maintenance changes after required checks.

Reopen if released Progress cannot express required status/accessibility,
styled Checkbox loses binary interaction semantics, or an operation guard or
selection boundary cannot be maintained. Do not remove the mixed-state adapter.
