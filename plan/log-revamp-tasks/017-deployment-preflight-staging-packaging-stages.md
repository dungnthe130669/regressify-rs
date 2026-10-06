<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 017 — Preflight, snapshot/staging, and packaging/optimization stage instrumentation

- Slug: `deployment-preflight-staging-packaging-stages`
- Implements plan section(s): `Phase 4 — Deployment Instrumentation#0`, `Deployment Diagnostics#0`
- Depends on: deployment-journal-reuse

## Goal

Instrument preflight (provider, safe target identity, selected suites/run, validation outcome, credential presence only), snapshot/staging (generation/staging duration, dependency counts/bytes, version checks), and packaging/optimization (provider budget, before/after sizes, file counts, compression decision, duration) stages for deployments.

## Why this exists

Deployment Diagnostics' required-context table mandates these three stages; a mocked provider run with a staging failure must show the failing stage and artifact identity without leaking credentials, per Task 4.2's own acceptance criterion.

## Settled investigation

- src/desktop/worker.rs::run_stdio already validates deployment settings (job.validate()?, line 52) and stages reports via super::deployment::stage_reports(&output, &suites, &destination) wrapped in with_heartbeat("Staging selected reports", ...) (lines 97-104) — these are the exact existing call sites for the preflight and staging stages this task instruments.
- Packaging/optimization is shared across Azure and Cloudflare via src/desktop/deployment/optimization.rs per the deployment-packaging skill ('Azure and Cloudflare share image optimization'); Netlify's packaging path is explicitly 'unchanged' (no compression).
- src/desktop/deployment/diagnostics.rs::sanitize()/failure() (lines 173/65) are the existing, already-tested helpers this stage's error paths must reuse for safe target identity/credential-shaped text.

## Inputs

- [Verified producer and sanitizer inventory](../../logging-audit.md).
- Task 4.1's deployment-kind journal integration
- src/desktop/deployment/optimization.rs and package.rs (reference)
- src/desktop/deployment/diagnostics.rs's sanitize()/failure() (reused, not reimplemented)

## Outputs

- Preflight, staging, and packaging/optimization diagnostic records attached to the deployment operation's journal

## Implementation steps

- Add a preflight diagnostic record around job.validate() in src/desktop/worker.rs::run_stdio capturing provider, a sanitized target identity (via existing sanitize()), selected suite/run identity, validation outcome, and only whether a credential is present.
- Add a staging diagnostic record around super::deployment::stage_reports(...) capturing staging duration, selected dependency counts/bytes, and version-check outcomes (reusing the existing get_asset_version-based ?v= validation).
- Add a packaging/optimization diagnostic record around the provider-specific packaging call (Azure/Cloudflare's shared optimization.rs path, and Netlify's unchanged manifest path), capturing the applicable provider budget, before/after sizes, file counts, and the compression decision.
- Write staging_failure_identifies_stage_and_artifact: force a staging failure (e.g. a missing/invalid dependency) in a mocked provider run and assert the resulting record set names the staging stage and the specific failing artifact identity, with no credential or signed-URL text present in any record.

## Done when

- Test staging_failure_identifies_stage_and_artifact forces a staging failure (a missing dependency file) in a mocked Azure deployment and asserts the resulting record set names the staging stage and the specific missing artifact's identity.
- The same test's full captured record set contains zero occurrences of the mocked run's injected synthetic deployment token.

## Evidence to capture

- The staging-failure test's asserted stage/artifact-identity fields and an explicit scan of the recorded fields confirming absence of the injected synthetic token

## Do not change

- Do not re-implement sanitize()/failure()'s existing secret/signed-URL redaction inside this task's new stage instrumentation; call the existing functions.
- Do not assume Netlify takes the same packaging/optimization path as Azure/Cloudflare — the deployment-packaging skill is explicit that Netlify's packaging remains unchanged (no compression).

## Reopen triggers

- Task 6.3's packaging-budget instrumentation finds this stage's recorded byte/file counts insufficient to produce an 'actionable packaging failure' message.

## Context

This is the first of two Phase 4 stage-instrumentation tasks; it covers everything before the network upload itself, where the existing sanitize()/failure() helpers are most directly reusable since they were originally built for exactly this kind of provider-facing error text.

## Options & tradeoffs

n/a for stage selection (fixed by the Deployment Diagnostics table); the only choice is whether packaging/optimization instrumentation lives in src/desktop/deployment.rs (the shared orchestrator) or inside each of package.rs/optimization.rs individually. Instrumenting at the shared orchestration call sites (chosen) keeps provider-specific files untouched where the existing packaging logic is already well-tested.

## Decision

Instrument preflight/staging/packaging at their existing shared call sites in src/desktop/worker.rs and src/desktop/deployment.rs, reusing diagnostics.rs's sanitizer for any error/identity text.

## Acceptance criteria

- Test staging_failure_identifies_stage_and_artifact identifies the staging stage and the specific failing artifact by name.
- The same test's full captured record set contains zero occurrences of an injected synthetic credential string.

## File touchpoints

- src/desktop/deployment.rs
- src/desktop/worker.rs
- src/desktop/deployment/diagnostics.rs (reference)
- src/desktop/deployment/optimization.rs (reference)
- src/desktop/deployment/package.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::deployment::tests -- staging_failure_identifies_stage_and_artifact

## Dependencies

- deployment-journal-reuse

## Risks

- Capturing 'selected dependency counts/bytes' naively could include full file paths that leak workspace layout; mitigate by recording counts/sizes only, with paths passed through the same containment/sanitization checks the deployment-packaging skill already requires for staging.
