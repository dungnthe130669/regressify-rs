# 020: Final Acceptance And Reconciliation

Status: Not started. Depends on all tasks 001-019.

Owner-approved verification scheduling: one fresh consolidated pass after the
task 019 implementation may supply both task records, avoiding a duplicate long
run while only documentation/status changes afterward. Preserve separate feature
gate results and per-task commits; any implementation change requires affected
verification again. This approval does not waive a failing or unavailable gate.

## Scope

Reconcile every requirement in the [refined plan](../gpui-component-reuse.md)
against implementation, tests, documentation, platform evidence and code
accounting. This task does not authorize unrelated changes or plan deletion.

## Ordered Work

1. Compare the ledger and actual source for every R1-R12 item, Other Matching
   Components, all pilots, stock theme/normal sizing and explicit retentions.
   Resolve missing coverage and approved deferrals before claiming completion.
1. Run fresh final gates after all implementation, not copied earlier results:

   ```sh
   cargo fmt --all -- --check
   cargo test --locked --all-targets
   cargo test --locked --features gui-tests --all-targets -- --test-threads=1
   cargo clippy --locked --features gui --lib -- -D warnings
   cargo build --locked --features gui --bin regressify-desktop
   cargo test --locked --features desktop --lib
   ```

1. Run headless smoke harness checks and guarded native default, `-ArtifactFlows`,
   `-SchemaFlows`, `-ComparisonFlows`, `-GitFlows`, `-PublishingFlows`, and
   `-DeploymentFlows` through [desktop-smoke.ps1](../../ci/desktop-smoke.ps1).
   Use a current binary and preserve isolation, unlocked-desktop, user-window,
   clipboard and preference backup/restore requirements.
1. Use existing local Linux for the desktop-model gate or explicitly leave it
   pending for CI, as approved. Do not install WSL or claim Windows proves Linux.
   No live publishing, real remote operation or installed release is required.
1. Account per task for deleted rendering versus adapters/state/subscriptions/
   wrappers. Do not count relocated code or deleted tests; net increase is
   allowed for correct toolkit integration. Preserve approved retentions and
   headless feature independence; no copied toolkit code or new fork dependency.
1. Reconcile maintained docs/skills/shared memory and confirmed issue records.
   If guidance changed, run its validator/self-tests and scoped Markdown lint.
   Record exact counts, failures/ignored cases, smoke/screenshot evidence,
   Linux pending status and earlier commit IDs in README.
1. Keep source plan and bundle. The interview authorizes a local commit for this
   verified acceptance documentation unit; no branch/push/release/credentials.

## Acceptance And Checks

All earlier criteria and approved deferrals must be closed with fresh evidence.
All gates pass except explicitly pending Linux CI when local Linux is
unavailable. Zero-test, failed, skipped or inaccessible checks are not green.
Missing smoke or unexplained regressions require a user decision, not a weaker
substitute. Browser automation is not native GPUI interaction evidence.

Follow [implement-plan](../../.github/skills/implement-plan/SKILL.md),
[adversarial review](../../.github/skills/adversarial-review/SKILL.md),
[feature checks](../../.github/skills/rust-feature-checks/SKILL.md),
[native UI](../../.github/skills/desktop-native-ui/SKILL.md) and
[guidance maintenance](../../.github/skills/maintain-agent-guidance/SKILL.md).
Update [OUTSTANDING_ISSUES.md](../../OUTSTANDING_ISSUES.md) only for confirmed
gaps. Commit final in-scope documentation/maintenance after verification; do not
embed the commit's own hash in itself. Reopen for missing scope, unknown/zero
filters, unverified required smoke, stale guidance or crossed authorization.
