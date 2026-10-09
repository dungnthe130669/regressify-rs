# 020: Final Acceptance And Reconciliation

Status: Windows acceptance and reconciliation complete in this final record.
Tasks 001-019 are committed; task 019 is `005dc33`. The approved
Linux baseline/CI disposition remains explicitly non-green and release-blocking.

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

## Requirement Reconciliation

| Requirement                          | Completed Units | Preserved Contract                                                                               |
| ------------------------------------ | --------------- | ------------------------------------------------------------------------------------------------ |
| Published Kit/root compatibility     | 001             | Exact 0.7.1 dependency; optional GUI boundary; single root/overlay ownership                     |
| Progress and binary selection        | 002             | Exact progress semantics; mixed aggregates retained                                              |
| Numeric Fields and Source preference | 003, 018        | Lossless config drafts; Source integer 10..24, last-valid blur and serialized saves              |
| Forms and content primitives         | 004, 005        | Domain state, semantic names, errors, masks and reset ownership unchanged                        |
| Dialog footers and toolbars          | 006, 007        | Explicit busy/action guards, focus handling, normal controls and wrapping                        |
| Managed context menus                | 008             | Stable identity, guarded dismissal, safe targets and weak ownership                              |
| Value selectors                      | 009             | Unknown values, guarded writes, one history entry per accepted toggle                            |
| Clipboard and logs                   | 010, 011        | Exact current text, availability gates, bounded pages and deliberate Follow exit                 |
| Application menus                    | 012             | File/Publish/Help; explicit open-document versus focused-row targeting                           |
| Controlled disclosure                | 013             | Full forced opening for search/errors; user collapse preference retained                         |
| Lists, Tree and Overview Table       | 014, 015, 016   | Stable identity, independent selection, virtualization, full selectable Details                  |
| Command palette                      | 017             | Stock substring search, two-step Escape, close/focus/dispatch once                               |
| Stock presentation and minimum size  | 019             | Normal controls, stock typography/radii, approved contrast-only colors, 1000x640 minimum         |
| Specialized retentions               | 001-019         | Source/history, diagnostics escaping, image geometry, split diff, bounded logs and domain models |

## Final Evidence

The owner-approved consolidated run was performed after implementation and
verified against an unchanged runtime fingerprint before task 019 was committed.
Task 020 changes documentation/status only; the long suite is not duplicated.

| Gate                                                                                 | Result                                                                               |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| `cargo test --locked --all-targets -- --quiet`                                       | 524 passed, 0 failed, 23 ignored                                                     |
| `cargo test --locked --features desktop --lib -- --quiet` on Windows                 | 864 passed, 0 failed, 31 ignored                                                     |
| `cargo test --locked --features gui-tests --all-targets -- --test-threads=1 --quiet` | Library: 1,146 passed, 0 failed, 32 ignored; workers: 11 passed, 0 failed, 4 ignored |
| `cargo fmt --all -- --check`                                                         | Passed                                                                               |
| `cargo clippy --locked --features gui --lib -- -D warnings`                          | Passed                                                                               |
| `cargo build --locked --features gui --bin regressify-desktop`                       | Passed; production-feature binary restored after tests                               |
| Headless smoke harness                                                               | Passed: 18 modules, 50 helpers, 45 dispatch/cleanup, 41 sizing cases                 |
| Guidance                                                                             | 46 skills validated; 10 self-tests passed; scoped Markdown passed                    |

Native Default, Titlebar, Schema, Comparison (including error images), Git,
Artifact, Publishing and Deployment flows have passing minimum/large light/dark
evidence with original behavioral guards and cleanup. Exact command histories,
candidate timing, retries, inspected screenshots and per-unit accounting remain
in the [ledger](README.md#task-019-native-acceptance).

## Exceptions And Retention

The compact-header exception is closed with focused and native evidence.
The separately approved Linux baseline failures remain non-green; current Linux
CI is required before release. Windows tests do not establish Linux coverage,
and ignored tests are not passing executed coverage. Historical clipboard restore
failures remain recorded even though final native cleanup passes.

The source plan and bundle are retained. No branch, push, remote publication,
release installation, toolkit edit or cleanup is authorized by this record.
Task 020 is identified by this documentation-only Git commit; no own-hash placeholder is used.
