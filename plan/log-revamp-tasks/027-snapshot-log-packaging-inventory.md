<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 027 — Snapshot inventory/content-hash/staging/packaging updates for logs

- Slug: `snapshot-log-packaging-inventory`
- Implements plan section(s): `Phase 6 — Frontend and Packaging#0`, `Snapshot Frontend and Publication#0`
- Depends on: frontend-report-schema-log-fields

## Goal

Add log dependencies to snapshot inventory, content-version hashing, deployment staging, and exact package manifests, including only logs referenced by selected captures/suites, and produce an actionable packaging failure (not silent truncation) when a budget is exceeded.

## Why this exists

Task 6.3's own Task Breakdown acceptance criterion requires a deployed-fixture upload to include only referenced logs (verified against a package-manifest test) and a budget-exceeded case to fail actionably instead of silently truncating.

## Settled investigation

- src/snapshot/site.rs::generate_site (pub(crate) fn generate_site(root: &Path, suites: &[String], args: &SnapshotArgs, ...), line 364) is the existing report-generation entry point this task's new log-asset inventory must plug into, alongside the existing get_asset_version(bytes, 20) ?v= versioning scheme used for every script/CSS/font/manifest/suite-JSON/image URL.
- The deployment-packaging skill is explicit that staging must 'Stage only the active manifest, its suite JSON and versioned image dependencies... Require the manifest's suite set to match selection; never walk capture folders or choose the newest run independently' — this task's new log-asset dependency must follow that same existing selection/staging rule set.
- Per-provider byte budgets are already established and tested (Azure 250 MiB Free/unknown, 500 MiB Standard; Cloudflare 2 GiB of selected public assets) — this task's new log assets count against those same existing budgets, they do not get a separate budget.

## Inputs

- Task 6.1's execution-log schema/asset reference
- The existing snapshot inventory/staging/packaging pipeline in src/snapshot/site.rs and src/desktop/deployment.rs/package.rs (extended, not replaced)

## Outputs

- Log assets included in snapshot inventory, content-hash versioning, deployment staging, and exact package manifests, scoped to only the logs referenced by selected captures/suites

## Implementation steps

- Extend generate_site's (src/snapshot/site.rs line 364) inventory/content-hashing step to also emit a versioned log-asset path (via the existing get_asset_version scheme) for each capture that has a Task 6.1 executionLog reference, written as a separate bounded JSON chunk.
- Extend the deployment staging selection logic to include a log asset if and only if its owning capture's suite is in the selected set, reusing the existing manifest-suite-set-matching rule.
- Extend the exact package-manifest test(s) to assert log assets appear exactly when their owning capture is selected, and are absent otherwise.
- Add log_budget_exceeded_fails_actionably: synthesize a 260 MiB total log fixture (10 MiB over Azure's 250 MiB Free-tier budget) and assert an explicit, actionable packaging failure naming the exceeded budget.

## Done when

- Test package_manifest_includes_only_selected_logs builds a fixture with 2 suites (one selected, one not), each having a capture with an executionLog reference, and asserts the resulting package manifest/ZIP inventory includes exactly the selected suite's log asset and excludes the unselected suite's log asset.
- Test log_budget_exceeded_fails_actionably synthesizes a 260 MiB total log fixture, exceeding Azure's existing 250 MiB Free-tier budget by 10 MiB, and asserts packaging returns an explicit error naming the exceeded budget and the offending capture, not a truncated package.

## Evidence to capture

- package_manifest_includes_only_selected_logs's exact selected/unselected suite IDs and the resulting inventory diff (present vs. absent)
- log_budget_exceeded_fails_actionably's exact injected byte total (260 MiB) vs. the 250 MiB Azure Free-tier budget it exceeds, and the resulting error message text

## Do not change

- Do not walk capture folders directly to discover logs to include — selection must flow from the same manifest/suite-set source of truth the existing staging logic already uses.
- Do not silently clip/truncate an oversized log to fit a budget; it must be an actionable packaging failure, not a truncation.

## Reopen triggers

- Task 6.4's rename/delete provenance work finds this task's log-asset inclusion rule does not correctly follow a renamed suite's new identity.

## Context

This task makes the Capture Log feature's publication path a first-class citizen of the existing, already-strict staging/packaging pipeline documented in the deployment-packaging skill, so the new log assets must earn their place in that same pipeline, not bypass it.

## Options & tradeoffs

Packaging every log unconditionally (regardless of selection) would be simpler but directly violates 'Include only logs referenced by selected captures/suites' and risks exceeding provider budgets needlessly; selection-scoped inclusion (chosen) requires threading the selection set through one more asset type but matches the existing, already-strict staging contract.

## Decision

Add log assets to the existing inventory/hashing/staging/packaging pipeline as a selection-scoped asset type, sharing the existing per-provider budgets rather than a separate allowance, and failing actionably (not truncating) when a budget is exceeded.

## Acceptance criteria

- Test package_manifest_includes_only_selected_logs shows exactly the logs owned by selected captures present, and logs from unselected suites/captures absent, in at least one provider's package manifest.
- Test log_budget_exceeded_fails_actionably, using a synthetic 260 MiB total log fixture exceeding Azure's existing 250 MiB Free-tier budget by 10 MiB, produces a named, actionable error rather than a truncated package.

## File touchpoints

- src/snapshot/site.rs
- src/desktop/deployment.rs
- src/desktop/deployment/package.rs
- src/desktop/deployment/netlify.rs
- src/desktop/deployment/cloudflare.rs

## Tests

- cargo test --features desktop --lib desktop::deployment::package::tests -- package_manifest_includes_only_selected_logs
- cargo test --features desktop --lib desktop::deployment::tests -- log_budget_exceeded_fails_actionably

## Dependencies

- frontend-report-schema-log-fields

## Risks

- Reusing the exact existing per-provider budgets without first estimating typical log sizes at scale could make large-suite deployments newly budget-constrained in a way they were not before this feature; mitigate by surfacing the budget-exceeded failure with enough detail (which capture's log, how many bytes) that a user can selectively exclude it, and flagging this interaction for Task 7.2's benchmark/Task 7.3's documentation to call out explicitly.
