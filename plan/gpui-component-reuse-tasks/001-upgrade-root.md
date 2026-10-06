# 001: Upgrade Kit And Migrate Window Root

Status: Verified on Windows; see README for exact results and the approved
non-green Linux baseline exception. Depends on 000. Local commit pending.

## Scope

Exact Kit version/lock resolution and required 0.7.1 compatibility changes in
[Cargo.toml](../../Cargo.toml), [Cargo.lock](../../Cargo.lock),
[launch](../../src/desktop/ui/mod.rs), [fixtures](../../src/desktop/ui/tests.rs),
and directly affected compatibility documentation/guidance. Preserve optional
features and tests. No independently versioned GPUI dependencies, clone/path
dependencies, toolkit edits, or unrelated API cleanup.

## Ordered Work

1. Inspect status/diffs and storage. Record baseline check/test outcomes, keeping
   environmental error 112 separate from source failures. Use ephemeral
   `CARGO_INCREMENTAL=0` after the authorized cache cleanup. Verify approved
   0.7.1 remains published and non-yanked; do not chase a newer release.
1. Before edits, run `cargo check --locked --features gui --bin regressify-desktop`
   and `cargo test --locked --features gui-tests --all-targets -- --test-threads=1`.
   Record counts and exact failures. Storage or other prerequisites must be
   resolved before claiming a baseline; do not count unrun tests as passing.
1. Change only the Kit pin to `=0.7.1`, preserve `gui` optionality and
   `gui-tests = ["gui", "gpui-kit/test-support"]`, then run
   `cargo update -p gpui-kit`. Review release notes and resolved Kit-related
   packages; avoid unrelated dependency or feature churn.
1. Migrate launch to `gpui_kit::open_window(options, &mut App, builder)` inside
   `cx.update` for async callers. Return workbench content, not another Root;
   remove manual overlay mounting. Retain content entities in migrated fixtures.
1. Immediately run the GUI compile. Resolve only required compiler/API changes,
   preserving interaction assertions. Add focused native regression coverage
   for single overlay mounting, content ownership, dismissal and focus return.
1. Preserve workspace activation/ownership, taskbar setup, unsaved and landing
   guards, cancellation, serialized preference saves and update-install handoff.
   Maintain affected docs/skills/memory in this unit.
1. Review the dependency and lifecycle diff adversarially; record exact evidence
   in README and commit this verified unit before presentation work.

## Acceptance And Checks

```sh
cargo check --locked --features gui --bin regressify-desktop
cargo test --locked --all-targets
cargo test --locked --features gui-tests --all-targets -- --test-threads=1
cargo clippy --locked --features gui --lib -- -D warnings
cargo build --locked --features gui --bin regressify-desktop
cargo test --locked --features desktop --lib
cargo fmt --all -- --check
```

Run behavior-scoped native filters before broad gates: `suite_name_dialog_`,
`command_palette_`, `source_line_commands`, `external_reload`, `update_progress_`,
`navigator_`, `image_previews_`, and relevant Git/deployment tests. Require nonzero
counts and preserve existing assertions; introduce explicit lifecycle tests in
the existing module where coverage is missing.

Run the headless smoke harness, then guarded native default, `-ArtifactFlows`,
`-SchemaFlows`, `-ComparisonFlows`, `-GitFlows`, `-PublishingFlows`, and
`-DeploymentFlows` through [desktop-smoke.ps1](../../ci/desktop-smoke.ps1).
Honor unlocked-desktop/no-other-instance guards and preserve user windows.
Use an existing Linux environment for headless models or explicitly leave Linux
pending for CI. That approved exception does not block Windows implementation;
the separately approved Linux baseline-failure exception in README also permits
continuation without claiming a green Linux gate. Other unavailable or failed
upgrade gates block dependent work.

## Maintenance And Boundary

Follow [feature checks](../../.github/skills/rust-feature-checks/SKILL.md),
[native UI](../../.github/skills/desktop-native-ui/SKILL.md),
[worker lifecycle](../../.github/skills/desktop-worker-lifecycle/SKILL.md) if
process behavior is affected, and
[guidance maintenance](../../.github/skills/maintain-agent-guidance/SKILL.md).
Commit only dependency/lock, necessary compatibility, tests and maintenance.
Reopen if 0.7.1 is unavailable/yanked, dependency scope expands, lifecycle cannot
be preserved, or a gate exposes changes outside the narrow upgrade. Never defer
an unexplained upgrade failure to a component task.
