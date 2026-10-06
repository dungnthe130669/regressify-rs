# 006: Dialog Footers And Actions

Status: Not started. Depends on 001.

## Scope

R10 footer layout in [files](../../src/desktop/ui/files.rs),
[deployment](../../src/desktop/ui/deployment.rs),
[tokens](../../src/desktop/ui/tokens.rs),
[landing](../../src/desktop/ui/landing.rs), Git confirmation and update dialogs.
Public DialogAction/DialogClose is a bounded pilot, not permission to alter
application-specific dialog policy.

## Ordered Work

1. Baseline `suite_name_dialog_` and neighboring tests for each touched dialog.
   Record nonzero counts before edits; load relevant domain skills below.
1. Replace repeated footer alignment with DialogFooter and existing Buttons.
   Preserve wrapping, names, variants, disabled state and callbacks. Validate
   one footer immediately before converting the next.
1. Pilot default button props only for simple non-busy dialogs. New/Rename/Save
   and publishing retain explicit busy/disabled buttons and repeat-submit locks.
1. Keep the filename-dialog focus guard until Enter on input/primary versus
   Cancel/X and busy tests prove parity. Never use private DialogDispatchAnchor.
   Preserve Save/Discard/Cancel and inline approval as distinct policies.
1. Extend existing tests for pending saves, repeated submission, busy Escape/X/
   backdrop, errors and retry, keyboard order, cancellation and owner release.
1. Review relevant screenshots, update docs/guidance/memory and record code
   reduction including retained guards/adapters.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib suite_name_dialog_ -- --test-threads=1`
and touched-area native filters, all nonzero. Add missing footer/focus tests in
existing modules, explicitly identifying new filters. Run
`cargo fmt --all -- --check` and affected native/smoke gates.

Follow [native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md),
[deployment targets](../../.github/skills/deployment-targets/SKILL.md),
[landing apps](../../.github/skills/desktop-landing-apps/SKILL.md) and
[Git review](../../.github/skills/desktop-git-review/SKILL.md).
Commit footer presentation, tests and maintenance only. Investigate and ask
before deferring a public-API blocker; never weaken dismissal or save policy.
