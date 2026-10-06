# 004: Form And Field Presentation

Status: Not started. Depends on 001. Task 013 depends on this task.

## Scope

R3 in [Fields](../../src/desktop/ui/mod.rs),
[run settings](../../src/desktop/ui/runs/settings.rs),
[deployment](../../src/desktop/ui/deployment.rs),
[tokens](../../src/desktop/ui/tokens.rs), and
[landing](../../src/desktop/ui/landing.rs). Pilot run options, then cover all
identified forms. Use normal unframed GroupBox for static named sections.

## Ordered Work

1. Baseline `setting_search_` and neighboring native tests for the first form;
   record nonzero counts before edits. Load the owning skills listed below.
1. Convert one small run-options form to Form/Field/GroupBox. Alias toolkit Field
   as `FormField`. Immediately test the pilot and record removed versus added
   code before propagation. Do not create another form framework or move code
   solely to shorten a module.
1. Convert the Fields, deployment, token and landing forms. Use columns/spans
   where appropriate; keep URLs, descriptions, lists, structured/action controls
   wide. Preserve explicit scroll handles and outer accessibility IDs.
1. Retain input IDs/names, focus, inherited hints, reset, validation, hidden
   errors, Binding/field_errors, password masking, saved-secret placeholders,
   provider-dependent fields, asynchronous locks, conflicts and draft guards.
   Keep a flat layout without nested filled/outlined cards.
1. Extend existing tests for required/wide fields, pending input on Save or
   navigation, inheritance/reset, invalid drafts, hidden errors, masking,
   provider switches, long labels, offscreen inputs and both themes/sizes.
1. Update docs/skills/memory, account for adapters and commit the verified unit.

## Acceptance And Checks

Run `cargo test --locked --features gui-tests --lib setting_search_ -- --test-threads=1`
and relevant existing dialog/deployment filters after each surface. Require
nonzero counts. Add a new `form_field_` filter in existing native tests if needed
for the run pilot and propagation contracts. Run `cargo fmt --all -- --check`
and affected feature/smoke gates, including failure and async-save cases.

Follow [lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md),
[deployment targets](../../.github/skills/deployment-targets/SKILL.md),
[landing apps](../../.github/skills/desktop-landing-apps/SKILL.md),
[workspace files](../../.github/skills/desktop-workspace-files/SKILL.md), and
[native UI](../../.github/skills/desktop-native-ui/SKILL.md).
Commit R3 presentation, existing-module tests and maintenance only. Investigate
and ask before retaining a blocked custom form. Reopen for lost accessibility,
scroll ownership, credentials, conflicts or coerced source drafts.
